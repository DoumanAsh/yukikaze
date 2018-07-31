///!Response extractors

use ::std::io;
use ::std::io::Write;
use ::std::string;
use ::std::str;
use ::std::mem;

use ::header;
use ::utils;

#[cfg(feature = "flate2")]
use ::flate2;
use ::etag;
use ::hyper;
use ::http;
use ::futures;
use ::futures::{Future, Stream};
use ::bytes;
use ::serde_json;
use ::serde::de::DeserializeOwned;

//The size of buffer to use by default.
const BUFFER_SIZE: usize = 4096;
//The default limit on body size 2mb.
const DEFEAULT_LIMIT: u64 = 2 * 1024 * 1024;

///Extracts ETags from response.
///
///It skips invalid tags without reporint errors.
pub struct Etag<'a> {
    inner: str::Split<'a, char>
}

impl<'a> Etag<'a> {
    ///Creates extractor.
    ///
    ///Panics if header value is not UTF-8 string.
    pub fn new(etag: &'a header::HeaderValue) -> Self {
        let etag = etag.as_bytes();
        let etag = str::from_utf8(etag).expect("UTF-8 header value");

        Self {
            inner: etag.split(',')
        }
    }
}

impl<'a> Iterator for Etag<'a> {
    type Item = etag::EntityTag;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(value) = self.inner.next() {
            match value.trim().parse::<etag::EntityTag>() {
                Ok(etag) => return Some(etag),
                Err(_) => ()
            }
        }

        None
    }
}

#[derive(Debug)]
///Describes possible errors when reading body.
pub enum BodyReadError {
    ///Hyper's error.
    Hyper(hyper::Error),
    ///Hit limit
    Overflow,
    ///Unable to decode body as UTF-8
    EncodingError,
    ///Json serialization error.
    JsonError(serde_json::error::Error),
    ///Error happened during deflate decompression.
    DeflateError(io::Error),
    ///Error happened during gzip decompression.
    GzipError(io::Error),
}

impl From<serde_json::error::Error> for BodyReadError {
    #[inline]
    fn from(error: serde_json::error::Error) -> Self {
        BodyReadError::JsonError(error)
    }
}

impl From<string::FromUtf8Error> for BodyReadError {
    #[inline]
    fn from(_: string::FromUtf8Error) -> Self {
        BodyReadError::EncodingError
    }
}

impl From<hyper::Error> for BodyReadError {
    #[inline]
    fn from(err: hyper::Error) -> Self {
        BodyReadError::Hyper(err)
    }
}

enum BodyType {
    Plain(hyper::Body, bytes::BytesMut),
    Deflate(hyper::Body, flate2::write::DeflateDecoder<utils::BytesWriter>),
    Gzip(hyper::Body, flate2::write::GzDecoder<utils::BytesWriter>),
}

///Reads raw bytes from HTTP Response
///
///The extractor provides way to read plain response's body or
///compressed one.
///
///The method with which to read body determined by `Content-Encoding` header.
///
///Note that `ContentEncoding::Deflate` may have potential problem with particular
///way to compress it. For example httpbin `/deflate` endpoint compresses the data
///in unsupported by `flate2` crate way. Relevant [issue](https://github.com/requests/httpbin/issues/419)
pub struct RawBody {
    parts: http::response::Parts,
    body: BodyType,
    //The remaining bytes to read.
    limit: u64,
}

impl RawBody {
    ///Creates new instance.
    pub fn new(response: super::Response) -> Self {
        let encoding = response.content_encoding();
        let buffer_size = match response.content_len() {
            Some(len) => len as usize,
            None => BUFFER_SIZE
        };

        let (parts, body) = response.inner.into_parts();

        let body = match encoding {
            #[cfg(feature = "flate2")]
            header::ContentEncoding::Deflate => BodyType::Deflate(body, flate2::write::DeflateDecoder::new(utils::BytesWriter::with_capacity(buffer_size))),
            #[cfg(feature = "flate2")]
            header::ContentEncoding::Gzip => BodyType::Gzip(body, flate2::write::GzDecoder::new(utils::BytesWriter::with_capacity(buffer_size))),
            _ => BodyType::Plain(body, bytes::BytesMut::with_capacity(buffer_size)),

        };

        RawBody {
            parts,
            body,
            limit: DEFEAULT_LIMIT,
        }
    }

    #[inline]
    ///Disables decompression.
    pub fn no_decompress(mut self) -> Self {
        if let &BodyType::Plain(_, _) = &self.body {
            return self
        }

        let body = match self.body {
            #[cfg(feature = "flate2")]
            BodyType::Deflate(body, _) => body,
            #[cfg(feature = "flate2")]
            BodyType::Gzip(body, _) => body,
            BodyType::Plain(_, _) => unreachable!(),
        };

        self.body = BodyType::Plain(body, bytes::BytesMut::with_capacity(BUFFER_SIZE));
        self
    }

    #[inline]
    ///Retrieves length of content to receive, if `Content-Length` exists.
    pub fn content_len(&self) -> Option<u64> {
        self.parts.headers
            .get(header::CONTENT_LENGTH)
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.parse().ok())
    }

    #[inline]
    ///Sets limit on body reading. Default is 2mb.
    ///
    ///When read hits the limit, it is aborted with error.
    ///Use it when you need to control limit on your reads.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = limit;
        self
    }
}

impl Future for RawBody {
    type Item = bytes::Bytes;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            match self.body {
                BodyType::Plain(ref mut body, ref mut buffer) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        if self.limit < (buffer.len() + chunk.len()) as u64 {
                            return Err(BodyReadError::Overflow);
                        }

                        buffer.extend_from_slice(&chunk);
                        //We loop, to schedule more IO
                    },
                    Ok(futures::Async::Ready(None)) => {
                        let buffer = mem::replace(buffer, bytes::BytesMut::new());
                        return Ok(futures::Async::Ready(buffer.freeze()))
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error.into())
                },
                #[cfg(feature = "flate2")]
                BodyType::Deflate(ref mut body, ref mut decoder) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        decoder.write_all(&chunk).map_err(|error| BodyReadError::DeflateError(error))?;
                        decoder.flush().map_err(|error| BodyReadError::DeflateError(error))?;

                        if self.limit < decoder.total_out() {
                            return Err(BodyReadError::Overflow);
                        }
                        //We loop, to schedule more IO
                    },
                    Ok(futures::Async::Ready(None)) => {
                        decoder.try_finish().map_err(|error| BodyReadError::DeflateError(error))?;
                        let buffer = decoder.get_mut().freeze();
                        return Ok(futures::Async::Ready(buffer))
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error.into())

                },
                #[cfg(feature = "flate2")]
                BodyType::Gzip(ref mut body, ref mut decoder) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        decoder.write_all(&chunk).map_err(|error| BodyReadError::GzipError(error))?;
                        decoder.flush().map_err(|error| BodyReadError::GzipError(error))?;

                        if self.limit < decoder.get_ref().len() as u64 {
                            return Err(BodyReadError::Overflow);
                        }
                        //We loop, to schedule more IO
                    },
                    Ok(futures::Async::Ready(None)) => {
                        decoder.try_finish().map_err(|error| BodyReadError::GzipError(error))?;
                        let buffer = decoder.get_mut().freeze();
                        return Ok(futures::Async::Ready(buffer))
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error.into())

                },
            }
        }
    }
}

///Reads String from HTTP Response.
pub enum Text {
    #[doc(hidden)]
    Init(Option<RawBody>),
    #[doc(hidden)]
    Future(futures::AndThen<RawBody, Result<String, BodyReadError>, fn(bytes::Bytes) -> Result<String, BodyReadError>>)
}

impl Text {
    ///Creates new instance.
    pub fn new(response: super::Response) -> Self {
        Text::Init(Some(RawBody::new(response)))
    }

    #[inline]
    ///Retrieves length of content to receive, if `Content-Length` exists.
    pub fn content_len(&self) -> Option<u64> {
        match self {
            Text::Init(Some(raw)) => raw.content_len(),
            _ => None
        }
    }

    #[inline]
    ///Sets limit on body reading. Default is 2mb.
    ///
    ///When read hits the limit, it is aborted with error.
    ///Use it when you need to control limit on your reads.
    pub fn limit(self, limit: u64) -> Self {
        match self {
            Text::Init(Some(raw)) => {
                Text::Init(Some(raw.limit(limit)))
            }
            _ => self
        }
    }

    fn encode(bytes: bytes::Bytes) -> Result<String, BodyReadError> {
        String::from_utf8(bytes.to_vec()).map_err(|error| error.into())
    }
}

impl Future for Text {
    type Item = String;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let new_state = match self {
                Text::Future(fut) => return fut.poll(),
                Text::Init(raw) => Text::Future(raw.take().expect("To have body").and_then(Self::encode))
            };

            *self = new_state;
        }
    }
}

///Reads raw bytes from HTTP Response and deserializes as JSON struct
pub enum Json<J> {
    #[doc(hidden)]
    Init(Option<RawBody>),
    #[doc(hidden)]
    Future(futures::AndThen<RawBody, Result<J, BodyReadError>, fn(bytes::Bytes) -> Result<J, BodyReadError>>)
}

impl<J: DeserializeOwned> Json<J> {
    ///Creates new instance.
    pub fn new(response: super::Response) -> Self {
        Json::Init(Some(RawBody::new(response)))
    }

    #[inline]
    ///Retrieves length of content to receive, if `Content-Length` exists.
    pub fn content_len(&self) -> Option<u64> {
        match self {
            Json::Init(Some(raw)) => raw.content_len(),
            _ => None
        }
    }

    #[inline]
    ///Sets limit on body reading. Default is 2mb.
    ///
    ///When read hits the limit, it is aborted with error.
    ///Use it when you need to control limit on your reads.
    pub fn limit(self, limit: u64) -> Self {
        match self {
            Json::Init(Some(raw)) => {
                Json::Init(Some(raw.limit(limit)))
            }
            _ => self
        }
    }

    fn encode(bytes: bytes::Bytes) -> Result<J, BodyReadError> {
        serde_json::from_slice(&bytes).map_err(BodyReadError::from)
    }
}

impl<J: DeserializeOwned> Future for Json<J> {
    type Item = J;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let new_state = match self {
                Json::Future(fut) => return fut.poll(),
                Json::Init(raw) => Json::Future(raw.take().expect("To have body").and_then(Self::encode))
            };

            *self = new_state;
        }
    }
}
