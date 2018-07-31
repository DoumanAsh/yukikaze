extern crate yukikaze;
extern crate tokio;

use ::std::time;

use yukikaze::http;
use yukikaze::client;
use yukikaze::client::HttpClient;

const BIN_URL: &'static str = "https://httpbin.org";
const BIN_DEFLATE: &'static str = "https://httpbin.org/deflate";
const BIN_GZIP: &'static str = "https://httpbin.org/gzip";

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

#[test]
fn make_request_w_gzip_body() {
    let request = client::request::Request::get(BIN_GZIP).expect("To create google get request")
                                                         .accept_encoding(yukikaze::header::ContentEncoding::Gzip)
                                                         .empty();

    let mut rt = get_tokio_rt();

    let client = client::Client::default();

    let result = rt.block_on(client.execute(request));
    let result = result.expect("To get");

    println!("Content-Encoding={:?}", result.content_encoding());
    println!("result={:?}", result);
    let body = result.body();
    let result = rt.block_on(body);
    println!("result={:?}", result);
    //TODO: flush returns WriteZero error
    assert!(!result.is_err());
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

    //TODO: Cannot deflate? It erroers on first chunk.
    println!("Content-Encoding={:?}", result.content_encoding());
    println!("result={:?}", result);
    let body = result.body();
    let result = rt.block_on(body);
    println!("result={:?}", result);
    assert!(!result.is_err());
}
