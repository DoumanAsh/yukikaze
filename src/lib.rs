//!# 雪風(Yukikaze)
//!
//!Beautiful and elegant Yukikaze is little HTTP library based on [hyper](https://crates.io/crates/hyper).
//!
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

pub extern crate bytes;
pub extern crate http;
pub extern crate percent_encoding;
pub extern crate async_timer;
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
