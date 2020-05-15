use percent_encoding::{utf8_percent_encode, percent_decode_str};
use crate::utils::enc::HEADER_VALUE_ENCODE_SET;

use core::fmt;
use core::str::FromStr;

use std::error::Error;

#[derive(Debug)]
///Filename parameter of `Content-Disposition`
pub enum Filename {
    ///Regular `filename`
    Name(Option<String>),
    ///Extended `filename*`
    ///
    ///Charset is always UTF-8, because whatelse you need?
    ///
    ///Values:
    ///1. Optional language tag.
    ///2. Correctly percent encoded string
    Extended(Option<String>, String)
}

impl Filename {
    ///Returns default `Filename` with empty name field.
    pub fn new() -> Self {
        Filename::Name(None)
    }

    ///Creates file name.
    pub fn with_name(name: String) -> Self {
        Filename::Name(Some(name))
    }

    ///Creates file name, and checks whether it should be encoded.
    ///
    ///Note that actual encoding would happen only when header is written.
    ///The value itself would remain unchanged in the `Filename`.
    pub fn with_encoded_name(name: std::borrow::Cow<'_, str>) -> Self {
        match name.is_ascii() {
            true => Self::with_name(name.into_owned()),
            false => match utf8_percent_encode(&name, HEADER_VALUE_ENCODE_SET).into() {
                std::borrow::Cow::Owned(encoded) => Self::with_extended(None, encoded),
                std::borrow::Cow::Borrowed(maybe_encoded) => match maybe_encoded == name {
                    true => Self::with_extended(None, maybe_encoded.to_owned()),
                    false => Self::with_name(name.into_owned()),
                }
            }
        }
    }

    #[inline]
    ///Creates extended file name.
    pub fn with_extended(lang: Option<String>, name: String) -> Self {
        Filename::Extended(lang, name)
    }

    #[inline]
    ///Returns whether filename is of extended type.
    pub fn is_extended(&self) -> bool {
        match self {
            Filename::Extended(_, _) => true,
            _ => false
        }
    }

    ///Returns file name, percent decoded if necessary.
    ///
    ///Note: expects to work with utf-8 only.
    pub fn name(&self) -> Option<std::borrow::Cow<'_, str>> {
        match self {
            Filename::Name(None) => None,
            Filename::Name(Some(ref name)) => Some(name.as_str().into()),
            Filename::Extended(_, name) => Some(percent_decode_str(&name).decode_utf8_lossy()),
        }
    }

    ///Consumes self and returns file name, if present.
    ///
    ///Note: expects to work with utf-8 only.
    pub fn into_name(self) -> Option<String> {
        match self {
            Filename::Name(None) => None,
            Filename::Name(Some(name)) => Some(name),
            Filename::Extended(_, name) => Some(percent_decode_str(&name).decode_utf8_lossy().into_owned()),
        }
    }
}

#[derive(Debug)]
/// A `Content-Disposition` header, defined in [RFC6266](https://tools.ietf.org/html/rfc6266).
///
/// The Content-Disposition response header field is used to convey
/// additional information about how to process the response payload, and
/// also can be used to attach additional metadata, such as the filename
/// to use when saving the response payload locally.
pub enum ContentDisposition {
    ///Tells that content should be displayed inside web page.
    Inline,
    ///Tells that content should be downloaded.
    Attachment(Filename),
    ///Tells that content is field of form with name and filename
    ///
    ///## Note
    ///
    ///This is an extension that can be used only inside of multipart
    ///body, it is not expected value for header.
    FormData(Option<String>, Filename)
}

fn split_into_two(text: &str, sep: char) -> (&str, &str) {
    match text.find(sep) {
        Some(end) => (&text[..end].trim_end(), &text[end+1..].trim_start()),
        None => (text, ""),
    }
}

macro_rules! parse_file_ext {
    ($param:ident) => {{
        let mut parts = $param.splitn(3, '\'');

        //Should be utf-8, but since we parse from str, should be always utf-8
        let _ = match parts.next() {
            Some(charset) => charset.to_owned(),
            None => continue
        };
        let lang = parts.next().map(|lang| lang.to_owned());
        let value = match parts.next() {
            Some(value) => value.to_owned(),
            None => continue
        };

        Filename::Extended(lang, value)
    }}
}

#[derive(Debug)]
pub enum ParseError {
    InvalidDispositionType,
    UnknownAttachmentParam,
    UnknownFormParam,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &ParseError::InvalidDispositionType => f.write_str("Specified disposition type is not valid. Should be inline, attachment or form-data"),
            &ParseError::UnknownAttachmentParam => f.write_str("Form-data parameter is invalid. Allowed: filename[*]"),
            &ParseError::UnknownFormParam => f.write_str("Form-data parameter is invalid. Allowed: name, filename[*]"),
        }
    }
}

impl Error for ParseError {
}

impl FromStr for ContentDisposition {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        const NAME: &str = "name";
        const FILENAME: &str = "filename";

        let text = text.trim();

        let (disp_type, arg) = split_into_two(text, ';');

        if disp_type.eq_ignore_ascii_case("inline") {
            Ok(ContentDisposition::Inline)
        } else if disp_type.eq_ignore_ascii_case("attachment") {
            let mut file_name = Filename::Name(None);

            for arg in arg.split(';').map(|arg| arg.trim()) {
                let (name, value) = split_into_two(arg, '=');

                if value.len() == 0 {
                    continue;
                }

                if name.len() < FILENAME.len() {
                    return Err(ParseError::UnknownAttachmentParam)
                }

                let prefix = &name[..FILENAME.len()];
                if prefix.eq_ignore_ascii_case("filename") {
                    let value = value.trim_matches('"');

                    if let Some(_) = name.rfind('*') {
                        file_name = parse_file_ext!(value);
                        break;
                    } else {
                        file_name = Filename::Name(Some(value.to_owned()));
                    }
                } else {
                    return Err(ParseError::UnknownAttachmentParam)
                }
            }

            Ok(ContentDisposition::Attachment(file_name))
        } else if disp_type.eq_ignore_ascii_case("form-data") {
            let mut name_param = None;
            let mut file_name = Filename::Name(None);

            for arg in arg.split(';').map(|arg| arg.trim()) {
                let (name, value) = split_into_two(arg, '=');

                if value.len() == 0 {
                    continue;
                }

                if name.eq_ignore_ascii_case(NAME) {
                    name_param = Some(value.trim_matches('"').to_owned());
                    continue;
                }
                else if name.len() < FILENAME.len() {
                    return Err(ParseError::UnknownFormParam)
                }

                let prefix = &name[..FILENAME.len()];
                if prefix.eq_ignore_ascii_case("filename") {
                    let value = value.trim_matches('"');

                    if let Some(_) = name.rfind('*') {
                        file_name = parse_file_ext!(value);
                    } else if !file_name.is_extended() {
                        file_name = Filename::Name(Some(value.to_owned()));
                    }
                } else {
                    return Err(ParseError::UnknownFormParam)
                }
            }

            Ok(ContentDisposition::FormData(name_param, file_name))
        } else {
            Err(ParseError::InvalidDispositionType)
        }
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ContentDisposition::Inline => write!(f, "inline"),
            ContentDisposition::Attachment(file) => match file {
                Filename::Name(Some(name)) => write!(f, "attachment; filename=\"{}\"", name),
                Filename::Name(None) => write!(f, "attachment"),
                Filename::Extended(lang, value) => {
                    write!(f, "attachment; filename*=utf-8'{}'{}",
                           lang.as_ref().map(|lang| lang.as_str()).unwrap_or(""),
                           value)
                },
            },
            ContentDisposition::FormData(None, file) => match file {
                Filename::Name(Some(name)) => write!(f, "form-data; filename=\"{}\"", name),
                Filename::Name(None) => write!(f, "form-data"),
                Filename::Extended(lang, value) => {
                    write!(f, "form-data; filename*=utf-8'{}'{}",
                           lang.as_ref().map(|lang| lang.as_str()).unwrap_or(""),
                           value)
                },
            },
            ContentDisposition::FormData(Some(name), file) => match file {
                Filename::Name(Some(file_name)) => write!(f, "form-data; name=\"{}\"; filename=\"{}\"", name, file_name),
                Filename::Name(None) => write!(f, "form-data; name=\"{}\"", name),
                Filename::Extended(lang, value) => {
                    write!(f, "form-data; name=\"{}\"; filename*=utf-8'{}'{}",
                           name,
                           lang.as_ref().map(|lang| lang.as_str()).unwrap_or(""),
                           value)
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use percent_encoding::{percent_decode};
    use super::{FromStr, ContentDisposition, Filename};

    #[test]
    fn parse_file_name_extended_ascii() {
        const INPUT: &'static str = "rori.mp4";
        let file_name = Filename::with_encoded_name(INPUT.into());
        assert!(!file_name.is_extended());
    }

    #[test]
    fn parse_file_name_extended_non_ascii() {
        const INPUT: &'static str = "ロリへんたい.mp4";
        let file_name = Filename::with_encoded_name(INPUT.into());
        assert!(file_name.is_extended());
    }

    #[test]
    fn parse_inline_disp() {
        const INPUT: &'static str = "inline";

        let result = ContentDisposition::from_str(INPUT).expect("To have inline Disposition");

        let result = match result {
            ContentDisposition::Inline => result.to_string(),
            _ => panic!("Invalid Content Disposition")
        };

        assert_eq!(result, INPUT);
    }


    #[test]
    fn parse_attach_disp_wo_filename() {
        const INPUT: &'static str = "attachment; filename";

        let result = ContentDisposition::from_str(INPUT).expect("To have attachment Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::Attachment(file) => {
                match file {
                    Filename::Name(name) => assert!(name.is_none()),
                    _ => panic!("Wrong Filename type"),
                }
            },
            _ => panic!("Invalid Content Disposition")
        }

        assert_eq!(result_text, "attachment");
    }

    #[test]
    fn parse_attach_disp_w_filename() {
        const INPUT: &'static str = "attachment; filename=\"lolka.jpg\";filename=\"lolka2.jpg\"";

        let result = ContentDisposition::from_str(INPUT).expect("To have attachment Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::Attachment(file) => {
                match file {
                    Filename::Name(name) => assert_eq!(name.expect("Filename value"), "lolka2.jpg"),
                    _ => panic!("Wrong Filename type"),
                }
            },
            _ => panic!("Invalid Content Disposition")
        }

        assert_eq!(result_text, "attachment; filename=\"lolka2.jpg\"");
    }

    #[test]
    fn parse_attach_disp_w_filename_ext() {
        const EXPECT_INPUT: &'static str = "attachment; filename*=utf-8'en'%C2%A3%20and%20%E2%82%AC%20rates";
        const INPUT: &'static str = "attachment;\t filename*=UTF-8'en'%C2%A3%20and%20%E2%82%AC%20rates";

        let result = ContentDisposition::from_str(INPUT).expect("To have attachment Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::Attachment(file) => {
                assert!(file.is_extended());

                let expected_value = percent_decode("%C2%A3%20and%20%E2%82%AC%20rates".as_bytes()).decode_utf8_lossy();
                let value = file.name().expect("To have file name");
                assert_eq!(value, expected_value);
            },
            _ => panic!("Invalid Content Disposition")
        }

        assert_eq!(result_text, EXPECT_INPUT);
    }

    #[test]
    fn parse_form_data() {
        const EXPECT_INPUT: &'static str = "form-data; name=\"lolka\"; filename=\"lolka.jpg\"";
        const INPUT: &'static str = "form-data;\t name=\"lolka\";filename=\"lolka.jpg\"";

        let result = ContentDisposition::from_str(INPUT).expect("To have form-data Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::FormData(name, file) => {
                assert_eq!(name.expect("To have form-data name"), "lolka");
                match file {
                    Filename::Name(name) => assert_eq!(name.expect("Filename value"), "lolka.jpg"),
                    _ => panic!("Wrong Filename type"),
                }
            },
            _ => panic!("Invalid Content Disposition")
        }

        assert_eq!(result_text, EXPECT_INPUT);
    }

    #[test]
    fn parse_form_data_wo_params() {
        const INPUT: &'static str = "form-data";

        let result = ContentDisposition::from_str(INPUT).expect("To have form-data Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::FormData(name, file) => {
                assert!(name.is_none());
                match file {
                    Filename::Name(name) => assert!(name.is_none()),
                    _ => panic!("Wrong Filename type"),
                }
            },
            _ => panic!("Invalid Content Disposition")
        }

        assert_eq!(result_text, INPUT);
    }

    #[test]
    fn parse_form_data_wo_name() {
        const INPUT: &'static str = "form-data; filename=\"lolka.jpg\"";

        let result = ContentDisposition::from_str(INPUT).expect("To have form-data Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::FormData(name, file) => {
                assert!(name.is_none());
                match file {
                    Filename::Name(name) => assert_eq!(name.expect("Filename value"), "lolka.jpg"),
                    _ => panic!("Wrong Filename type"),
                }
            },
            _ => panic!("Invalid Content Disposition")
        }

        assert_eq!(result_text, INPUT);
    }

    #[test]
    fn parse_form_data_wo_filename() {
        const INPUT: &'static str = "form-data; name=\"lolka\"";

        let result = ContentDisposition::from_str(INPUT).expect("To have form-data Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::FormData(name, file) => {
                assert_eq!(name.expect("To have form-data name"), "lolka");
                match file {
                    Filename::Name(name) => assert!(name.is_none()),
                    _ => panic!("Wrong Filename type"),
                }
            },
            _ => panic!("Invalid Content Disposition")
        }

        assert_eq!(result_text, INPUT);
    }

}
