//!Client module
//!
//!Entry point to HTTP client side.
//!
//!## API highlights
//!
//!- [Client](struct.Client.html) - Wraps `hyper::Client` and provides various async methods to send requests
//!- [Request](request/struct.Request.html) - Entry point to creating requests.
//!- [Response](response/struct.Response.html) - Result of successful requests. Provides various async methods to read body.
//!
//!## Usage
//!
//!### Simple
//!
//!```rust, no_run
//!#![feature(async_await)]
//!
//!use yukikaze::{awaitic, client};
//!
//!async fn example() {
//!    let client = client::Client::default();
//!
//!    let req = client::Request::get("https://google.com").expect("To create request").empty();
//!    let mut result = awaitic!(client.send(req)).expect("Not timedout").expect("Successful");
//!    assert!(result.is_success());
//!
//!    let html = awaitic!(result.text()).expect("To read HTML");
//!    println!("Google page:\n{}", html);
//!}
//!```
//!
//!### Custom configuration
//!
//!```rust, no_run
//!#![feature(async_await)]
//!use yukikaze::{awaitic, client};
//!
//!use core::time;
//!
//!pub struct TimeoutCfg;
//!
//!impl client::config::Config for TimeoutCfg {
//!    type Connector = client::config::DefaultConnector;
//!    type Timer = client::config::DefaultTimer;
//!
//!    fn new_connector() -> Self::Connector {
//!        Self::Connector::new(4)
//!    }
//!
//!    fn timeout() -> time::Duration {
//!        //never times out
//!        time::Duration::from_secs(0)
//!    }
//!}
//!
//!async fn example() {
//!    let client = client::Client::<TimeoutCfg>::new();
//!
//!    let req = client::Request::get("https://google.com").expect("To create request").empty();
//!    let result = awaitic!(client.send(req)).expect("Not timedout").expect("Successful");
//!    assert!(result.is_success());
//!}
//!```

use hyper::client::connect::Connect;
use futures_util::future::FutureExt;

use core::marker::PhantomData;
use core::future::Future;
use core::fmt;
use std::path::Path;

use crate::header;

pub mod config;
pub mod request;
pub mod response;

pub use request::Request;
pub use response::Response;

///HTTP Client
pub struct Client<C=config::DefaultCfg> where C: config::Config, C: 'static,
//TODO: This shit should be removed once trait bounds for associated types will allow where clauses
<C::Connector as Connect>::Future: 'static, <C::Connector as Connect>::Transport: 'static
{
    inner: hyper::Client<C::Connector>,
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

///Alias to result of sending request.
pub type RequestResult = Result<response::Response, hyper::Error>;

impl<C: config::Config> Client<C> {
    ///Creates new instance of client with specified configuration.
    ///
    ///Use `Default` if you'd like to use [default](config/struct.DefaultCfg.html) config.
    pub fn new() -> Client<C> {
        let inner = C::config_hyper(&mut hyper::Client::builder()).build(C::new_connector());

        Self {
            inner,
            _config: PhantomData
        }
    }

    fn apply_headers(request: &mut request::Request) {
        C::default_headers(request);

        #[cfg(feature = "compu")]
        {
            const DEFAULT_COMPRESS: &'static str = "br, gzip, deflate";

            if C::decompress() {
                let headers = request.headers_mut();
                if !headers.contains_key(header::ACCEPT_ENCODING) && headers.contains_key(header::RANGE) {
                    headers.insert(header::ACCEPT_ENCODING, header::HeaderValue::from_static(DEFAULT_COMPRESS));
                }
            }
        }
    }

    ///Sends request, and returns response
    pub async fn request(&self, mut req: request::Request) -> RequestResult {
        Self::apply_headers(&mut req);

        #[cfg(feature = "carry_extensions")]
        let mut extensions = req.extract_extensions();

        let ongoing = self.inner.request(req.into());
        let ongoing = futures_util::compat::Compat01As03::new(ongoing).map(|res| res.map(|resp| response::Response::new(resp)));

        #[cfg(feature = "carry_extensions")]
        {
            awaitic!(ongoing).map(move |resp| resp.replace_extensions(&mut extensions))
        }
        #[cfg(not(feature = "carry_extensions"))]
        {
            awaitic!(ongoing)
        }
    }

    ///Sends request and returns response. Timed version.
    ///
    ///On timeout error it returns `async_timer::timed::Expired` as `Error`
    ///`Expired` implements `Future` that can be used to re-spawn ongoing request again.
    ///
    ///If request resolves in time returns `Result<response::Response, hyper::Error>` as `Ok`
    ///variant.
    pub async fn send(&self, mut req: request::Request) -> Result<RequestResult, async_timer::timed::Expired<impl Future<Output=RequestResult>, C::Timer>> {
        Self::apply_headers(&mut req);

        #[cfg(feature = "carry_extensions")]
        let mut extensions = req.extract_extensions();

        let ongoing = self.inner.request(req.into());
        let ongoing = futures_util::compat::Compat01As03::new(ongoing).map(|res| res.map(|resp| response::Response::new(resp)));

        let timeout = C::timeout();
        match timeout.as_secs() == 0 && timeout.subsec_nanos() == 0 {
            #[cfg(not(feature = "carry_extensions"))]
            true => Ok(awaitic!(ongoing)),
            #[cfg(feature = "carry_extensions")]
            true => Ok(awaitic!(ongoing).map(move |resp| resp.replace_extensions(&mut extensions))),
            false => {
                let job = async_timer::Timed::<_, C::Timer>::new(ongoing, timeout);
                #[cfg(not(feature = "carry_extensions"))]
                {
                    awaitic!(job)
                }
                #[cfg(feature = "carry_extensions")]
                {
                    awaitic!(job).map(move |res| res.map(move |resp| resp.replace_extensions(&mut extensions)))
                }
            }
        }
    }

    ///Sends request and returns response, while handling redirects. Timed version.
    ///
    ///On timeout error it returns `async_timer::timed::Expired` as `Error`
    ///`Expired` implements `Future` that can be used to re-spawn ongoing request again.
    ///
    ///If request resolves in time returns `Result<response::Response, hyper::Error>` as `Ok`
    ///variant.
    pub async fn send_redirect(&'static self, req: request::Request) -> Result<RequestResult, async_timer::timed::Expired<impl Future<Output=RequestResult> + 'static, C::Timer>> {
        let timeout = C::timeout();
        match timeout.as_secs() == 0 && timeout.subsec_nanos() == 0 {
            true => Ok(awaitic!(self.redirect_request(req))),
            false => {
                //Note on unsafety.
                //Here we assume that all references to self, as it is being 'static will be safe
                //within ongoing request regardless of when user will restart expired request.
                //But technically, even though it is static, user still should be able to move it
                //around so it is a bit unsafe in some edgy cases.
                let ongoing = self.redirect_request(req);
                let job = unsafe { async_timer::Timed::<_, C::Timer>::new_unchecked(ongoing, timeout) };
                awaitic!(job)
            }
        }
    }

    ///Sends request and returns response, while handling redirects.
    pub async fn redirect_request(&self, mut req: request::Request) -> RequestResult {
        use http::{Method, StatusCode};

        Self::apply_headers(&mut req);

        let mut rem_redirect = C::max_redirect_num();

        let mut method = req.parts.method.clone();
        let uri = req.parts.uri.clone();
        let mut headers = req.parts.headers.clone();
        let mut body = req.body.clone();
        #[cfg(feature = "carry_extensions")]
        let mut extensions = req.extract_extensions();

        loop {
            let ongoing = self.inner.request(req.into());
            let ongoing = futures_util::compat::Compat01As03::new(ongoing).map(|res| res.map(|resp| response::Response::new(resp)));
            let res = awaitic!(ongoing)?;

            match res.status() {
                StatusCode::SEE_OTHER => {
                    rem_redirect -= 1;
                    match rem_redirect {
                        #[cfg(feature = "carry_extensions")]
                        0 => return Ok(res.replace_extensions(&mut extensions)),
                        #[cfg(not(feature = "carry_extensions"))]
                        0 => return Ok(res),
                        _ => {
                            //All requests should be changed to GET with no body.
                            //In most cases it is result of successful POST.
                            body = None;
                            method = Method::GET;
                        }
                    }
                },
                StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND | StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT => {
                    rem_redirect -= 1;
                    match rem_redirect {
                        #[cfg(feature = "carry_extensions")]
                        0 => return Ok(res.replace_extensions(&mut extensions)),
                        #[cfg(not(feature = "carry_extensions"))]
                        0 => return Ok(res),
                        _ => (),
                    }
                }
                #[cfg(feature = "carry_extensions")]
                _ => return Ok(res.replace_extensions(&mut extensions)),
                #[cfg(not(feature = "carry_extensions"))]
                _ => return Ok(res),
            }

            let location = match res.headers().get(header::LOCATION).and_then(|loc| loc.to_str().ok()).and_then(|loc| loc.parse::<hyper::Uri>().ok()) {
                Some(loc) => match loc.scheme_part().is_some() {
                    //We assume that if scheme is present then it is absolute redirect
                    true => {
                        //Well, it is unlikely that host would be empty, but just in case, right?
                        if let Some(prev_host) = uri.authority_part().map(|part| part.host()) {
                            match loc.authority_part().map(|part| part.host() == prev_host).unwrap_or(false) {
                                true => (),
                                false => {
                                    headers.remove("authorization");
                                    headers.remove("cookie");
                                    headers.remove("cookie2");
                                    headers.remove("www-authenticate");
                                }
                            }
                        }

                        loc
                    },
                    //Otherwise it is relative to current location.
                    false => {
                        let current = Path::new(uri.path());
                        let loc = Path::new(loc.path());
                        let loc = current.join(loc);
                        let loc = loc.to_str().expect("Valid UTF-8 path").parse::<hyper::Uri>().expect("Valid URI");
                        let mut loc_parts = loc.into_parts();

                        loc_parts.scheme = uri.scheme_part().cloned();
                        loc_parts.authority = uri.authority_part().cloned();

                        hyper::Uri::from_parts(loc_parts).expect("Create redirect URI")
                    },
                },
                #[cfg(feature = "carry_extensions")]
                None => return Ok(res.replace_extensions(&mut extensions)),
                #[cfg(not(feature = "carry_extensions"))]
                None => return Ok(res),
            };

            let (mut parts, _) = hyper::Request::<()>::new(()).into_parts();
            parts.method = method.clone();
            parts.uri = location;
            parts.headers = headers.clone();

            req = request::Request {
                parts,
                body: body.clone()
            };
        }
    }
}
