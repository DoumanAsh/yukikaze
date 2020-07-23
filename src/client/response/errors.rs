//!Response errors.

use std::error::Error;
use core::fmt;

///Describes errors related to content type.
#[derive(Debug)]
pub enum ContentTypeError {
    ///Mime parsing error.
    Mime(mime::FromStrError),
    ///Unknown encoding of Content-Type.
    UnknownEncoding,
}

impl Error for ContentTypeError {}

impl fmt::Display for ContentTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ContentTypeError::Mime(err) => write!(f, "Failed to parse Mime: {}", err),
            ContentTypeError::UnknownEncoding => f.write_str("Unable to recognize encoding"),
        }
    }
}

impl From<mime::FromStrError> for ContentTypeError {
    fn from(err: mime::FromStrError) -> Self {
        ContentTypeError::Mime(err)
    }
}
