//!Multipart support

use ::bytes::Bytes;
use ::mime::Mime;

use ::std::io::Write;

use ::header::{ContentDisposition, Filename};
use ::utils::BytesWriter;

const DEFAULT_BOUNDARY: &'static str = "yuki";

///Multipart Form.
///
///Default boundary is `yuki`.
pub struct Form {
    ///Boundary to use.
    pub boundary: &'static str,
    storage: BytesWriter,
}

impl Form {
    ///Creates new instance of form.
    pub fn new() -> Self {
        Self::with_boundary(DEFAULT_BOUNDARY)
    }

    ///Creates new instance with provided boundary.
    ///
    ///# Panic
    ///
    ///In debug builds, it asserts whether string contains only ASCII characters or not.
    pub fn with_boundary(boundary: &'static str) -> Self {
        debug_assert!(boundary.is_ascii());

        Self {
            boundary,
            storage: BytesWriter::new()
        }
    }

    ///Adds new field with jsut name.
    pub fn add_field(&mut self, name: String, data: &[u8]) {
        let content_disposition = ContentDisposition::FormData(Some(name), Filename::new());
        let _ = write!(&mut self.storage, "--{}\r\nContent-Disposition: {}\r\n\r\n", self.boundary, content_disposition);
        let _ = self.storage.write(data);
        let _ = write!(&mut self.storage, "\r\n--{}\r\n", self.boundary);
    }

    ///Adds new field with file.
    pub fn add_file_field(&mut self, field_name: String, file_name: String, mime: &Mime, data: &[u8]) {
        let content_disposition = ContentDisposition::FormData(Some(field_name), Filename::with_encoded_name(file_name));
        let _ = write!(&mut self.storage, "--{}\r\nContent-Disposition: {}\r\n", self.boundary, content_disposition);
        let _ = write!(&mut self.storage, "Content-Type: {}\r\n\r\n", mime);
        let _ = self.storage.write(data);
        let _ = write!(&mut self.storage, "\r\n--{}\r\n", self.boundary);
    }

    ///Finishes creating form and produces body with its length
    pub fn finish(self) -> (u64, Bytes) {
        let mut bytes = self.storage.into_inner();
        let len = bytes.len();
        if len == 0 {
            return (0, bytes.freeze());
        }

        *(&mut bytes[len-2]) = 45; //'-'
        *(&mut bytes[len-1]) = 45;

        bytes.extend_from_slice("\r\n".as_bytes());
        let len = len as u64 + 2;

        (len, bytes.freeze())
    }
}

#[cfg(test)]
mod tests {
    use ::mime::TEXT_PLAIN;
    use ::std::str;
    use super::Form;

    #[test]
    fn multipart_form_add_simple_field() {
        static EXPECTED: &'static str = "--yuki\r\nContent-Disposition: form-data; name=\"SimpleField\"\r\n\r\nsimple test\r\n--yuki--\r\n";

        let mut form = Form::new();
        form.add_field("SimpleField".to_string(), "simple test".as_bytes());

        let (len, body) = form.finish();
        let str_body = str::from_utf8(&body).expect("To get str slice of body");
        assert_eq!(len, EXPECTED.len() as u64);
        assert_eq!(str_body, EXPECTED);
    }

    #[test]
    fn multipart_form_add_multiple_fields() {
        static EXPECTED: &'static str = "--yuki\r\nContent-Disposition: form-data; name=\"SimpleField\"\r\n\r\nsimple test\r\n--yuki\r\n--yuki\r\nContent-Disposition: form-data; name=\"SimpleFile\"; filename=\"File.txt\"\r\nContent-Type: text/plain\r\n\r\nsimple file\r\n--yuki--\r\n";

        let mut form = Form::new();
        form.add_field("SimpleField".to_string(), "simple test".as_bytes());
        form.add_file_field("SimpleFile".to_string(), "File.txt".to_string(), &TEXT_PLAIN, "simple file".as_bytes());

        let (len, body) = form.finish();
        let str_body = str::from_utf8(&body).expect("To get str slice of body");
        assert_eq!(len, EXPECTED.len() as u64);
        assert_eq!(str_body, EXPECTED);
    }

}
