//! TLS module

#[cfg(feature = "rustls-on")]
pub mod rustls;

///Describes Connector interface
pub trait Connector<R: hyper::client::connect::dns::Resolve>: hyper::client::connect::Connect {
    ///Creates new instance with specified resolver.
    fn with(resolver: R) -> Self;
}

impl<R: hyper::client::connect::dns::Resolve + Clone + Send + Sync> Connector<R> for hyper::client::HttpConnector<R> where R::Future: Send {
    fn with(resolver: R) -> Self {
        let mut this = hyper::client::HttpConnector::new_with_resolver(resolver);
        this.enforce_http(false);
        this
    }
}
