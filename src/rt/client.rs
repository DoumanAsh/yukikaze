//!Global client module

use crate::client::HttpClient;
use crate::client::config::Config;

use ::std::sync::atomic::{AtomicUsize, Ordering};

//Not set yet
const UNINITIALIZED: usize = 0;
//Being set
const INITIALIZING: usize = 1;
//Set
const INITIALIZED: usize = 2;

static GLOBAL_GUARD: AtomicUsize = AtomicUsize::new(UNINITIALIZED);

struct DummyClient;

impl HttpClient for DummyClient {
    fn execute(&self, _: crate::client::request::Request) -> crate::client::response::Future {
        panic!("Dummy client is used")
    }

    fn with_redirect(&self, _: crate::client::request::Request) -> crate::client::response::RedirectFuture {
        panic!("Dummy client is used")
    }

    fn execute_raw_hyper(&self, _: crate::client::request::HyperRequest) -> hyper::client::ResponseFuture {
        panic!("Dummy client is used")
    }
}

static DUMMY: DummyClient = DummyClient;
static mut GLOBAL_CLIENT: &'static HttpClient = &DUMMY;

///Global client guard.
///
///When it goes out of scope, client is removed.
pub struct GlobalClient {
    inner: &'static HttpClient
}

impl GlobalClient {
    ///Creates new global guard by taking provided one.
    ///
    ///## Panics
    ///
    ///Settings client more than once.
    pub fn new<C: HttpClient + 'static>(client: C) -> Self {
        match GLOBAL_GUARD.compare_and_swap(UNINITIALIZED, INITIALIZING, Ordering::Release) {
            UNINITIALIZED => unsafe {
                let inner = Box::leak(Box::new(client));
                let result = Self {
                    inner,
                };

                GLOBAL_CLIENT = result.inner;
                GLOBAL_GUARD.store(INITIALIZED, Ordering::SeqCst);

                result
            },
            _ => panic!("Setting client twice")
        }
    }

    ///Sets global client using specified config.
    ///
    ///## Panics
    ///
    ///Settings client more than once.
    pub fn with_config<C: Config + 'static>() -> Self {
        Self::new(crate::client::Client::<C>::new())
    }
}

impl Default for GlobalClient {
    fn default() -> Self {
        Self::new(crate::client::Client::default())
    }
}

impl Drop for GlobalClient {
    fn drop(&mut self) {
        match GLOBAL_GUARD.compare_and_swap(INITIALIZED, INITIALIZING, Ordering::Release) {
            INITIALIZED => unsafe {
                GLOBAL_CLIENT = &DUMMY;
                GLOBAL_GUARD.store(UNINITIALIZED, Ordering::SeqCst);

                Box::from_raw(self.inner as *const HttpClient as *mut HttpClient);
            },
            _ => panic!("Client is not set, but dropping global guard!")
        }
    }
}

///Trait to bootstrap your requests.
pub trait AutoClient {
    ///Sends request using default client.
    fn send(self) -> crate::client::response::Future;

    ///Sends request using default client with redirect support.
    fn send_with_redirect(self) -> crate::client::response::RedirectFuture;
}

impl AutoClient for crate::client::Request {
    #[inline]
    fn send(self) -> crate::client::response::Future {
        execute(self)
    }

    #[inline]
    fn send_with_redirect(self) -> crate::client::response::RedirectFuture {
        execute_with_redirect(self)
    }
}

///Executes HTTP request on global client
pub fn execute(req: crate::client::Request) -> crate::client::response::Future {
    match GLOBAL_GUARD.load(Ordering::Acquire) {
        INITIALIZED => unsafe { GLOBAL_CLIENT.execute(req) },
        _ => panic!("Client is not set")
    }
}

///Executes HTTP request on global client with redirect supprot
pub fn execute_with_redirect(req: crate::client::Request) -> crate::client::response::RedirectFuture {
    match GLOBAL_GUARD.load(Ordering::Acquire) {
        INITIALIZED => unsafe { GLOBAL_CLIENT.with_redirect(req) },
        _ => panic!("Client is not set")
    }
}

pub(crate) fn execute_raw_hyper(req: crate::client::request::HyperRequest) -> ::hyper::client::ResponseFuture {
    match GLOBAL_GUARD.load(Ordering::Acquire) {
        INITIALIZED => unsafe { GLOBAL_CLIENT.execute_raw_hyper(req) },
        _ => panic!("Client is not set")
    }
}
