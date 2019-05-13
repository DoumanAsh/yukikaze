//!Response errors.

use std::error::Error;

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
