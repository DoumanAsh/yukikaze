//! Upgrade extension for client side

use super::request;
use super::response;

///Connection's header value for upgrade
pub const CONNECTION_TYPE: &str = "Upgrade";

#[cfg(feature = "websocket")]
pub mod websocket;
#[cfg(feature = "websocket")]
pub use self::websocket::{WebsocketUpgradeOpts, WebsocketUpgrade};

///Describes upgrade protocol
pub trait Upgrade {
    ///Result of upgrading
    type Result;
    ///Upgrade options.
    type Options;

    ///Prepares Request for upgrade
    fn prepare_request(req: request::Builder, options: Self::Options) -> request::Request;

    ///Upgrades Response
    fn upgrade_response(res: response::Response) -> Self::Result;
}
