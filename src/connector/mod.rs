//! TLS module

#[cfg(feature = "rustls-on")]
pub mod rustls;

use std::io;
use core::{pin, task, fmt};
use core::future::Future;

use hyper::client::connect::{self, Connected};

///Describes Connector interface
pub trait Connector: hyper::client::connect::Connect {
    ///Creates new instance
    fn new() -> Self;
}

async fn connect_tcp(dst: connect::Destination) -> io::Result<(tokio_tcp::TcpStream, Connected)> {
    use std::net::ToSocketAddrs;

    let host = dst.host();
    let port = match dst.port() {
        Some(port) => port,
        None => match dst.scheme() {
            "https" => 443,
            _ => 80,
        }
    };

    let addrs = (host, port).to_socket_addrs()?;

    for addr in addrs {
        match matsu!(tokio_tcp::TcpStream::connect(&addr)) {
            Ok(io) => return Ok((io, Connected::new())),
            Err(_) => continue,
        }
    }

    return Err(io::Error::new(io::ErrorKind::NotFound, "Unable to connect"));
}

struct DummyPin<T>(T);

impl<T> Unpin for DummyPin<T> {}

impl<T: Future<Output = io::Result<(tokio_tcp::TcpStream, Connected)>>> Future for DummyPin<T> {
    type Output = io::Result<(tokio_tcp::TcpStream, Connected)>;

    fn poll(mut self: pin::Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let mut fut = self.as_mut();
        Future::poll(unsafe { pin::Pin::new_unchecked(&mut fut.0) }, ctx)
    }
}

#[derive(Clone)]
///Plain HTTP Connector
pub struct HttpConnector {
}

impl hyper::client::connect::Connect for HttpConnector {
    type Transport = tokio_tcp::TcpStream;
    type Error = io::Error;
    type Future = impl Future<Output = Result<(Self::Transport, Connected), Self::Error>> + Unpin + Send;

    fn connect(&self, dst: connect::Destination) -> Self::Future {
        //TODO: Check if Unpin is not available due to internal machinery of generated future or not
        DummyPin(connect_tcp(dst))
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
