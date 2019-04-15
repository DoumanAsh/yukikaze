use std::time;
use std::fmt;
use std::fs;
use std::io;
use std::string;
use std::error::Error;

use tokio_timer;
use mime;
use hyper;
use serde_json;

use super::FutureResponse;

///Describes errors related to content type.
#[derive(Debug, derive_more::From, derive_more::Display)]
pub enum ContentTypeError {
    #[display(fmt = "Failed to parse Mime: {}", "_0")]
    ///Mime parsing error.
    Mime(mime::FromStrError),
    #[display(fmt = "Unable to recognize encoding")]
    ///Unknown encoding of Content-Type.
    UnknownEncoding,
}

impl Error for ContentTypeError {}

#[derive(Debug)]
///Represents failed due to timeout request.
///
///It is possible to fire request again
///In a case you suspect potential network problems
///but you don't want to set too high timeout value for your
///client you can rely on it to continue your request.
pub struct Timeout<F> {
    inner: (F, super::FutureResponseParams)
}

impl<F> Timeout<F> {
    pub(crate) fn new(inner: (F, super::FutureResponseParams)) -> Self {
        Self {
            inner
        }
    }

    ///Starts request again with new timeout.
    pub fn retry(self, timeout: time::Duration) -> FutureResponse<F> {
        FutureResponse::new(self.inner.0, timeout, self.inner.1)
    }
}

#[derive(Debug, derive_more::Display)]
///Describes possible response errors.
pub enum ResponseError<F> {
    #[display(fmt = "Request timed out.")]
    ///Response failed due to timeout.
    Timeout(Timeout<F>),
    #[display(fmt = "IO timer error happened while executing request: {}", "_0")]
    ///Hyper Error.
    HyperError(hyper::error::Error),
    #[display(fmt = "Request failed due to HTTP error: {}", "_0")]
    ///Tokio timer threw error.
    Timer(tokio_timer::Error, Timeout<F>)
}

impl<F> ResponseError<F> {
    ///Attempts to retry, if it is possible.
    ///
    ///Currently retry can be made only for timed out request or when
    ///timer error happened.
    pub fn retry(self, timeout: time::Duration) -> Result<FutureResponse<F>, hyper::error::Error> {
        match self {
            ResponseError::Timeout(tim) => Ok(tim.retry(timeout)),
            ResponseError::HyperError(error) => Err(error),
            ResponseError::Timer(_, tim) => Ok(tim.retry(timeout)),
        }
    }
}

impl<F> From<hyper::error::Error> for ResponseError<F> {
    fn from(error: hyper::error::Error) -> ResponseError<F> {
        ResponseError::HyperError(error)
    }
}

impl<F: fmt::Debug> Error for ResponseError<F> {}

#[derive(Debug, derive_more::Display)]
///Describes possible errors when reading body.
pub enum BodyReadError {
    #[display(fmt = "Failed to read due to HTTP error: {}", "_0")]
    ///Hyper's error.
    Hyper(hyper::Error),
    #[display(fmt = "Read limit is reached. Aborted reading.")]
    ///Hit limit
    Overflow,
    #[display(fmt = "Unable to decode content into UTF-8")]
    ///Unable to decode body as UTF-8
    EncodingError,
    #[display(fmt = "Failed to extract JSON. Error: {}", "_0")]
    ///Json serialization error.
    JsonError(serde_json::error::Error),
    #[display(fmt = "Failed to decompress(deflate) content. Error: {}", "_0")]
    ///Error happened during deflate decompression.
    DeflateError(io::Error),
    #[display(fmt = "Failed to decompress(gzip) content. Error: {}", "_0")]
    ///Error happened during gzip decompression.
    GzipError(io::Error),
    #[display(fmt = "Error file writing response into file. Error: {}", "_1")]
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

impl Error for BodyReadError {}
