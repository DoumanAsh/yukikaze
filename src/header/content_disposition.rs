use percent_encoding::{percent_encode, PATH_SEGMENT_ENCODE_SET, percent_decode, EncodeSet};
use pest::Parser;

use std::fmt;

#[cfg(debug_assertions)]
const _GRAMMAR: &'static str = include_str!("content_disposition.pest");

#[derive(Parser)]
#[grammar = "header/content_disposition.pest"]
struct CdParser;

#[derive(Debug)]
///Filename parameter of `Content-Disposition`
pub enum Filename {
    ///Regular `filename`
    Name(Option<String>),
    ///Extended `filename*`
    ///
    ///Values:
    ///1. Charset.
    ///2. Optional language tag.
    ///3. Raw bytes of name.
    Extended(String, Option<String>, Vec<u8>)
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
    pub fn with_encoded_name(name: String) -> Self {
        let is_non_ascii = name.as_bytes().iter().any(|byte| PATH_SEGMENT_ENCODE_SET.contains(*byte));

        match is_non_ascii {
            false => Self::with_name(name),
            true => {
                let bytes = name.into_bytes();
                Filename::Extended("utf-8".to_owned(), None, bytes)
            }
        }
    }

    ///Creates extended file name.
    pub fn with_extended(charset: String, lang: Option<String>, name: Vec<u8>) -> Self {
        Filename::Extended(charset, lang, name)
    }

    #[inline]
    ///Returns whether filename is of extended type.
    pub fn is_extended(&self) -> bool {
        match self {
            Filename::Extended(_, _, _) => true,
            _ => false
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

macro_rules! parse_file_ext {
    ($param:ident) => {{
        let mut parts = $param.as_str().splitn(3, '\'');

        let charset = match parts.next() {
            Some(charset) => charset.to_owned(),
            None => continue
        };
        let lang = parts.next().map(|lang| lang.to_owned());
        let value: Vec<u8> = match parts.next() {
            Some(value) => percent_decode(value.as_bytes()).collect(),
            None => continue
        };

        Filename::Extended(charset, lang, value)
    }}
}

impl ContentDisposition {
    ///Parses string into self, if possible.
    pub fn from_str(text: &str) -> Option<Self> {
        let result = CdParser::parse(Rule::disposition, text).ok().and_then(|mut result| result.next());

        let result = match result {
            Some(result) => result.into_inner().next().expect("To have inner pairs"),
            None => return None,
        };

        let res = match result.as_rule() {
            Rule::inline => ContentDisposition::Inline,
            Rule::attachment => {
                let mut file_name = Filename::Name(None);

                let result = result.into_inner()
                                   .map(|param| param.into_inner().next().unwrap()) //Skip param
                                   .map(|param| (param.as_rule(), param.into_inner().next().expect("value"))); //Take key's rule and value's match

                for (rule, param) in result {
                    match rule {
                        Rule::filename => {
                            match param.as_rule() {
                                Rule::filename_value => {
                                    file_name = Filename::Name(Some(param.as_str().trim().to_owned()));
                                },
                                Rule::filename_value_ext => {
                                    file_name = parse_file_ext!(param);
                                    //extended should have priority so break here.
                                    break;
                                },
                                _ => unreachable!()
                            }
                        }
                        _ => unreachable!()
                    }
                }

                ContentDisposition::Attachment(file_name)
            },
            Rule::form_data => {
                let mut name = None;
                let mut file_name = Filename::Name(None);

                let result = result.into_inner()
                                   .map(|param| param.into_inner().next().unwrap()) //Skip param
                                   .map(|param| (param.as_rule(), param.into_inner().next().expect("value"))); //Take key's rule and value's match

                for (rule, param) in result {
                    match rule {
                        Rule::filename => {
                            match param.as_rule() {
                                Rule::filename_value => {
                                    //Extended should have priority
                                    if !file_name.is_extended() {
                                        file_name = Filename::Name(Some(param.as_str().trim().to_owned()));
                                    }
                                },
                                Rule::filename_value_ext => {
                                    file_name = parse_file_ext!(param);
                                },
                                _ => unreachable!()
                            }
                        },
                        Rule::form_name => {
                            match param.as_rule() {
                                Rule::form_name_value => {
                                    name = Some(param.as_str().trim().to_owned());
                                },
                                _ => unreachable!()
                            }
                        }
                        _ => unreachable!()
                    }
                }

                ContentDisposition::FormData(name, file_name)
            }
            _ => return None
        };

        Some(res)
    }
}

impl fmt::Display for ContentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ContentDisposition::Inline => write!(f, "inline"),
            ContentDisposition::Attachment(file) => match file {
                Filename::Name(Some(name)) => write!(f, "attachment; filename=\"{}\"", name),
                Filename::Name(None) => write!(f, "attachment"),
                Filename::Extended(charset, lang, value) => {
                    write!(f, "attachment; filename*={}'{}'{}",
                           charset,
                           lang.as_ref().map(|lang| lang.as_str()).unwrap_or(""),
                           percent_encode(&value, PATH_SEGMENT_ENCODE_SET).to_string())
                },
            },
            ContentDisposition::FormData(None, file) => match file {
                Filename::Name(Some(name)) => write!(f, "form-data; filename=\"{}\"", name),
                Filename::Name(None) => write!(f, "form-data"),
                Filename::Extended(charset, lang, value) => {
                    write!(f, "form-data; filename*={}'{}'{}",
                           charset,
                           lang.as_ref().map(|lang| lang.as_str()).unwrap_or(""),
                           percent_encode(&value, PATH_SEGMENT_ENCODE_SET).to_string())
                },
            },
            ContentDisposition::FormData(Some(name), file) => match file {
                Filename::Name(Some(file_name)) => write!(f, "form-data; name=\"{}\"; filename=\"{}\"", name, file_name),
                Filename::Name(None) => write!(f, "form-data; name=\"{}\"", name),
                Filename::Extended(charset, lang, value) => {
                    write!(f, "form-data; name=\"{}\"; filename*={}'{}'{}",
                           name,
                           charset,
                           lang.as_ref().map(|lang| lang.as_str()).unwrap_or(""),
                           percent_encode(&value, PATH_SEGMENT_ENCODE_SET).to_string())
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ::percent_encoding::{percent_decode};
    use super::{ContentDisposition, Filename};

    #[test]
    fn parse_file_name_extended_ascii() {
        const INPUT: &'static str = "rori.mp4";
        let file_name = Filename::with_encoded_name(INPUT.to_string());
        assert!(!file_name.is_extended());
    }

    #[test]
    fn parse_file_name_extended_non_ascii() {
        const INPUT: &'static str = "ロリへんたい.mp4";
        let file_name = Filename::with_encoded_name(INPUT.to_string());
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
        const EXPECT_INPUT: &'static str = "attachment; filename*=UTF-8'en'%C2%A3%20and%20%E2%82%AC%20rates";
        const INPUT: &'static str = "attachment;\t filename*=UTF-8'en'%C2%A3%20and%20%E2%82%AC%20rates";

        let result = ContentDisposition::from_str(INPUT).expect("To have attachment Disposition");

        let result_text = result.to_string();

        match result {
            ContentDisposition::Attachment(file) => {
                match file {
                    Filename::Extended(charset, lang, value) => {
                        assert_eq!(charset, "UTF-8");
                        assert_eq!(lang.expect("Lang value"), "en");
                        let expected_value = percent_decode("%C2%A3%20and%20%E2%82%AC%20rates".as_bytes()).collect::<Vec<u8>>();
                        assert_eq!(value, expected_value);
                    },
                    _ => panic!("Wrong Filename type"),
                }
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
