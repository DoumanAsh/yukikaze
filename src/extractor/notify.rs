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

use std::sync::mpsc as std_mpsc;

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
