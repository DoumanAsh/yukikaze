//! Provides response future that handles redirection

use ::std::path::Path;

use ::bytes;
use ::futures;
use ::futures::Future;
use ::hyper;
use ::http::{Method, Uri, HeaderMap, StatusCode};

use ::header;
use ::rt;

use super::{HyperResponse};

#[derive(Debug)]
pub(crate) struct Cache {
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Option<bytes::Bytes>
}

impl Cache {
    pub(crate) fn new(req: &super::super::Request) -> Self {
        Self {
            method: req.method().clone(),
            uri: req.uri().clone(),
            headers: req.headers().clone(),
            body: req.body.clone()
        }
    }
}

#[must_use = "Future must be polled to actually get HTTP response"]
#[derive(Debug)]
///Hyper's Future response with redirect support.
///
///Redirect policy:
///
///Yukikaze-sama, being wise, decided to avoid historical somewhat "buggy" behavior when
///client changes POST to GET and strips body in all redirects aside from 303.
///
///Note that standard doesn't require 301/302 to change initial request
///While 308/307 guarantee request to be unchanged.
///
///## Permanent redirections
///
///- 301 - Redirection without changes.
///- 308 - Redirection without changes.
///
///## Temporary  redirections
///
///- 302 - Redirection without changes.
///- 303 - All requests are transformed into GET without body.
///- 307 - Redirection without changes.
pub struct HyperRedirectFuture {
    inner: Option<hyper::client::ResponseFuture>,
    cache: Cache,
    rem_redirect: usize,
}

impl HyperRedirectFuture {
    pub(crate) fn new(inner: hyper::client::ResponseFuture, cache: Cache, rem_redirect: usize) -> Self {
        Self {
            inner: Some(inner),
            cache,
            rem_redirect,
        }
    }
}

impl Future for HyperRedirectFuture {
    type Item = HyperResponse;
    type Error = hyper::Error;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let redirect = match self.inner.as_mut() {
                Some(inner) => match inner.poll() {
                    Ok(futures::Async::Ready(result)) => match result.status() {
                        StatusCode::SEE_OTHER => {
                            self.rem_redirect -= 1;
                            match self.rem_redirect {
                                0 => return Ok(futures::Async::Ready(result.into())),
                                _ => {
                                    //All requests should be changed to GET with no body.
                                    //In most cases it is result of successful POST.
                                    self.cache.body = None;
                                    self.cache.method = Method::GET;
                                    result
                                }
                            }
                        },
                        StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND | StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT => {
                            self.rem_redirect -= 1;
                            match self.rem_redirect {
                                0 => return Ok(futures::Async::Ready(result.into())),
                                _ => result,
                            }
                        },
                        _ => return Ok(futures::Async::Ready(result.into())),
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error),
                },
                None => unreach!()
            };

            let location = match redirect.headers().get(header::LOCATION).and_then(|loc| loc.to_str().ok()).and_then(|loc| loc.parse::<hyper::Uri>().ok()) {
                Some(loc) => match loc.scheme_part().is_some() {
                    //We assume that if scheme is present then it is absolute redirect
                    true => {
                        //Well, it is unlikely that host would be empty, but just in case, right?
                        if let Some(prev_host) = self.cache.uri.authority_part().map(|part| part.host()) {
                            match loc.authority_part().map(|part| part.host() == prev_host).unwrap_or(false) {
                                true => (),
                                false => {
                                    self.cache.headers.remove("authorization");
                                    self.cache.headers.remove("cookie");
                                    self.cache.headers.remove("cookie2");
                                    self.cache.headers.remove("www-authenticate");
                                }
                            }
                        }

                        loc
                    },
                    //Otherwise it is relative to current location.
                    false => {
                        let current = Path::new(self.cache.uri.path());
                        let loc = Path::new(loc.path());
                        let loc = current.join(loc);
                        let loc = loc.to_str().expect("Valid UTF-8 path").parse::<hyper::Uri>().expect("Valid URI");
                        let mut loc_parts = loc.into_parts();

                        loc_parts.scheme = self.cache.uri.scheme_part().cloned();
                        loc_parts.authority = self.cache.uri.authority_part().cloned();

                        hyper::Uri::from_parts(loc_parts).expect("Create redirect URI")
                    },
                },
                None => return Ok(futures::Async::Ready(redirect.into()))
            };

            let body = self.cache.body.as_ref().map(|body| body.clone().into()).unwrap_or(hyper::Body::empty());
            let mut new_req = hyper::Request::builder().method(self.cache.method.clone())
                                                       .uri(location)
                                                       .body(body)
                                                       .expect("To crate redirect");
            *new_req.headers_mut() = self.cache.headers.clone();

            self.inner = Some(rt::client::execute_raw_hyper(new_req));
        }
    }
}
