//!Tokio runtime module

use tokio::runtime::current_thread::{Runtime, Handle};
use futures::{IntoFuture, Future};

use std::cell::Cell;
use std::marker::PhantomData;

const RUNTIME_NOT_AVAIL: &'static str = "Runtime is not available! Initialize it or do not use it within blocking calls!";

thread_local!(static TOKIO: Cell<Option<Runtime>> = Cell::new(None));

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
    TOKIO.with(|rt| match unsafe { &mut *rt.as_ptr() } {
        Some(tokio) => tokio.spawn(fut),
        None => panic!(RUNTIME_NOT_AVAIL)
    });
}

///Retrieves tokio's handle.
pub fn handle() -> Handle {
    TOKIO.with(|rt| match unsafe { &*rt.as_ptr() } {
        Some(tokio) => tokio.handle(),
        None => panic!(RUNTIME_NOT_AVAIL)
    })
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
        TOKIO.with(|rt| match unsafe { &mut *rt.as_ptr() } {
            Some(tokio) => tokio.block_on(self),
            None => panic!(RUNTIME_NOT_AVAIL)
        })
    }
}

impl<F: Future + Sized> AutoRuntime for F {}
