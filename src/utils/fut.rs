//!Future related utilities
use core::task;
use core::future::Future;
use core::pin::Pin;
use core::marker::Unpin;

///Either variant.
pub enum Either<A, B> {
    ///Left
    Left(A),
    ///Right
    Right(B),
}

unsafe impl<A: Sync, B: Sync> Sync for Either<A, B> {}
unsafe impl<A: Send, B: Send> Send for Either<A, B> {}
impl<A: Unpin, B: Unpin> Unpin for Either<A, B> {}

impl<A: Unpin, B: Unpin> Future for Either<A, B> where A: Future, B: Future<Output = A::Output> {
    type Output = A::Output;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        match *self {
            Either::Left(ref mut left) => Future::poll(Pin::new(left), ctx),
            Either::Right(ref mut right) => Future::poll(Pin::new(right), ctx),
        }
    }
}

///Create pair of future that being processed together.
///
///First goes left, then  right.
pub struct Pair<A: Unpin, B: Unpin> {
    inner: Option<(A, B)>,
}

impl<A: Unpin, B: Unpin> Pair<A, B> {
    ///Creates new instance
    pub fn new(left: A, right: B) -> Self {
        Self {
            inner: Some((left, right)),
        }
    }
}

impl<A: Unpin, B: Unpin> Future for Pair<A, B> where A: Future, B: Future {
    type Output = Either<(A::Output, B), (B::Output, A)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let (ref mut left, ref mut right) = match self.inner.as_mut() {
            Some(value) => value,
            None => unreach!()
        };

        match Pin::new(left).poll(cx) {
            task::Poll::Ready(res) => {
                let (_, right) = self.inner.take().unwrap();
                task::Poll::Ready(Either::Left((res, right)))
            },
            task::Poll::Pending => match Pin::new(right).poll(cx) {
                task::Poll::Ready(res) => {
                    let (left, _) = self.inner.take().unwrap();
                    task::Poll::Ready(Either::Right((res, left)))
                },
                task::Poll::Pending => task::Poll::Pending,
            }
        }
    }
}
