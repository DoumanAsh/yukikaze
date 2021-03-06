//!Client request

use core::{mem, fmt};
use core::convert::TryFrom;
use std::io::Write;

use crate::{header, utils};

use http::header::HeaderValue;
use bytes::BufMut;

pub mod tags;
pub mod multipart;

pub(crate) type HyperRequest = hyper::Request<hyper::Body>;

#[derive(Debug)]
///Http request.
pub struct Request {
    pub(crate) parts: http::request::Parts,
    pub(crate) body: Option<bytes::Bytes>,
}

impl Request {
    ///Creates new request.
    pub fn new<U: AsRef<str>>(method: hyper::Method, uri: U) -> Result<Builder, http::uri::InvalidUri> {
        let uri = uri.as_ref().parse::<hyper::Uri>()?;
        Ok(Builder::new(uri, method))
    }

    ///Creates HEAD request.
    pub fn head<U: AsRef<str>>(uri: U) -> Result<Builder, http::uri::InvalidUri> {
        Self::new(hyper::Method::HEAD, uri)
    }

    ///Creates GET request.
    pub fn get<U: AsRef<str>>(uri: U) -> Result<Builder, http::uri::InvalidUri> {
        Self::new(hyper::Method::GET, uri)
    }

    ///Creates POST request.
    pub fn post<U: AsRef<str>>(uri: U) -> Result<Builder, http::uri::InvalidUri> {
        Self::new(hyper::Method::POST, uri)
    }

    ///Creates PUT request.
    pub fn put<U: AsRef<str>>(uri: U) -> Result<Builder, http::uri::InvalidUri> {
        Self::new(hyper::Method::PUT, uri)
    }

    ///Creates DELETE request.
    pub fn delete<U: AsRef<str>>(uri: U) -> Result<Builder, http::uri::InvalidUri> {
        Self::new(hyper::Method::DELETE, uri)
    }

    #[inline]
    ///Returns reference to method.
    pub fn method(&self) -> &http::Method {
        &self.parts.method
    }

    #[inline]
    ///Returns mutable reference to method.
    pub fn method_mut(&mut self) -> &mut http::Method {
        &mut self.parts.method
    }

    #[inline]
    ///Returns reference to headers.
    pub fn headers(&self) -> &http::HeaderMap {
        &self.parts.headers
    }

    #[inline]
    ///Returns mutable reference to headers.
    pub fn headers_mut(&mut self) -> &mut http::HeaderMap {
        &mut self.parts.headers
    }

    #[inline]
    ///Returns reference to uri.
    pub fn uri(&self) -> &http::Uri {
        &self.parts.uri
    }

    #[inline]
    ///Returns mutable reference to uri.
    pub fn uri_mut(&mut self) -> &mut http::Uri {
        &mut self.parts.uri
    }

    #[inline]
    ///Retrieves reference to http extension map
    pub fn extensions(&self) -> &http::Extensions {
        &self.parts.extensions
    }

    #[inline]
    ///Retrieves mutable reference to http extension map
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        &mut self.parts.extensions
    }

    #[inline]
    ///Extracts extensions out and leaves empty in `Self`
    pub fn extract_extensions(&mut self) -> http::Extensions {
        let mut extensions = http::Extensions::new();
        mem::swap(&mut extensions, self.extensions_mut());
        extensions
    }
}

impl Into<HyperRequest> for Request {
    fn into(self) -> HyperRequest {
        let body = self.body.map(|body| body.into()).unwrap_or_else(hyper::Body::empty);
        HyperRequest::from_parts(self.parts, body)
    }
}

///Http request builder.
///
///Each method that may cause troubles shall
///panic.
pub struct Builder {
    parts: http::request::Parts,
    cookies: Option<cookie::CookieJar>,
}

impl Builder {
    #[inline]
    ///Starts process of creating request.
    pub fn new(uri: hyper::Uri, method: hyper::Method) -> Self {
        //Workaround to get just Parts as it is more  convenient
        //to modify Parts as you can take out elements
        let mut temp = hyper::Request::<()>::new(());
        *temp.method_mut() = method;
        *temp.uri_mut() = uri;

        let (parts, _) = temp.into_parts();

        Self {
            parts,
            cookies: None
        }
    }

    #[inline]
    ///Retrieves reference to http extension map
    pub fn extensions(&self) -> &http::Extensions {
        &self.parts.extensions
    }

    #[inline]
    ///Retrieves mutable reference to http extension map
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        &mut self.parts.extensions
    }

    #[inline]
    ///Gets reference to headers.
    pub fn headers(&mut self) -> &mut http::HeaderMap {
        &mut self.parts.headers
    }

    #[inline]
    ///Invokes closure with `value` and `Self` as arguments, if `value` contains something
    ///
    pub fn if_some<T, F: FnOnce(T, Self) -> Self>(self, value: Option<T>, cb: F) -> Self {
        match value {
            Some(value) => cb(value, self),
            None => self,
        }
    }

    #[inline]
    ///Sets new header to request.
    ///
    ///If header exists, it replaces it.
    ///
    ///# Panics
    ///
    ///- On attempt to set invalid header value.
    pub fn set_header<K: header::IntoHeaderName, V>(mut self, key: K, value: V) -> Self where HeaderValue: TryFrom<V> {
        let value = match HeaderValue::try_from(value) {
            Ok(value) => value,
            Err(_) => panic!("Attempt to set invalid header"),
        };

        let _ = self.headers().insert(key, value);

        self
    }

    #[inline]
    ///Sets new header to request, only if it wasn't set previously.
    ///
    ///# Panics
    ///
    ///- On attempt to set invalid header value.
    pub fn set_header_if_none<K: header::IntoHeaderName, V>(mut self, key: K, value: V) -> Self where HeaderValue: TryFrom<V> {
        match self.headers().entry(key) {
            http::header::Entry::Vacant(entry) => match HeaderValue::try_from(value) {
                Ok(value) => {
                    entry.insert(value);
                },
                Err(_) => panic!("Attempt to set invalid header value")
            },
            _ => (),
        }

        self
    }

    ///Sets ETag value into corresponding header.
    ///
    ///If it is set, then value is appended to existing header as per standard after
    ///semicolon.
    pub fn set_etag<E: tags::EtagMode>(mut self, etag: &etag::EntityTag, _: E) -> Self {
        let mut buffer = utils::BytesWriter::with_smol_capacity();
        let _ = match self.headers().remove(E::header_name()) {
            Some(old) => write!(&mut buffer, "{}, {}", old.to_str().expect("Invalid ETag!"), etag),
            None => write!(&mut buffer, "{}", etag),
        };

        let value = unsafe { http::header::HeaderValue::from_maybe_shared_unchecked(buffer.freeze()) };
        self.headers().insert(E::header_name(), value);
        self
    }

    ///Sets HttpDate value into corresponding header.
    pub fn set_date<E: tags::DateMode>(mut self, date: httpdate::HttpDate, _: E) -> Self {
        let mut buffer = utils::BytesWriter::with_smol_capacity();
        let _ = write!(&mut buffer, "{}", date);
        let value = unsafe { http::header::HeaderValue::from_maybe_shared_unchecked(buffer.freeze()) };

        self.headers().insert(E::header_name(), value);
        self
    }

    ///Sets cookie jar to request.
    ///
    ///If jar already exists, the cookies from jar
    ///are appended.
    pub fn set_cookie_jar(mut self, jar: cookie::CookieJar) -> Self {
        if self.cookies.is_none() {
            self.cookies = Some(jar);
        } else {
            let self_jar = self.cookies.as_mut().unwrap();

            for cookie in jar.iter().cloned() {
                self_jar.add(cookie.into_owned());
            }
        }

        self
    }

    ///Adds cookie.
    pub fn add_cookie(mut self, cookie: cookie::Cookie<'static>) -> Self {
        if self.cookies.is_none() {
            let mut jar = cookie::CookieJar::new();
            jar.add(cookie);
            self.cookies = Some(jar);
        } else {
            self.cookies.as_mut().unwrap().add(cookie.into_owned());
        }

        self
    }

    #[inline]
    ///Sets `Content-Length` header.
    ///
    ///It replaces previous one, if there was any.
    pub fn content_len(self, len: u64) -> Self {
        self.set_header(http::header::CONTENT_LENGTH, len)
    }

    #[inline]
    ///Sets `Accept-Encoding` header.
    ///
    ///Replaces previous value, if any.
    pub fn accept_encoding(self, encoding: header::ContentEncoding) -> Self {
        self.set_header(header::ACCEPT_ENCODING, encoding.as_str())
    }

    ///Sets `Content-Disposition` header.
    ///
    ///Replaces previous value, if any.
    pub fn content_disposition(mut self, disp: &header::ContentDisposition) -> Self {
        let mut buffer = utils::BytesWriter::with_smol_capacity();

        let _ = write!(&mut buffer, "{}", disp);
        let value = unsafe { http::header::HeaderValue::from_maybe_shared_unchecked(buffer.freeze()) };

        self.headers().insert(header::CONTENT_DISPOSITION, value);
        self
    }

    ///Adds basic authentication header.
    pub fn basic_auth<U: fmt::Display, P: fmt::Display>(mut self, username: U, password: Option<P>) -> Self {
        const BASIC: &'static str = "Basic ";

        let auth = match password {
            Some(password) => format!("{}:{}", username, password),
            None => format!("{}:", username)
        };
        let encode_len = data_encoding::BASE64.encode_len(auth.as_bytes().len());
        let header_value = unsafe {
            let mut header_value = bytes::BytesMut::with_capacity(encode_len + BASIC.as_bytes().len());
            header_value.put_slice(BASIC.as_bytes());
            {
                let dest = &mut *(&mut header_value.bytes_mut()[..encode_len] as *mut [core::mem::MaybeUninit<u8>] as *mut [u8]);
                data_encoding::BASE64.encode_mut(auth.as_bytes(), dest);
            }
            header_value.advance_mut(encode_len);
            http::header::HeaderValue::from_maybe_shared_unchecked(header_value.freeze())
        };

        let _ = self.headers().insert(http::header::AUTHORIZATION, header_value);

        self
    }

    ///Adds bearer authentication header.
    ///
    ///Generally tokens already contain only valid symbols for header.
    ///So the function doesn't encode it using base64.
    pub fn bearer_auth(mut self, token: &str) -> Self {
        const TYPE: &'static str = "Bearer ";

        let header_value = unsafe {
            let mut header_value = bytes::BytesMut::with_capacity(token.as_bytes().len() + TYPE.as_bytes().len());
            header_value.put_slice(TYPE.as_bytes());
            header_value.put_slice(token.as_bytes());
            http::header::HeaderValue::from_maybe_shared_unchecked(header_value.freeze())
        };

        let _ = self.headers().insert(http::header::AUTHORIZATION, header_value);

        self
    }

    ///Sets request's query by overwriting existing one, if any.
    ///
    ///# Panics
    ///
    ///- If unable to encode data.
    ///- If URI creation fails
    pub fn query<Q: serde::Serialize>(mut self, query: &Q) -> Self {
        let mut uri_parts = self.parts.uri.into_parts();
        let path = uri_parts.path_and_query;

        let mut buffer = utils::BytesWriter::with_smol_capacity();
        let query = serde_urlencoded::to_string(&query).expect("To url-encode");

        let _ = match path {
            Some(path) => write!(buffer, "{}?{}", path.path(), query),
            None => write!(buffer, "?{}", query),
        };

        uri_parts.path_and_query = Some(http::uri::PathAndQuery::from_maybe_shared(buffer.into_inner().freeze()).expect("To create path and query"));

        self.parts.uri = match http::Uri::from_parts(uri_parts) {
            Ok(uri) => uri,
            Err(error) => panic!("Unable to set query for URI: {}", error)
        };
        self
    }

    ///Prepares upgrade for the request.
    ///
    ///Existing mechanisms:
    ///
    ///- [Websocket](../../upgrade/websocket/index.html)
    pub fn upgrade<U: crate::upgrade::Upgrade>(mut self, _: U, options: U::Options) -> Request {
        U::prepare_request(&mut self.parts.headers, &mut self.parts.extensions, options);
        self.empty()
    }

    ///Creates request with specified body.
    ///
    ///Adds `Content-Length` if not specified by user.
    ///Following RFC, adds zero length only for `PUT` and `POST` requests
    pub fn body<B: Into<bytes::Bytes>>(mut self, body: Option<B>) -> Request {
        use bytes::Buf;
        use crate::utils::enc::USER_INFO_ENCODE_SET;
        use percent_encoding::{utf8_percent_encode};

        // set cookies
        if let Some(jar) = self.cookies.take() {
            let mut buffer = utils::BytesWriter::new();

            for cook in jar.delta() {
                let name = utf8_percent_encode(cook.name(), USER_INFO_ENCODE_SET);
                let value = utf8_percent_encode(cook.value(), USER_INFO_ENCODE_SET);
                let _ = write!(&mut buffer, "; {}={}", name, value);
            }

            let mut buffer = buffer.into_inner();
            buffer.advance(2);
            let cookie = unsafe { http::header::HeaderValue::from_maybe_shared_unchecked(buffer.freeze()) };

            let _ = self.headers().insert(http::header::COOKIE, cookie);
        }

        let body = body.map(|body| body.into());

        //We automatically insert Content-Length: 0 for empty requests
        //with POST/PUT and removed it otherwise.
        //For everything else we just add Content-Length unless it is already in
        match body.as_ref() {
            None => match self.parts.method {
                hyper::Method::PUT | hyper::Method::POST => match self.parts.headers.entry(http::header::CONTENT_LENGTH) {
                    http::header::Entry::Vacant(value) => {
                        value.insert(utils::content_len_value(0));
                    },
                    _ => (),
                },
                _ => {
                    self.parts.headers.remove(http::header::CONTENT_LENGTH);
                },
            },
            Some(body) => match self.parts.headers.entry(http::header::CONTENT_LENGTH) {
                http::header::Entry::Vacant(value) => {
                    value.insert(utils::content_len_value(body.len() as u64));
                },
                _ => (),
            },
        }

        Request {
            parts: self.parts,
            body,
        }
    }

    ///Creates request with Form payload.
    pub fn form<F: serde::Serialize>(self, body: &F) -> Result<Request, serde_urlencoded::ser::Error> {
        let body = serde_urlencoded::to_string(&body)?;
        Ok(self.set_header_if_none(header::CONTENT_TYPE, "application/x-www-form-urlencoded").body(Some(body)))
    }

    ///Creates request with JSON payload.
    pub fn json<J: serde::Serialize>(self, body: &J) -> serde_json::Result<Request> {
        let mut buffer = utils::BytesWriter::new();
        let _ = serde_json::to_writer(&mut buffer, &body)?;
        let body = buffer.into_inner().freeze();
        Ok(self.set_header_if_none(header::CONTENT_TYPE, "application/json").body(Some(body)))
    }

    ///Creates request with multipart body.
    pub fn multipart(self, body: multipart::Form) -> Request {
        let mut content_type = utils::BytesWriter::with_capacity(30 + body.boundary.len());
        let _ = write!(&mut content_type, "multipart/form-data; boundary={}", body.boundary);
        let content_type = unsafe { http::header::HeaderValue::from_maybe_shared_unchecked(content_type.freeze()) };

        let (_, body) = body.finish();
        self.set_header_if_none(header::CONTENT_TYPE, content_type).body(Some(body))
    }

    ///Creates request with no body.
    ///
    ///Explicitly sets `Content-Length` to 0, if necessary
    pub fn empty(self) -> Request {
        self.body::<bytes::Bytes>(None)
    }
}
