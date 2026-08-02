use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use uuid::Uuid;

/// Tracks active organization sessions and exposes lightweight cancellation
/// tokens that can be checked between filesystem operations.
pub struct CancellationRegistry {
    accepting_work: AtomicBool,
    sessions: DashMap<Uuid, Arc<AtomicBool>>,
    active_folders: DashMap<String, Uuid>,
    session_folders: DashMap<Uuid, String>,
    active_undos: DashMap<Uuid, ()>,
}

impl Default for CancellationRegistry {
    fn default() -> Self {
        Self {
            accepting_work: AtomicBool::new(true),
            sessions: DashMap::new(),
            active_folders: DashMap::new(),
            session_folders: DashMap::new(),
            active_undos: DashMap::new(),
        }
    }
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
        if !self.accepting_work.load(Ordering::Acquire) {
            return CancellationToken {
                cancelled: Arc::new(AtomicBool::new(true)),
            };
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        self.sessions.insert(session_id, cancelled.clone());
        if !self.accepting_work.load(Ordering::Acquire) {
            cancelled.store(true, Ordering::Release);
        }
        CancellationToken { cancelled }
    }

    /// Start a session only when no other organization is active for the same
    /// canonical folder key.
    pub fn begin_for_folder(
        &self,
        session_id: Uuid,
        folder_key: String,
    ) -> Option<CancellationToken> {
        if !self.accepting_work.load(Ordering::Acquire) {
            return None;
        }
        // Register the token before publishing the folder lease. Otherwise a
        // concurrent pause/stop could observe the active folder in the tiny
        // gap before its cancellation token existed and fail to cancel it.
        let token = self.begin(session_id);
        if !self.begin_folder_operation(session_id, folder_key) {
            self.sessions.remove(&session_id);
            return None;
        }
        // Re-check after publishing both records. If shutdown raced the first
        // check, either its cancel sweep saw this token or we cancel it here.
        if !self.accepting_work.load(Ordering::Acquire) {
            token.cancelled.store(true, Ordering::Release);
            self.finish(session_id);
            return None;
        }
        Some(token)
    }

    /// Acquire the same per-folder lease used by organization sessions for a
    /// short configuration or service transition. This prevents two mode or
    /// rule changes from racing each other or an active file operation.
    pub fn begin_folder_operation(&self, operation_id: Uuid, folder_key: String) -> bool {
        if !self.accepting_work.load(Ordering::Acquire) {
            return false;
        }
        let inserted = match self.active_folders.entry(folder_key.clone()) {
            dashmap::mapref::entry::Entry::Occupied(_) => false,
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(operation_id);
                self.session_folders.insert(operation_id, folder_key);
                true
            }
        };
        if inserted && !self.accepting_work.load(Ordering::Acquire) {
            self.finish(operation_id);
            return false;
        }
        inserted
    }

    /// Returns `true` when the requested session was still active.
    pub fn cancel(&self, session_id: Uuid) -> bool {
        let Some(cancelled) = self.sessions.get(&session_id) else {
            return false;
        };
        cancelled.store(true, Ordering::Release);
        true
    }

    /// Cancel the organization session currently holding a folder lease.
    /// Folder keys must use the same canonical spelling supplied to
    /// `begin_for_folder`.
    pub fn cancel_folder(&self, folder_key: &str) -> bool {
        let Some(active_session) = self.active_folders.get(folder_key) else {
            return false;
        };
        let session_id = *active_session;
        drop(active_session);
        self.cancel(session_id)
    }

    /// Signal every cancellable organization session at once. This is used
    /// during application shutdown after watcher producers have
    /// been disabled, so no new background work should be published.
    pub fn cancel_all(&self) -> usize {
        let mut cancelled = 0;
        for session in &self.sessions {
            if !session.swap(true, Ordering::AcqRel) {
                cancelled += 1;
            }
        }
        cancelled
    }

    /// Permanently reject new work and cancel every session already
    /// registered. A registry belongs to one application process, so shutdown
    /// is intentionally one-way.
    pub fn begin_shutdown(&self) -> usize {
        self.accepting_work.store(false, Ordering::Release);
        self.cancel_all()
    }

    /// Whether any filesystem operation still owns a cancellation token,
    /// folder lease, or undo lease. Shutdown only uses this as a bounded drain
    /// hint; it never waits indefinitely for a stuck blocking filesystem call.
    pub fn has_active_work(&self) -> bool {
        !self.sessions.is_empty()
            || !self.session_folders.is_empty()
            || !self.active_undos.is_empty()
    }

    pub fn is_folder_active(&self, folder_key: &str) -> bool {
        self.active_folders.contains_key(folder_key)
    }

    pub fn begin_undo(&self, session_id: Uuid) -> bool {
        if !self.accepting_work.load(Ordering::Acquire) {
            return false;
        }
        let inserted = match self.active_undos.entry(session_id) {
            dashmap::mapref::entry::Entry::Occupied(_) => false,
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(());
                true
            }
        };
        if inserted && !self.accepting_work.load(Ordering::Acquire) {
            self.active_undos.remove(&session_id);
            return false;
        }
        inserted
    }

    pub fn finish_undo(&self, session_id: Uuid) {
        self.active_undos.remove(&session_id);
    }

    pub fn finish(&self, session_id: Uuid) {
        // Keep the token reachable until the folder lease is unpublished.
        // This makes `cancel_folder` linearizable with a finishing session:
        // if it can still see the lease, it can still flip that token.
        if let Some(folder_key) = self
            .session_folders
            .get(&session_id)
            .map(|entry| entry.clone())
        {
            let belongs_to_session = self
                .active_folders
                .get(&folder_key)
                .is_some_and(|active| *active == session_id);
            if belongs_to_session {
                self.active_folders.remove(&folder_key);
            }
            self.session_folders.remove(&session_id);
        }
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

    #[test]
    fn prevents_overlapping_sessions_for_one_folder() {
        let registry = CancellationRegistry::default();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert!(registry
            .begin_for_folder(first, "c:/watched".into())
            .is_some());
        assert!(registry
            .begin_for_folder(second, "c:/watched".into())
            .is_none());
        registry.finish(first);
        assert!(registry
            .begin_for_folder(second, "c:/watched".into())
            .is_some());
    }

    #[test]
    fn configuration_lease_blocks_organization_and_other_mutations() {
        let registry = CancellationRegistry::default();
        let mutation = Uuid::new_v4();
        let organization = Uuid::new_v4();
        let key = "c:/watched".to_string();

        assert!(registry.begin_folder_operation(mutation, key.clone()));
        assert!(!registry.begin_folder_operation(Uuid::new_v4(), key.clone()));
        assert!(registry
            .begin_for_folder(organization, key.clone())
            .is_none());
        registry.finish(mutation);
        assert!(registry.begin_for_folder(organization, key).is_some());
    }

    #[test]
    fn cancel_folder_reaches_the_active_session_token() {
        let registry = CancellationRegistry::default();
        let session_id = Uuid::new_v4();
        let key = "c:/watched".to_string();
        let token = registry.begin_for_folder(session_id, key.clone()).unwrap();

        assert!(registry.cancel_folder(&key));
        assert!(token.is_cancelled());
        assert!(!registry.cancel_folder("c:/another-folder"));

        registry.finish(session_id);
        assert!(!registry.cancel_folder(&key));
    }

    #[test]
    fn cancel_all_signals_every_session_and_active_work_drains() {
        let registry = CancellationRegistry::default();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let first_token = registry.begin_for_folder(first, "c:/first".into()).unwrap();
        let second_token = registry
            .begin_for_folder(second, "c:/second".into())
            .unwrap();

        assert!(registry.has_active_work());
        assert_eq!(registry.cancel_all(), 2);
        assert_eq!(registry.cancel_all(), 0);
        assert!(first_token.is_cancelled());
        assert!(second_token.is_cancelled());

        registry.finish(first);
        assert!(registry.has_active_work());
        registry.finish(second);
        assert!(!registry.has_active_work());
    }

    #[test]
    fn shutdown_rejects_new_sessions_and_folder_operations() {
        let registry = CancellationRegistry::default();
        registry.begin_shutdown();

        assert!(registry
            .begin_for_folder(Uuid::new_v4(), "c:/late".into())
            .is_none());
        assert!(!registry.begin_folder_operation(Uuid::new_v4(), "c:/late".into()));
        assert!(!registry.begin_undo(Uuid::new_v4()));
        assert!(registry.begin(Uuid::new_v4()).is_cancelled());
        assert!(!registry.has_active_work());
    }
}
