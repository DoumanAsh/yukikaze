//!Extractors module
//!
//!Various utilities that are used by yukikaze client module

use std::{string, io};
use std::error::Error;
use std::fs;
use core::fmt;

mod notify;
mod cookie;
mod body;

pub use self::cookie::CookieIter;
pub use notify::{Notifier, Noop};
pub use body::{*};

#[derive(Debug)]
///Describes possible errors when reading body.
pub enum BodyReadError {
    ///Hits limit, contains already read data.
    Overflow(bytes::Bytes),
    ///Unable to decode body as UTF-8
    EncodingError,
    ///Json serialization error.
    JsonError(serde_json::error::Error),
    #[cfg(feature = "compu")]
    ///Error happened during decompression.
    CompuError(compu::decoder::DecoderResult),
    ///Failed to decompress content as it is not complete.
    IncompleteDecompression,
    ///Error happened when writing to file.
    FileError(fs::File, io::Error),
    ///Some IO Error during reading
    ///
    ///Convertion from `io::Error` creates this  variant
    ReadError(io::Error),
    ///Hyper's error.
    ///
    ///Disabled when `client` feature is not enabled
    Hyper(hyper::Error),
}

impl fmt::Display for BodyReadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BodyReadError::Overflow(_) => f.write_str("Read limit is reached. Aborted reading."),
            BodyReadError::EncodingError => f.write_str("Unable to decode content into UTF-8"),
            BodyReadError::JsonError(err) => write!(f, "Failed to extract JSON. Error: {}", err),
            #[cfg(feature = "compu")]
            BodyReadError::CompuError(err) => write!(f, "Failed to decompress content. Error: {:?}", err),
            BodyReadError::IncompleteDecompression => f.write_str("Failed to decompress content as it is not complete"),
            BodyReadError::FileError(_, err) => write!(f, "Error file writing response into file. Error: {}", err),
            BodyReadError::ReadError(err) => write!(f, "IO Error while reading: {}", err),
            BodyReadError::Hyper(err) => write!(f, "Failed to read due to HTTP error: {}", err),
        }
    }
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

impl From<io::Error> for BodyReadError {
    #[inline]
    fn from(error: io::Error) -> Self {
        BodyReadError::ReadError(error)
    }
}

impl From<hyper::Error> for BodyReadError {
    #[inline]
    fn from(err: hyper::Error) -> Self {
        BodyReadError::Hyper(err)
    }
}

impl Error for BodyReadError {}
