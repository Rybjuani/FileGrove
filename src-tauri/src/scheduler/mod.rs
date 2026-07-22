//! Scheduler service — see SPEC §8.3.
//!
//! Wraps `tokio-cron-scheduler` to drive cron-style scheduled folders.

pub mod service;

pub use service::SchedulerService;
