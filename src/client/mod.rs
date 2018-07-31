//!Client module

use ::header;

use ::hyper;
use ::hyper_rustls;

use std::marker::PhantomData;

type HyperClient = hyper::Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>;

pub mod config;
pub mod request;
pub mod response;

pub use self::request::Request;

///Describes HTTP Client functionality
pub trait HttpClient {
    ///Starts sending HTTP request.
    fn execute(&self, request: request::Request) -> response::FutureResponse;
}

///HTTP Client
pub struct Client<C=config::DefaultCfg> {
    inner: HyperClient,
    _config: PhantomData<C>
}

impl Default for Client {
    ///Creates Client with default configuration.
    fn default() -> Self {
        Client::<config::DefaultCfg>::new()
    }
}

impl<C: config::Config> Client<C> {
    ///Creates new instance of client with specified configuration.
    ///
    ///Use `Default` if you'd like to use [default](config/struct.DefaultCfg.html) config.
    pub fn new() -> Client<C> {
        let https = hyper_rustls::HttpsConnector::new(C::dns_threads());
        let inner = C::config_hyper(&mut hyper::Client::builder()).build(https);

        Self {
            inner,
            _config: PhantomData
        }
    }
}

impl<C: config::Config> HttpClient for Client<C> {
    fn execute(&self, request: request::Request) -> response::FutureResponse {
        const DEFAULT_COMPRESS: &'static str = "gzip, deflate";

        let mut request = request.inner;

        #[cfg(feature = "flate2")]
        {
            if C::decompress() {
                let headers = request.headers_mut();
                if !headers.contains_key(header::ACCEPT_ENCODING) && headers.contains_key(header::RANGE) {
                    headers.insert(header::ACCEPT_ENCODING, header::HeaderValue::from_static(DEFAULT_COMPRESS));
                }
            }
        }

        response::FutureResponse::new(self.inner.request(request), C::timeout())
    }
}
