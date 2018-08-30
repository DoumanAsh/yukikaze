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

use ::client::config::Config;

use ::tokio::runtime::current_thread::{Runtime, Handle};
use ::futures::{IntoFuture, Future};

use ::std::cell::Cell;
use ::std::marker::PhantomData;

use ::client;

thread_local!(static TOKIO: Cell<Option<Runtime>> = Cell::new(None));
thread_local!(static CLIENT: Cell<Option<Box<client::HttpClient>>> = Cell::new(None));

///Guard that controls lifetime of Runtime module
///
///Currently runtime uses tokio's current_thread Runtime and therefore it cannot
///be shared between threads.
///
///On drop, it terminates runtime making it impossible to use it any longer.
pub struct Guard {
    //workaround for unstable !Send
    _dummy: PhantomData<*mut u8>
}
impl Drop for Guard {
    fn drop(&mut self) {
        TOKIO.with(|rt| rt.replace(None));
    }
}

///Initializes new runtime and returns guard that controls its lifetime.
///
///This function must be called prior to any usage of runtime related functionality:
///
///- [run](fn.run.html)
///- [handle](fn.handle.html)
///- [AutoRuntime](trait.AutoRuntime.html)
///
///## Panics
///
///If runtime is already initialized.
pub fn init() -> Guard {
    TOKIO.with(|rt| match rt.replace(None) {
        None => {
            rt.set(Some(Runtime::new().expect("To crate tokio runtime")));
        },
        Some(old) => {
            drop(old);
            panic!("Double set of runtime!")
        }
    });

    Guard {
        _dummy: PhantomData
    }
}

///Sets global client in thread local storage.
pub fn set<C: client::HttpClient + 'static>(client: C) {
    CLIENT.with(move |store| store.set(Some(Box::new(client))))
}

///Sets global client using specified config in thread local storage.
pub fn set_with_config<C: Config + 'static>() {
    let client = client::Client::<C>::new();
    set(client)
}

///Sets default client as global in thread local storage.
pub fn set_default() {
    let client = client::Client::default();
    set(client)
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
        None => panic!("Runtime is not available! Initialize it or do not use it within blocking calls!"),
    });
}

///Retrieves tokio's handle.
pub fn handle() -> Handle {
    TOKIO.with(|rt| match rt.replace(None) {
        Some(tokio) => {
            let res = tokio.handle();
            rt.set(Some(tokio));
            res
        },
        None => panic!("Runtime is not available! Initialize it or do not use it within blocking calls!"),
    })
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
            None => panic!("Runtime is not available! Initialize it or do not use it within blocking calls!"),
        })
    }
}

impl<F: Future + Sized> AutoRuntime for F {}
