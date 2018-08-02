//!# 雪風(Yukikaze)
//!
//!Beautiful and elegant Yukikaze is little HTTP client library based on [hyper](https://crates.io/crates/hyper).
//!
//!## Available features
//!
//!- `flate2-c` - Enables decompression using `flate2` crate with C backend. Default on.
//!- `flate2-rust` - Enables decompression using `flate2` crate with Rust backend. Default on.
//!- `encoding` - Enables encoding crate support. Default off.
//!- `rt` - Enables runtime module. Default off.
#![warn(missing_docs)]
#![doc(html_logo_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png", html_favicon_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png")]

pub extern crate serde;
pub extern crate serde_json;
pub extern crate serde_urlencoded;
pub extern crate cookie;
pub extern crate percent_encoding;
pub extern crate http;
pub extern crate mime;
pub extern crate etag;
pub extern crate tokio;
pub extern crate hyper;
pub extern crate futures;
pub extern crate data_encoding;
pub extern crate bytes;
#[cfg(feature = "flate2")]
pub extern crate flate2;
#[cfg(feature = "encoding")]
pub extern crate encoding;
extern crate hyper_rustls;

#[macro_use]
mod utils;
pub mod header;
pub mod client;
#[cfg(feature = "rt")]
pub mod rt;
