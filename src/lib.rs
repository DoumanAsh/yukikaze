//!# 雪風(Yukikaze)
//!
//!Beautiful and elegant Yukikaze is little HTTP library based on [hyper](https://crates.io/crates/hyper).
//!
//!## Getting started:
//!
//!- [Client](client/index.html)
//!- [Runtime](rt/index.html)
//!
//!## Features
//!
//!- Uses rustls for TLS
//!- Support of various types of bodies: Plain text, JSON, multipart and forms
//!- Simple redirect policy with option to limit number of redirections.
//!- Support for text encodings aside from UTF-8.
//!- Various helpers to extract useful headers: Cookies, ETag/Last-Modified, Content related headers.
//!- File redirection support for response's body.
//!- Notify interface to indicate progress of body's download.
//!
//!## Available cargo features
//!
//!- `rustls` - Enables use of `rustls` for default SSL implementation. By default `on`.
//!- `compu` - Enables compression support. By default `on`.
//!- `encoding` - Enables `encoding` crate support. Default `off`.
//!- `websocket` - Enables Websocket Upgrade mechanism. Default `off`. Enables `carry_extensions` when `on`.
//!- `carry_extensions` - Carries `http::Extensions` from request to resolved `Response`. Default `off`.
//!
//!## Examples
//!
//!### Client
//!
//!```rust,no_run
//!use yukikaze::client::Request;
//!
//!mod global {
//!    yukikaze::declare_global_client!();
//!}
//!
//!use global::{GlobalRequest};
//!
//!async fn google() {
//!    let res = Request::get("https://google.com").expect("To create get request")
//!                                                .empty()
//!                                                .global() //Makes request to go to global client
//!                                                .send();
//!    let result = yukikaze::matsu!(res).expect("To get without timeout")
//!                                      .expect("Successful response");
//!    assert!(result.is_success());
//!}
//!```
//!

#![warn(missing_docs)]
#![doc(html_logo_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png", html_favicon_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png")]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]

#[macro_use]
pub mod utils;
pub mod header;
pub mod extractor;
pub mod upgrade;
pub mod connector;
pub mod client;
#[macro_use]
pub mod rt;

pub extern crate mime;
pub extern crate bytes;
pub extern crate http;
pub extern crate http_body;
pub extern crate percent_encoding;
pub extern crate async_timer;
#[cfg(feature = "encoding")]
pub extern crate encoding_rs;
#[cfg(feature = "compu")]
pub extern crate compu;
pub extern crate hyper;
pub extern crate etag;
pub extern crate cookie;
pub extern crate serde;
pub extern crate serde_json;
pub extern crate serde_urlencoded;
pub extern crate data_encoding;
pub extern crate httpdate;
