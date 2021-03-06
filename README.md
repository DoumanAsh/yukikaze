# 雪風(Yukikaze)

[![Build](https://gitlab.com/Douman/yukikaze/badges/master/pipeline.svg)](https://gitlab.com/Douman/yukikaze/pipelines)
[![Crates.io](https://img.shields.io/crates/v/yukikaze.svg)](https://crates.io/crates/yukikaze)
[![Documentation](https://docs.rs/yukikaze/badge.svg)](https://docs.rs/crate/yukikaze/)
[![dependency status](https://deps.rs/crate/yukikaze/1.0.8/status.svg)](https://deps.rs/crate/yukikaze)

![Yukikaze image](https://gitlab.com/Douman/yukikaze/raw/master/Yukikaze.png)

Beautiful and elegant Yukikaze is little HTTP client library based on [hyper](https://crates.io/crates/hyper).

## Features

- Uses rustls for TLS
- Support of various types of bodies: Plain text, JSON, multipart and forms
- Simple redirect policy with option to limit number of redirections.
- Support for text encodings aside from UTF-8.
- Various helpers to extract useful headers: Cookies, ETag/Last-Modified, Content related headers.
- File redirection support for response's body.
- Notify interface to indicate progress of body's download.

## Available cargo features

- `rustls` - Enables use of `rustls` for default SSL implementation. By default `on`
- `compu` - Enables compression support. By default `on`.
- `encoding` - Enables `encoding` crate support. Default `off`.
- `websocket` - Enables Websocket Upgrade mechanism. Default `off`. Enables `carry_extensions` when `on`.
- `carry_extensions` - Carries `http::Extensions` from request to resolved `Response`. Default `off`.

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
