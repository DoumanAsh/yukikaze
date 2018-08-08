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

use ::tokio::runtime::current_thread::Runtime;
use ::futures::{IntoFuture, Future};

use ::std::cell::Cell;

use ::client;

thread_local!(static TOKIO: Cell<Option<Runtime>> = Cell::new(Some(Runtime::new().expect("To crate tokio runtime"))));
thread_local!(static CLIENT: Cell<Option<Box<client::HttpClient>>> = Cell::new(None));

///Sets global client in thread local storage.
pub fn set<C: client::HttpClient + 'static>(client: C) {
    CLIENT.with(move |store| store.set(Some(Box::new(client))))
}

///Sets default client as global in thread local storage.
pub fn set_default() {
    let client = client::Client::default();
    CLIENT.with(move |store| store.set(Some(Box::new(client))))
}

///Executes HTTP request on global client
pub fn execute(req: client::Request) -> client::response::FutureResponse {
    CLIENT.with(move |store| match store.replace(None) {
        Some(client) => {
            let res = client.execute(req);
            store.set(Some(client));
            res
        },
        None => panic!("Client is not set"),
    })
}

///Starts function within tokio runtime that returns future
///and waits for it to finish
///
///Yukikaze-sama uses `current_thread` runtime internally which
///is stored in `thread_local` storage.
///
///## Note
///
///It must not be used within blocking call like [run](fn.run.html)
pub fn run<R: IntoFuture, F: FnOnce() -> R>(runner: F) -> Result<R::Item, R::Error> {
    runner().into_future().finish()
}

///Spawns future on runtime's event loop.
///
///## Note
///
///It must not be used within blocking call like [run](fn.run.html)
pub fn spawn<F: Future<Item=(), Error=()> + 'static>(fut: F) {
    TOKIO.with(|rt| match rt.replace(None) {
        Some(mut tokio) => {
            tokio.spawn(fut);
            rt.set(Some(tokio));
        },
        None => panic!("Recursive call to rt is detected! Do not use it within blocking calls!"),
    });
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
    ///Runs futures to competition.
    ///
    ///Yukikaze-sama uses `current_thread` runtime internally which
    ///is stored in `thread_local` storage.
    ///
    ///## Note
    ///
    ///It must not be used within blocking call like [run](fn.run.html)
    fn finish(self) -> Result<Self::Item, Self::Error> {
        TOKIO.with(|rt| match rt.replace(None) {
            Some(mut tokio) => {
                let res = tokio.block_on(self);
                rt.set(Some(tokio));
                res
            },
            None => panic!("Recursive call to rt is detected! Do not use it within blocking calls!"),
        })
    }
}

impl<F: Future + Sized> AutoRuntime for F {}
