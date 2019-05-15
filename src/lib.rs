//!# 雪風(Yukikaze)
//!
//!Beautiful and elegant Yukikaze is little HTTP library based on [hyper](https://crates.io/crates/hyper).
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
//!- `client` - Enables client module. By default `on`.
//!- `rustls` - Enables use of `rustls` for default SSL implementation. By default `on`
//!- `flate2-c` - Enables decompression using `flate2` crate with C backend. Default `on`.
//!- `flate2-rust` - Enables decompression using `flate2` crate with Rust backend. Default `off`.
//!- `encoding` - Enables `encoding` crate support. Default `off`.
//!- `websocket` - Enables Websocket Upgrade mechanism. Default `off`. Enables `carry_extensions` when `on`.
//!- `carry_extensions` - Carries `http::Extensions` from request to resolved `Response`. Default `off`.
//!
//!## Getting started:
//!
//!- [Client](client/struct.Client.html)

#![warn(missing_docs)]
#![doc(html_logo_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png", html_favicon_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png")]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]
#![feature(async_await)]

#[macro_use]
pub mod utils;
pub mod header;
pub mod extractor;
pub mod upgrade;
#[cfg(feature = "client")]
pub mod client;
#[macro_use]
pub mod rt;

pub extern crate lazy_static;
pub extern crate bytes;
pub extern crate http;
pub extern crate percent_encoding;
pub extern crate async_timer;
#[cfg(feature = "encoding")]
pub extern crate encoding_rs;
#[cfg(feature = "flate2")]
pub extern crate flate2;
#[cfg(feature = "websocket")]
pub extern crate ring;
#[cfg(feature = "client")]
pub extern crate hyper;
#[cfg(feature = "client")]
pub extern crate etag;
#[cfg(feature = "client")]
pub extern crate cookie;
#[cfg(feature = "client")]
pub extern crate serde;
#[cfg(feature = "client")]
pub extern crate serde_json;
#[cfg(feature = "client")]
pub extern crate serde_urlencoded;
#[cfg(feature = "client")]
pub extern crate data_encoding;
#[cfg(feature = "client")]
pub extern crate httpdate;
