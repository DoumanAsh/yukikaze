macro_rules! async_unwrap {
    ($result:expr) => ({
        use ::futures;

        match $result {
            futures::Async::Ready(result) => result,
            futures::Async::NotReady => return Ok(futures::Async::NotReady)
        }
    })
}

