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
#[cfg(feature = "rt-tokio")]
pub mod tokio;

#[cfg(feature = "rt-client")]
pub use self::client::{GlobalClient, AutoClient};
#[cfg(feature = "rt-tokio")]
pub use self::tokio::{init, AutoRuntime};
