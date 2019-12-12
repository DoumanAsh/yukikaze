//! Rustls connector

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::client::TlsStream;

use super::super::{HttpConnector};
use crate::utils;

use std::io;
use std::sync::Arc;
use core::fmt;
use core::future::Future;
use core::task::{Poll, Context};
use core::pin::{Pin};
use core::mem::MaybeUninit;

///HTTPS Stream
pub struct HttpsStream<T> {
    inner: TlsStream<T>,
}

impl hyper::client::connect::Connection for HttpsStream<tokio::net::TcpStream> {
    fn connected(&self) -> hyper::client::connect::Connected {
        self.inner.get_ref().0.connected()
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncRead for HttpsStream<T> {
    #[inline(always)]
    unsafe fn prepare_uninitialized_buffer(&self, buff: &mut [MaybeUninit<u8>]) -> bool {
        self.inner.prepare_uninitialized_buffer(buff)
    }

    #[inline(always)]
    fn poll_read(mut self: Pin<&mut Self>, ctx: &mut Context<'_>, buff: &mut [u8]) -> Poll<io::Result<usize>> {
        AsyncRead::poll_read(Pin::new(&mut self.inner), ctx, buff)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for HttpsStream<T> {
    #[inline(always)]
    fn poll_write(mut self: Pin<&mut Self>, ctx: &mut Context<'_>, buff: &[u8]) -> Poll<io::Result<usize>> {
        AsyncWrite::poll_write(Pin::new(&mut self.inner), ctx, buff)
    }

    #[inline(always)]
    fn poll_flush(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.inner), ctx)
    }

    #[inline(always)]
    fn poll_shutdown(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.inner), ctx)
    }
}

impl<T> From<TlsStream<T>> for HttpsStream<T> {
    #[inline(always)]
    fn from(tls: TlsStream<T>) -> Self {
        HttpsStream {
            inner: tls,
        }
    }
}

impl<T> Into<TlsStream<T>> for HttpsStream<T> {
    #[inline(always)]
    fn into(self) -> TlsStream<T> {
        self.inner
    }
}

/// A stream that might be protected with TLS.
pub enum MaybeHttpsStream<T> {
    /// A stream over plain text.
    Http(T),
    /// A stream protected with TLS.
    Https(TlsStream<T>),
}

impl hyper::client::connect::Connection for MaybeHttpsStream<tokio::net::TcpStream> {
    fn connected(&self) -> hyper::client::connect::Connected {
        match self {
            MaybeHttpsStream::Http(tcp) => tcp.connected(),
            MaybeHttpsStream::Https(tls) => tls.get_ref().0.connected(),
        }
    }
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
    unsafe fn prepare_uninitialized_buffer(&self, buff: &mut [MaybeUninit<u8>]) -> bool {
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
    ///Underlying HTTP connector
    pub http: HttpConnector,
    config: Arc<tokio_rustls::rustls::ClientConfig>,
}

impl Default for HttpsConnector {
    fn default() -> Self {
        let mut config = tokio_rustls::rustls::ClientConfig::new();
        config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

        Self {
            http: HttpConnector::default(),
            config: Arc::new(config),
        }
    }
}

impl fmt::Debug for HttpsConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpsConnector")
    }
}

impl hyper::service::Service<hyper::Uri> for HttpsConnector {
    type Response = MaybeHttpsStream<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>;
    type Error = io::Error;
    type Future = utils::fut::Either<MaybeHttpsConnecting<<HttpConnector as hyper::service::Service<hyper::Uri>>::Future>, MaybeHttpConnecting<<HttpConnector as hyper::service::Service<hyper::Uri>>::Future>>;

    #[inline(always)]
    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.http.poll_ready(ctx).map_err(Into::into)
    }

    fn call(&mut self, dst: hyper::Uri) -> Self::Future {
        let is_https = dst.scheme().unwrap().as_str() == "https";

        match is_https {
            true => {
                let state = HttpsOnlyConnectingState::Conneting(self.http.call(dst.clone()));

                let fut = HttpsOnlyConnecting {
                    dst,
                    config: self.config.clone(),
                    state,
                };

                utils::fut::Either::Left(MaybeHttpsConnecting(fut))
            },
            false => {
                utils::fut::Either::Right(MaybeHttpConnecting(self.http.call(dst)))
            }
        }
    }
}

#[derive(Clone)]
///HTTPs only connect based on Rustls.
///
///Any attempt to connect over plain HTTP will result in corrupt message error.
pub struct HttpsOnlyConnector {
    ///Underlying HTTP connector
    pub http: HttpConnector,
    config: Arc<tokio_rustls::rustls::ClientConfig>,
}

impl Default for HttpsOnlyConnector {
    ///Creates new instance with specified connector.
    fn default() -> Self {
        let mut config = tokio_rustls::rustls::ClientConfig::new();
        config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

        Self {
            http: HttpConnector::default(),
            config: Arc::new(config),
        }
    }
}

impl fmt::Debug for HttpsOnlyConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpsOnlyConnector")
    }
}

impl hyper::service::Service<hyper::Uri> for HttpsOnlyConnector {
    type Response = HttpsStream<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>;
    type Error = io::Error;
    type Future = HttpsOnlyConnecting<<HttpConnector as hyper::service::Service<hyper::Uri>>::Future>;

    #[inline(always)]
    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.http.poll_ready(ctx)
    }

    fn call(&mut self, dst: hyper::Uri) -> Self::Future {
        let state = HttpsOnlyConnectingState::Conneting(self.http.call(dst.clone()));

        HttpsOnlyConnecting {
            dst,
            config: self.config.clone(),
            state,
        }
    }
}

enum HttpsOnlyConnectingState<T> {
    Conneting(T),
    Tls(tokio_rustls::Connect<tokio::net::TcpStream>),
}

///Ongoing HTTPS only connect
pub struct HttpsOnlyConnecting<T> {
    dst: hyper::Uri,
    config: Arc<tokio_rustls::rustls::ClientConfig>,
    state: HttpsOnlyConnectingState<T>,
}

impl<F: Unpin + Future<Output = io::Result<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>>> Future for HttpsOnlyConnecting<F> {
    type Output = Result<HttpsStream<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>, io::Error>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        use tokio_rustls::rustls::Session;
        use tokio_rustls::webpki::{DNSNameRef};

        loop {
            self.state = match self.state {
                HttpsOnlyConnectingState::Conneting(ref mut connecting) => match Future::poll(unsafe { Pin::new_unchecked(connecting) }, ctx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                    Poll::Ready(Ok(tcp)) => match DNSNameRef::try_from_ascii_str(self.dst.host().unwrap()) {
                        Ok(dns_name) => {
                            let cfg = self.config.clone();
                            let connector = tokio_rustls::TlsConnector::from(cfg);
                            HttpsOnlyConnectingState::Tls(connector.connect(dns_name, tcp))
                        },
                        Err(_) => return Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid DNS name"))),
                    }
                },
                HttpsOnlyConnectingState::Tls(ref mut connecting) => match Future::poll(unsafe { Pin::new_unchecked(connecting) }, ctx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                    Poll::Ready(Ok(tls)) => match tls.get_ref().1.get_alpn_protocol() {
                        Some(b"h2") => return Poll::Ready(Ok(tls.into())),
                        _ => return Poll::Ready(Ok(tls.into())),
                    }
                }
            }
        }
    }
}

///Ongoing HTTPS connect
pub struct MaybeHttpsConnecting<T>(HttpsOnlyConnecting<T>);

impl<F: Unpin + Future<Output = io::Result<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>>> Future for MaybeHttpsConnecting<F> {
    type Output = Result<MaybeHttpsStream<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>, io::Error>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|this| &mut this.0) };
        Future::poll(inner, ctx).map(|res| res.map(|tls| MaybeHttpsStream::Https(tls.into())))
    }
}

///Ongoing HTTP connect
pub struct MaybeHttpConnecting<T>(T);

impl<F: Unpin + Future<Output = io::Result<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>>> Future for MaybeHttpConnecting<F> {
    type Output = Result<MaybeHttpsStream<<HttpConnector as hyper::service::Service<hyper::Uri>>::Response>, io::Error>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|this| &mut this.0) };
        Future::poll(inner, ctx).map(|res| res.map(|tcp| MaybeHttpsStream::Http(tcp)))
    }
}
