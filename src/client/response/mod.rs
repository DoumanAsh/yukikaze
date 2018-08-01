//!Response primitives.

use ::std::fs;
use ::std::time;
use ::std::ops::{Deref, DerefMut};

use ::header;

use ::cookie;
use ::tokio;
use ::hyper;
use ::futures;
use ::futures::Future;
use ::serde::de::DeserializeOwned;

type HyperResponse = hyper::Response<hyper::Body>;

///Extractor module.
pub mod extractor;

#[derive(Debug)]
///HTTP Response
pub struct Response {
    inner: HyperResponse,
}

impl Response {
    #[inline]
    ///Returns whether Response's status is informational.
    ///
    ///The response status code is in range 100 to 199
    pub fn is_info(&self) -> bool {
        self.inner.status().is_informational()
    }

    #[inline]
    ///Returns whether Response's status is successful.
    ///
    ///The response status code is in range 200 to 299
    pub fn is_success(&self) -> bool {
        self.inner.status().is_success()
    }

    #[inline]
    ///Returns whether Response's status is redirectional.
    ///
    ///The response status code is in range 300 to 399
    pub fn is_redirect(&self) -> bool {
        self.inner.status().is_redirection()
    }

    #[inline]
    ///Returns whether Response's status is error..
    ///
    ///The response status code is in range 400 to 599
    pub fn is_error(&self) -> bool {
        self.is_client_error() || self.is_internal_error()
    }

    #[inline]
    ///Returns whether Response's status is error caused by client.
    ///
    ///The response status code is in range 400 to 499
    pub fn is_client_error(&self) -> bool {
        self.inner.status().is_client_error()
    }

    #[inline]
    ///Returns whether Response's status is error caused by server.
    ///
    ///The response status code is in range 500 to 599
    pub fn is_internal_error(&self) -> bool {
        self.inner.status().is_client_error()
    }

    #[inline]
    ///Retrieves length of content to receive, if `Content-Length` exists.
    pub fn content_len(&self) -> Option<u64> {
        self.inner.headers()
                  .get(header::CONTENT_LENGTH)
                  .and_then(|header| header.to_str().ok())
                  .and_then(|header| header.parse().ok())
    }

    #[inline]
    ///Retrieves `Content-Encoding`, if header is not present `ContentEncoding::Idenity` is
    ///assumed.
    pub fn content_encoding(&self) -> header::ContentEncoding {
        self.inner.headers()
                  .get(header::CONTENT_ENCODING)
                  .and_then(|header| header.to_str().ok())
                  .map(|header| header.into())
                  .unwrap_or(header::ContentEncoding::Identity)
    }

    #[inline]
    ///Creates iterator of cookie from `Set-Cookie` header.
    pub fn cookies_iter(&self) -> extractor::CookieIter {
        extractor::CookieIter {
            iter: self.headers().get_all(header::SET_COOKIE).iter()
        }
    }

    #[inline]
    ///Retrieves owned cookies from `Set-Cookie` header.
    pub fn cookies(&self) -> Result<Vec<cookie::Cookie<'static>>, cookie::ParseError> {
        let mut cookies = Vec::new();

        for cook in self.cookies_iter() {
            cookies.push(cook?.into_owned());
        }

        Ok(cookies)
    }

    #[inline]
    ///Extracts Etags, if any.
    pub fn etag<'a>(&'a self) -> Option<extractor::Etag<'a>> {
        self.inner.headers().get(header::ETAG).map(|header| extractor::Etag::new(header))
    }

    #[inline]
    ///Extracts body as raw bytes.
    pub fn body(self) -> extractor::RawBody {
        extractor::RawBody::new(self)
    }

    #[inline]
    ///Extracts body as UTF-8 String
    pub fn text(self) -> extractor::Text {
        extractor::Text::new(self)
    }

    #[inline]
    ///Extracts body as JSON
    pub fn json<J: DeserializeOwned>(self) -> extractor::Json<J> {
        extractor::Json::new(self)
    }

    #[inline]
    ///Extracts body to file.
    pub fn file(self, file: fs::File) -> extractor::FileBody {
        extractor::FileBody::new(self, file)
    }
}

impl From<HyperResponse> for Response {
    fn from(inner: HyperResponse) -> Self {
        Self {
            inner
        }
    }
}

impl Deref for Response {
    type Target = HyperResponse;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Response {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Debug)]
///Describes possible response errors.
pub enum ResponseError {
    ///Response failed due to timeout
    Timeout,
    ///Hyper Error.
    HyperError(hyper::error::Error)
}

impl ResponseError {
    fn from_deadline(error: tokio::timer::DeadlineError<hyper::error::Error>) -> Self {
        match error.into_inner() {
            Some(error) => ResponseError::HyperError(error),
            None => ResponseError::Timeout,
        }
    }
}

#[must_use = "Future must be polled to actually get HTTP response"]
///Ongoing HTTP request.
pub struct FutureResponse {
    inner: tokio::timer::Deadline<hyper::client::ResponseFuture>
}

impl FutureResponse {
    pub(crate) fn new(inner: hyper::client::ResponseFuture, timeout: time::Duration) -> Self {
        let inner = tokio::timer::Deadline::new(inner, tokio::clock::now() + timeout);
        Self {
            inner
        }
    }
}

impl Future for FutureResponse {
    type Item = Response;
    type Error = ResponseError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        let result = async_unwrap!(self.inner.poll().map_err(|error| ResponseError::from_deadline(error))?);

        Ok(futures::Async::Ready(result.into()))
    }
}
