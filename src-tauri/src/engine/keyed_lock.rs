//! Small keyed async-lock registry.
//!
//! Weak references keep the registry from retaining one mutex forever for
//! every file or folder the application has ever seen.

use std::hash::Hash;
use std::sync::{Arc, Weak};

use dashmap::DashMap;
use tokio::sync::Mutex;

pub struct KeyedAsyncLock<K> {
    locks: DashMap<K, Weak<Mutex<()>>>,
}

impl<K> Default for KeyedAsyncLock<K>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self {
            locks: DashMap::new(),
        }
    }
}

impl<K> KeyedAsyncLock<K>
where
    K: Eq + Hash + Clone,
{
    pub fn for_key(&self, key: K) -> Arc<Mutex<()>> {
        match self.locks.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                if let Some(lock) = entry.get().upgrade() {
                    return lock;
                }
                let lock = Arc::new(Mutex::new(()));
                entry.insert(Arc::downgrade(&lock));
                lock
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let lock = Arc::new(Mutex::new(()));
                entry.insert(Arc::downgrade(&lock));
                lock
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_live_locks_and_reclaims_unused_ones() {
        let locks = KeyedAsyncLock::<String>::default();
        let first = locks.for_key("folder".into());
        let second = locks.for_key("folder".into());
        assert!(Arc::ptr_eq(&first, &second));
        drop(first);
        drop(second);

        let replacement = locks.for_key("folder".into());
        assert_eq!(Arc::strong_count(&replacement), 1);
    }
}
