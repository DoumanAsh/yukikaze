//!Yukikaze-sama utilities.
use core::mem;
use std::io::{self, Write};

const DEFAULT_CAPACITY: usize = 4096;
const SMOL_CAPCITY: usize = 64;

#[macro_export]
///Await future in async context.
///
///Because `.await` is retarded.
macro_rules! matsu {
    ($exp:expr) => {
        ($exp).await
    }
}

#[doc(hidden)]
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! unreach {
    () => ({
        unsafe {
            std::hint::unreachable_unchecked();
        }
    })
}

#[doc(hidden)]
#[macro_export]
#[cfg(debug_assertions)]
macro_rules! unreach {
    () => ({
        unreachable!()
    })
}

pub mod fut;
pub mod enc;

///Convenience wrapper over `bytes::BytesMut`
///
///Provides `io::Write` that automatically resizes.
pub struct BytesWriter {
    buf: bytes::BytesMut,
}

impl BytesWriter {
    ///Creates new instance with default capacity 4096
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    #[inline]
    ///Creates new instance with smol capacity 64
    pub fn with_smol_capacity() -> Self {
        Self::with_capacity(SMOL_CAPCITY)
    }

    #[inline]
    ///Creates new instance with provided capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: bytes::BytesMut::with_capacity(capacity)
        }
    }

    #[inline]
    ///Converts into underlying `bytes::BytesMut`
    pub fn into_inner(self) -> bytes::BytesMut {
        self.buf
    }

    #[inline]
    ///Converts into `bytes::Bytes`
    pub fn freeze(&mut self) -> bytes::Bytes {
        mem::replace(&mut self.buf, bytes::BytesMut::new()).freeze()
    }

    #[inline]
    ///Returns buffer length.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    ///Splits off, the same as `bytes::BytesMut::split_off`
    pub fn split_off(&mut self, at: usize) -> Self {
        Self {
            buf: self.buf.split_off(at)
        }
    }

    #[inline]
    ///Reserve extra memory, the same as `bytes::BytesMut::reserve`
    pub fn reserve(&mut self, add: usize) {
        self.buf.reserve(add);
    }
}

impl io::Write for BytesWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.buf.extend_from_slice(buf);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

///Converts integer to header's value.
pub fn content_len_value(len: u64) -> http::header::HeaderValue {
    let mut res = BytesWriter::with_capacity(1);
    let _ = write!(&mut res, "{}", len);
    unsafe { http::header::HeaderValue::from_shared_unchecked(res.freeze()) }
}
