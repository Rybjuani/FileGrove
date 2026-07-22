//! Domain types — entities that the application operates on.
//!
//! These are pure data structures with no business logic. They are
//! serializable (serde) so they can cross the IPC boundary directly.

pub mod action;
pub mod activity;
pub mod condition;
pub mod folder;
pub mod rule;
pub mod schedule;
pub mod undo;

pub use action::*;
pub use condition::*;
pub use folder::*;
pub use rule::*;
pub use schedule::*;
