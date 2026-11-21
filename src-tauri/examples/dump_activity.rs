//! Dump activity/diagnostic summaries from a FileGrove SQLite database.
//!
//! Usage: `cargo run --example dump_activity -- <path-to-app.db>`
//!
//! Prints aggregate error counts (no personal file paths), total activity
//! count and the configured language setting.

use rusqlite::Connection;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: dump_activity <path-to-app.db>");
    let conn = Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap_or_else(|error| panic!("cannot open {path}: {error}"));

    println!("=== failed-move error codes (top 20) ===");
    let mut stmt = conn
        .prepare(
            "SELECT error, COUNT(*) FROM activity \
             WHERE success = 0 GROUP BY error ORDER BY 2 DESC LIMIT 20",
        )
        .expect("prepare error count query");
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .expect("query error counts");
    let mut found = false;
    for row in rows {
        let (error, count) = row.expect("row");
        found = true;
        println!("{error} | {count}");
    }
    if !found {
        println!("(no failed activity rows)");
    }

    println!();
    println!("=== total activity rows ===");
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM activity", [], |row| row.get(0))
        .expect("count activity");
    println!("{total}");

    println!();
    println!("=== settings.language ===");
    match conn.query_row(
        "SELECT value FROM settings WHERE key = 'language'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => println!("{value}"),
        Err(rusqlite::Error::QueryReturnedNoRows) => println!("(not set)"),
        Err(error) => println!("(query failed: {error})"),
    }
}
