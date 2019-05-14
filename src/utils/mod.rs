use core::mem;
use std::io::{self, Write};

const DEFAULT_CAPACITY: usize = 4096;
const SMOL_CAPCITY: usize = 64;

#[macro_export]
///Await future in async context.
///
///Because `.await` is retarded.
macro_rules! awaitic {
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

//pub mod fut;

pub(crate) struct BytesWriter {
    buf: bytes::BytesMut,
}

impl BytesWriter {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    #[inline]
    pub fn with_smol_capacity() -> Self {
        Self::with_capacity(SMOL_CAPCITY)
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: bytes::BytesMut::with_capacity(capacity)
        }
    }

    #[inline]
    pub fn into_inner(self) -> bytes::BytesMut {
        self.buf
    }

    #[inline]
    pub fn freeze(&mut self) -> bytes::Bytes {
        mem::replace(&mut self.buf, bytes::BytesMut::new()).freeze()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    pub fn split_off(&mut self, at: usize) -> Self {
        Self {
            buf: self.buf.split_off(at)
        }
    }

    #[inline]
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

pub fn content_len_value(len: u64) -> http::header::HeaderValue {
    let mut res = BytesWriter::with_capacity(1);
    let _ = write!(&mut res, "{}", len);
    unsafe { http::header::HeaderValue::from_shared_unchecked(res.freeze()) }
}