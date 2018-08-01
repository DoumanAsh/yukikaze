//!Describes client configuration

use ::std::time;

use ::hyper;

///Generic config trait.
///
///Each method describes single aspect of configuration
///and provided with sane defaults
pub trait Config {
    ///Specifies number of threads to use for DNS
    ///resolve.
    ///
    ///Default number is 4
    fn dns_threads() -> usize {
        4
    }

    ///Specifies whether to automatically request compressed response.
    ///
    ///Defaults to true.
    fn decompress() -> bool {
        true
    }

    ///Specifies request timeout.
    ///
    ///Default is 30 seconds
    fn timeout() -> time::Duration {
        time::Duration::from_secs(30)
    }

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
