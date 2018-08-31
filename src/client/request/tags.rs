//! Request setter tags

use ::header;

///Describes how to set ETag in request.
pub trait EtagMode {
    #[doc(hidden)]
    const HEADER_NAME: header::HeaderName;
}

///Sets ETag into `If-None-Match` header.
pub struct IfNoneMatch;

impl EtagMode for IfNoneMatch {
    const HEADER_NAME: header::HeaderName = header::IF_NONE_MATCH;
}

///Sets ETag into `If-Match` header.
pub struct IfMatch;

impl EtagMode for IfMatch {
    const HEADER_NAME: header::HeaderName = header::IF_MATCH;
}

///Describes how to set HttpDate in request.
pub trait DateMode {
    #[doc(hidden)]
    const HEADER_NAME: header::HeaderName;
}

///Sets HttpDate into `If-Modified-Since` header.
pub struct IfModifiedSince;

impl DateMode for IfModifiedSince {
    const HEADER_NAME: header::HeaderName = header::IF_MODIFIED_SINCE;
}

///Sets HttpDate into `If-Unmodified-Since` header.
pub struct IfUnmodifiedSince;

impl DateMode for IfUnmodifiedSince {
    const HEADER_NAME: header::HeaderName = header::IF_UNMODIFIED_SINCE;
}
