//!Client runtime
//!
//!Due to limitation of `async fn`, the global client is provided by means of macro
//![declare_global_client](../../macro.declare_global_client.html).
//!The macro defines global client in current scope, alongside companion `Request` wrapper and `GlobalRequest` trait.
//!Refer to macro documentation for details.
//!
//!## Usage
//!
//!```rust,no_run
//!#![feature(async_await)]
//!use yukikaze::client::Request;
//!
//!mod generated {
//!    use yukikaze::client;
//!    use core::time;
//!
//!    pub struct TimeoutCfg;
//!
//!    impl client::config::Config for TimeoutCfg {
//!        type Connector = client::config::DefaultConnector;
//!        type Timer = client::config::DefaultTimer;
//!
//!        fn timeout() -> time::Duration {
//!            time::Duration::from_millis(50)
//!        }
//!    }
//!
//!    yukikaze::declare_global_client!(TimeoutCfg);
//!}
//!
//!use generated::{GlobalRequest};
//!
//!async fn google() {
//!    let res = Request::get("https://google.com").expect("To create get request")
//!                                                .empty()
//!                                                .global()
//!                                                .send();
//!    let result = yukikaze::matsu!(res).expect("To get without timeout")
//!                                       .expect("Successful response");
//!    assert!(result.is_success());
//!}
//!```

#[macro_export]
///Declares global client for use.
///
///If no argument is specified, uses [`DefaultCfg`](client/config/struct.DefaultCfg.html)
///Otherwise you must provide accessible type of unit struct that implements [`Config`](client/config/trait.Config.html)
///
///Creates following:
///
///- `GLOBAL_CLIENT` which is initialized using `lazy_static`
///- `Request` which uses `GLOBAL_CLIENT` and wraps `yukikaze::client::Request`
///- Creates and defines trait `GlobalRequest` for generated `Request`.
///
///See example of [generated](rt/client/generated/struct.Request.html)
///
///See [usage](rt/client/index.html)
macro_rules! declare_global_client {
    ($config:ty) => {
        $crate::lazy_static::lazy_static! {
            ///Global client instance
            pub static ref GLOBAL_CLIENT: $crate::client::Client::<$config> = $crate::client::Client::<$config>::new();
        }

        ///Global request
        ///
        ///Implements `Deref` and `DerefMut` to access regular yukikaze's request.
        pub struct Request(pub $crate::client::Request);

        impl core::ops::Deref for Request {
            type Target = $crate::client::Request;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl core::ops::DerefMut for Request {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        ///Helper trait to convert request to global request.
        pub trait GlobalRequest {
            ///Type of global request
            type Result;

            ///Wraps request into global request
            fn global(self) -> Self::Result;
        }

        impl GlobalRequest for $crate::client::Request {
            type Result = Request;

            fn global(self) -> Self::Result {
                Request(self)
            }
        }

        use $crate::client::RequestResult;

        impl Request {
            #[inline(always)]
            ///Sends request, and returns future that resolves to response
            pub fn request(self) -> impl core::future::Future<Output=RequestResult> {
                GLOBAL_CLIENT.request(self.0)
            }

            #[inline(always)]
            ///Sends request and returns response. Timed version.
            ///
            ///On timeout error it returns `async_timer::timed::Expired` as `Error`
            ///`Expired` implements `Future` that can be used to re-spawn ongoing request again.
            ///
            ///If request resolves in time returns `Result<response::Response, hyper::Error>` as `Ok`
            ///variant.
            pub fn send(self) -> impl core::future::Future<Output=Result<RequestResult, $crate::async_timer::timed::Expired<impl core::future::Future<Output=RequestResult>, impl $crate::async_timer::Oneshot>>> {
                GLOBAL_CLIENT.send(self.0)
            }

            #[inline(always)]
            ///Sends request and returns response, while handling redirects. Timed version.
            ///
            ///On timeout error it returns `async_timer::timed::Expired` as `Error`
            ///`Expired` implements `Future` that can be used to re-spawn ongoing request again.
            ///
            ///If request resolves in time returns `Result<response::Response, hyper::Error>` as `Ok`
            ///variant.
            pub fn send_redirect(self) -> impl core::future::Future<Output=Result<RequestResult, $crate::async_timer::timed::Expired<impl core::future::Future<Output=RequestResult> + 'static, impl $crate::async_timer::Oneshot>>> {
                GLOBAL_CLIENT.send_redirect(self.0)
            }

            #[inline(always)]
            ///Sends request and returns response, while handling redirects.
            pub fn redirect_request(self) -> impl core::future::Future<Output=RequestResult> {
                GLOBAL_CLIENT.redirect_request(self.0)
            }
        }
    };
    () => {
        use $crate::client::config::DefaultCfg;
        declare_global_client!(DefaultCfg);
    };
}

#[cfg(feature = "docs")]
///Example of generated global client
pub mod generated {
    declare_global_client!();
}
