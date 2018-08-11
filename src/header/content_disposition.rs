use ::percent_encoding::{percent_encode, USERINFO_ENCODE_SET, percent_decode};
use ::pest::Parser;

use ::std::fmt;

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
    Attachment(Filename)
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
                                    let mut parts = param.as_str().splitn(3, '\'');

                                    let charset = match parts.next() {
                                        Some(charset) => charset.to_owned(),
                                        None => continue
                                    };
                                    let lang = parts.next().map(|lang| lang.to_owned());
                                    let value: Vec<u8> = match parts.next() {
                                        Some(value) => percent_decode(value.as_bytes()).collect(),
                                        None => continue
                                    };

                                    file_name = Filename::Extended(charset, lang, value);
                                    //exte should have priority so break here.
                                    break;
                                },
                                _ => ()
                            }
                        }
                        _ => ()
                    }
                }

                ContentDisposition::Attachment(file_name)
            },
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
                           percent_encode(&value, USERINFO_ENCODE_SET).to_string())
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use ::percent_encoding::{percent_decode};
    use super::{ContentDisposition, Filename};

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

        let result = ContentDisposition::from_str(INPUT).expect("To have inline Disposition");

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
        const INPUT: &'static str = "attachment; filename=\"lolka.jpg\"; filename=\"lolka2.jpg\"";

        let result = ContentDisposition::from_str(INPUT).expect("To have inline Disposition");

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
        const INPUT: &'static str = "attachment; filename*=UTF-8'en'%C2%A3%20and%20%E2%82%AC%20rates";

        let result = ContentDisposition::from_str(INPUT).expect("To have inline Disposition");

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

        assert_eq!(result_text, INPUT);
    }

}
