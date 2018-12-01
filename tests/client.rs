extern crate yukikaze;
extern crate tokio;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate etag;
extern crate percent_encoding;
extern crate cookie;

use ::std::fs;
use ::std::io;
use ::std::io::Seek;
use ::std::time;

use yukikaze::http;
use yukikaze::client;
use yukikaze::client::HttpClient;

const BIN_URL: &'static str = "https://httpbin.org";
const BIN_GET: &'static str = "https://httpbin.org/get";
const BIN_DEFLATE: &'static str = "https://httpbin.org/deflate";
const BIN_GZIP: &'static str = "https://httpbin.org/gzip";
const BIN_JSON: &'static str = "http://httpbin.org/json";
const BIN_BASIC_AUTH: &'static str = "http://httpbin.org/basic-auth";
const BIN_ETAG: &'static str = "http://httpbin.org/etag";
const BIN_COOKIE: &'static str = "http://httpbin.org/cookies";

fn get_tokio_rt() -> tokio::runtime::current_thread::Runtime {
    tokio::runtime::current_thread::Runtime::new().expect("Build tokio runtime")
}

pub struct TimeoutCfg;

impl client::config::Config for TimeoutCfg {
    fn timeout() -> time::Duration {
        time::Duration::from_millis(50)
    }
}

pub struct SmolRedirect;

impl client::config::Config for SmolRedirect {
    fn max_redirect_num() -> usize {
        3
    }
}

#[test]
fn make_timeout() {
    let request = client::request::Request::get(BIN_URL).expect("To create get request").empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::<TimeoutCfg>::new();

    let result = rt.block_on(client.execute(request));
    println!("result={:?}", result);
    assert!(result.is_err());

    let timeout = match result.unwrap_err() {
        client::response::errors::ResponseError::Timeout(timeout) => timeout,
        _ => panic!("Unexpected error")
    };

    let result = rt.block_on(timeout.retry(time::Duration::from_secs(30)));
    println!("result={:?}", result);
    let result = result.expect("To have successful retry");
    assert!(result.is_success());
}

#[test]
fn make_request_w_limited_body() {
    let request = client::request::Request::get(BIN_URL).expect("To create get request").empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    let result = result.expect("To get");

    let body = result.text().limit(4_000);
    let result = rt.block_on(body);
    println!("result={:?}", result);
    assert!(result.is_err());
}

#[test]
fn make_request() {
    let request = client::Request::get(BIN_URL).expect("To create get request")
                                               .bearer_auth("lolka")
                                               .basic_auth("Lolka", Some("Pass"))
                                               .empty();

    {
        assert_eq!(request.method(), http::method::Method::GET);
        assert_eq!(request.uri(), BIN_URL);
        assert_eq!(request.headers().len(), 2);
        let auth = request.headers().get(http::header::AUTHORIZATION).expect("To have AUTHORIZATION header");
        let auth = auth.to_str().expect("Convert AUTHORIZATION to str");
        assert!(auth.starts_with("basic "));
        assert_eq!(auth, "basic TG9sa2E6UGFzcw==");
    }

    let mut rt = get_tokio_rt();
    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    let result = result.expect("To get");
    println!("result={:?}", result);

    assert_eq!(result.status().as_u16(), 200);
    assert!(!result.is_error());
    assert!(result.is_success());

    let body = result.text();
    let _result = rt.block_on(body).expect("Read body");
}

#[derive(Deserialize, Debug)]
struct BinGzippedRsp {
    gzipped: bool,
    method: String,
}

#[test]
fn make_request_w_gzip_body() {
    let request = client::request::Request::get(BIN_GZIP).expect("To create get request")
                                                         .accept_encoding(yukikaze::header::ContentEncoding::Gzip)
                                                         .empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    let result = result.expect("To get");
    println!("result={:?}", result);
    assert!(result.is_success());

    let body = result.json::<BinGzippedRsp>();
    let result = rt.block_on(body);
    println!("result={:?}", result);
    let result = result.expect("Get JSON");
    assert!(result.gzipped);
    assert_eq!(result.method, "GET");
}

#[test]
fn make_request_w_deflate_body() {
    let request = client::request::Request::get(BIN_DEFLATE).expect("To create get request")
                                                            .accept_encoding(yukikaze::header::ContentEncoding::Deflate)
                                                            .empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    let result = result.expect("To get");
    println!("result={:?}", result);
    assert!(result.is_success());

    let body = result.text();
    let result = rt.block_on(body);
    println!("result={:?}", result);
    assert!(!result.is_err());
}

#[derive(Deserialize, Serialize, Debug)]
struct Query {
    name: String,
    password: String
}

#[derive(Deserialize, Debug)]
struct GetResponse {
    args: Query,
    url: String
}

#[test]
fn make_get_query() {
    let query = Query {
        name: "Test".to_owned(),
        password: "TestPassword".to_owned()
    };
    let request = client::request::Request::get(BIN_GET).expect("To create get request").query(&query).empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    println!("result={:?}", result);
    let result = result.expect("Success");

    let body = result.json::<GetResponse>();
    let result = rt.block_on(body);
    println!("result={:?}", result);
    let result = result.expect("Get JSON");
    assert_eq!(result.args.name, query.name);
    assert_eq!(result.args.password, query.password);
    assert_eq!(result.url, format!("{}?name={}&password={}", BIN_GET, query.name, query.password));

}

#[test]
fn make_request_w_gzip_body_stored_as_file() {
    let request = client::request::Request::get(BIN_GZIP).expect("To create get request")
                                                         .accept_encoding(yukikaze::header::ContentEncoding::Gzip)
                                                         .empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    let result = result.expect("To get");
    assert!(result.is_success());

    let file = fs::OpenOptions::new().truncate(true).read(true).write(true).create(true).open("gzip.json").expect("To create file");
    let body = result.file(file);
    let result = rt.block_on(body);
    let mut file = result.expect("Get File");
    file.seek(io::SeekFrom::Start(0)).expect("Move to the beggining of file");
    let result: BinGzippedRsp = serde_json::from_reader(file).expect("To get gzip.json");
    assert!(result.gzipped);
    assert_eq!(result.method, "GET");

    let _ = fs::remove_file("gzip.json");
}

#[test]
fn make_request_w_gzip_body_stored_as_file_with_notify() {
    use std::sync::mpsc::channel;

    let request = client::request::Request::get(BIN_GZIP).expect("To create get request")
                                                         .accept_encoding(yukikaze::header::ContentEncoding::Gzip)
                                                         .empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    let result = result.expect("To get");
    assert!(result.is_success());

    let (sender, receiver) = channel();

    let file = fs::OpenOptions::new().truncate(true).read(true).write(true).create(true).open("gzip2.json").expect("To create file");
    let body = result.file(file).with_notify(sender);
    let result = rt.block_on(body);

    for bytes in receiver {
        assert!(bytes != 0);
    }

    let mut file = result.expect("Get File");
    file.seek(io::SeekFrom::Start(0)).expect("Move to the beggining of file");
    let result: BinGzippedRsp = serde_json::from_reader(file).expect("To get gzip.json");
    assert!(result.gzipped);
    assert_eq!(result.method, "GET");

    let _ = fs::remove_file("gzip2.json");
}


#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct HttpBinJson {
  slideshow: Slideshow,
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct Slides {
  title: String,
  #[serde(rename = "type")]
  _type: String,
  items: Option<Vec<String>>,
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct Slideshow {
  author: String,
  date: String,
  slides: Vec<Slides>,
  title: String,
}

#[test]
fn get_json_response() {
    let mut rt = get_tokio_rt();
    let client = client::Client::default();

    let request = client::request::Request::get(BIN_JSON).expect("Error with request!").empty();
    let response = rt.block_on(client.execute(request)).expect("Error with response!");

    let json = response.json::<HttpBinJson>();
    let result = rt.block_on(json);
    let res = result.expect("Error with json!");

    let real_json = HttpBinJson {
        slideshow: Slideshow {
            author: String::from("Yours Truly"),
            date: String::from("date of publication"),
            slides: vec![Slides {title: String::from("Wake up to WonderWidgets!"),
                                 _type: String::from("all"),
                                 items: None},
                         Slides {title: String::from("Overview"),
                                 _type: String::from("all"),
                                 items:
                                   Some(vec![String::from("Why <em>WonderWidgets</em> are great"),
                                             String::from("Who <em>buys</em> WonderWidgets")])}],
            title: String::from("Sample Slide Show"),
        },
    };

    assert_eq!(res, real_json);
}

#[derive(Debug, Deserialize)]
pub struct BasicAuthRsp {
    authenticated: bool,
    user: String
}

#[test]
fn pass_basic_auth() {
    const LOGIN: &'static str = "loli";
    const PASSWORD: &'static str = "password";

    let mut rt = get_tokio_rt();
    let client = client::Client::default();

    let url = format!("{}/{}/{}", BIN_BASIC_AUTH, LOGIN, PASSWORD);
    let request = client::request::Request::get(url).expect("Error with request!").basic_auth(LOGIN, Some(PASSWORD)).empty();
    let response = rt.block_on(client.execute(request)).expect("Error with response!");

    let json = response.json::<BasicAuthRsp>();
    let result = rt.block_on(json);
    let res = result.expect("Error with json!");

    assert_eq!(res.user, LOGIN);
    assert!(res.authenticated);
}

#[test]
fn test_etag() {
    use percent_encoding::{utf8_percent_encode, USERINFO_ENCODE_SET};

    let mut rt = get_tokio_rt();
    let client = client::Client::default();
    const ETAG: &'static str = "12345";

    let url = format!("{}/{}", BIN_ETAG, utf8_percent_encode(&format!("\"{}\"", ETAG), USERINFO_ENCODE_SET));
    let request = client::request::Request::get(&url).expect("Error with request!").empty();
    let response = rt.block_on(client.execute(request)).expect("Error with response!");

    assert!(response.is_success());
    let rsp_etag = response.etag().expect("To have etag");
    assert_eq!(rsp_etag.tag(), ETAG);

    let if_none_match = etag::EntityTag::strong(ETAG.to_string());
    let request = client::request::Request::get(&url).expect("Error with request!")
                                                     .set_etag(&if_none_match, yukikaze::client::request::tags::IfNoneMatch)
                                                     .empty();
    let response = rt.block_on(client.execute(request)).expect("Error with response!");
    let rsp_etag = response.etag().expect("To have etag");
    assert_eq!(rsp_etag, if_none_match);

    //TODO: it seems httpbin doesn't return 304 for some reason here
}

#[test]
fn test_cookie() {
    #[derive(Deserialize, Debug)]
    struct CookiesJson {
        cookies: Cookies
    }

    #[derive(Deserialize, Debug)]
    struct Cookies {
        #[serde(rename = "WAIFU")]
        waifu: String,
        #[serde(rename = "First")]
        first: String
    }

    let mut rt = get_tokio_rt();
    let client = client::Client::default();

    let request = client::request::Request::get(BIN_COOKIE).expect("Error with request!").empty();
    let response = rt.block_on(client.execute(request)).expect("Error with response!");
    assert!(response.is_success());
    assert!(response.cookies_iter().next().is_none());

    const KEY: &'static str = "WAIFU";
    const VALUE: &'static str = "YUKIKAZE";
    let url = format!("{}/set/{}/{}", BIN_COOKIE, KEY, VALUE);

    let request = client::request::Request::get(&url).expect("Error with request!").empty();
    let response = rt.block_on(client.execute(request)).expect("Error with response!");
    assert!(response.is_redirect());
    let cookie = response.cookies_iter().next().expect("To have cookie").expect("To parse cookie");
    assert_eq!(cookie.name(), KEY);
    assert_eq!(cookie.value(), VALUE);

    let extra_cookie = cookie::Cookie::build("First", "Mehisha").path("/").http_only(true).finish();
    let request = client::request::Request::get(BIN_COOKIE).expect("Error with request!").add_cookie(cookie.into_owned()).add_cookie(extra_cookie).empty();
    println!("request={:?}", request);
    let response = rt.block_on(client.execute(request)).expect("Error with response!");
    println!("response={:?}", response);
    assert!(response.is_success());

    let json = response.json::<CookiesJson>();
    let result = rt.block_on(json);
    let res = result.expect("Error with json!");
    assert_eq!(res.cookies.waifu, VALUE);
    assert_eq!(res.cookies.first, "Mehisha");
}

#[cfg(feature = "encoding")]
#[test]
fn decode_non_utf8() {
    use yukikaze::header;

    const URL: &'static str = "http://seiya-saiga.com/";

    let mut rt = get_tokio_rt();
    let client = client::Client::default();

    let request = client::request::Request::get(URL).expect("Error with request!").empty();
    let mut response = rt.block_on(client.execute(request)).expect("Error with response!");

    //Pretend that it acctually sets Content-Type correctly
    response.headers_mut().insert(header::CONTENT_TYPE, header::HeaderValue::from_static("text/html; charset=shift_jis"));
    println!("response={:?}", response);
    let _last_modified = response.last_modified().expect("To get last_modified");
    let text = response.text();
    let result = rt.block_on(text);
    let res = result.expect("Error with decoding text!");
    assert!(res.len() > 0);
}

#[cfg(feature = "rt")]
#[test]
fn test_global_client() {
    const BIN_ABS_REDIRECT_2: &'static str = "http://httpbin.org/absolute-redirect/2";
    const BIN_REL_REDIRECT_2: &'static str = "http://httpbin.org/redirect/2";
    const BIN_ABS_REDIRECT_3: &'static str = "http://httpbin.org/absolute-redirect/3";

    use yukikaze::rt::{AutoRuntime, AutoClient, init};

    let _guard = init();

    {
        let _global = yukikaze::rt::GlobalClient::with_config::<SmolRedirect>();

        let request = client::request::Request::get(BIN_ABS_REDIRECT_2).expect("To create get request").empty();
        let result = request.send_with_redirect().finish();
        println!("result={:?}", result);
        let result = result.expect("Success get with redirect");
        assert!(result.is_success());

        let request = client::request::Request::get(BIN_REL_REDIRECT_2).expect("To create get request").empty();
        let result = request.send_with_redirect().finish();
        println!("result={:?}", result);
        let result = result.expect("Success get with redirect");
        assert!(result.is_success());

        let request = client::request::Request::get(BIN_ABS_REDIRECT_3).expect("To create get request").empty();
        let result = request.send_with_redirect().finish();
        println!("result={:?}", result);
        let result = result.expect("Success get with redirect");
        assert!(result.is_redirect());
    }

    {
        //Try to init it second time
        let _global = yukikaze::rt::GlobalClient::with_config::<SmolRedirect>();
    }
}

#[cfg(feature = "rt")]
#[should_panic]
#[test]
fn test_global_client_not_set() {
    use yukikaze::rt::{AutoRuntime, AutoClient};
    let request = client::request::Request::get(BIN_URL).expect("To create get request").empty();

    let result = request.send().finish();
    println!("result={:?}", result);
}
