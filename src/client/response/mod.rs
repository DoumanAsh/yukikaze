//! Response module.

use core::ops::{Deref, DerefMut};
use core::str::FromStr;
use core::future::Future;
use core::mem;
use std::fs;

use crate::{extractor, header, upgrade};

pub mod errors;

pub(crate) type HyperResponse = hyper::Response<hyper::Body>;

#[derive(Debug)]
///HTTP Response
pub struct Response {
    inner: HyperResponse,
}

impl Response {
    #[inline]
    ///Creates new instance from existing hyper response.
    pub fn new(hyper: HyperResponse) -> Self {
        Self {
            inner: hyper
        }
    }

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

    #[cfg(feature = "carry_extensions")]
    #[inline]
    ///Retrieves mutable reference to http extension map
    pub(crate) fn replace_extensions(mut self, extensions: &mut http::Extensions) -> Self {
        core::mem::swap(extensions, self.extensions_mut());
        self
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
    pub fn content_len(&self) -> Option<usize> {
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
        extractor::CookieIter::new(self.headers().get_all(header::SET_COOKIE).iter())
    }

    #[inline]
    ///Creates jar from cookies in response.
    pub fn cookies_jar(&self) -> Result<cookie::CookieJar, cookie::ParseError> {
        let mut jar = cookie::CookieJar::new();

        for cook in self.cookies_iter() {
            jar.add(cook?.into_owned());
        }

        Ok(jar)
    }

    #[inline]
    ///Retrieves all cookies from `Set-Cookie` headers.
    pub fn cookies(&self) -> Result<Vec<cookie::Cookie<'static>>, cookie::ParseError> {
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

    #[inline(always)]
    fn extract_body(&mut self) -> (header::ContentEncoding, Option<usize>, hyper::Body) {
        let encoding = self.content_encoding();
        let buffer_size = self.content_len();
        let mut body = hyper::Body::empty();

        mem::swap(&mut body, self.inner.body_mut());

        (encoding, buffer_size, body)
    }

    ///Extracts Response's body as raw bytes.
    pub fn body(&mut self) -> impl Future<Output=Result<bytes::Bytes, extractor::BodyReadError>> {
        let (encoding, buffer_size, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        extractor::raw_bytes(body, encoding, buffer_size)
    }

    ///Extracts Response's body as text
    pub fn text(&mut self) -> impl Future<Output=Result<String, extractor::BodyReadError>> {
        let (encoding, buffer_size, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        #[cfg(feature = "encoding")]
        {
            let charset = self.charset_encoding().unwrap_or(encoding_rs::UTF_8);
            extractor::text_charset(body, encoding, buffer_size, charset)
        }

        #[cfg(not(feature = "encoding"))]
        {
            extractor::text(body, encoding, buffer_size)
        }
    }

    ///Extracts Response's body as JSON
    pub fn json<J: serde::de::DeserializeOwned>(&mut self) -> impl Future<Output=Result<J, extractor::BodyReadError>> {
        let (encoding, buffer_size, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        #[cfg(feature = "encoding")]
        {
            let charset = self.charset_encoding().unwrap_or(encoding_rs::UTF_8);
            extractor::json_charset(body, encoding, buffer_size, charset)
        }

        #[cfg(not(feature = "encoding"))]
        {
            extractor::json(body, encoding, buffer_size)
        }
    }

    ///Extracts Response's body into file
    pub fn file(&mut self, file: fs::File) -> impl Future<Output=Result<fs::File, extractor::BodyReadError>> {
        #[cfg(debug_assertions)]
        {
            let meta = file.metadata().expect("To be able to get metadata");
            debug_assert!(!meta.permissions().readonly(), "File is read-only");
        }

        let (encoding, _, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        extractor::file(file, body, encoding)
    }

    ///Extracts Response's body as raw bytes.
    pub fn body_notify<N: extractor::Notifier>(&mut self, notify: N) -> impl Future<Output=Result<bytes::Bytes, extractor::BodyReadError>> {
        let (encoding, buffer_size, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        extractor::raw_bytes_notify(body, encoding, buffer_size, notify)
    }

    ///Extracts Response's body as text
    pub fn text_notify<N: extractor::Notifier>(&mut self, notify: N) -> impl Future<Output=Result<String, extractor::BodyReadError>> {
        let (encoding, buffer_size, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        #[cfg(feature = "encoding")]
        {
            let charset = self.charset_encoding().unwrap_or(encoding_rs::UTF_8);
            extractor::text_charset_notify(body, encoding, buffer_size, charset, notify)
        }

        #[cfg(not(feature = "encoding"))]
        {
            extractor::text_notify(body, encoding, buffer_size, notify)
        }
    }

    ///Extracts Response's body as JSON
    pub fn json_notify<N: extractor::Notifier, J: serde::de::DeserializeOwned>(&mut self, notify: N) -> impl Future<Output=Result<J, extractor::BodyReadError>> {
        let (encoding, buffer_size, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        #[cfg(feature = "encoding")]
        {
            let charset = self.charset_encoding().unwrap_or(encoding_rs::UTF_8);
            extractor::json_charset_notify(body, encoding, buffer_size, charset, notify)
        }

        #[cfg(not(feature = "encoding"))]
        {
            extractor::json_notify(body, encoding, buffer_size, notify)
        }
    }

    ///Extracts Response's body into file
    pub fn file_notify<N: extractor::Notifier>(&mut self, file: fs::File, notify: N) -> impl Future<Output=Result<fs::File, extractor::BodyReadError>> {
        #[cfg(debug_assertions)]
        {
            let meta = file.metadata().expect("To be able to get metadata");
            debug_assert!(!meta.permissions().readonly(), "File is read-only");
        }

        let (encoding, _, body) = self.extract_body();
        let body = futures_util::compat::Compat01As03::new(body);

        extractor::file_notify(file, body, encoding, notify)
    }


    ///Prepares upgrade for the request.
    pub async fn upgrade<U: upgrade::Upgrade>(self, _: U) -> Result<Result<(Self, hyper::upgrade::Upgraded), hyper::Error>, U::VerifyError> {
        if let Err(error) = U::verify_response(self.status(), self.inner.headers(), self.inner.extensions()) {
            return Err(error);
        }

        let (head, body) = self.inner.into_parts();
        Ok(match awaitic!(upgrade::upgrade_response(head, body.on_upgrade())) {
            Ok((hyper, body)) => Ok((Self::new(hyper), body)),
            Err(err) => Err(err),
        })
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
