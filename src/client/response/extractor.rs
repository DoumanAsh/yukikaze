///!Response extractors

use ::std::fs;
use ::std::io;
use ::std::io::Write;
use ::std::string;
use ::std::mem;
use ::std::fmt;

use ::header;
use ::utils;
use super::errors;

#[cfg(feature = "encoding")]
use ::encoding;
use ::mime;
#[cfg(feature = "flate2")]
use ::flate2;
use ::hyper;
use ::http;
use ::futures;
use ::futures::{Future, Stream};
use ::bytes;
use ::serde_json;
use ::serde::de::DeserializeOwned;
use ::cookie;

//The size of buffer to use by default.
const BUFFER_SIZE: usize = 4096;
//The default limit on body size 2mb.
const DEFEAULT_LIMIT: u64 = 2 * 1024 * 1024;

///Cookie extractor.
///
///As it returns references they would tie
///up original response, if you want avoid it
///you can use `Cookie::into_owned()`
pub struct CookieIter<'a> {
    pub(crate) iter: header::ValueIter<'a, header::HeaderValue>,
}

impl<'a> Iterator for CookieIter<'a> {
    type Item = Result<cookie::Cookie<'a>, cookie::ParseError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        use ::percent_encoding::percent_decode;

        if let Some(cook) = self.iter.by_ref().next() {
            let cook = percent_decode(cook.as_bytes());
            let cook = cook.decode_utf8().map_err(|error| cookie::ParseError::Utf8Error(error))
                                         .and_then(|cook| cookie::Cookie::parse(cook));
            Some(cook)
        } else {
            None
        }
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
    ///Error happened when writing to file.
    FileError(fs::File, io::Error),
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

impl fmt::Display for BodyReadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &BodyReadError::Hyper(ref error) => write!(f, "Failed to read due to HTTP error: {}", error),
            &BodyReadError::Overflow => write!(f, "Read limit is reached. Aborted reading."),
            &BodyReadError::EncodingError => write!(f, "Unable to decode content into UTF-8"),
            &BodyReadError::JsonError(ref error) => write!(f, "Failed to extract JSON. Error: {}", error),
            &BodyReadError::DeflateError(ref error) => write!(f, "Failed to decompress content. Error: {}", error),
            &BodyReadError::GzipError(ref error) => write!(f, "Failed to decompress content. Error: {}", error),
            &BodyReadError::FileError(_, ref error) => write!(f, "Error file writing response into file. Error: {}", error),
        }
    }
}

enum BodyType {
    Plain(hyper::Body, bytes::BytesMut),
    Deflate(hyper::Body, flate2::write::ZlibDecoder<utils::BytesWriter>),
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
pub struct RawBody {
    parts: http::response::Parts,
    body: BodyType,
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
            header::ContentEncoding::Deflate => BodyType::Deflate(body, flate2::write::ZlibDecoder::new(utils::BytesWriter::with_capacity(buffer_size))),
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
    pub fn charset_encoding(&self) -> Result<encoding::EncodingRef, errors::ContentTypeError> {
        let mime = self.mime()?;
        let mime = mime.as_ref().and_then(|mime| mime.get_param(mime::CHARSET));

        match mime {
            Some(charset) => match encoding::label::encoding_from_whatwg_label(charset.as_str()) {
                Some(enc) => Ok(enc),
                None => Err(errors::ContentTypeError::UnknownEncoding)
            },
            None => Ok(encoding::all::UTF_8),
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
///
///# Encoding feature
///
///If `Content-Encoding` contains charset information it
///shall be automatically applied when decoding data.
pub enum Text {
    #[doc(hidden)]
    Init(Option<RawBody>),
    #[cfg(feature = "encoding")]
    #[doc(hidden)]
    Future(RawBody, Option<encoding::EncodingRef>),
    #[cfg(not(feature = "encoding"))]
    #[doc(hidden)]
    Future(RawBody),
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
}

impl Future for Text {
    type Item = String;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            let new_state = match self {
                //Encoding
                #[cfg(feature = "encoding")]
                Text::Future(fut, enc) => match fut.poll() {
                    Ok(futures::Async::Ready(bytes)) => return match enc {
                        Some(enc) => enc.decode(&bytes, encoding::types::DecoderTrap::Strict)
                                        .map_err(|_| BodyReadError::EncodingError)
                                        .map(|st| futures::Async::Ready(st)),
                        None => String::from_utf8(bytes.to_vec()).map_err(|error| error.into()).map(|st| futures::Async::Ready(st))
                    },
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error)
                },
                #[cfg(feature = "encoding")]
                Text::Init(raw) => {
                    let raw = raw.take().expect("To have body");
                    let encoding = raw.charset_encoding().ok().and_then(|enc| match enc.name() {
                        "utf-8" => None,
                        _ => Some(enc)
                    });
                    Text::Future(raw, encoding)
                },
                //No Encoding
                #[cfg(not(feature = "encoding"))]
                Text::Init(raw) => Text::Future(raw.take().expect("To have body")),
                #[cfg(not(feature = "encoding"))]
                Text::Future(fut) => match fut.poll() {
                    Ok(futures::Async::Ready(bytes)) => return String::from_utf8(bytes.to_vec()).map_err(|error| error.into())
                                                                                                .map(|st| futures::Async::Ready(st)),
                    Ok(futures::Async::NotReady) => return Ok(futures::Async::NotReady),
                    Err(error) => return Err(error)
                }
            };

            *self = new_state;
        }
    }
}

///Reads raw bytes from HTTP Response and de-serializes as JSON struct
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
            Json::Init(Some(raw)) => Json::Init(Some(raw.limit(limit))),
            _ => self,
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

enum FileBodyType {
    Plain(hyper::Body, Option<io::BufWriter<fs::File>>),
    Deflate(hyper::Body, Option<flate2::write::ZlibDecoder<io::BufWriter<fs::File>>>),
    Gzip(hyper::Body, Option<flate2::write::GzDecoder<io::BufWriter<fs::File>>>),
}

///Redirects body to file.
pub struct FileBody {
    parts: http::response::Parts,
    body: FileBodyType,
}

impl FileBody {
    ///Creates new instance.
    pub fn new(response: super::Response, file: fs::File) -> Self {
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
        }
    }

    #[inline]
    ///Retrieves `Content-Disposition`, if it valid one is present.
    pub fn content_disposition(&self) -> Option<header::ContentDisposition> {
        self.parts.headers
                  .get(header::CONTENT_DISPOSITION)
                  .and_then(|header| header.to_str().ok())
                  .and_then(|header| header::ContentDisposition::from_str(header))
    }


    #[inline]
    ///Retrieves length of content to receive, if `Content-Length` exists.
    pub fn content_len(&self) -> Option<u64> {
        self.parts.headers
            .get(header::CONTENT_LENGTH)
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.parse().ok())
    }
}

impl Future for FileBody {
    type Item = fs::File;
    type Error = BodyReadError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        loop {
            match self.body {
                FileBodyType::Plain(ref mut body, ref mut buffer) => match body.poll() {
                    Ok(futures::Async::Ready(Some(chunk))) => {
                        buffer.as_mut().unwrap().write_all(&chunk).map_err(|error| {
                            let file = buffer.take().unwrap();
                            //TODO: consider how to get File without stumbling into error
                            BodyReadError::FileError(file.into_inner().expect("To get File"), error)
                        })?;
                        //We loop, to schedule more IO
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
            }
        }
    }
}
