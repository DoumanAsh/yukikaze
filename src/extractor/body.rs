use core::marker::Unpin;
use core::cmp;
use std::io::{self, Write};
use std::fs::File;

use super::BodyReadError;
use crate::header::ContentEncoding;

use http_body::Body as HttpBody;

#[cfg(feature = "encoding")]
use encoding_rs::Encoding;
#[cfg(feature = "compu")]
use compu::decoder::Decoder;

use super::Notifier;

const BUFFER_SIZE: usize = 4096;

#[inline(always)]
fn calculate_buffer_size(limit: Option<usize>) -> (usize, usize) {
    match limit {
        Some(limit) => (limit, cmp::min(BUFFER_SIZE, limit)),
        None => (BUFFER_SIZE, BUFFER_SIZE)
    }
}

#[cfg(feature = "compu")]
macro_rules! impl_compu_bytes {
    ($decoder:expr, $body:expr, $limit:expr) => {
        use compu::decoder::DecoderResult;

        let mut decoder = compu::decompressor::memory::Decompressor::new($decoder);

        while let Some(chunk) = matsu!($body.data()) {
            let chunk = chunk.map(Into::into).map_err(Into::into)?;

            match decoder.push(&chunk) {
                DecoderResult::Finished => break,
                DecoderResult::NeedInput => (),
                result => return Err(BodyReadError::CompuError(result)),
            }

            if $limit < decoder.output().len() {
                return Err(BodyReadError::Overflow(decoder.take().into()))
            }
        }

        match decoder.decoder().is_finished() {
            true => return Ok(decoder.take().into()),
            false => return Err(BodyReadError::IncompleteDecompression),
        }
    };
    ($decoder:expr, $body:expr, $limit:expr, $notify:expr) => {
        use compu::decoder::DecoderResult;

        let mut decoder = compu::decompressor::memory::Decompressor::new($decoder);

        while let Some(chunk) = matsu!($body.data()) {
            let chunk = chunk.map(Into::into).map_err(Into::into)?;

            $notify.send(chunk.len());

            match decoder.push(&chunk) {
                DecoderResult::Finished => break,
                DecoderResult::NeedInput => (),
                result => return Err(BodyReadError::CompuError(result)),
            }

            if $limit < decoder.output().len() {
                return Err(BodyReadError::Overflow(decoder.take().into()))
            }
        }

        match decoder.decoder().is_finished() {
            true => return Ok(decoder.take().into()),
            false => return Err(BodyReadError::IncompleteDecompression),
        }
    }
}
#[cfg(feature = "compu")]
macro_rules! impl_compu_file {
    ($decoder:expr, $body:expr, $file:expr) => {
        use compu::decoder::DecoderResult;

        let mut decoder = compu::decompressor::write::Decompressor::new($decoder, $file);

        while let Some(chunk) = matsu!($body.data()) {
            let chunk = chunk.map(Into::into).map_err(Into::into)?;

            match decoder.push(&chunk)? {
                (DecoderResult::Finished, _) => break,
                (DecoderResult::NeedInput, _) => (),
                (result, _) => return Err(BodyReadError::CompuError(result)),
            }
        }

        match decoder.decoder().is_finished() {
            true => (),
            false => return Err(BodyReadError::IncompleteDecompression),
        }
    };
    ($decoder:expr, $body:expr, $file:expr, $notify:expr) => {
        use compu::decoder::DecoderResult;

        let mut decoder = compu::decompressor::write::Decompressor::new($decoder, $file);

        while let Some(chunk) = matsu!($body.data()) {
            let chunk = chunk.map(Into::into).map_err(Into::into)?;

            $notify.send(chunk.len());

            match decoder.push(&chunk)? {
                (DecoderResult::Finished, _) => break,
                (DecoderResult::NeedInput, _) => (),
                (result, _) => return Err(BodyReadError::CompuError(result)),
            }
        }

        match decoder.decoder().is_finished() {
            true => decoder.take(),
            false => return Err(BodyReadError::IncompleteDecompression),
        }
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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>,
{
    let (limit, buffer_size) = calculate_buffer_size(limit);

    match encoding {
        #[cfg(feature = "compu")]
        ContentEncoding::Brotli => {
            impl_compu_bytes!(compu::decoder::brotli::BrotliDecoder::default(), body, limit);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Gzip => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Gzip);
            impl_compu_bytes!(compu::decoder::zlib::ZlibDecoder::new(&options), body, limit);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Deflate => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Zlib);
            impl_compu_bytes!(compu::decoder::zlib::ZlibDecoder::new(&options), body, limit);
        },
        _ => {
            let mut buffer = bytes::BytesMut::with_capacity(buffer_size);

            while let Some(chunk) = matsu!(body.data()) {
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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>,
{
    let bytes = matsu!(raw_bytes(body, encoding, limit))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>,
{
    let bytes = matsu!(raw_bytes(body, encoding, limit))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>, J: serde::de::DeserializeOwned
{
    let bytes = matsu!(raw_bytes(body, encoding, limit))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>, J: serde::de::DeserializeOwned
{
    let bytes = matsu!(raw_bytes(body, encoding, limit))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>
{
    let mut file = io::BufWriter::new(file);

    match encoding {
        #[cfg(feature = "compu")]
        ContentEncoding::Brotli => {
            impl_compu_file!(compu::decoder::brotli::BrotliDecoder::default(), body, &mut file);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Gzip => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Gzip);
            impl_compu_file!(compu::decoder::zlib::ZlibDecoder::new(&options), body, &mut file);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Deflate => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Zlib);
            impl_compu_file!(compu::decoder::zlib::ZlibDecoder::new(&options), body, &mut file);
        },
        _ => while let Some(chunk) = matsu!(body.data()) {
            let chunk = chunk.map(Into::into).map_err(Into::into)?;

            match file.write_all(&chunk[..]) {
                Ok(_) => (),
                //TODO: consider how to get File without stumbling into error
                Err(error) => return Err(BodyReadError::FileError(file.into_inner().expect("To get File"), error)),
            }
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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>
{
    let (limit, buffer_size) = calculate_buffer_size(limit);

    match encoding {
        #[cfg(feature = "compu")]
        ContentEncoding::Brotli => {
            impl_compu_bytes!(compu::decoder::brotli::BrotliDecoder::default(), body, limit);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Gzip => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Gzip);
            impl_compu_bytes!(compu::decoder::zlib::ZlibDecoder::new(&options), body, limit);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Deflate => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Zlib);
            impl_compu_bytes!(compu::decoder::zlib::ZlibDecoder::new(&options), body, limit);
        },
        _ => {
            let mut buffer = bytes::BytesMut::with_capacity(buffer_size);

            while let Some(chunk) = matsu!(body.data()) {
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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>
{
    let bytes = matsu!(raw_bytes_notify(body, encoding, limit, notify))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>, N: Notifier
{
    let bytes = matsu!(raw_bytes_notify(body, encoding, limit, notify))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>, J: serde::de::DeserializeOwned, N: Notifier
{
    let bytes = matsu!(raw_bytes_notify(body, encoding, limit, notify))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>, J: serde::de::DeserializeOwned, N: Notifier
{
    let bytes = matsu!(raw_bytes_notify(body, encoding, limit, notify))?;

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
    where S: HttpBody<Data=I, Error=E> + Unpin, I: Into<bytes::Bytes> + bytes::Buf, E: Into<BodyReadError>
{
    let mut file = io::BufWriter::new(file);

    match encoding {
        #[cfg(feature = "compu")]
        ContentEncoding::Brotli => {
            impl_compu_file!(compu::decoder::brotli::BrotliDecoder::default(), body, &mut file, notify);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Gzip => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Gzip);
            impl_compu_file!(compu::decoder::zlib::ZlibDecoder::new(&options), body, &mut file, notify);
        },
        #[cfg(feature = "compu")]
        ContentEncoding::Deflate => {
            let options = compu::decoder::zlib::ZlibOptions::default().mode(compu::decoder::zlib::ZlibMode::Zlib);
            impl_compu_file!(compu::decoder::zlib::ZlibDecoder::new(&options), body, &mut file, notify);
        },
        _ => while let Some(chunk) = matsu!(body.data()) {
            let chunk = chunk.map(Into::into).map_err(Into::into)?;

            match file.write_all(&chunk[..]) {
                Ok(_) => notify.send(chunk.len()),
                //TODO: consider how to get File without stumbling into error
                Err(error) => return Err(BodyReadError::FileError(file.into_inner().expect("To get File"), error)),
            }
        }
    };

    let mut file = file.into_inner().expect("To get File out of BufWriter");
    match file.flush() {
        Ok(_) => Ok(file),
        Err(error) => Err(BodyReadError::FileError(file, error))
    }
}
