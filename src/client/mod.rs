//!Client module
//!
//!Yukikaze-sama's HTTP Client is plain wrapper over hyper's client.
//!In order to configure it user should use [Config](config/trait.Config.html)
//!
//!## Providing configuration
//!
//!```rust
//!extern crate yukikaze;
//!
//!use yukikaze::client;
//!use yukikaze::client::config::{Config, DefaultCfg};
//!
//!use std::time::Duration;
//!
//!struct Conf;
//!
//!impl Config for Conf {
//!    fn timeout() -> Duration {
//!        Duration::from_secs(10)
//!    }
//!
//!    fn default_headers(request: &mut client::Request) {
//!        DefaultCfg::default_headers(request);
//!        //We can set Yukikaze-sama default headers
//!        //and our own!
//!    }
//!}
//!
//!let _client = client::Client::<Conf>::new();
//!//Use client now
//!
//!```

use ::header;

use ::hyper;
use ::hyper_rustls;

use std::fmt;
use std::marker::PhantomData;

type HyperClient = hyper::Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>;

pub mod config;
pub mod request;
pub mod response;

use self::request::HyperRequest;

pub use self::request::Request;

///Describes HTTP Client functionality
pub trait HttpClient {
    ///Starts sending HTTP request.
    fn execute(&self, request: request::Request) -> response::Future;
    #[cfg(feature = "rt")]
    ///Starts sending HTTP request with redirect support.
    fn with_redirect(&self, request: request::Request) -> response::RedirectFuture;
    ///Executes raw hyper request and returns its future.
    fn execute_raw_hyper(&self, request: HyperRequest) -> hyper::client::ResponseFuture;
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

impl<C: config::Config> fmt::Debug for Client<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Yukikaze {{ HyperClient={:?} }}", self.inner)
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

    fn apply_headers(request: &mut request::Request) {
        C::default_headers(request);

        #[cfg(feature = "flate2")]
        {
            const DEFAULT_COMPRESS: &'static str = "gzip, deflate";

            if C::decompress() {
                let headers = request.headers_mut();
                if !headers.contains_key(header::ACCEPT_ENCODING) && headers.contains_key(header::RANGE) {
                    headers.insert(header::ACCEPT_ENCODING, header::HeaderValue::from_static(DEFAULT_COMPRESS));
                }
            }
        }
    }
}

impl<C: config::Config> HttpClient for Client<C> {
    fn execute(&self, mut request: request::Request) -> response::Future {
        Self::apply_headers(&mut request);

        response::FutureResponse::new(self.inner.request(request.into()), C::timeout())
    }

    #[cfg(feature = "rt")]
    fn with_redirect(&self, mut request: request::Request) -> response::RedirectFuture {
        Self::apply_headers(&mut request);
        let cache = response::redirect::Cache::new(&request);
        let future = response::redirect::HyperRedirectFuture::new(self.inner.request(request.into()), cache, C::max_redirect_num());

        response::RedirectFuture::new(future, C::timeout())
    }

    fn execute_raw_hyper(&self, request: HyperRequest) -> hyper::client::ResponseFuture {
        self.inner.request(request)
    }
}
