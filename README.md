# 雪風(Yukikaze)

[![Build](https://gitlab.com/Douman/yukikaze/badges/master/build.svg)](https://gitlab.com/Douman/yukikaze/pipelines)
[![Crates.io](https://img.shields.io/crates/v/yukikaze.svg)](https://crates.io/crates/yukikaze)
[![Documentation](https://docs.rs/yukikaze/badge.svg)](https://docs.rs/crate/yukikaze/)
[![dependency status](https://deps.rs/crate/yukikaze/0.5.0/status.svg)](https://deps.rs/crate/yukikaze)

![Yukikaze image](https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png)

Beautiful and elegant Yukikaze is little HTTP client library based on [hyper](https://crates.io/crates/hyper).

## Available features

- `flate2-c` - Enables decompression using `flate2` crate with C backend. Default on.
- `flate2-rust` - Enables decompression using `flate2` crate with Rust backend. Default off.
- `encoding` - Enables `encoding` crate support. Default off.
- `rt-tokio` - Enables tokio runtime module. Default off.
- `rt-client` - Enables Yukikaze client runtime module. Default off.
- `rt` - Enables all runtime modules. Default off.

## Features

- Uses rustls for TLS
- Support of various types of bodies: Plain text, JSON, multipart and forms
- Simple redirect policy with option to limit number of redirections.
- Support for encodings aside from UTF-8.
- Various helpers to extract useful headers: Cookies, ETag/Last-Modified, Content related headers.
- File redirection support for response's body.
- Notify interface to indicate progress of body's download.

## Usage

In order to use Yukikaze-sama you need to create [Client](client/struct.Client.html).

Configuration of of client can be defined using trait parameter [Config](client/config/trait.Config.html).
But default configuration in most cases reasonable for simple requests.

But if you need to work with client in generic context you can use its trait [HttpClient](client/trait.HttpClient.html).

### Making request

Request [builder](client/request/struct.Builder.html) provides rich set of methods
to configure it, but in simple terms making request boils down to following code:

```rust
extern crate yukikaze;
extern crate tokio;

use yukikaze::client::{Client, HttpClient, Request};

fn main() {
    let mut tokio_rt = tokio::runtime::current_thread::Runtime::new().expect("To create runtime");
    let client = Client::default();

    let request = Request::get("http://127.0.0.1").expect("To create get request").empty();

    let response = client.execute(request); //Creates future response
    let response = tokio_rt.block_on(response); //Waits for response

    println!("response={:?}", response);
}
```

You can use `rt` module to simplify your workflow though.

## Examples

- [fie](https://github.com/DoumanAsh/fie) - CLI shit posting tool for various social medias.

## Q&A

**Q:** Why not just use [reqwest](https://github.com/seanmonstar/reqwest)/[actix-web](https://github.com/actix/actix-web)/[mio_httpc](https://github.com/SergejJurecko/mio_httpc)?

**A:** Reqwest API sucks, actix-web client comes with lots of server code and mio_httpc is
unknown beast to me(I just found out about it when created Yukikaze).

- - -

**Q:** I see some moon runes and anime picture. Are you one of these disgusting weebs?

**A:** Obviously yes ;)

- - -

**Q:** Why so many inlines? Are you this stupid to use pre-mature optimizations!?

**A:** Yes, I'm stupid enough because compiler doesn't want inline methods across crates by default(unless you turn on LTO)

- - -

**Q:** Why builder methods panic? Why don't you store error in builder and return it when finishing creating request?

**A:** I believe in normal cases you are not supposed to create invalid requests so I consider such
errors as quite exceptional

- - -
