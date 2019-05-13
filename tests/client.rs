use yukikaze::client;

use core::time;
use core::pin::Pin;

const BIN_URL: &'static str = "https://httpbin.org";
const BIN_GET: &'static str = "https://httpbin.org/get";

pub struct TimeoutCfg;

impl client::config::Config for TimeoutCfg {
    type Connector = client::config::DefaultConnector;
    type Timer = client::config::DefaultTimer;

    fn new_connector() -> Self::Connector {
        Self::Connector::new(4)
    }

    fn timeout() -> time::Duration {
        time::Duration::from_millis(50)
    }
}

fn get_tokio_rt() -> tokio::runtime::current_thread::Runtime {
    tokio::runtime::current_thread::Runtime::new().expect("Build tokio runtime")
}

#[test]
fn should_time_out() {
    let mut rt = get_tokio_rt();

    let client = client::Client::<TimeoutCfg>::new();

    let request = client::request::Request::get(BIN_GET).expect("To create get request").empty();
    let mut request = client.send(request);
    let request = unsafe { Pin::new_unchecked(&mut request) };
    let request = futures_util::compat::Compat::new(request);
    let result = rt.block_on(request);
    assert!(result.is_err());
}

#[test]
fn should_handle_redirect() {
    pub struct SmolRedirect;

    impl client::config::Config for SmolRedirect {
        type Connector = client::config::DefaultConnector;
        type Timer = client::config::DefaultTimer;

        fn new_connector() -> Self::Connector {
            Self::Connector::new(4)
        }

        fn max_redirect_num() -> usize {
            3
        }
    }

    const BIN_ABS_REDIRECT_2: &'static str = "http://httpbin.org/absolute-redirect/2";
    const BIN_REL_REDIRECT_2: &'static str = "http://httpbin.org/redirect/2";
    const BIN_ABS_REDIRECT_3: &'static str = "http://httpbin.org/absolute-redirect/3";

    let mut rt = get_tokio_rt();
    let client = client::Client::<SmolRedirect>::new();

    let request = client::Request::get(BIN_ABS_REDIRECT_2).expect("To create get request").empty();
    let mut request = client.redirect_request(request);
    let request = unsafe { Pin::new_unchecked(&mut request) };
    let request = futures_util::compat::Compat::new(request);
    let result = rt.block_on(request);
    let result = result.expect("To get successful response");
    assert!(result.is_success());

    let request = client::Request::get(BIN_REL_REDIRECT_2).expect("To create get request").empty();
    let mut request = client.redirect_request(request);
    let request = unsafe { Pin::new_unchecked(&mut request) };
    let request = futures_util::compat::Compat::new(request);
    let result = rt.block_on(request);
    let result = result.expect("To get successful response");
    assert!(result.is_success());

    let request = client::Request::get(BIN_ABS_REDIRECT_3).expect("To create get request").empty();
    let mut request = client.redirect_request(request);
    let request = unsafe { Pin::new_unchecked(&mut request) };
    let request = futures_util::compat::Compat::new(request);
    let result = rt.block_on(request);
    let result = result.expect("To get successful response");
    assert!(result.is_redirect());
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
        assert_eq!(request.headers().len(), 1);
        let auth = request.headers().get(http::header::AUTHORIZATION).expect("To have AUTHORIZATION header");
        let auth = auth.to_str().expect("Convert AUTHORIZATION to str");
        assert!(auth.starts_with("Basic "));
        assert_eq!(auth, "Basic TG9sa2E6UGFzcw==");
    }

    let mut rt = get_tokio_rt();
    let client = client::Client::default();

    let mut request = client.send(request);
    let request = unsafe { Pin::new_unchecked(&mut request) };
    let request = futures_util::compat::Compat::new(request);
    let result = rt.block_on(request);
    let result = result.expect("To get without timeout");
    println!("result={:?}", result);
    let mut result = result.expect("To get without error");

    assert_eq!(result.status().as_u16(), 200);
    assert!(!result.is_error());
    assert!(result.is_success());

    let mut body = result.text();
    let body = unsafe { Pin::new_unchecked(&mut body) };
    let body = futures_util::compat::Compat::new(body);
    let _result = rt.block_on(body).expect("Read body");
}
