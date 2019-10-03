//! Upgrade extension for client side

///Connection's header value for upgrade
pub const CONNECTION_TYPE: &str = "Upgrade";

#[cfg(feature = "websocket")]
pub mod websocket;
#[cfg(feature = "websocket")]
pub use self::websocket::{WebsocketUpgradeOpts, WebsocketUpgrade};

///Describes upgrade protocol
pub trait Upgrade {
    ///Result of upgrading
    type VerifyError;
    ///Upgrade options.
    type Options;

    ///Prepares Request for upgrade
    fn prepare_request(headers: &mut http::HeaderMap, extensions: &mut http::Extensions, options: Self::Options);

    ///Upgrades Response
    fn verify_response(status: http::StatusCode, headers: &http::HeaderMap, extensions: &http::Extensions) -> Result<(), Self::VerifyError>;
}

pub(crate) type UpgradeRes = Result<(http::Response<hyper::Body>, hyper::upgrade::Upgraded), hyper::Error>;
///Utility to upgrade using hyper's upgrade mechanism
pub async fn upgrade_response(parts: http::response::Parts, body: hyper::upgrade::OnUpgrade) -> UpgradeRes {
    matsu!(body).map(|upgraded| {
        let response = http::Response::from_parts(parts, hyper::Body::empty());
        (response, upgraded)
    })
}
