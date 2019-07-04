///!Response extractors

use std::fs;
use std::io;
use std::io::Write;
use std::mem;
use std::marker::PhantomData;
use std::str::FromStr;

use crate::header;
use crate::utils;
use super::errors;
use super::errors::BodyReadError;

use futures::{Future, Stream};
use serde::de::DeserializeOwned;

//The size of buffer to use by default.
const BUFFER_SIZE: usize = 4096;
//The default limit on body size 2mb.
const DEFEAULT_LIMIT: u64 = 2 * 1024 * 1024;

pub mod notify;
pub use self::notify::Notifier;

///Cookie extractor.
///
///As it returns references they would tie
///up original response, if you want avoid it
///you can use `Cookie::into_owned()`
pub struct CookieIter<'a> {
    pub(crate) iter: header::ValueIter<'a, header::HeaderValue>,
}

impl<'a> Iterator for CookieIter<'a> {
    type Item = Result<cookie2::Cookie<'a>, cookie2::ParseError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        use ::percent_encoding::percent_decode;

        if let Some(cook) = self.iter.by_ref().next() {
            let cook = percent_decode(cook.as_bytes());
            let cook = cook.decode_utf8().map_err(|error| cookie2::ParseError::Utf8Error(error))
                                         .and_then(|cook| cookie2::Cookie::parse(cook));
            Some(cook)
        } else {
            None
        }
    }
}

pub(crate) enum BodyType {
    Plain(hyper::Body, bytes::BytesMut),
    #[cfg(feature = "flate2")]
    Deflate(hyper::Body, flate2::write::ZlibDecoder<utils::BytesWriter>),
    #[cfg(feature = "flate2")]
    Gzip(hyper::Body, flate2::write::GzDecoder<utils::BytesWriter>),
}

///Reads raw bytes from HTTP Response
///
///The extractor provides way to read plain response's body or
///compressed one.
///
///The method with which to read body determined by `Content-Encoding` header.
///
///Note that `ContentEncoding::Deflate` supports zlib encoded data with deflate compression.
///Plain deflate is non-conforming and not supported.
pub struct RawBody<N> {
    parts: http::response::Parts,
    body: BodyType,
    limit: u64,
    notifier: N,
}

impl<N: Notifier> RawBody<N> {
    ///Creates new instance.
    pub fn new(response: super::Response, notifier: N) -> Self {
        let encoding = response.content_encoding();
        let buffer_size = match response.content_len() {
            Some(len) => len as usize,
            None => BUFFER_SIZE
        };

        let (parts, body) = response.inner.into_parts();

        let body = match encoding {
            #[cfg(feature = "flate2")]
            header::ContentEncoding::Deflate => BodyType::Deflate(body, flate2::write::ZlibDecoder::new(utils::BytesWriter::with_capacity(buffer_size))),
            #[cfg(feature = "flate2")]
            header::ContentEncoding::Gzip => BodyType::Gzip(body, flate2::write::GzDecoder::new(utils::BytesWriter::with_capacity(buffer_size))),
            _ => BodyType::Plain(body, bytes::BytesMut::with_capacity(buffer_size)),

        };

        RawBody {
            parts,
            body,
            limit: DEFEAULT_LIMIT,
            notifier
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
    ///Retrieves `Content-Type` as Mime, if any.
    pub fn mime(&self) -> Result<Option<mime::Mime>, errors::ContentTypeError> {
        let content_type = self.parts.headers.get(header::CONTENT_TYPE)
                                             .and_then(|content_type| content_type.to_str().ok());

        if let Some(content_type) = content_type {
            content_type.parse::<mime::Mime>().map(|mime| Some(mime)).map_err(errors::ContentTypeError::from)
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

    #[inline]
    ///Transforms self into future with new [Notifier](notify/trait.Notifier.html)
    pub fn with_notify<T: notify::Notifier>(self, notifier: T) -> RawBody<T> {
        RawBody::<T> {
            parts: self.parts,
            body: self.body,
            limit: self.limit,
            notifier,
        }
    }
}

impl<N: Notifier> Future for RawBody<N> {
    type Item = bytes::Bytes;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let bytes = match self.body {
                BodyType::Plain(ref mut body, ref mut buffer) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        if self.limit < (buffer.len() + chunk.len()) as u64 {
                            return Err(BodyReadError::Overflow);
                        }

                        buffer.extend_from_slice(&chunk);
                        //We loop, to schedule more IO
                        chunk.len()
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
                        chunk.len()
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
                        chunk.len()
                    },
                    Ok(futures::Async::Ready(None)) => {
                        decoder.try_finish().map_err(|error| BodyReadError::GzipError(error))?;
                        let buffer = decoder.get_mut().freeze();
                        return Ok(futures::Async::Ready(buffer))
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error.into())

                },
            };

            self.notifier.send(bytes);
        }
    }
}

///Reads String from HTTP Response.
///
///# Encoding feature
///
///If `Content-Encoding` contains charset information it
///shall be automatically applied when decoding data.
pub enum Text<N> {
    #[doc(hidden)]
    Init(Option<RawBody<N>>),
    #[cfg(feature = "encoding")]
    #[doc(hidden)]
    Future(RawBody<N>, Option<&'static encoding_rs::Encoding>),
    #[cfg(not(feature = "encoding"))]
    #[doc(hidden)]
    Future(RawBody<N>),
}

impl<N: Notifier> Text<N> {
    ///Creates new instance.
    pub fn new(response: super::Response, notifier: N) -> Self {
        Text::Init(Some(RawBody::new(response, notifier)))
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

    #[inline]
    ///Transforms self into future with new [Notifier](notify/trait.Notifier.html)
    pub fn with_notify<T: notify::Notifier>(self, notifier: T) -> Text<T> {
        match self {
            Text::Init(body) => Text::Init(body.map(|body| body.with_notify(notifier))),
            _ => unreachable!(),
        }
    }
}

impl<N: Notifier> Future for Text<N> {
    type Item = String;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let new_state = match self {
                //Encoding
                #[cfg(feature = "encoding")]
                Text::Future(fut, enc) => match fut.poll() {
                    Ok(futures::Async::Ready(bytes)) => return match enc {
                        Some(enc) => match enc.decode(&bytes) {
                            (result, _, false) => Ok(futures::Async::Ready(result.into_owned())),
                            (_, _, true) => Err(BodyReadError::EncodingError)
                        },
                        None => String::from_utf8(bytes.to_vec()).map_err(|error| error.into()).map(|st| futures::Async::Ready(st))
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error)
                },
                //No Encoding
                #[cfg(not(feature = "encoding"))]
                Text::Future(fut) => match fut.poll() {
                    Ok(futures::Async::Ready(bytes)) => return String::from_utf8(bytes.to_vec()).map_err(|error| error.into())
                                                                                                .map(|st| futures::Async::Ready(st)),
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error)
                },
                //Encoding
                #[cfg(feature = "encoding")]
                Text::Init(raw) => {
                    let raw = raw.take().expect("To have body");
                    let encoding = raw.charset_encoding().ok().and_then(|enc| match enc == encoding_rs::UTF_8 {
                        true => None,
                        _ => Some(enc),
                    });
                    Text::Future(raw, encoding)
                },
                //No Encoding
                #[cfg(not(feature = "encoding"))]
                Text::Init(raw) => Text::Future(raw.take().expect("To have body")),
            };

            *self = new_state;
        }
    }
}

///Reads raw bytes from HTTP Response and de-serializes as JSON struct
pub enum Json<J, N> where N: Notifier {
    #[doc(hidden)]
    Init(Option<RawBody<N>>),
    #[cfg(feature = "encoding")]
    #[doc(hidden)]
    Future(RawBody<N>, Option<&'static encoding_rs::Encoding>, PhantomData<J>),
    #[cfg(not(feature = "encoding"))]
    #[doc(hidden)]
    Future(RawBody<N>, PhantomData<J>),
}

impl<J: DeserializeOwned, N: Notifier> Json<J, N> {
    ///Creates new instance.
    pub fn new(response: super::Response, notifier: N) -> Self {
        Json::Init(Some(RawBody::new(response, notifier)))
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
            Json::Init(Some(raw)) => Json::Init(Some(raw.limit(limit))),
            _ => self,
        }
    }

    #[inline]
    ///Transforms self into future with new [Notifier](notify/trait.Notifier.html)
    pub fn with_notify<T: notify::Notifier>(self, notifier: T) -> Text<T> {
        match self {
            Json::Init(body) => Text::Init(body.map(|body| body.with_notify(notifier))),
            _ => unreachable!()
        }
    }
}

impl<J: DeserializeOwned, N: Notifier> Future for Json<J, N> {
    type Item = J;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let new_state = match self {
                //Encoding
                #[cfg(feature = "encoding")]
                Json::Future(fut, enc, _) => match fut.poll() {
                    Ok(futures::Async::Ready(bytes)) => return match enc {
                        Some(enc) => match enc.decode(&bytes) {
                            (result, _, false) => serde_json::from_str(&result).map_err(BodyReadError::from)
                                                                               .map(|result| futures::Async::Ready(result)),
                            (_, _, true) => Err(BodyReadError::EncodingError)
                        },
                        None => return serde_json::from_slice(&bytes).map_err(BodyReadError::from)
                                                                     .map(|st| futures::Async::Ready(st)),
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error)
                },
                //No Encoding
                #[cfg(not(feature = "encoding"))]
                Json::Future(fut, _) => match fut.poll() {
                    Ok(futures::Async::Ready(bytes)) => return serde_json::from_slice(&bytes).map_err(BodyReadError::from)
                                                                                             .map(|st| futures::Async::Ready(st)),
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error)
                },
                //Encoding
                #[cfg(feature = "encoding")]
                Json::Init(raw) => {
                    let raw = raw.take().expect("To have body");
                    let encoding = raw.charset_encoding().ok().and_then(|enc| match enc == encoding_rs::UTF_8 {
                        true => None,
                        _ => Some(enc)
                    });
                    Json::Future(raw, encoding, PhantomData)
                },
                //No Encoding
                #[cfg(not(feature = "encoding"))]
                Json::Init(raw) => Json::Future(raw.take().expect("To have body"), PhantomData),
            };

            *self = new_state;
        }
    }
}

enum FileBodyType {
    Plain(hyper::Body, Option<io::BufWriter<fs::File>>),
    Deflate(hyper::Body, Option<flate2::write::ZlibDecoder<io::BufWriter<fs::File>>>),
    Gzip(hyper::Body, Option<flate2::write::GzDecoder<io::BufWriter<fs::File>>>),
}

///Redirects body to file.
pub struct FileBody<N> {
    parts: http::response::Parts,
    body: FileBodyType,
    notifier: N,
}

impl<N: Notifier> FileBody<N> {
    ///Creates new instance.
    pub fn new(response: super::Response, file: fs::File, notifier: N) -> Self {
        let encoding = response.content_encoding();
        let (parts, body) = response.inner.into_parts();
        let file = io::BufWriter::new(file);

        let body = match encoding {
            #[cfg(feature = "flate2")]
            header::ContentEncoding::Deflate => FileBodyType::Deflate(body, Some(flate2::write::ZlibDecoder::new(file))),
            #[cfg(feature = "flate2")]
            header::ContentEncoding::Gzip => FileBodyType::Gzip(body, Some(flate2::write::GzDecoder::new(file))),
            _ => FileBodyType::Plain(body, Some(file)),
        };

        Self {
            parts,
            body,
            notifier
        }
    }

    #[inline]
    ///Retrieves `Content-Disposition`, if it valid one is present.
    pub fn content_disposition(&self) -> Option<header::ContentDisposition> {
        self.parts.headers
                  .get(header::CONTENT_DISPOSITION)
                  .and_then(|header| header.to_str().ok())
                  .and_then(|header| header::ContentDisposition::from_str(header).ok())
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
    ///Transforms self into future with new [Notifier](notify/trait.Notifier.html)
    pub fn with_notify<T: notify::Notifier>(self, notifier: T) -> FileBody<T> {
        FileBody::<T> {
            parts: self.parts,
            body: self.body,
            notifier,
        }
    }
}

impl<N: Notifier> Future for FileBody<N> {
    type Item = fs::File;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let bytes = match self.body {
                FileBodyType::Plain(ref mut body, ref mut buffer) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        buffer.as_mut().unwrap().write_all(&chunk).map_err(|error| {
                            let file = buffer.take().unwrap();
                            //TODO: consider how to get File without stumbling into error
                            BodyReadError::FileError(file.into_inner().expect("To get File"), error)
                        })?;
                        //We loop, to schedule more IO
                        chunk.len()
                    },
                    Ok(futures::Async::Ready(None)) => {
                        let file = buffer.take().unwrap();
                        let mut file = file.into_inner()
                                           .map_err(|error| BodyReadError::FileError(buffer.take().unwrap().into_inner().expect("To get file"), error.into()))?;
                        file.flush().map_err(|error| BodyReadError::FileError(buffer.take().unwrap().into_inner().expect("To get file"), error))?;

                        return Ok(futures::Async::Ready(file))
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error.into())
                },
                #[cfg(feature = "flate2")]
                FileBodyType::Deflate(ref mut body, ref mut decoder) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        decoder.as_mut().unwrap().write_all(&chunk).map_err(|error| BodyReadError::DeflateError(error))?;
                        //We loop, to schedule more IO
                        chunk.len()
                    },
                    Ok(futures::Async::Ready(None)) => {
                        let mut decoder = decoder.take().unwrap();
                        decoder.flush().map_err(|error| BodyReadError::DeflateError(error))?;
                        let file = decoder.finish().map_err(|error| BodyReadError::DeflateError(error))?;

                        let mut file = file.into_inner().expect("Retrieve File from BufWriter");
                        return match file.flush() {
                            Ok(_) => Ok(futures::Async::Ready(file)),
                            Err(error) => Err(BodyReadError::FileError(file, error))
                        }
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error.into())

                },
                #[cfg(feature = "flate2")]
                FileBodyType::Gzip(ref mut body, ref mut decoder) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        decoder.as_mut().unwrap().write_all(&chunk).map_err(|error| BodyReadError::GzipError(error))?;
                        //We loop, to schedule more IO
                        chunk.len()
                    },
                    Ok(futures::Async::Ready(None)) => {
                        let mut decoder = decoder.take().unwrap();
                        decoder.flush().map_err(|error| BodyReadError::GzipError(error))?;
                        let file = decoder.finish().map_err(|error| BodyReadError::GzipError(error))?;

                        let mut file = file.into_inner().expect("Retrieve File from BufWriter");

                        return match file.flush() {
                            Ok(_) => Ok(futures::Async::Ready(file)),
                            Err(error) => Err(BodyReadError::FileError(file, error))
                        }
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error.into())

                },
            };

            self.notifier.send(bytes);
        }
    }
}