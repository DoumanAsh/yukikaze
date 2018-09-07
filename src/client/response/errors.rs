use ::std::time;
use ::std::fmt;

use ::tokio;
use ::mime;
use ::hyper;

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
pub struct Timeout {
    inner: hyper::client::ResponseFuture,
}

impl Timeout {
    ///Starts request again with new timeout.
    pub fn retry(self, timeout: time::Duration) -> FutureResponse {
        FutureResponse::new(self.inner, timeout)
    }
}

impl Into<Timeout> for hyper::client::ResponseFuture {
    fn into(self) -> Timeout {
        Timeout {
            inner: self
        }
    }
}

#[derive(Debug)]
///Describes possible response errors.
pub enum ResponseError {
    ///Response failed due to timeout.
    Timeout(Timeout),
    ///Hyper Error.
    HyperError(hyper::error::Error),
    ///Tokio timer threw error.
    Timer(tokio::timer::Error, Timeout)
}

impl ResponseError {
    ///Attempts to retry, if it is possible.
    ///
    ///Currently retry can be made only for timed out request or when
    ///timer error happened.
    pub fn retry(self, timeout: time::Duration) -> Result<FutureResponse, hyper::error::Error> {
        match self {
            ResponseError::Timeout(tim) => Ok(tim.retry(timeout)),
            ResponseError::HyperError(error) => Err(error),
            ResponseError::Timer(_, tim) => Ok(tim.retry(timeout)),
        }
    }
}

impl From<hyper::error::Error> for ResponseError {
    fn from(error: hyper::error::Error) -> ResponseError {
        ResponseError::HyperError(error)
    }
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ResponseError::Timeout(_) => write!(f, "Request timed out."),
            &ResponseError::Timer(ref error, _) => write!(f, "IO timer error happened while executing request: {}", error),
            &ResponseError::HyperError(ref error) => write!(f, "Request failed due to HTTP error: {}", error)
        }
    }
}

