//! Filesystem watcher service — wraps `notify-debouncer-full`.

pub mod filters;
#[cfg(not(test))]
pub mod service;

#[cfg(not(test))]
pub use service::WatcherService;
