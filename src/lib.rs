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
//!
//!## Usage
//!
//!In order to use Yukikaze-sama you need to create [Client](client/struct.Client.html).
//!
//!Configuration of of client can be defined using trait parameter [Config](client/config/trait.Config.html).
//!But default configuration in most cases reasonable for simple requests.
//!
//!But if you need to work with client in generic context you can use its trait [HttpClient](client/trait.HttpClient.html).
//!
//!### Making request
//!
//!Request [builder](client/request/struct.Builder.html) provides rich set of methods
//!to configure it, but in simple terms making request boils down to following code:
//!
//!```rust
//!extern crate yukikaze;
//!extern crate tokio;
//!
//!use yukikaze::client::{Client, HttpClient, Request};
//!
//!fn main() {
//!    let mut tokio_rt = tokio::runtime::current_thread::Runtime::new().expect("To create runtime");
//!    let client = Client::default();
//!
//!    let request = Request::get("http://127.0.0.1").expect("To create get request").empty();
//!
//!    let response = client.execute(request); //Creates future response
//!    let response = tokio_rt.block_on(response); //Waits for response
//!
//!    println!("response={:?}", response);
//!}
//!```
//!
//!You can use `rt` module to simplify your workflow though.
//!
#![warn(missing_docs)]
#![doc(html_logo_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png", html_favicon_url = "https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png")]

pub extern crate serde;
pub extern crate serde_json;
pub extern crate serde_urlencoded;
pub extern crate cookie;
pub extern crate percent_encoding;
pub extern crate http;
pub extern crate httpdate;
pub extern crate mime;
extern crate mime_guess;
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

extern crate pest;
#[macro_use]
extern crate pest_derive;

#[macro_use]
mod utils;
pub mod header;
pub mod client;
#[cfg(feature = "rt")]
pub mod rt;
