//!Request primitives.

use ::std::mem;
use ::std::io::Write;
use ::std::fmt;
use ::std::ops::{Deref, DerefMut};

use ::cookie;
use ::data_encoding::BASE64;
use ::http;
use ::hyper;
use ::bytes;
use ::bytes::BufMut;
use ::serde::Serialize;
use ::serde_json;
use ::serde_urlencoded;

use ::header;
use ::http::HttpTryFrom;
use ::http::header::HeaderValue;

use ::utils;

type HyperRequest = hyper::Request<hyper::Body>;

#[derive(Debug)]
///Http request.
pub struct Request {
    pub(crate) inner: HyperRequest
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
}

impl From<HyperRequest> for Request {
    fn from(inner: HyperRequest) -> Self {
        Self {
            inner
        }
    }
}

impl Deref for Request {
    type Target = HyperRequest;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Request {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

///Http request builder.
///
///Each method that may cause troubles shall
///panic
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
        mem::replace(temp.method_mut(), method);
        mem::replace(temp.uri_mut(), uri);

        let (parts, _) = temp.into_parts();

        Self {
            parts,
            cookies: None
        }
    }

    #[inline]
    ///Gets reference to headers.
    pub fn headers(&mut self) -> &mut http::HeaderMap {
        &mut self.parts.headers
    }

    #[inline]
    ///Sets new header to request.
    ///
    ///If header exists, it replaces it.
    ///
    ///# Panics
    ///
    ///- On attempt to set invalid header value.
    pub fn set_header<K: header::IntoHeaderName, V>(mut self, key: K, value: V) -> Self where HeaderValue: HttpTryFrom<V> {
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
    pub fn set_header_if_none<K: header::AsHeaderName, V>(mut self, key: K, value: V) -> Self where HeaderValue: HttpTryFrom<V> {
        match self.headers().entry(key).expect("Valid header name") {
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

    #[inline]
    ///Adds cookie.
    pub fn add_cookie<'c>(mut self, cookie: cookie::Cookie<'c>) -> Self {
        if self.cookies.is_none() {
            let mut jar = cookie::CookieJar::new();
            jar.add(cookie.into_owned());
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
    ///Replaces previous value, if any..
    pub fn accept_encoding(self, encoding: header::ContentEncoding) -> Self {
        self.set_header(header::ACCEPT_ENCODING, encoding.as_str())
    }

    ///Adds authentication header.
    pub fn basic_auth<U: fmt::Display, P: fmt::Display>(mut self, username: U, password: Option<P>) -> Self {
        const BASIC: &'static str = "basic ";

        let auth = match password {
            Some(password) => format!("{}:{}", username, password),
            None => format!("{}:", username)
        };
        let encode_len = BASE64.encode_len(auth.as_bytes().len());
        let header_value = unsafe {
            let mut header_value = bytes::BytesMut::with_capacity(encode_len + BASIC.as_bytes().len());
            header_value.put_slice(BASIC.as_bytes());
            BASE64.encode_mut(auth.as_bytes(), &mut header_value.bytes_mut()[..encode_len]);
            header_value.advance_mut(encode_len);
            http::header::HeaderValue::from_shared_unchecked(header_value.freeze())
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
    pub fn query<Q: Serialize>(mut self, query: Q) -> Self {
        let mut uri_parts = self.parts.uri.into_parts();
        let path = uri_parts.path_and_query;

        let mut buffer = utils::BytesWriter::new();
        let query = serde_urlencoded::to_string(&query).expect("To url-encode");

        let _ = match path {
            Some(path) => write!(buffer, "{}?{}", path.path(), query),
            None => write!(buffer, "?{}", query),
        };

        uri_parts.path_and_query = Some(http::uri::PathAndQuery::from_shared(buffer.into_inner().freeze()).expect("To create path and query"));

        self.parts.uri = match http::Uri::from_parts(uri_parts) {
            Ok(uri) => uri,
            Err(error) => panic!("Unable to set query for URI: {}", error)
        };
        self
    }

    ///Creates request with specified body.
    pub fn body<B: Into<hyper::Body>>(mut self, body: B) -> Request {
        use ::percent_encoding::{utf8_percent_encode, USERINFO_ENCODE_SET};

        // set cookies
        if let Some(jar) = self.cookies.take() {
            let mut buffer = utils::BytesWriter::new();

            for cook in jar.delta() {
                let name = utf8_percent_encode(cook.name(), USERINFO_ENCODE_SET);
                let value = utf8_percent_encode(cook.value(), USERINFO_ENCODE_SET);
                let _ = write!(&mut buffer, "; {}={}", name, value);
            }

            let mut buffer = buffer.into_inner();
            buffer.split_to(2);
            let cookie = unsafe { http::header::HeaderValue::from_shared_unchecked(buffer.freeze()) };

            let _ = self.headers().insert(http::header::AUTHORIZATION, cookie);
        }

        let inner = HyperRequest::from_parts(self.parts, body.into());
        Request {
            inner
        }
    }

    ///Creates request with Form payload.
    pub fn form<F: Serialize>(self, body: F) -> Result<Request, serde_urlencoded::ser::Error> {
        let body = serde_urlencoded::to_string(&body)?;
        Ok(self.set_header_if_none(header::CONTENT_TYPE, "application/x-www-form-urlencoded").body(body))
    }

    ///Creates request with JSON payload.
    pub fn json<J: Serialize>(self, body: J) -> serde_json::Result<Request> {
        let mut buffer = utils::BytesWriter::new();
        let _ = serde_json::to_writer(&mut buffer, &body)?;
        let body = buffer.into_inner().freeze();
        Ok(self.set_header_if_none(header::CONTENT_TYPE, "application/json").body(body))
    }

    ///Creates request with no body.
    ///
    ///Explicitly sets `Content-Length` to 0
    pub fn empty(self) -> Request {
        self.content_len(0).body(hyper::Body::empty())
    }
}
