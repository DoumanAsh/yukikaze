//! Rustls connector

use tokio_io::{AsyncRead, AsyncWrite};
use tokio_rustls::client::TlsStream;
use hyper::client::connect::{self, Connected, Connect};
use futures_util::{TryFutureExt, FutureExt};

use super::super::{HttpConnector, Connector};

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
    type Future = impl Future<Output = Result<(Self::Transport, Connected), Self::Error>> + Unpin + Send;

    fn connect(&self, dst: connect::Destination) -> Self::Future {
        use tokio_rustls::rustls::Session;
        use tokio_rustls::webpki::{DNSName, DNSNameRef};

        let is_https = dst.scheme() == "https";

        match is_https {
            true => {
                let cfg = self.config.clone();
                let connector = tokio_rustls::TlsConnector::from(cfg);

                let hostname = dst.host().to_string();
                let fut = self.http.connect(dst).err_into().and_then(move |(tcp, conn)| match DNSNameRef::try_from_ascii_str(&hostname) {
                    Ok(dns_name) => futures_util::future::ready(Ok((tcp, conn, DNSName::from(dns_name)))),
                    Err(_) => futures_util::future::ready(Err("invalid DNS name".into())),
                }).and_then(move |(tcp, conn, dns_name)| connector.connect(dns_name.as_ref(), tcp).and_then(|tls| match tls.get_ref().1.get_alpn_protocol() {
                    Some(b"h2") => futures_util::future::ready(Ok((MaybeHttpsStream::Https(tls), conn.negotiated_h2()))),
                    _ => futures_util::future::ready(Ok((MaybeHttpsStream::Https(tls), conn))),

                }).err_into());

                crate::utils::fut::Either::Left(fut)
            },
            false => {
                let fut = self.http.connect(dst).map(|res| res.map(|(tcp, conn)| (MaybeHttpsStream::Http(tcp), conn)).map_err(Into::into));
                crate::utils::fut::Either::Right(fut)
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
    type Future = impl Future<Output = Result<(Self::Transport, Connected), Self::Error>> + Unpin + Send;

    fn connect(&self, dst: connect::Destination) -> Self::Future {
        use tokio_rustls::rustls::Session;
        use tokio_rustls::webpki::{DNSName, DNSNameRef};

        let cfg = self.config.clone();
        let connector = tokio_rustls::TlsConnector::from(cfg);

        let hostname = dst.host().to_string();
        self.http.connect(dst).err_into().and_then(move |(tcp, conn)| match DNSNameRef::try_from_ascii_str(&hostname) {
            Ok(dns_name) => futures_util::future::ready(Ok((tcp, conn, DNSName::from(dns_name)))),
            Err(_) => futures_util::future::ready(Err("invalid DNS name".into())),
        }).and_then(move |(tcp, conn, dns_name)| connector.connect(dns_name.as_ref(), tcp).and_then(|tls| match tls.get_ref().1.get_alpn_protocol() {
            Some(b"h2") => futures_util::future::ready(Ok((tls, conn.negotiated_h2()))),
            _ => futures_util::future::ready(Ok((tls, conn))),

        }).err_into())
    }
}
