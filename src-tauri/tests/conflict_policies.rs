#[cfg(target_os = "linux")]
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use filegrove_lib::integration_test_support::{
    undo_entries, Action, ActionResult, ActivityFilters, ActivityType, Condition, ConflictBehavior,
    Database, DuplicateBehavior, FileItem, Folder, FolderMode, Organizer, Rule, SubfolderConfig,
    WatcherStatus,
};
use tempfile::TempDir;
use uuid::Uuid;

struct PolicyFixture {
    #[allow(dead_code)]
    temporary: TempDir,
    watched: PathBuf,
    destination: PathBuf,
    db: Database,
    rule_id: Uuid,
    action: Action,
}

impl PolicyFixture {
    fn new(source_contents: &[u8], destination_contents: &[u8]) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let watched = temporary.path().join("watched");
        let destination = watched.join("Documents");
        fs::create_dir_all(&destination).unwrap();
        fs::write(watched.join("report.txt"), source_contents).unwrap();
        fs::write(destination.join("report.txt"), destination_contents).unwrap();

        let db = Database::open(&temporary.path().join("app.db")).unwrap();
        db.run_migrations().unwrap();
        let rule_id = Uuid::new_v4();
        let action = Action::Move {
            destination: "Documents".into(),
        };
        db.rules()
            .insert(&Rule {
                id: rule_id,
                name: "Documents".into(),
                is_quick_rule: false,
                enabled: true,
                order: 0,
                icon: "File".into(),
                conditions: vec![Condition::Extension {
                    value: vec![".txt".into()],
                }],
                action: action.clone(),
                created_at: Utc::now(),
            })
            .unwrap();
        db.folders()
            .insert(&Folder {
                id: Uuid::new_v4(),
                path: watched.to_string_lossy().into_owned(),
                name: "Watched".into(),
                mode: FolderMode::Single,
                rule_ids: vec![rule_id],
                watcher_status: WatcherStatus::Stopped,
                created_at: Utc::now(),
                subfolders: SubfolderConfig::default(),
                exclude_folders: Vec::new(),
                exclude_patterns: Vec::new(),
            })
            .unwrap();

        Self {
            temporary,
            watched,
            destination,
            db,
            rule_id,
            action,
        }
    }

    fn source(&self) -> PathBuf {
        self.watched.join("report.txt")
    }

    fn requested_destination(&self) -> PathBuf {
        self.destination.join("report.txt")
    }

    fn renamed_destination(&self) -> PathBuf {
        self.destination.join("report (1).txt")
    }

    #[cfg(target_os = "linux")]
    fn isolated_data_home(&self) -> PathBuf {
        self.temporary.path().join("xdg-data")
    }

    async fn execute(
        &self,
        conflict_behavior: ConflictBehavior,
        duplicate_behavior: DuplicateBehavior,
    ) -> (Uuid, ActionResult) {
        let session_id = Uuid::new_v4();
        let file = FileItem::from_path_sync(self.source().to_string_lossy().as_ref()).unwrap();
        let result = Organizer {
            db: &self.db,
            session_id,
            scan_root: self.watched.clone(),
            conflict_behavior,
            duplicate_behavior,
            preserve_relative_structure: false,
            cancellation: None,
        }
        .execute(&file, &self.action, Some(self.rule_id), Some("Documents"))
        .await;
        (session_id, result)
    }

    fn only_activity_type(&self) -> ActivityType {
        let (activity, total) = self
            .db
            .activity()
            .list(&ActivityFilters::default(), 10, 0)
            .unwrap();
        assert_eq!(total, 1);
        assert!(activity[0].success, "{:?}", activity[0].error);
        activity[0].r#type
    }
}

#[cfg(target_os = "linux")]
struct EnvironmentGuard {
    key: &'static str,
    previous: Option<OsString>,
}

#[cfg(target_os = "linux")]
impl EnvironmentGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

#[cfg(target_os = "linux")]
impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn assert_contents(path: impl AsRef<Path>, expected: &[u8]) {
    assert_eq!(fs::read(path).unwrap(), expected);
}

#[tokio::test]
async fn every_conflict_and_duplicate_policy_preserves_its_safety_contract() {
    let rename = PolicyFixture::new(b"new", b"old");
    let (session_id, result) = rename
        .execute(ConflictBehavior::Rename, DuplicateBehavior::KeepBoth)
        .await;
    assert!(result.success, "{:?}", result.error);
    assert!(!rename.source().exists());
    assert_contents(rename.requested_destination(), b"old");
    assert_contents(rename.renamed_destination(), b"new");
    assert_eq!(rename.db.undo().stats(session_id).unwrap().entry_count, 1);

    let skip = PolicyFixture::new(b"new", b"old");
    let (session_id, result) = skip
        .execute(ConflictBehavior::Skip, DuplicateBehavior::KeepBoth)
        .await;
    assert!(result.success, "{:?}", result.error);
    assert!(result.skipped);
    assert_contents(skip.source(), b"new");
    assert_contents(skip.requested_destination(), b"old");
    assert_eq!(skip.db.undo().stats(session_id).unwrap().entry_count, 0);

    let overwrite = PolicyFixture::new(b"new", b"old");
    let (session_id, result) = overwrite
        .execute(ConflictBehavior::Overwrite, DuplicateBehavior::KeepBoth)
        .await;
    assert!(result.success, "{:?}", result.error);
    assert!(!overwrite.source().exists());
    assert_contents(overwrite.requested_destination(), b"new");
    let entries = overwrite.db.undo().read_reverse(session_id).unwrap();
    assert_eq!(entries.len(), 1);
    let backup = entries[0].backup_path.clone().unwrap();
    assert_contents(&backup, b"old");
    let undo = undo_entries(&overwrite.db, entries, |_| {}).await;
    assert_eq!((undo.done, undo.failed), (1, 0));
    assert_contents(overwrite.source(), b"new");
    assert_contents(overwrite.requested_destination(), b"old");
    assert!(!backup.exists());

    let same_size_but_different = PolicyFixture::new(b"new", b"old");
    let (_, result) = same_size_but_different
        .execute(ConflictBehavior::Rename, DuplicateBehavior::Delete)
        .await;
    assert!(result.success, "{:?}", result.error);
    assert_contents(same_size_but_different.requested_destination(), b"old");
    assert_contents(same_size_but_different.renamed_destination(), b"new");
    assert_eq!(
        same_size_but_different.only_activity_type(),
        ActivityType::Move
    );

    let keep_both = PolicyFixture::new(b"identical", b"identical");
    let (session_id, result) = keep_both
        .execute(ConflictBehavior::Overwrite, DuplicateBehavior::KeepBoth)
        .await;
    assert!(result.success, "{:?}", result.error);
    assert_contents(keep_both.requested_destination(), b"identical");
    assert_contents(keep_both.renamed_destination(), b"identical");
    assert_eq!(
        keep_both.db.undo().stats(session_id).unwrap().entry_count,
        1
    );

    let delete = PolicyFixture::new(b"identical", b"identical");
    let (session_id, result) = delete
        .execute(ConflictBehavior::Rename, DuplicateBehavior::Delete)
        .await;
    assert!(result.success, "{:?}", result.error);
    assert!(!delete.source().exists());
    assert_contents(delete.requested_destination(), b"identical");
    assert_eq!(delete.db.undo().stats(session_id).unwrap().entry_count, 0);
    assert_eq!(delete.only_activity_type(), ActivityType::Delete);

    let trash = PolicyFixture::new(b"identical", b"identical");
    #[cfg(target_os = "linux")]
    let data_home = trash.isolated_data_home();
    #[cfg(target_os = "linux")]
    fs::create_dir_all(&data_home).unwrap();
    #[cfg(target_os = "linux")]
    let environment = EnvironmentGuard::set_path("XDG_DATA_HOME", &data_home);
    let (session_id, result) = trash
        .execute(ConflictBehavior::Rename, DuplicateBehavior::Trash)
        .await;
    assert!(result.success, "{:?}", result.error);
    assert!(!trash.source().exists());
    assert_contents(trash.requested_destination(), b"identical");
    assert_eq!(trash.db.undo().stats(session_id).unwrap().entry_count, 0);
    assert_eq!(trash.only_activity_type(), ActivityType::Trash);
    #[cfg(target_os = "linux")]
    {
        assert_contents(data_home.join("Trash/files/report.txt"), b"identical");
        drop(environment);
    }
}
