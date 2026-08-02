//! Bounded, deduplicated watcher-event accumulator.

use std::collections::HashSet;
use std::path::PathBuf;

use parking_lot::Mutex;

pub struct CoalescedPaths {
    limit: usize,
    state: Mutex<PendingState>,
}

#[derive(Default)]
struct PendingState {
    paths: HashSet<PathBuf>,
    requires_catch_up: bool,
}

pub struct CoalescedBatch {
    pub paths: Vec<PathBuf>,
    pub requires_catch_up: bool,
}

impl CoalescedPaths {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            state: Mutex::new(PendingState::default()),
        }
    }

    pub fn try_extend(&self, paths: impl IntoIterator<Item = PathBuf>) -> bool {
        let mut state = self.state.lock();
        if state.requires_catch_up {
            return false;
        }
        for path in paths {
            state.paths.insert(path);
            if state.paths.len() > self.limit {
                state.paths.clear();
                state.requires_catch_up = true;
                return false;
            }
        }
        true
    }

    pub fn mark_catch_up_required(&self) {
        let mut state = self.state.lock();
        state.paths.clear();
        state.requires_catch_up = true;
    }

    pub fn clear(&self) {
        *self.state.lock() = PendingState::default();
    }

    pub fn take(&self) -> CoalescedBatch {
        let mut state = self.state.lock();
        let current = std::mem::take(&mut *state);
        CoalescedBatch {
            paths: current.paths.into_iter().collect(),
            requires_catch_up: current.requires_catch_up,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_and_switches_to_catch_up_when_full() {
        let pending = CoalescedPaths::new(2);
        assert!(pending.try_extend([PathBuf::from("a"), PathBuf::from("a")]));
        assert!(pending.try_extend([PathBuf::from("b")]));
        assert!(!pending.try_extend([PathBuf::from("c")]));
        let batch = pending.take();
        assert!(batch.paths.is_empty());
        assert!(batch.requires_catch_up);
        assert!(pending.take().paths.is_empty());
    }
}
