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
use ::std::sync::atomic::{AtomicUsize, Ordering};

use ::client;

thread_local!(static TOKIO: Cell<Option<Runtime>> = Cell::new(None));

//Not set yet
const UNINITIALIZED: usize = 0;
//Being set
const INITIALIZING: usize = 1;
//Set
const INITIALIZED: usize = 2;

static GLOBAL_GUARD: AtomicUsize = AtomicUsize::new(UNINITIALIZED);
static mut GLOBAL_CLIENT: Option<Box<client::HttpClient + 'static + Sync>> = None;

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

///Sets global client.
///
///## Panics
///
///Settings client more than once.
pub fn set<C: client::HttpClient + 'static + Sync>(client: C) {
    match GLOBAL_GUARD.compare_and_swap(UNINITIALIZED, INITIALIZING, Ordering::Release) {
        UNINITIALIZED => unsafe {
            GLOBAL_CLIENT = Some(Box::new(client));
            GLOBAL_GUARD.store(INITIALIZED, Ordering::SeqCst);
        },
        _ => panic!("Setting client twice")
    }
}

///Sets global client using specified config.
///
///## Panics
///
///Settings client more than once.
pub fn set_with_config<C: Config + Sync + 'static>() {
    let client = client::Client::<C>::new();
    set(client)
}

///Sets default client as global.
///
///## Panics
///
///Settings client more than once.
pub fn set_default() {
    let client = client::Client::default();
    set(client)
}

///Executes HTTP request on global client
pub fn execute(req: client::Request) -> client::response::Future {
    match GLOBAL_GUARD.load(Ordering::Acquire) {
        INITIALIZED => unsafe { match GLOBAL_CLIENT.as_ref() {
            Some(ref client) => client.execute(req),
            None => unreach!(),
        }},
        _ => panic!("Client is not set")
    }
}

///Executes HTTP request on global client with redirect supprot
pub fn execute_with_redirect(req: client::Request) -> client::response::RedirectFuture {
    match GLOBAL_GUARD.load(Ordering::Acquire) {
        INITIALIZED => unsafe { match GLOBAL_CLIENT.as_ref() {
            Some(ref client) => client.with_redirect(req),
            None => unreach!(),
        }},
        _ => panic!("Client is not set")
    }
}

pub(crate) fn execute_raw_hyper(req: client::request::HyperRequest) -> ::hyper::client::ResponseFuture {
    match GLOBAL_GUARD.load(Ordering::Acquire) {
        INITIALIZED => unsafe { match GLOBAL_CLIENT.as_ref() {
            Some(ref client) => client.execute_raw_hyper(req),
            None => unreach!(),
        }},
        _ => panic!("Client is not set")
    }
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
    fn send(self) -> client::response::Future;

    ///Sends request using default client with redirect support.
    fn send_with_redirect(self) -> client::response::RedirectFuture;
}

impl AutoClient for client::Request {
    #[inline]
    fn send(self) -> client::response::Future {
        execute(self)
    }

    #[inline]
    fn send_with_redirect(self) -> client::response::RedirectFuture {
        execute_with_redirect(self)
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
