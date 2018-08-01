//!Describes client configuration

use ::std::time;

use ::header;
use ::hyper;

///Generic config trait.
///
///Each method describes single aspect of configuration
///and provided with sane defaults
pub trait Config {
    #[inline]
    ///Specifies number of threads to use for DNS
    ///resolve.
    ///
    ///Default number is 4
    fn dns_threads() -> usize {
        4
    }

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
    fn timeout() -> time::Duration {
        time::Duration::from_secs(30)
    }

    #[inline]
    ///Allows to sets default headers before request
    ///is sent out
    ///
    ///It is called as soon as request is being sent out,
    ///but before `Accept-Encoding` is set.
    fn default_headers(_headers: &mut header::HeaderMap) {
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
pub struct DefaultCfg;

impl Config for DefaultCfg {
}
