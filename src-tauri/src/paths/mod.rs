//! Path templates — see SPEC §5.5.
//!
//! Destination paths in `Action::Move` may contain tokens like `{year}`,
//! `{month}`, `{camera}`, etc. This module resolves those tokens against
//! a file's metadata.

pub mod safety;
pub mod tokens;
