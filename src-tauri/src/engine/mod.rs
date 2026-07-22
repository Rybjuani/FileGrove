//! Rule engine — condition matching, plan computation, action execution.
//!
//! See `docs/SPEC.md` §5 for the formal semantics this module implements.

#[cfg(not(test))]
pub mod executor;
pub mod matcher;
pub mod planner;
