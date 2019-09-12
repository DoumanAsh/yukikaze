//! Rustls connector

use tokio_io::{AsyncRead, AsyncWrite};
use tokio_rustls::client::TlsStream;
use hyper::client::connect::{self, Connected, Connect};

use super::super::{HttpConnector, Connector};
use crate::utils::{self, OptionExt};

use std::io;
use std::sync::Arc;
use core::fmt;
use core::future::Future;
use core::task::{Poll, Context};
use core::pin::{Pin};

/// A stream that might be protected with TLS.
pub enum MaybeHttpsStream<T> {
    /// A stream over plain text.
    Http(T),
    /// A stream protected with TLS.
    Https(TlsStream<T>),
}

impl<T: fmt::Debug> fmt::Debug for MaybeHttpsStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MaybeHttpsStream::Http(..) => f.pad("Http(..)"),
            MaybeHttpsStream::Https(..) => f.pad("Https(..)"),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for MaybeHttpsStream<T> {
    unsafe fn prepare_uninitialized_buffer(&self, buff: &mut [u8]) -> bool {
        match *self {
            MaybeHttpsStream::Http(ref s) => s.prepare_uninitialized_buffer(buff),
            MaybeHttpsStream::Https(ref s) => s.prepare_uninitialized_buffer(buff),
        }
    }

    fn poll_read(mut self: Pin<&mut Self>, ctx: &mut Context<'_>, buff: &mut [u8]) -> Poll<io::Result<usize>> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => AsyncRead::poll_read(Pin::new(s), ctx, buff),
            MaybeHttpsStream::Https(ref mut s) => AsyncRead::poll_read(Pin::new(s), ctx, buff),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for MaybeHttpsStream<T> {
    fn poll_write(mut self: Pin<&mut Self>, ctx: &mut Context<'_>, buff: &[u8]) -> Poll<io::Result<usize>> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => AsyncWrite::poll_write(Pin::new(s), ctx, buff),
            MaybeHttpsStream::Https(ref mut s) => AsyncWrite::poll_write(Pin::new(s), ctx, buff),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => AsyncWrite::poll_flush(Pin::new(s), ctx),
            MaybeHttpsStream::Https(ref mut s) => AsyncWrite::poll_flush(Pin::new(s), ctx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match *self {
            MaybeHttpsStream::Http(ref mut s) => AsyncWrite::poll_shutdown(Pin::new(s), ctx),
            MaybeHttpsStream::Https(ref mut s) => AsyncWrite::poll_shutdown(Pin::new(s), ctx),
        }
    }
}

#[derive(Clone)]
///HTTPs connect based on Rustls.
pub struct HttpsConnector {
    http: HttpConnector,
    config: Arc<tokio_rustls::rustls::ClientConfig>,
}

impl Connector for HttpsConnector {
    fn new() -> Self {
        let mut config = tokio_rustls::rustls::ClientConfig::new();
        config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

        Self {
            http: HttpConnector::new(),
            config: Arc::new(config),
        }
    }
}

impl fmt::Debug for HttpsConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpsConnector")
    }
}

impl Connect for HttpsConnector {
    type Transport = MaybeHttpsStream<<HttpConnector as Connect>::Transport>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = utils::fut::Either<MaybeHttpsConnecting<<HttpConnector as Connect>::Future>, MaybeHttpConnecting<<HttpConnector as Connect>::Future>>;

    fn connect(&self, dst: connect::Destination) -> Self::Future {
        let is_https = dst.scheme() == "https";

        match is_https {
            true => {
                let state = HttpsOnlyConnectingState::Conneting(self.http.connect(dst.clone()));

                let fut = HttpsOnlyConnecting {
                    dst,
                    config: self.config.clone(),
                    state,
                };

                utils::fut::Either::Left(MaybeHttpsConnecting(fut))
            },
            false => {
                utils::fut::Either::Right(MaybeHttpConnecting(self.http.connect(dst)))
            }
        }
    }
}

#[derive(Clone)]
///HTTPs only connect based on Rustls.
///
///Any attempt to connect over plain HTTP will result in corrupt message error.
pub struct HttpsOnlyConnector {
    http: HttpConnector,
    config: Arc<tokio_rustls::rustls::ClientConfig>,
}

impl Connector for HttpsOnlyConnector {
    ///Creates new instance with specified connector.
    fn new() -> Self {
        let mut config = tokio_rustls::rustls::ClientConfig::new();
        config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

        Self {
            http: HttpConnector::new(),
            config: Arc::new(config),
        }
    }
}

impl fmt::Debug for HttpsOnlyConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpsOnlyConnector")
    }
}

impl Connect for HttpsOnlyConnector {
    type Transport = TlsStream<<HttpConnector as Connect>::Transport>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = HttpsOnlyConnecting<<HttpConnector as Connect>::Future>;

    fn connect(&self, dst: connect::Destination) -> Self::Future {
        let state = HttpsOnlyConnectingState::Conneting(self.http.connect(dst.clone()));

        HttpsOnlyConnecting {
            dst,
            config: self.config.clone(),
            state,
        }
    }
}

enum HttpsOnlyConnectingState<T> {
    Conneting(T),
    Tls(tokio_rustls::Connect<tokio_net::tcp::TcpStream>, Option<Connected>),
}

///Ongoing HTTPS only connect
pub struct HttpsOnlyConnecting<T> {
    dst: connect::Destination,
    config: Arc<tokio_rustls::rustls::ClientConfig>,
    state: HttpsOnlyConnectingState<T>,
}

impl<F: Unpin + Future<Output = io::Result<(<HttpConnector as Connect>::Transport, Connected)>>> Future for HttpsOnlyConnecting<F> {
    type Output = Result<(TlsStream<<HttpConnector as Connect>::Transport>, Connected), Box<dyn std::error::Error + Send + Sync>>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        use tokio_rustls::rustls::Session;
        use tokio_rustls::webpki::{DNSNameRef};

        loop {
            self.state = match self.state {
                HttpsOnlyConnectingState::Conneting(ref mut connecting) => match Future::poll(unsafe { Pin::new_unchecked(connecting) }, ctx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(error.into())),
                    Poll::Ready(Ok((tcp, conn))) => match DNSNameRef::try_from_ascii_str(self.dst.host()) {
                        Ok(dns_name) => {
                            let cfg = self.config.clone();
                            let connector = tokio_rustls::TlsConnector::from(cfg);
                            HttpsOnlyConnectingState::Tls(connector.connect(dns_name, tcp), Some(conn))
                        },
                        Err(_) => return Poll::Ready(Err("invalid DNS name".into())),
                    }
                },
                HttpsOnlyConnectingState::Tls(ref mut connecting, ref mut conn) => match Future::poll(unsafe { Pin::new_unchecked(connecting) }, ctx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(error.into())),
                    Poll::Ready(Ok(tls)) => match tls.get_ref().1.get_alpn_protocol() {
                        Some(b"h2") => return Poll::Ready(Ok((tls, conn.take().unreach_none().negotiated_h2()))),
                        _ => return Poll::Ready(Ok((tls, conn.take().unreach_none()))),
                    }
                }
            }
        }
    }
}

///Ongoing HTTPS connect
pub struct MaybeHttpsConnecting<T>(HttpsOnlyConnecting<T>);

impl<F: Unpin + Future<Output = io::Result<(<HttpConnector as Connect>::Transport, Connected)>>> Future for MaybeHttpsConnecting<F> {
    type Output = Result<(MaybeHttpsStream<<HttpConnector as Connect>::Transport>, Connected), Box<dyn std::error::Error + Send + Sync>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|this| &mut this.0) };
        Future::poll(inner, ctx).map(|res| res.map(|(tls, conn)| (MaybeHttpsStream::Https(tls), conn)))
    }
}

///Ongoing HTTP connect
pub struct MaybeHttpConnecting<T>(T);

impl<F: Unpin + Future<Output = io::Result<(<HttpConnector as Connect>::Transport, Connected)>>> Future for MaybeHttpConnecting<F> {
    type Output = Result<(MaybeHttpsStream<<HttpConnector as Connect>::Transport>, Connected), Box<dyn std::error::Error + Send + Sync>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|this| &mut this.0) };
        Future::poll(inner, ctx).map(|res| res.map(|(tcp, conn)| (MaybeHttpsStream::Http(tcp), conn))).map_err(|error| error.into())
    }
}
