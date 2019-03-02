const URL: &'static str = "http://google.com";

#[test]
fn builder_empty_body() {
    let req = yukikaze::client::Request::get(URL).expect("To create request").empty();
    assert!(!req.headers().contains_key(yukikaze::http::header::CONTENT_LENGTH));

    let req = yukikaze::client::Request::post(URL).expect("To create request").empty();
    let len = req.headers().get(yukikaze::http::header::CONTENT_LENGTH).expect("To have len in empty POST");
    assert_eq!(len, "0");

    let req = yukikaze::client::Request::put(URL).expect("To create request").empty();
    let len = req.headers().get(yukikaze::http::header::CONTENT_LENGTH).expect("To have len in empty PUT");
    assert_eq!(len, "0");

    let req = yukikaze::client::Request::delete(URL).expect("To create request").empty();
    assert!(!req.headers().contains_key(yukikaze::http::header::CONTENT_LENGTH));
}

#[test]
fn builder_no_override_len() {
    let req = yukikaze::client::Request::post(URL).expect("To create request").content_len(25).empty();
    let len = req.headers().get(yukikaze::http::header::CONTENT_LENGTH).expect("To have len in empty POST");
    assert_eq!(len, "25");

    let req = yukikaze::client::Request::put(URL).expect("To create request").content_len(25).empty();
    let len = req.headers().get(yukikaze::http::header::CONTENT_LENGTH).expect("To have len in empty POST");
    assert_eq!(len, "25");

    let req = yukikaze::client::Request::post(URL).expect("To create request").content_len(25).body(Some("Lolka"));
    let len = req.headers().get(yukikaze::http::header::CONTENT_LENGTH).expect("To have len in empty POST");
    assert_eq!(len, "25");
}

#[test]
fn builder_empty_body_remove_len() {
    let req = yukikaze::client::Request::get(URL).expect("To create request").content_len(25).empty();
    assert!(!req.headers().contains_key(yukikaze::http::header::CONTENT_LENGTH));

    let req = yukikaze::client::Request::delete(URL).expect("To create request").content_len(25).empty();
    assert!(!req.headers().contains_key(yukikaze::http::header::CONTENT_LENGTH));
}

#[test]
fn builder_add_len() {
    let req = yukikaze::client::Request::post(URL).expect("To create request").body(Some("Lolka"));
    let len = req.headers().get(yukikaze::http::header::CONTENT_LENGTH).expect("To have len in empty POST");
    assert_eq!(len, "5");
}
