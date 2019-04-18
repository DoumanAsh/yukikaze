//!Runtime module
//!
//!Yukikaze-sama is benevolent soul and it pains her when you cannot be lazy.
//!As such you can use this module to simplify your workflow.
//!
//!## Dependencies:
//!
//!```toml
//![dependencies]
//!tokio-global = { version = "0.2", features = ["single"] }
//!yukikaze = { version = "0.8", features = ["rt"] }
//!```
//!
//!## Example
//!
//!
//!```rust
//!extern crate yukikaze;
//!extern crate tokio_global; //external dependency
//!use yukikaze::client;
//!use yukikaze::rt::{AutoClient};
//!use tokio_global::AutoRuntime;
//!
//!let _guard = tokio_global::single::init();
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

#[cfg(feature = "rt-client")]
pub use self::client::{GlobalClient, AutoClient};

