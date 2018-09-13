//!Extractors that provides notification on progress.
//!
//!These are the same as in root module,
//!but allow to notify when each data chunk arrieves
//!with their size
//!
//!## Notifier
//!
//!The trait that describes how to send notification.
//!User may use already existing impls or create own `Notifier`
//!
//!## Example
//!
//!```rust, no_run
//!extern crate yukikaze;
//!extern crate tokio;
//!
//!use yukikaze::client::{Client, HttpClient};
//!use yukikaze::client::request::Request;
//!use yukikaze::client::response::extractor::notify::Notifier;
//!
//!struct Printer;
//!impl Notifier for Printer {
//!    fn send(&mut self, bytes: usize) {
//!        println!("Got {} bytes", bytes);
//!    }
//!}
//!
//!fn main() {
//!    let mut rt = tokio::runtime::current_thread::Runtime::new().expect("Build tokio runtime");
//!    let client = Client::default();
//!
//!    let request = Request::get("http://google.com").expect("To create get request")
//!                                                   .empty();
//!    let response = rt.block_on(client.execute(request)).expect("To get response");
//!    let body = response.body().with_notify(Printer);
//!    let body = rt.block_on(body).expect("To get body");
//!
//!    println!("Total size: {} bytes", body.len());
//!
//!}
//!```

use ::std::sync::mpsc as std_mpsc;

use ::futures::sync::mpsc;

///Describes Body download progress
pub trait Notifier {
    ///Sends data over Notifier.
    fn send(&mut self, num: usize);
}

///Noop Notifier.
///
///This one is used by default
pub struct Noop;

impl Notifier for Noop {
    #[inline]
    fn send(&mut self, _: usize) { }
}

impl Notifier for std_mpsc::Sender<usize> {
    #[inline]
    fn send(&mut self, num: usize) {
        let _ = std_mpsc::Sender::send(self, num);
    }
}

impl Notifier for mpsc::UnboundedSender<usize> {
    #[inline]
    fn send(&mut self, num: usize) {
        let _ = self.unbounded_send(num);
    }
}
