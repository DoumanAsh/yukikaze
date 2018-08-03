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
//!use yukikaze::rt::{AutoClient, AutoRuntime};
//!
//!let client = client::Client::default();
//!//We set global client to be used anywhere
//!yukikaze::rt::set(client);
//!
//!let request = client::request::Request::get("https://google.com")
//!                                       .expect("To create google get request")
//!                                       .empty();
//!
//!let result = request.send().finish();
//!println!("result={:?}", result);
//!```

use ::tokio;
use ::futures::{IntoFuture, Future};

use ::std::cell::Cell;

use ::client;

enum State {
    None,
    Client(Box<client::HttpClient>),
}

thread_local!(static CLIENT: Cell<State> = Cell::new(State::None));

///Sets global client in thread local storage.
pub fn set<C: client::HttpClient + 'static>(client: C) {
    CLIENT.with(move |store| store.set(State::Client(Box::new(client))))
}

///Sets default client as global in thread local storage.
pub fn set_default() {
    let client = client::Client::default();
    CLIENT.with(move |store| store.set(State::Client(Box::new(client))))
}

///Executes HTTP request on global client
pub fn execute(req: client::Request) -> client::response::FutureResponse {
    CLIENT.with(move |store| match store.replace(State::None) {
        State::Client(client) => {
            let res = client.execute(req);
            store.set(State::Client(client));
            res
        },
        State::None => panic!("Client is not set"),
    })
}

///Starts function within tokio runtime and finishes
///as soon as all futures are resolved.
pub fn run<R: IntoFuture, F: FnOnce() -> R>(runner: F) -> Result<R::Item, R::Error> {
    tokio::executor::current_thread::block_on_all(runner().into_future())
}

///Trait to bootstrap your requests.
pub trait AutoClient {
    ///Sends request using default client.
    fn send(self) -> client::response::FutureResponse;
}

impl AutoClient for client::Request {
    #[inline]
    fn send(self) -> client::response::FutureResponse {
        execute(self)
    }
}

///Trait to bootstrap your futures.
pub trait AutoRuntime: Future + Sized {
    #[inline]
    ///Runs futures to competition.
    ///
    ///Under hood it uses tokio's current thread executor
    ///that runs this future and all others which are spawn by it
    ///to its competition.
    fn finish(self) -> Result<Self::Item, Self::Error> {
        tokio::executor::current_thread::block_on_all(self)
    }
}

impl<F: Future + Sized> AutoRuntime for F {}
