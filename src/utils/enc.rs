//!Encoding utilities
use percent_encoding::AsciiSet;

/// As defined in https://url.spec.whatwg.org/#fragment-percent-encode-set
pub const FRAGMENT_ENCODE_SET: &AsciiSet = &percent_encoding::CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

/// As defined in https://url.spec.whatwg.org/#path-percent-encode-set
pub const PATH_ENCODE_SET: &AsciiSet = &FRAGMENT_ENCODE_SET.add(b'#').add(b'?').add(b'{').add(b'}');

/// As defined in https://url.spec.whatwg.org/#userinfo-percent-encode-set
pub const USER_INFO_ENCODE_SET: &AsciiSet = &PATH_ENCODE_SET.add(b'/').add(b':').add(b';').add(b'=').add(b'@').add(b'[').add(b'\\').add(b']').add(b'^').add(b'|');

/// As defined in https://tools.ietf.org/html/rfc5987#section-3.2.1
pub const HEADER_VALUE_ENCODE_SET: &AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'!').remove(b'#').remove(b'$').remove(b'&').remove(b'+').remove(b'-').remove(b'.').remove(b'^').remove(b'_').remove(b'`').remove(b'|').remove(b'~');
