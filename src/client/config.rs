//!Describes client configuration

use std::io::Write;
use core::time;

use crate::utils;
use crate::header;

///Default timer, which is used by [DefaultCfg](struct.DefaultCfg.html)
pub type DefaultTimer = async_timer::oneshot::Timer;
#[cfg(feature = "rustls-on")]
///Default connector, which is used by [DefaultCfg](struct.DefaultCfg.html)
pub type DefaultConnector = crate::connector::rustls::HttpsConnector;
#[cfg(not(feature = "rustls-on"))]
///Default connector, which is used by [DefaultCfg](struct.DefaultCfg.html)
pub type DefaultConnector = crate::connector::HttpConnector;

///Generic config trait.
///
///Each method describes single aspect of configuration
///and provided with sane defaults
pub trait Config {
    ///Connector type.
    type Connector: hyper::service::Service<hyper::Uri> + Default + Clone + Send + Sync;
    ///Timer type.
    type Timer: async_timer::oneshot::Oneshot;

    #[inline]
    ///Specifies whether to automatically request compressed response.
    ///
    ///Defaults to true.
    fn decompress() -> bool {
        true
    }

    #[inline]
    ///Specifies request timeout.
    ///
    ///Default is 30 seconds
    ///
    ///Zero duration means infinite
    fn timeout() -> time::Duration {
        time::Duration::from_secs(30)
    }

    ///Specifies how to set user agent
    ///
    ///By default it set's `Yukikaze/<lib version>`, if agent is not present
    fn default_user_agent(request: &mut super::request::Request) {
        if !request.headers().contains_key(header::USER_AGENT) {
            request.headers_mut().insert(header::USER_AGENT, header::HeaderValue::from_static(concat!("Yukikaze/", env!("CARGO_PKG_VERSION"))));
        }
    }

    #[inline]
    ///Allows to sets default headers before request
    ///is sent out
    ///
    ///It is called as soon as request is being sent out,
    ///but before `Accept-Encoding` is set.
    ///
    ///By default it sets following, if not present:
    ///
    ///- Set default user agent;
    ///- `HOST` header with host, and optionally port, taken from URI;
    fn default_headers(request: &mut super::request::Request) {
        Self::default_user_agent(request);

        if !request.headers().contains_key(header::HOST) {
            let host = request.uri().host().and_then(|host| match request.uri().port().map(|port| port.as_u16()) {
                None | Some(80) | Some(443) => header::HeaderValue::from_str(host).ok(),
                Some(port) => {
                    let mut buffer = utils::BytesWriter::with_capacity(host.len() + 5);
                    let _ = write!(&mut buffer, "{}:{}", host, port);

                    http::header::HeaderValue::from_maybe_shared(buffer.freeze()).ok()
                },
            });

            if let Some(host) = host {
                request.headers_mut().insert(header::HOST, host);
            }
        }
    }

    #[inline]
    ///Returns max number of redirects
    ///
    ///By default it is 8.
    fn max_redirect_num() -> usize {
        8
    }

    #[inline]
    ///Allows to hook hyper's Client configuration.
    ///
    ///By default it uses hyper's defaults
    fn config_hyper(builder: &mut hyper::client::Builder) -> &mut hyper::client::Builder {
        builder
    }
}

///Default configuration.
///
///Uses default [Config](trait.Config.html) impl.
///
///Connector:
///
///- When `rustls` enabled uses `yukikaze::connector::rustls::HttpsConnector` as default
///
///- Otherwise uses `yukikaze::connector::HttpConnector`
pub struct DefaultCfg;

impl Config for DefaultCfg {
    type Connector = DefaultConnector;
    type Timer = DefaultTimer;
}
