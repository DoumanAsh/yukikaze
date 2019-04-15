//!Response primitives.

use std::fs;
use std::str::FromStr;
use std::ops::{Deref, DerefMut};

use crate::header;
use super::upgrade;

use serde::de::DeserializeOwned;

pub(crate) type HyperResponse = hyper::Response<hyper::Body>;

///Response errors.
pub mod errors;
///Extractor module.
pub mod extractor;
mod future;
#[cfg(feature = "rt-client")]
///Redirect support module.
pub(crate) mod redirect;

pub(crate) use self::future::FutureResponseParams;
pub use self::future::FutureResponse;
#[cfg(feature = "rt-client")]
pub use self::redirect::HyperRedirectFuture;

///Yukikaze-sama's regular future response.
pub type Future = FutureResponse<hyper::client::ResponseFuture>;
#[cfg(feature = "rt-client")]
///Yukikaze-sama's future response with redirect support.
pub type RedirectFuture = FutureResponse<redirect::HyperRedirectFuture>;

#[derive(Debug)]
///HTTP Response
pub struct Response {
    pub(crate) inner: HyperResponse,
}

impl Response {
    #[inline]
    ///Retrieves status code
    pub fn status(&self) -> http::StatusCode {
        self.inner.status()
    }

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
    ///Returns whether Response's status is re-directional.
    ///
    ///The response status code is in range 300 to 399
    pub fn is_redirect(&self) -> bool {
        self.inner.status().is_redirection()
    }

    #[inline]
    ///Returns whether Response's status is error.
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
    ///Returns whether Response's status indicates upgrade
    pub fn is_upgrade(&self) -> bool {
        self.inner.status() == http::StatusCode::SWITCHING_PROTOCOLS
    }

    #[inline]
    ///Retrieves reference to http extension map
    pub fn extensions(&self) -> &http::Extensions {
        self.inner.extensions()
    }

    #[inline]
    ///Retrieves mutable reference to http extension map
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        self.inner.extensions_mut()
    }

    #[inline]
    ///Access response's headers
    pub fn headers(&self) -> &http::HeaderMap {
        self.inner.headers()
    }

    #[inline]
    ///Retrieves `Content-Type` as Mime, if any.
    pub fn mime(&self) -> Result<Option<mime::Mime>, errors::ContentTypeError> {
        let content_type = self.headers().get(header::CONTENT_TYPE)
                                         .and_then(|content_type| content_type.to_str().ok());

        if let Some(content_type) = content_type {
            content_type.parse::<mime::Mime>().map(Some).map_err(errors::ContentTypeError::from)
        } else {
            Ok(None)
        }
    }

    #[cfg(feature = "encoding")]
    ///Retrieves content's charset encoding, if any.
    ///
    ///If it is omitted, UTF-8 is assumed.
    pub fn charset_encoding(&self) -> Result<&'static encoding_rs::Encoding, errors::ContentTypeError> {
        let mime = self.mime()?;
        let mime = mime.as_ref().and_then(|mime| mime.get_param(mime::CHARSET));

        match mime {
            Some(charset) => match encoding_rs::Encoding::for_label(charset.as_str().as_bytes()) {
                Some(enc) => Ok(enc),
                None => Err(errors::ContentTypeError::UnknownEncoding)
            },
            None => Ok(encoding_rs::UTF_8),
        }
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
    ///Retrieves `Content-Disposition`, if it valid one is present.
    pub fn content_disposition(&self) -> Option<header::ContentDisposition> {
        self.inner.headers()
                  .get(header::CONTENT_DISPOSITION)
                  .and_then(|header| header.to_str().ok())
                  .and_then(|header| header::ContentDisposition::from_str(header).ok())
    }

    #[inline]
    ///Creates iterator of cookie from `Set-Cookie` header.
    pub fn cookies_iter(&self) -> extractor::CookieIter {
        extractor::CookieIter {
            iter: self.headers().get_all(header::SET_COOKIE).iter()
        }
    }

    #[inline]
    ///Creates jar from cookies in response.
    pub fn cookies_jar(&self) -> Result<cookie2::CookieJar, cookie2::ParseError> {
        let mut jar = cookie2::CookieJar::new();

        for cook in self.cookies_iter() {
            jar.add(cook?.into_owned());
        }

        Ok(jar)
    }

    #[inline]
    ///Retrieves all cookies from `Set-Cookie` headers.
    pub fn cookies(&self) -> Result<Vec<cookie2::Cookie<'static>>, cookie2::ParseError> {
        let mut cookies = Vec::new();

        for cook in self.cookies_iter() {
            cookies.push(cook?.into_owned());
        }

        Ok(cookies)
    }

    #[inline]
    ///Extracts `Last-Modified` date, if valid one is present.
    pub fn last_modified(&self) -> Option<httpdate::HttpDate> {
        self.inner.headers().get(header::LAST_MODIFIED)
                            .and_then(|header| header.to_str().ok())
                            .and_then(|header| httpdate::HttpDate::from_str(header.trim()).ok())
    }

    #[inline]
    ///Extracts Etags, if any.
    pub fn etag(&self) -> Option<etag::EntityTag> {
        self.inner.headers().get(header::ETAG)
                            .and_then(|header| header.to_str().ok())
                            .and_then(|header| header.trim().parse().ok())
    }

    #[inline]
    ///Extracts body as raw bytes.
    pub fn body(self) -> extractor::RawBody<extractor::notify::Noop> {
        extractor::RawBody::new(self, extractor::notify::Noop)
    }

    #[inline]
    ///Extracts body as UTF-8 String
    pub fn text(self) -> extractor::Text<extractor::notify::Noop> {
        extractor::Text::new(self, extractor::notify::Noop)
    }

    #[inline]
    ///Extracts body as JSON
    pub fn json<J: DeserializeOwned>(self) -> extractor::Json<J, extractor::notify::Noop> {
        extractor::Json::new(self, extractor::notify::Noop)
    }

    #[inline]
    ///Extracts body to file.
    ///
    ///# Panics
    ///
    ///- If file is read-only. Checked only when debug assertions are on.
    pub fn file(self, file: fs::File) -> extractor::FileBody<extractor::notify::Noop> {
        #[cfg(debug_assertions)]
        {
            let meta = file.metadata().expect("To be able to get metadata");
            debug_assert!(!meta.permissions().readonly(), "File is read-only");
        }

        extractor::FileBody::new(self, file, extractor::notify::Noop)
    }

    ///Prepares upgrade for the request.
    pub fn upgrade<U: upgrade::Upgrade>(self, _: U) -> U::Result {
        U::upgrade_response(self)
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
