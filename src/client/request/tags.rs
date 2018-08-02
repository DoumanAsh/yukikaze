//! Request setter tags

use ::header;

///Describes how to set ETag in request
pub trait EtagMode {
    #[doc(hidden)]
    const HEADER_NAME: header::HeaderName;
}

///Sets ETag into `If-None-Match` header
pub struct IfNoneMatch;

impl EtagMode for IfNoneMatch {
    const HEADER_NAME: header::HeaderName = header::IF_NONE_MATCH;
}

///Sets ETag into `If-Match` header
pub struct IfMatch;

impl EtagMode for IfMatch {
    const HEADER_NAME: header::HeaderName = header::IF_MATCH;
}
