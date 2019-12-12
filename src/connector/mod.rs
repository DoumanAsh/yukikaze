//! TLS module

#[cfg(feature = "rustls-on")]
pub mod rustls;

use std::io;
use core::{task, pin, fmt};
use core::future::Future;

async fn connect_tcp(dst: hyper::Uri) -> io::Result<tokio::net::TcpStream> {
    let host = match dst.host() {
        Some(host) => host,
        None => return Err(io::Error::new(io::ErrorKind::InvalidInput, "No host specified")),
    };

    let port = match dst.port() {
        Some(port) => port.as_u16(),
        None => match dst.scheme().map(|scheme| scheme.as_str()) {
            Some("https") => 443,
            _ => 80,
        }
    };

    match matsu!(tokio::net::TcpStream::connect((host, port))) {
        Ok(io) => return Ok(io),
        Err(_) => Err(io::Error::new(io::ErrorKind::NotFound, "Unable to connect")),
    }
}

#[derive(Clone, Copy, Default)]
///Plain HTTP Connector
pub struct HttpConnector {
}

impl hyper::service::Service<hyper::Uri> for HttpConnector {
    type Response = tokio::net::TcpStream;
    type Error = io::Error;
    type Future = pin::Pin<Box<dyn Future<Output = io::Result<tokio::net::TcpStream>> + Send>>;

    #[inline(always)]
    fn poll_ready(&mut self, _: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
        task::Poll::Ready(Ok(()))
    }

    #[inline(always)]
    fn call(&mut self, dst: hyper::Uri) -> Self::Future {
        //TODO: remove uncessary allocations
        //      Most likely need to work-around Unpin requirement
        Box::pin(connect_tcp(dst))
    }
}

impl fmt::Debug for HttpConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpConnector")
    }
}
