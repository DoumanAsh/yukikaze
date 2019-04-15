//! Websocket protocol upgrade module
//!
//! ## Implementation
//!
//! On upgrade stores `SecKey` inside extension map.
//!
//! #### Request modifications
//!
//! - Apply `UpgradeOptions` to request's headers
//! - Add `Upgrade` header, if there is None
//! - Add `Connection` header, if there is None
//! - Add `Sec-Websocket-Version` as 13
//! - Add `Sec-Websocket-Key` which  is randomly generated.
//!
//! #### Verification of response headers:
//!
//! - Checks the value of `Upgrade` header, should be `UPGRADE_NAME`
//! - Checks the value of `Connection` header, should be `CONNECTION_TYPE`
//! - Using key, stored in `http::Extensions` verifies `Sec-Websocket-Accept` value. Panics if no key is stored in extensions.
//!
//! ## Usage
//!
//! ```rust
//! const WS_TEST: &str = "http://echo.websocket.org/?encoding=text";
//! use yukikaze::client::{self, HttpClient};
//!
//! let mut rt = tokio::runtime::current_thread::Runtime::new().expect("Build tokio runtime");
//! let client = client::Client::default();
//!
//! let request = client::request::Request::get(WS_TEST).expect("Error with request!")
//! //We specify that request should be prepared for websocket upgrade.
//! //But we leave out options, we can supply `yukikaze::client::upgrade::websocket::UpgradeOption`
//! //though
//!                                                     .upgrade(client::upgrade::WebsocketUpgrade, None);
//!
//! let response = rt.block_on(client.execute(request)).expect("Error with response!");
//! let upgrade = response.upgrade(client::upgrade::WebsocketUpgrade).expect("To validate upgrade");
//! //Response is original response that was consumed by calling `Response::upgrade`
//! //And ws_stream is Hyper's Upgraded that implements `AsyncRead`/`AsyncWrite`
//! let (response, ws_stream) = rt.block_on(upgrade).expect("To finish upgrade");
//!
//! ```

use std::ops::Deref;
use std::error::Error;
use std::mem;

use super::{response, request, CONNECTION_TYPE};

use futures::Future;
use bytes::BufMut;
use hyper::upgrade::{OnUpgrade, Upgraded};
use data_encoding::BASE64;

#[derive(Debug, derive_more::From, derive_more::Display)]
///Websocket upgrade errors
pub enum WebsocketUpgradeError {
    #[display(fmt = "Invalid status code of response. Should be 101, but got {}", "_0")]
    ///Unexpected Status code
    InvalidStatus(http::StatusCode),
    #[display(fmt = "Invalid upgrade type for Websocket protocol")]
    ///Unexpected type of upgrade
    InvalidUpgradeType,
    #[display(fmt = "Invalid Connection Header")]
    ///Unexpected Connection header
    InvalidConnectionHeader,
    #[display(fmt = "Sec-Websocket-Accept header is missing")]
    ///Sec-Websocket-Accept header is missing
    MissingChallenge,
    #[display(fmt = "Sec-Websocket-Accept has invalid challenge")]
    ///Sec-Websocket-Accept has invalid challenge.
    InvalidChallenge,
    #[display(fmt = "Protocol error during upgrade")]
    ///Protocol error during upgrade.
    HyperError(hyper::Error),
}

impl Error for WebsocketUpgradeError {}

///Websocket's `Sec-Websocket-Key` value
///
///Added during upgrade
pub struct SecKey(bytes::Bytes);
impl Deref for SecKey {
    type Target = bytes::Bytes;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SecKey {
    ///Performs validation of challenge
    pub fn validate_challenge(&self, challenge: &[u8]) -> bool {
        let mut hasher = ring::digest::Context::new(&ring::digest::SHA1);

        hasher.update(&self.0);
        hasher.update(GUID.as_bytes());

        let res = hasher.finish();
        let encoded = BASE64.encode(res.as_ref());

        encoded.as_bytes() == challenge
    }
}

const UPGRADE_NAME: &str = "websocket";
///Version set by `WebsocketUpgrade`
pub const WEBSOCKET_VERSION: usize = 13;
///GUID used for websocket challenge by server.
pub const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

///Options for `WebsocketUpgrade`
pub struct WebsocketUpgradeOpts {
    ///Specifies value of header `Sec-WebSocket-Protocol`
    pub protocols: &'static str
}

impl WebsocketUpgradeOpts {
    #[inline(always)]
    fn apply(self, req: request::Builder) -> request::Builder {
        req.set_header_if_none(http::header::SEC_WEBSOCKET_PROTOCOL, http::header::HeaderValue::from_static(self.protocols))
    }
}

///Websocket upgrade method
pub struct WebsocketUpgrade;

impl super::Upgrade for WebsocketUpgrade {
    type Result = Result<FutureWs, WebsocketUpgradeError>;
    type Options = Option<WebsocketUpgradeOpts>;

    fn prepare_request(mut req: request::Builder, options: Self::Options) -> request::Request {
        let sec_key: [u8; 16] = rand::random();

        let encode_len = BASE64.encode_len(sec_key.len());
        let mut key = bytes::BytesMut::with_capacity(encode_len);
        unsafe {
            BASE64.encode_mut(&sec_key, &mut key.bytes_mut()[..encode_len]);
            key.advance_mut(encode_len);
        }
        let key = key.freeze();
        let stored_key = SecKey(key.clone());
        req.extensions_mut().insert(stored_key);

        req.set_header_if_none(http::header::UPGRADE, UPGRADE_NAME)
           .set_header_if_none(http::header::CONNECTION, CONNECTION_TYPE)
           .set_header(http::header::SEC_WEBSOCKET_VERSION, WEBSOCKET_VERSION)
           .set_header(http::header::SEC_WEBSOCKET_KEY, key)
           .if_some(options, WebsocketUpgradeOpts::apply)
           .empty()
    }

    fn upgrade_response(res: response::Response) -> Self::Result {
        if !res.is_upgrade() {
            return Err(res.status().into())
        }

        if !res.headers().get(http::header::UPGRADE).and_then(|val| val.to_str().ok()).map(|val| val.eq_ignore_ascii_case(UPGRADE_NAME)).unwrap_or(false) {
            return Err(WebsocketUpgradeError::InvalidUpgradeType);
        }

        if !res.headers().get(http::header::CONNECTION).and_then(|val| val.to_str().ok()).map(|val| val.eq_ignore_ascii_case(CONNECTION_TYPE)).unwrap_or(false) {
            return Err(WebsocketUpgradeError::InvalidConnectionHeader);
        }

        match res.extensions().get::<SecKey>() {
            Some(sec_key) => match res.headers().get(http::header::SEC_WEBSOCKET_ACCEPT) {
                Some(challenge) => match sec_key.validate_challenge(challenge.as_bytes()) {
                    true => (),
                    false => return Err(WebsocketUpgradeError::InvalidChallenge)
                },
                None => return Err(WebsocketUpgradeError::MissingChallenge)
            },
            None => panic!("Missing websocket Sec-Key. Did you start upgrade?")
        }

        Ok(res.into())
    }
}

///Represents ongoing Websocket upgrade.
///
///In result you'll get original `Response` and `hyper::Upgraded`
///that can be used to read/write via async `Read`/`Write` ops
pub struct FutureWs {
    head: http::response::Parts,
    upgrade: OnUpgrade,
}

impl From<response::Response> for FutureWs {
    fn from(res: response::Response) -> Self {
        let (head, body) = res.inner.into_parts();

        Self {
            head,
            upgrade: body.on_upgrade(),
        }
    }
}

impl Future for FutureWs {
    type Item = (response::Response, Upgraded);
    type Error = WebsocketUpgradeError;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        match self.upgrade.poll()? {
            futures::Async::Ready(upgraded) => {
                let response = hyper::Response::new(hyper::Body::empty());

                let (mut new_head, new_body) = response.into_parts();
                mem::swap(&mut new_head, &mut self.head);

                let response = hyper::Response::from_parts(new_head, new_body);
                return Ok(futures::Async::Ready((response.into(), upgraded)))
            },
            futures::Async::NotReady => return Ok(futures::Async::NotReady),
        }
    }
}
