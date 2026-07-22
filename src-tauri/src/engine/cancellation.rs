use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use uuid::Uuid;

/// Tracks active organization sessions and exposes lightweight cancellation
/// tokens that can be checked between filesystem operations.
#[derive(Default)]
pub struct CancellationRegistry {
    sessions: DashMap<Uuid, Arc<AtomicBool>>,
}

#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl CancellationRegistry {
    pub fn begin(&self, session_id: Uuid) -> CancellationToken {
        let cancelled = Arc::new(AtomicBool::new(false));
        self.sessions.insert(session_id, cancelled.clone());
        CancellationToken { cancelled }
    }

    /// Returns `true` when the requested session was still active.
    pub fn cancel(&self, session_id: Uuid) -> bool {
        let Some(cancelled) = self.sessions.get(&session_id) else {
            return false;
        };
        cancelled.store(true, Ordering::Release);
        true
    }

    pub fn finish(&self, session_id: Uuid) {
        self.sessions.remove(&session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_visible_to_existing_tokens() {
        let registry = CancellationRegistry::default();
        let session_id = Uuid::new_v4();
        let token = registry.begin(session_id);

        assert!(!token.is_cancelled());
        assert!(registry.cancel(session_id));
        assert!(token.is_cancelled());

        registry.finish(session_id);
        assert!(!registry.cancel(session_id));
    }
}
