//!Response extractors

use crate::header;

///Cookie extractor.
///
///As it returns references they would tie
///up original response, if you want avoid it
///you can use `Cookie::into_owned()`
pub struct CookieIter<'a> {
    pub(crate) iter: header::ValueIter<'a, header::HeaderValue>,
}

impl<'a> Iterator for CookieIter<'a> {
    type Item = Result<cookie::Cookie<'a>, cookie::ParseError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        use percent_encoding::percent_decode;

        if let Some(cook) = self.iter.by_ref().next() {
            let cook = percent_decode(cook.as_bytes());
            let cook = cook.decode_utf8().map_err(|error| cookie::ParseError::Utf8Error(error))
                                         .and_then(|cook| cookie::Cookie::parse(cook));
            Some(cook)
        } else {
            None
        }
    }
}
