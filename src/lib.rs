//!# 雪風(Yukikaze)
//!
//!Beautiful and elegant Yukikaze is little HTTP library based on [hyper](https://crates.io/crates/hyper).
//!
#![warn(missing_docs)]
#![doc(html_logo_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png", html_favicon_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png")]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::style))]
#![feature(async_await)]

#[macro_use]
mod utils;
pub mod header;
pub mod extractor;
#[cfg(feature = "client")]
pub mod client;

pub extern crate bytes;
pub extern crate http;
pub extern crate percent_encoding;
pub extern crate async_timer;
