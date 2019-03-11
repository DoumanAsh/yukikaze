//!Tokio multi-threaded runtime

use futures::{IntoFuture, Future};
use tokio::runtime;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::io;

//Not set yet
const UNINITIALIZED: usize = 0;
//Being set
const INITIALIZING: usize = 1;
//Set
const INITIALIZED: usize = 2;

static GLOBAL_GUARD: AtomicUsize = AtomicUsize::new(UNINITIALIZED);
static mut TOKIO: Option<runtime::Runtime> = None;
const RUNTIME_NOT_AVAIL: &str = "Runtime is not set";

///Tokio runtime guard
///
///Runtime gets terminated as soon as it goes out of scope
pub struct Runtime {
}

impl Runtime {
    ///Creates new instance with default configuration
    pub fn new() -> io::Result<Self> {
        Self::from_build(runtime::Builder::new().name_prefix("yukikaze"))
    }

    ///Creates new instance from provided builder
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
    pub fn from_build(builder: &mut runtime::Builder) -> io::Result<Self> {
        let runtime = builder.build()?;
        match GLOBAL_GUARD.compare_and_swap(UNINITIALIZED, INITIALIZING, Ordering::Release) {
            UNINITIALIZED => unsafe {
                TOKIO = Some(runtime);
                GLOBAL_GUARD.store(INITIALIZED, Ordering::SeqCst);
            },
            _ => panic!("Setting tokio runtime twice")
        }

        Ok(Self {
        })
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        match GLOBAL_GUARD.compare_and_swap(INITIALIZED, INITIALIZING, Ordering::Release) {
            INITIALIZED => unsafe {
                match TOKIO.take() {
                    Some(runtime) => {
                        runtime.shutdown_now();
                    },
                    None => unreach!(),
                }

                GLOBAL_GUARD.store(UNINITIALIZED, Ordering::SeqCst);
            },
            _ => panic!("Runtime is not set, but dropping global guard!"),
        }
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
pub fn init() -> Runtime {
    Runtime::new().expect("To create runtime")
}

///Starts function within tokio runtime that returns future
///and waits for it to finish
///
///Yukikaze-sama uses global runtime to work on your futures
///
///## Note
///
///It must not be used within blocking call like [run](fn.run.html)
pub fn run<F, R, IF, I, E>(runner: F) -> Result<R::Item, R::Error>
    where F: FnOnce() -> R,
          R: IntoFuture<Future=IF, Item=I, Error=E>,
          IF: Future<Item=I, Error=E> + Send + 'static,
          I: Send + 'static,
          E: Send + 'static,
{
    runner().into_future().finish()
}

///Spawns future on runtime's event loop.
///
///## Note
///
///It must not be used within blocking call like [run](fn.run.html)
pub fn spawn<F: Future<Item=(), Error=()> + 'static + Send>(fut: F) {
    match GLOBAL_GUARD.load(Ordering::Acquire) {
        INITIALIZED => unsafe { match TOKIO.as_mut() {
            Some(runtime) => {
                runtime.spawn(fut);
            },
            None => unreach!(),
        }},
        _ => panic!(RUNTIME_NOT_AVAIL)
    }
}

///Trait to bootstrap your futures.
pub trait AutoRuntime: Future {
    ///Runs futures to competition.
    ///
    ///Yukikaze-sama uses global runtime to work on your futures
    ///
    ///## Note
    ///
    ///It must not be used within blocking call like [run](fn.run.html)
    fn finish(self) -> Result<Self::Item, Self::Error>;
}

impl<F: Send + 'static + Future<Item=I, Error=E>, I: Send + 'static, E: Send + 'static> AutoRuntime for F {
    fn finish(self) -> Result<Self::Item, Self::Error> {
        match GLOBAL_GUARD.load(Ordering::Acquire) {
            INITIALIZED => unsafe { match TOKIO.as_mut() {
                Some(runtime) => runtime.block_on(self),
                None => unreach!(),
            }},
            _ => panic!(RUNTIME_NOT_AVAIL)
        }
    }
}
