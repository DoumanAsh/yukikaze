//! TLS module

#[cfg(feature = "rustls-on")]
pub mod rustls;

use std::io;
use core::{pin, fmt};
use core::future::Future;

use hyper::client::connect::{self, Connected};

///Describes Connector interface
pub trait Connector: hyper::client::connect::Connect {
    ///Creates new instance
    fn new() -> Self;
}

async fn connect_tcp(dst: connect::Destination) -> io::Result<(tokio_net::tcp::TcpStream, Connected)> {
    let host = dst.host();
    let port = match dst.port() {
        Some(port) => port,
        None => match dst.scheme() {
            "https" => 443,
            _ => 80,
        }
    };

    match matsu!(tokio_net::tcp::TcpStream::connect((host, port))) {
        Ok(io) => return Ok((io, Connected::new())),
        Err(_) => Err(io::Error::new(io::ErrorKind::NotFound, "Unable to connect")),
    }
}

#[derive(Clone)]
///Plain HTTP Connector
pub struct HttpConnector {
}

impl hyper::client::connect::Connect for HttpConnector {
    type Transport = tokio_net::tcp::TcpStream;
    type Error = io::Error;
    type Future = pin::Pin<Box<dyn Future<Output = io::Result<(tokio_net::tcp::TcpStream, Connected)>> + Send>>;

    #[inline(always)]
    fn connect(&self, dst: connect::Destination) -> Self::Future {
        //TODO: remove uncessary allocations
        //      Most likely need to work-around Unpin requirement
        Box::pin(connect_tcp(dst))
    }
}

impl Connector for HttpConnector {
    fn new() -> Self {
        Self {
        }
    }
}

impl fmt::Debug for HttpConnector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("HttpConnector")
    }
}
