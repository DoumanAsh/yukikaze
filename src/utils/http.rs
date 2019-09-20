//!Extension to `http_body::Body`
//!
use core::future::Future;
use core::pin::Pin;
use core::task;

#[derive(Debug)]
///Future that resolves to the next data chunk from `Body`
pub struct NextData<'a, T: ?Sized>(pub(crate) &'a mut T);

impl<'a, T: http_body::Body + Unpin + ?Sized> Future for NextData<'a, T> {
    type Output = Option<Result<T::Data, T::Error>>;

    #[inline(always)]
    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let body = unsafe { self.map_unchecked_mut(|this| &mut this.0) };

        http_body::Body::poll_data(body, ctx)
    }
}

pub trait Body: http_body::Body {
    #[inline(always)]
    /// Returns future that resolves to next data chunk, if any.
    fn next(&mut self) -> NextData<'_, Self> where Self: Unpin {
        NextData(self)
    }
}

impl<T: http_body::Body> Body for T {}
