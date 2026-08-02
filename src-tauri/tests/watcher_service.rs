use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use filegrove_lib::integration_test_support::{
    Action, CancellationRegistry, Condition, Database, Folder, FolderMode, Rule, SubfolderConfig,
    WatcherService, WatcherStatus,
};
use serde_json::Value;
use tempfile::TempDir;
use uuid::Uuid;

struct Fixture {
    _temporary: TempDir,
    watched: PathBuf,
    db: Arc<Database>,
    folder: Folder,
}

impl Fixture {
    fn new(watcher_status: WatcherStatus) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let watched = temporary.path().join("watched");
        fs::create_dir(&watched).unwrap();

        let db = Arc::new(Database::open(&temporary.path().join("app.db")).unwrap());
        db.run_migrations().unwrap();
        db.settings()
            .set("notifications", &Value::Bool(false))
            .unwrap();

        let rule_id = Uuid::new_v4();
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
                action: Action::Move {
                    destination: "Documents".into(),
                },
                created_at: Utc::now(),
            })
            .unwrap();

        let folder = Folder {
            id: Uuid::new_v4(),
            path: watched.to_string_lossy().into_owned(),
            name: "Watched".into(),
            mode: FolderMode::Auto,
            rule_ids: vec![rule_id],
            watcher_status,
            created_at: Utc::now(),
            subfolders: SubfolderConfig::default(),
            exclude_folders: Vec::new(),
            exclude_patterns: Vec::new(),
        };
        db.folders().insert(&folder).unwrap();

        Self {
            _temporary: temporary,
            watched,
            db,
            folder,
        }
    }

    fn write(&self, name: &str) {
        fs::write(self.watched.join(name), format!("contents:{name}")).unwrap();
    }

    fn destination(&self, name: &str) -> PathBuf {
        self.watched.join("Documents").join(name)
    }
}

async fn wait_until(label: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn file_contents(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn burst_and_resume_catch_up_every_file_without_loss() {
    let fixture = Fixture::new(WatcherStatus::Stopped);
    let app = tauri::test::mock_app();
    let cancellations = Arc::new(CancellationRegistry::default());
    let watcher = WatcherService::new(fixture.db.clone(), cancellations);

    watcher
        .start_watching(app.handle(), &fixture.folder)
        .unwrap();

    let burst_names = (0..24)
        .map(|index| format!("burst-{index:02}.txt"))
        .collect::<Vec<_>>();
    for name in &burst_names {
        fixture.write(name);
    }

    wait_until("the complete watcher burst", || {
        burst_names
            .iter()
            .all(|name| fixture.destination(name).is_file())
    })
    .await;
    for name in &burst_names {
        assert_eq!(
            file_contents(fixture.destination(name)),
            format!("contents:{name}")
        );
    }

    watcher.pause(app.handle(), fixture.folder.id).unwrap();
    fixture.write("while-paused.txt");
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(fixture.watched.join("while-paused.txt").is_file());
    assert!(!fixture.destination("while-paused.txt").exists());

    watcher.resume(app.handle(), fixture.folder.id).unwrap();
    wait_until("resume catch-up", || {
        fixture.destination("while-paused.txt").is_file()
    })
    .await;
    assert_eq!(
        file_contents(fixture.destination("while-paused.txt")),
        "contents:while-paused.txt"
    );

    watcher.shutdown_all();
}

#[tokio::test]
async fn restart_restores_running_and_paused_watchers_without_losing_files() {
    let fixture = Fixture::new(WatcherStatus::Running);
    let app = tauri::test::mock_app();

    fixture.write("before-first-start.txt");
    let first = WatcherService::new(
        fixture.db.clone(),
        Arc::new(CancellationRegistry::default()),
    );
    assert_eq!(first.init(app.handle()).unwrap(), 1);
    wait_until("initial startup catch-up", || {
        fixture.destination("before-first-start.txt").is_file()
    })
    .await;
    first.shutdown_all();

    fixture.write("while-stopped.txt");
    let second = WatcherService::new(
        fixture.db.clone(),
        Arc::new(CancellationRegistry::default()),
    );
    assert_eq!(second.init(app.handle()).unwrap(), 1);
    assert_eq!(second.status(fixture.folder.id), WatcherStatus::Running);
    wait_until("restart catch-up", || {
        fixture.destination("while-stopped.txt").is_file()
    })
    .await;

    second.pause(app.handle(), fixture.folder.id).unwrap();
    second.shutdown_all();
    fixture.write("while-paused-and-stopped.txt");

    let third = WatcherService::new(
        fixture.db.clone(),
        Arc::new(CancellationRegistry::default()),
    );
    assert_eq!(third.init(app.handle()).unwrap(), 1);
    assert_eq!(third.status(fixture.folder.id), WatcherStatus::Paused);
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(fixture
        .watched
        .join("while-paused-and-stopped.txt")
        .is_file());
    assert!(!fixture.destination("while-paused-and-stopped.txt").exists());

    third.resume(app.handle(), fixture.folder.id).unwrap();
    wait_until("paused restart catch-up", || {
        fixture
            .destination("while-paused-and-stopped.txt")
            .is_file()
    })
    .await;
    assert_eq!(
        file_contents(fixture.destination("while-paused-and-stopped.txt")),
        "contents:while-paused-and-stopped.txt"
    );

    third.shutdown_all();
}
