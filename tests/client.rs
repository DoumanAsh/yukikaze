extern crate yukikaze;
extern crate tokio;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use ::std::fs;
use ::std::io;
use ::std::io::Seek;
use ::std::time;

use yukikaze::http;
use yukikaze::client;
use yukikaze::client::HttpClient;

const BIN_URL: &'static str = "https://httpbin.org";
const BIN_GET: &'static str = "https://httpbin.org/get";
const BIN_DEFLATE: &'static str = "https://api.stackexchange.com/2.2/answers?site=stackoverflow&pagesize=10";
const BIN_GZIP: &'static str = "https://httpbin.org/gzip";
const BIN_JSON: &'static str = "http://httpbin.org/json";

fn get_tokio_rt() -> tokio::runtime::current_thread::Runtime {
    tokio::runtime::current_thread::Runtime::new().expect("Build tokio runtime")
}

pub struct TimeoutCfg;

impl client::config::Config for TimeoutCfg {
    fn timeout() -> time::Duration {
        time::Duration::from_millis(1)
    }
}

#[test]
fn make_timeout() {
    let request = client::request::Request::get(BIN_URL).expect("To create google get request").empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::<TimeoutCfg>::new();

    let result = rt.block_on(client.execute(request));
    println!("result={:?}", result);
    assert!(result.is_err());
}

#[test]
fn make_request_w_limited_body() {
    let request = client::request::Request::get(BIN_URL).expect("To create google get request").empty();

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
    let request = client::Request::get(BIN_URL).expect("To create google get request")
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
    let request = client::request::Request::get(BIN_GZIP).expect("To create google get request")
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
    let request = client::request::Request::get(BIN_DEFLATE).expect("To create google get request")
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
    let request = client::request::Request::get(BIN_GET).expect("To create google get request").query(&query).empty();

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
    let request = client::request::Request::get(BIN_GZIP).expect("To create google get request")
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