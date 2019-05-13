//!Extractors module

use std::{string, io};
use std::error::Error;
use std::fs;

mod notify;
mod cookie;
mod body;

pub use self::cookie::CookieIter;
pub use notify::{Notifier, Noop};
pub use body::{*};

#[derive(Debug, derive_more::Display)]
///Describes possible errors when reading body.
pub enum BodyReadError {
    #[display(fmt = "Failed to read due to HTTP error: {}", "_0")]
    ///Hyper's error.
    Hyper(hyper::Error),
    #[display(fmt = "Read limit is reached. Aborted reading.")]
    ///Hits limit, contains already read data.
    Overflow(bytes::Bytes),
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
