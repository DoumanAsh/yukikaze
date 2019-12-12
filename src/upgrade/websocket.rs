//! Websocket upgrade module

use core::mem;
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
        use sha1::{Sha1, Digest};
        let mut hasher = Sha1::new();

        hasher.input(&self.0);
        hasher.input(GUID.as_bytes());

        let res = hasher.result();
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
        let mut sec_key: [u8; 16] = unsafe { mem::MaybeUninit::uninit().assume_init() };
        let _ = getrandom::getrandom(&mut sec_key);

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
