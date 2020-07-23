//! Websocket upgrade module
//!
//! ## Example
//!
//!```rust
//!use yukikaze::{matsu, client};
//!
//!async fn do_ws_handshaske() -> hyper::upgrade::Upgraded {
//!   const WS_TEST: &str = "http://echo.websocket.org/?encoding=text";
//!
//!   let request = client::request::Request::get(WS_TEST).expect("Error with request!")
//!                                                       .upgrade(yukikaze::upgrade::WebsocketUpgrade, None);
//!
//!   let client = client::Client::default();
//!
//!   let result = matsu!(client.send(request));
//!   let result = result.expect("To get without timeout");
//!   let response = result.expect("To get without error");
//!   assert!(response.is_upgrade());
//!
//!   let upgrade = matsu!(response.upgrade(yukikaze::upgrade::WebsocketUpgrade));
//!   let (response, upgraded) = upgrade.expect("To validate upgrade").expect("To finish upgrade");
//!   assert!(response.is_upgrade());
//!
//!   upgraded
//!}
//!```

use core::fmt;
use core::ops::Deref;
use std::error::Error;

use bytes::BufMut;
use data_encoding::BASE64;

use crate::utils;
use super::CONNECTION_TYPE;

const UPGRADE_NAME: &str = "websocket";
///Version set by `WebsocketUpgrade`
pub const WEBSOCKET_VERSION: usize = 13;
///GUID used for websocket challenge by server.
pub const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug)]
///Websocket upgrade errors
pub enum WebsocketUpgradeError {
    ///Unexpected Status code
    InvalidStatus(http::StatusCode),
    ///Unexpected type of upgrade
    InvalidUpgradeType,
    ///Unexpected Connection header
    InvalidConnectionHeader,
    ///Sec-Websocket-Accept header is missing
    MissingChallenge,
    ///Sec-Websocket-Accept has invalid challenge.
    InvalidChallenge,
}

impl fmt::Display for WebsocketUpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WebsocketUpgradeError::InvalidStatus(code) => write!(f, "Invalid status code of response. Should be 101, but got {}", code),
            WebsocketUpgradeError::InvalidUpgradeType => f.write_str("Invalid upgrade type for Websocket protocol"),
            WebsocketUpgradeError::InvalidConnectionHeader => f.write_str("Invalid Connection Header"),
            WebsocketUpgradeError::MissingChallenge => f.write_str("Sec-Websocket-Accept header is missing"),
            WebsocketUpgradeError::InvalidChallenge => f.write_str("Sec-Websocket-Accept has invalid challenge"),
        }
    }
}

impl From<http::StatusCode> for WebsocketUpgradeError {
    fn from(code: http::StatusCode) -> Self {
        WebsocketUpgradeError::InvalidStatus(code)
    }
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
    ///Performs validation of challenge, received in HTTP response
    pub fn validate_challenge(&self, challenge: &[u8]) -> bool {
        let mut ctx = ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);

        ctx.update(&self.0);
        ctx.update(GUID.as_bytes());

        let res = ctx.finish();
        let encoded = BASE64.encode(res.as_ref());

        encoded.as_bytes() == challenge
    }
}

///Options for `WebsocketUpgrade`
pub struct WebsocketUpgradeOpts {
    ///Specifies value of header `Sec-WebSocket-Protocol`
    pub protocols: &'static str
}

impl WebsocketUpgradeOpts {
    #[inline(always)]
    fn apply(self, headers: &mut http::HeaderMap) {
        match headers.entry(http::header::SEC_WEBSOCKET_PROTOCOL) {
            http::header::Entry::Vacant(entry) => {
                entry.insert(http::header::HeaderValue::from_static(self.protocols));
            },
            _ => (),
        }
    }
}

///Websocket upgrade method
pub struct WebsocketUpgrade;

impl super::Upgrade for WebsocketUpgrade {
    type VerifyError = WebsocketUpgradeError;
    type Options = Option<WebsocketUpgradeOpts>;

    fn prepare_request(headers: &mut http::HeaderMap, extensions: &mut http::Extensions, options: Self::Options) {
        use ring::rand::SecureRandom;

        let mut sec_key: [u8; 16] = [2, 3, 99, 255, 243, 125, 17, 29, 93, 105, 201, 152, 145, 192, 200, 221];
        let _ = ring::rand::SystemRandom::new().fill(&mut sec_key);

        let encode_len = BASE64.encode_len(sec_key.len());
        let mut key = bytes::BytesMut::with_capacity(encode_len);
        unsafe {
            {
                let dest = &mut *(&mut key.bytes_mut()[..encode_len] as *mut [core::mem::MaybeUninit<u8>] as *mut [u8]);
                BASE64.encode_mut(&sec_key, dest)
            }
            key.advance_mut(encode_len);
        }
        let key = key.freeze();
        let stored_key = SecKey(key.clone());
        extensions.insert(stored_key);

        let key = unsafe { http::header::HeaderValue::from_maybe_shared_unchecked(key) };

        match headers.entry(http::header::CONNECTION) {
            http::header::Entry::Vacant(entry) => {
                entry.insert(http::header::HeaderValue::from_static(CONNECTION_TYPE));
            },
            _ => (),
        }

        match headers.entry(http::header::UPGRADE) {
            http::header::Entry::Vacant(entry) => {
                entry.insert(http::header::HeaderValue::from_static(UPGRADE_NAME));
            },
            _ => (),
        }

        let _ = headers.insert(http::header::SEC_WEBSOCKET_VERSION, utils::content_len_value(WEBSOCKET_VERSION as u64));
        let _ = headers.insert(http::header::SEC_WEBSOCKET_KEY, key);

        if let Some(options) = options {
            options.apply(headers);
        }
    }

    fn verify_response(status: http::StatusCode, headers: &http::HeaderMap, extensions: &http::Extensions) -> Result<(), Self::VerifyError> {
        if status != http::StatusCode::SWITCHING_PROTOCOLS {
            return Err(status.into());
        }

        if !headers.get(http::header::UPGRADE).and_then(|val| val.to_str().ok()).map(|val| val.eq_ignore_ascii_case(UPGRADE_NAME)).unwrap_or(false) {
            return Err(WebsocketUpgradeError::InvalidUpgradeType);
        }

        if !headers.get(http::header::CONNECTION).and_then(|val| val.to_str().ok()).map(|val| val.eq_ignore_ascii_case(CONNECTION_TYPE)).unwrap_or(false) {
            return Err(WebsocketUpgradeError::InvalidConnectionHeader);
        }

        match extensions.get::<SecKey>() {
            Some(sec_key) => match headers.get(http::header::SEC_WEBSOCKET_ACCEPT) {
                Some(challenge) => match sec_key.validate_challenge(challenge.as_bytes()) {
                    true => (),
                    false => return Err(WebsocketUpgradeError::InvalidChallenge)
                },
                None => return Err(WebsocketUpgradeError::MissingChallenge)
            },
            None => panic!("Missing websocket Sec-Key. Did you start upgrade?")
        }

        Ok(())
    }
}
