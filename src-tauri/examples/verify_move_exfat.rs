use filegrove_lib::integration_test_support::*;
use std::path::PathBuf;
use uuid::Uuid;
use chrono::Utc;

#[tokio::main]
async fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "F:\\fg-t5-test".into()));
    std::fs::create_dir_all(&root).unwrap();
    for (name, bytes) in [("a.jpg", &b"jpg"[..]), ("b.txt", &b"txt"[..]), ("c.mp3", &b"mp3"[..])] {
        std::fs::write(root.join(name), bytes).unwrap();
    }
    let db_path = root.join("verify.db");
    let _ = std::fs::remove_file(&db_path);
    let db = Database::open(&db_path).unwrap();
    db.run_migrations().unwrap();
    let rule_id = Uuid::new_v4();
    db.rules().insert(&Rule {
        id: rule_id, name: "Imagenes".into(), is_quick_rule: false, enabled: true,
        order: 0, icon: "Image".into(),
        conditions: vec![Condition::Extension { value: vec![".jpg".into(), ".txt".into(), ".mp3".into()] }],
        action: Action::Move { destination: "Sorted".into() },
        created_at: Utc::now(),
    }).unwrap();
    db.folders().insert(&Folder {
        id: Uuid::new_v4(), path: root.to_string_lossy().into_owned(), name: "verify".into(),
        mode: FolderMode::Single, rule_ids: vec![rule_id], watcher_status: WatcherStatus::Stopped,
        created_at: Utc::now(), subfolders: SubfolderConfig::default(),
        exclude_folders: vec![], exclude_patterns: vec![],
    }).unwrap();
    let organizer = Organizer {
        db: &db, session_id: Uuid::new_v4(), scan_root: root.clone(),
        conflict_behavior: ConflictBehavior::Rename, duplicate_behavior: DuplicateBehavior::KeepBoth,
        preserve_relative_structure: false, cancellation: None,
    };
    for name in ["a.jpg", "b.txt", "c.mp3"] {
        let p = root.join(name);
        let file = FileItem::from_path_sync(p.to_str().unwrap()).unwrap();
        let res = organizer.execute(&file, &Action::Move { destination: "Sorted".into() }, Some(rule_id), Some("Imagenes")).await;
        println!("{name}: success={} skipped={} error={:?} new={:?}", res.success, res.skipped, res.error, res.new_path);
    }
}
