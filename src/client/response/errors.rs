use ::mime;

///Describes erorrs related to content type
#[derive(Debug)]
pub enum ContentTypeError {
    ///Mime parsing error.
    Mime(mime::FromStrError),
    ///Unknown encoding of Content-Type.
    UnknownEncoding,
}

impl From<mime::FromStrError> for ContentTypeError {
    #[inline]
    fn from(error: mime::FromStrError) -> Self {
        ContentTypeError::Mime(error)
    }
}

