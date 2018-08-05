# 雪風(Yukikaze)

[![Build](https://gitlab.com/Douman/yukikaze/badges/master/build.svg)](https://gitlab.com/Douman/yukikaze/pipelines)
[![Crates.io](https://img.shields.io/crates/v/yukikaze.svg)](https://crates.io/crates/yukikaze)
[![Documentation](https://docs.rs/yukikaze/badge.svg)](https://docs.rs/crate/yukikaze/)

![Yukikaze image](https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png)

[Documentation](https://doumanash.github.io/yukikaze-docs/to_deploy/index.html)

Beautiful and elegant Yukikaze is little HTTP client library based on [hyper](https://crates.io/crates/hyper)

## Usage

In order to use Yukikaze-sama you need to create Client.

Configuration of of client can be defined using trait parameter Config.
But default configuration in most cases reasonable for simple requests.

But if you need to work with client in generic context you can use its trait HttpClient.

### Making request

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

## Q&A

**Q:** Why not just use [reqwest](https://github.com/seanmonstar/reqwest)/[actix-web](https://github.com/actix/actix-web)/[mio_httpc](https://github.com/SergejJurecko/mio_httpc)?

**A:** Reqwest doesn't use rustls, actix-web client comes with lots of server code and mio_httpc is
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
