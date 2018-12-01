use ::std::time;
use ::std::fmt;
use ::std::fs;
use ::std::io;
use ::std::string;

use ::tokio_timer;
use ::mime;
use ::hyper;
use ::serde_json;

use super::FutureResponse;

///Describes errors related to content type.
#[derive(Debug)]
pub enum ContentTypeError {
    ///Mime parsing error.
    Mime(mime::FromStrError),
    ///Unknown encoding of Content-Type.
    UnknownEncoding,
}

impl fmt::Display for ContentTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ContentTypeError::Mime(ref error) => write!(f, "Failed to parse Mime: {}", error),
            &ContentTypeError::UnknownEncoding => write!(f, "Unable to recognize encoding")
        }
    }
}

impl From<mime::FromStrError> for ContentTypeError {
    #[inline]
    fn from(error: mime::FromStrError) -> Self {
        ContentTypeError::Mime(error)
    }
}

#[derive(Debug)]
///Represents failed due to timeout request.
///
///It is possible to fire request again
///In a case you suspect potential network problems
///but you don't want to set too high timeout value for your
///client you can rely on it to continue your request.
pub struct Timeout<F> {
    inner: F
}

impl<F> Timeout<F> {
    pub(crate) fn new(inner: F) -> Self {
        Self {
            inner
        }
    }

    ///Starts request again with new timeout.
    pub fn retry(self, timeout: time::Duration) -> FutureResponse<F> {
        FutureResponse::new(self.inner, timeout)
    }
}

#[derive(Debug)]
///Describes possible response errors.
pub enum ResponseError<F> {
    ///Response failed due to timeout.
    Timeout(Timeout<F>),
    ///Hyper Error.
    HyperError(hyper::error::Error),
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

impl<F> fmt::Display for ResponseError<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ResponseError::Timeout(_) => write!(f, "Request timed out."),
            &ResponseError::Timer(ref error, _) => write!(f, "IO timer error happened while executing request: {}", error),
            &ResponseError::HyperError(ref error) => write!(f, "Request failed due to HTTP error: {}", error)
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
