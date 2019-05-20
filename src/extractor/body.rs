use core::marker::Unpin;
use core::cmp;
use std::io::{self, Write};
use std::fs::File;

use super::BodyReadError;
use crate::header::ContentEncoding;

use futures_util::stream::StreamExt;
#[cfg(feature = "encoding")]
use encoding_rs::Encoding;

use super::Notifier;

const BUFFER_SIZE: usize = 4096;

#[inline(always)]
fn calculate_buffer_size(limit: Option<usize>) -> (usize, usize) {
    match limit {
        Some(limit) => (limit, cmp::min(BUFFER_SIZE, limit)),
        None => (BUFFER_SIZE, BUFFER_SIZE)
    }
}

///Extracts body as bytes from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
pub async fn raw_bytes<S, I, E>(mut body: S, encoding: ContentEncoding, limit: Option<usize>) -> Result<bytes::Bytes, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>,
{
    let (limit, buffer_size) = calculate_buffer_size(limit);

    match encoding {
        #[cfg(feature = "flate2")]
        ContentEncoding::Gzip => {
            let mut decoder = flate2::write::GzDecoder::new(crate::utils::BytesWriter::with_capacity(buffer_size));

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::GzipError(error))?;
                decoder.flush().map_err(|error| BodyReadError::GzipError(error))?;

                if limit < decoder.get_ref().len() {
                    let _ = decoder.try_finish();
                    return Err(BodyReadError::Overflow(decoder.get_mut().freeze()));
                }
            }

            decoder.try_finish().map_err(|error| BodyReadError::GzipError(error))?;
            Ok(decoder.get_mut().freeze())
        },
        #[cfg(feature = "flate2")]
        ContentEncoding::Deflate => {
            let mut decoder = flate2::write::ZlibDecoder::new(crate::utils::BytesWriter::with_capacity(buffer_size));

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::DeflateError(error))?;
                decoder.flush().map_err(|error| BodyReadError::DeflateError(error))?;

                if limit < decoder.get_ref().len() {
                    let _ = decoder.try_finish();
                    return Err(BodyReadError::Overflow(decoder.get_mut().freeze()));
                }
            }

            decoder.try_finish().map_err(|error| BodyReadError::DeflateError(error))?;
            Ok(decoder.get_mut().freeze())
        },
        _ => {
            let mut buffer = bytes::BytesMut::with_capacity(buffer_size);

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                buffer.extend_from_slice(&chunk[..]);
                if buffer.len() > limit {
                    return Err(BodyReadError::Overflow(buffer.freeze()));
                }
            }

            Ok(buffer.freeze())
        }
    }
}

///Extracts body as text from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
pub async fn text<S, I, E>(body: S, encoding: ContentEncoding, limit: Option<usize>) -> Result<String, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>,
{
    let bytes = awaitic!(raw_bytes(body, encoding, limit))?;

    String::from_utf8(bytes.to_vec()).map_err(|error| error.into())
}

#[cfg(feature = "encoding")]
///Extracts body as text from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
///- `charset` - Specifies charset to use, if omitted assumes `UTF-8`. Available only with feature `encoding`
pub async fn text_charset<S, I, E>(body: S, encoding: ContentEncoding, limit: Option<usize>, charset: &'static Encoding) -> Result<String, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>,
{
    let bytes = awaitic!(raw_bytes(body, encoding, limit))?;

    match charset.decode(&bytes) {
        (result, _, false) => Ok(result.into_owned()),
        (_, _, true) => Err(BodyReadError::EncodingError)
    }
}

///Extracts body as JSON from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
pub async fn json<S, I, E, J>(body: S, encoding: ContentEncoding, limit: Option<usize>) -> Result<J, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>, J: serde::de::DeserializeOwned
{
    let bytes = awaitic!(raw_bytes(body, encoding, limit))?;

    serde_json::from_slice(&bytes).map_err(BodyReadError::from)
}

#[cfg(feature = "encoding")]
///Extracts body as JSON from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
///- `charset` - Specifies charset to use, if omitted assumes `UTF-8`. Available only with feature `encoding`
pub async fn json_charset<S, I, E, J>(body: S, encoding: ContentEncoding, limit: Option<usize>, charset: &'static Encoding) -> Result<J, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>, J: serde::de::DeserializeOwned
{
    let bytes = awaitic!(raw_bytes(body, encoding, limit))?;

    match charset.decode(&bytes) {
        (result, _, false) => serde_json::from_str(&result).map_err(BodyReadError::from),
        (_, _, true) => Err(BodyReadError::EncodingError)
    }
}

///Extracts body as bytes from `Stream` and write it to file
///
///Params:
///
///- `file` - Into which to write
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies encoding to use.
pub async fn file<S, I, E>(file: File, mut body: S, encoding: ContentEncoding) -> Result<File, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>
{
    let file = io::BufWriter::new(file);

    let file = match encoding {
        #[cfg(feature = "flate2")]
        ContentEncoding::Gzip => {
            let mut decoder = flate2::write::GzDecoder::new(file);

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::GzipError(error))?;
            }

            decoder.finish().map_err(|error| BodyReadError::GzipError(error))?
        },
        #[cfg(feature = "flate2")]
        ContentEncoding::Deflate => {
            let mut decoder = flate2::write::ZlibDecoder::new(file);

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::DeflateError(error))?;
            }

            decoder.finish().map_err(|error| BodyReadError::DeflateError(error))?
        },
        _ => {
            let mut buffer = file;

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                match buffer.write_all(&chunk[..]) {
                    Ok(_) => (),
                    //TODO: consider how to get File without stumbling into error
                    Err(error) => return Err(BodyReadError::FileError(buffer.into_inner().expect("To get File"), error)),
                }
            }

            buffer
        }
    };

    let mut file = file.into_inner().expect("To get File out of BufWriter");
    match file.flush() {
        Ok(_) => Ok(file),
        Err(error) => Err(BodyReadError::FileError(file, error))
    }
}

//Notify

///Extracts body as bytes from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
pub async fn raw_bytes_notify<S, I, E, N: Notifier>(mut body: S, encoding: ContentEncoding, limit: Option<usize>, mut notify: N) -> Result<bytes::Bytes, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>
{
    let (limit, buffer_size) = calculate_buffer_size(limit);

    match encoding {
        #[cfg(feature = "flate2")]
        ContentEncoding::Gzip => {
            let mut decoder = flate2::write::GzDecoder::new(crate::utils::BytesWriter::with_capacity(buffer_size));

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::GzipError(error))?;
                decoder.flush().map_err(|error| BodyReadError::GzipError(error))?;

                notify.send(chunk.len());

                if limit < decoder.get_ref().len() {
                    let _ = decoder.try_finish();
                    return Err(BodyReadError::Overflow(decoder.get_mut().freeze()));
                }
            }

            decoder.try_finish().map_err(|error| BodyReadError::GzipError(error))?;
            Ok(decoder.get_mut().freeze())
        },
        #[cfg(feature = "flate2")]
        ContentEncoding::Deflate => {
            let mut decoder = flate2::write::ZlibDecoder::new(crate::utils::BytesWriter::with_capacity(buffer_size));

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::DeflateError(error))?;
                decoder.flush().map_err(|error| BodyReadError::DeflateError(error))?;

                notify.send(chunk.len());

                if limit < decoder.get_ref().len() {
                    let _ = decoder.try_finish();
                    return Err(BodyReadError::Overflow(decoder.get_mut().freeze()));
                }
            }

            decoder.try_finish().map_err(|error| BodyReadError::DeflateError(error))?;
            Ok(decoder.get_mut().freeze())
        },
        _ => {
            let mut buffer = bytes::BytesMut::with_capacity(buffer_size);

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                buffer.extend_from_slice(&chunk[..]);
                notify.send(chunk.len());
                if buffer.len() > limit {
                    return Err(BodyReadError::Overflow(buffer.freeze()));
                }
            }

            Ok(buffer.freeze())
        }
    }
}

///Extracts body as text from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
pub async fn text_notify<S, I, E, N: Notifier>(body: S, encoding: ContentEncoding, limit: Option<usize>, notify: N) -> Result<String, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>
{
    let bytes = awaitic!(raw_bytes_notify(body, encoding, limit, notify))?;

    String::from_utf8(bytes.to_vec()).map_err(|error| error.into())
}

#[cfg(feature = "encoding")]
///Extracts body as text from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
///- `charset` - Specifies charset to use, if omitted assumes `UTF-8`. Available only with feature `encoding`
pub async fn text_charset_notify<S, I, E, N>(body: S, encoding: ContentEncoding, limit: Option<usize>, charset: &'static Encoding, notify: N) -> Result<String, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>, N: Notifier,
{
    let bytes = awaitic!(raw_bytes_notify(body, encoding, limit, notify))?;

    match charset.decode(&bytes) {
        (result, _, false) => Ok(result.into_owned()),
        (_, _, true) => Err(BodyReadError::EncodingError)
    }
}

///Extracts body as JSON from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
pub async fn json_notify<S, I, E, N, J>(body: S, encoding: ContentEncoding, limit: Option<usize>, notify: N) -> Result<J, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>, J: serde::de::DeserializeOwned, N: Notifier
{
    let bytes = awaitic!(raw_bytes_notify(body, encoding, limit, notify))?;

    serde_json::from_slice(&bytes).map_err(BodyReadError::from)
}

#[cfg(feature = "encoding")]
///Extracts body as JSON from `Stream`
///
///Params:
///
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies content's encoding to use.
///- `limit` - Specifies limit on body size, if not specified uses default 4kb
///- `charset` - Specifies charset to use, if omitted assumes `UTF-8`. Available only with feature `encoding`
pub async fn json_charset_notify<S, I, E, N, J>(body: S, encoding: ContentEncoding, limit: Option<usize>, charset: &'static Encoding, notify: N) -> Result<J, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>, J: serde::de::DeserializeOwned, N: Notifier
{
    let bytes = awaitic!(raw_bytes_notify(body, encoding, limit, notify))?;

    match charset.decode(&bytes) {
        (result, _, false) => serde_json::from_str(&result).map_err(BodyReadError::from),
        (_, _, true) => Err(BodyReadError::EncodingError)
    }
}

///Extracts body as bytes from `Stream` and write it to file
///
///Params:
///
///- `file` - Into which to write
///- `body` - Stream of data chunks to read. If limit is hit, body is not exhausted completely.
///- `encoding` - Specifies encoding to use.
pub async fn file_notify<S, I, E, N: Notifier>(file: File, mut body: S, encoding: ContentEncoding, mut notify: N) -> Result<File, BodyReadError>
    where S: StreamExt<Item=Result<I, E>> + Unpin, I: Into<bytes::Bytes>, E: Into<BodyReadError>,
{
    let file = io::BufWriter::new(file);

    let file = match encoding {
        #[cfg(feature = "flate2")]
        ContentEncoding::Gzip => {
            let mut decoder = flate2::write::GzDecoder::new(file);

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::GzipError(error))?;
                notify.send(chunk.len());
            }

            decoder.finish().map_err(|error| BodyReadError::GzipError(error))?
        },
        #[cfg(feature = "flate2")]
        ContentEncoding::Deflate => {
            let mut decoder = flate2::write::ZlibDecoder::new(file);

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                decoder.write_all(&chunk[..]).map_err(|error| BodyReadError::DeflateError(error))?;
                notify.send(chunk.len());
            }

            decoder.finish().map_err(|error| BodyReadError::DeflateError(error))?
        },
        _ => {
            let mut buffer = file;

            while let Some(chunk) = awaitic!(body.next()) {
                let chunk = chunk.map(Into::into).map_err(Into::into)?;

                match buffer.write_all(&chunk[..]) {
                    Ok(_) => notify.send(chunk.len()),
                    //TODO: consider how to get File without stumbling into error
                    Err(error) => return Err(BodyReadError::FileError(buffer.into_inner().expect("To get File"), error)),
                }
            }

            buffer
        }
    };

    let mut file = file.into_inner().expect("To get File out of BufWriter");
    match file.flush() {
        Ok(_) => Ok(file),
        Err(error) => Err(BodyReadError::FileError(file, error))
    }
}
