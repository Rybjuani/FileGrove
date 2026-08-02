//! Rule engine — condition matching, plan computation, action execution.
//!
//! See `docs/SPEC.md` §5 for the formal semantics this module implements.

pub mod cancellation;
pub mod command;
pub mod executor;
pub mod file_ops;
pub mod keyed_lock;
pub mod matcher;
pub mod planner;
pub mod scanner;
pub mod undo;
