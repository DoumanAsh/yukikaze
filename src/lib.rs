//!# 雪風(Yukikaze)
//!
//!Beautiful and elegant Yukikaze is little HTTP client library based on [hyper](https://crates.io/crates/hyper).
//!
//!## Available features
//!
//!- `flate2-c` - Enables decompression using `flate2` crate with C backend
//!- `flate2-rust` - Enables decompression using `flate2` crate with Rust backend
#![warn(missing_docs)]

pub extern crate serde;
pub extern crate serde_json;
pub extern crate serde_urlencoded;
pub extern crate cookie;
pub extern crate percent_encoding;
pub extern crate http;
pub extern crate etag;
pub extern crate tokio;
pub extern crate hyper;
pub extern crate futures;
pub extern crate data_encoding;
pub extern crate bytes;
#[cfg(feature = "flate2")]
pub extern crate flate2;
extern crate hyper_rustls;

#[macro_use]
mod utils;
pub mod header;
pub mod client;
