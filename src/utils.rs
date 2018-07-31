use ::bytes;

use ::std::mem;
use ::std::io;

macro_rules! async_unwrap {
    ($result:expr) => ({
        use ::futures;

        match $result {
            futures::Async::Ready(result) => result,
            futures::Async::NotReady => return Ok(futures::Async::NotReady)
        }
    })
}

pub(crate) struct BytesWriter {
    buf: bytes::BytesMut,
}

impl BytesWriter {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(4096)
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
}

impl io::Write for BytesWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
