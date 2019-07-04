//! Futures responses
use futures::Future;

use super::{HyperResponse, Response, errors};

use std::{time};

#[derive(Debug)]
pub(crate) struct FutureResponseParams {
    #[cfg(feature = "carry_extensions")]
    extensions: http::Extensions,
}

impl FutureResponseParams {
    pub fn from_request(_request: &mut crate::client::request::Request) -> Self {
        Self {
            #[cfg(feature = "carry_extensions")]
            extensions: _request.extract_extensions(),
        }
    }

    fn apply(&mut self, _response: &mut Response) {
        #[cfg(feature = "carry_extensions")]
        std::mem::swap(&mut self.extensions, _response.extensions_mut());
    }
}

#[must_use = "Future must be polled to actually get HTTP response"]
///Yukikaze-sama's generic future for outgoing HTTP Request.
///
///This is beautiful foundation for requests.
///It wraps internal future into itself and provides timeout.
pub struct FutureResponse<F> {
    //We use Option here to
    //allow future to be moved into Timeout error
    //
    //Due to that all branches that handle None
    //is unreachable.
    //It should remain impossible for them to be reachable.
    inner: Option<(F, FutureResponseParams)>,
    delay: tokio_timer::Delay,
}

impl<F> FutureResponse<F> {
    pub(crate) fn new(inner: F, timeout: time::Duration, params: FutureResponseParams) -> Self {
        let delay = tokio_timer::Delay::new(tokio_timer::clock::now() + timeout);

        Self {
            inner: Some((inner, params)),
            delay,
        }
    }

    fn into_timeout(&mut self) -> errors::Timeout<F> {
        match self.inner.take() {
            Some(inner) => errors::Timeout::new(inner),
            None => unreach!()
        }
    }
}

impl<F: Future<Item=HyperResponse, Error=hyper::Error>> Future for FutureResponse<F> {
    type Item = Response;
    type Error = errors::ResponseError<F>;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        match self.inner.as_mut() {
            Some(inner) => match inner.0.poll() {
                Ok(futures::Async::Ready(result)) => {
                    let mut result = result.into();

                    inner.1.apply(&mut result);

                    return Ok(futures::Async::Ready(result));
                },
                Ok(futures::Async::NotReady) => (),
                Err(error) => return Err(errors::ResponseError::HyperError(error))
            },
            None => unreach!()
        }

        match self.delay.poll() {
            Ok(futures::Async::NotReady) => Ok(futures::Async::NotReady),
            Ok(futures::Async::Ready(_)) => Err(errors::ResponseError::Timeout(self.into_timeout())),
            Err(error) => Err(errors::ResponseError::Timer(error, self.into_timeout()))
        }
    }
}