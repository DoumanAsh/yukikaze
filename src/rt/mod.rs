//!Runtime module
//!
//!Yukikaze-sama is benevolent soul and it pains her when you cannot be lazy.
//!As such you can use this module to simplify your workflow.
//!
//!## Example
//!
//!```rust
//!extern crate yukikaze;
//!use yukikaze::client;
//!use yukikaze::rt::{AutoClient, AutoRuntime, init};
//!
//!let _guard = init();
//!//Now we can exeute futures using runtime
//!//When guard goes out of scope though,
//!//we no longer can use it.
//!
//!//We set global client to be used anywhere
//!//As soon as variable goes out of scope, client will reset.
//!let _global = yukikaze::rt::GlobalClient::default();
//!
//!let request = client::request::Request::get("https://google.com")
//!                                       .expect("To create google get request")
//!                                       .empty();
//!
//!let result = request.send().finish();
//!println!("result={:?}", result);
//!```

#[cfg(feature = "rt-client")]
pub mod client;
#[cfg(all(feature = "rt-tokio", not(feature = "rt-tokio-multi")))]
pub mod tokio;
#[cfg(feature = "rt-tokio-multi")]
pub mod tokio_multi;

///Trait to bootstrap your futures.
pub trait AutoRuntime: futures::Future {
    ///Runs futures to competition.
    ///
    ///Yukikaze-sama uses global runtime to work on your futures
    ///
    ///## Note
    ///
    ///It must not be used from within async context
    fn finish(self) -> Result<Self::Item, Self::Error>;
}

#[cfg(feature = "rt-tokio-multi")]
pub use tokio_multi as tokio;

#[cfg(feature = "rt-client")]
pub use self::client::{GlobalClient, AutoClient};
#[cfg(any(feature = "rt-tokio", feature = "rt-tokio-multi"))]
pub use self::tokio::{init};
