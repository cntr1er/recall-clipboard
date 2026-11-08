//! SQLite-backed clipboard history.
//!
//! Every clip is one row. `(kind, content)` is unique, so copying the same thing
//! again is an upsert that bumps `copied_at` instead of adding a duplicate.

use std::path::Path;

use rusqlite::{Connection, Row, params};
use serde::Serialize;

/// Longest prefix of a text clip sent to list views; `get` returns the full text.
const LIST_PREVIEW_CHARS: i64 = 1000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: i64,
    /// `"text"` or `"image"`.
    pub kind: String,
    /// The text itself (truncated in lists), or the image's content hash.
    pub content: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub source_app: Option<String>,
    pub pinned: bool,
    /// Unix milliseconds of the most recent copy.
    pub copied_at: i64,
}

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS items (
                 id         INTEGER PRIMARY KEY,
                 kind       TEXT    NOT NULL CHECK (kind IN ('text', 'image')),
                 content    TEXT    NOT NULL,
                 width      INTEGER,
                 height     INTEGER,
                 source_app TEXT,
                 pinned     INTEGER NOT NULL DEFAULT 0,
                 copied_at  INTEGER NOT NULL,
                 UNIQUE (kind, content)
             );
             CREATE INDEX IF NOT EXISTS items_order ON items (pinned DESC, copied_at DESC);",
        )?;
        Ok(Self { conn })
    }

    /// Records a clip, or moves an identical existing one to the top. Returns its id.
    pub fn insert(
        &self,
        kind: &str,
        content: &str,
        size: Option<(u32, u32)>,
        source_app: Option<&str>,
        now: i64,
    ) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "INSERT INTO items (kind, content, width, height, source_app, copied_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (kind, content) DO UPDATE
                 SET copied_at = excluded.copied_at, source_app = excluded.source_app
             RETURNING id",
            params![kind, content, size.map(|s| s.0), size.map(|s| s.1), source_app, now],
            |r| r.get(0),
        )
    }

    /// Pinned first, then newest first. An empty query matches everything;
    /// otherwise it's a case-insensitive substring match on text clips.
    pub fn list(&self, query: &str, limit: u32) -> rusqlite::Result<Vec<Item>> {
        let pattern = format!("%{}%", escape_like(query));
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, kind, substr(content, 1, ?4), width, height, source_app, pinned, copied_at
             FROM items
             WHERE ?1 = '' OR (kind = 'text' AND content LIKE ?2 ESCAPE '\\')
             ORDER BY pinned DESC, copied_at DESC
             LIMIT ?3",
        )?;
        stmt.query_map(params![query, pattern, limit, LIST_PREVIEW_CHARS], row_to_item)?
            .collect()
    }
}

fn row_to_item(r: &Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        kind: r.get(1)?,
        content: r.get(2)?,
        width: r.get(3)?,
        height: r.get(4)?,
        source_app: r.get(5)?,
        pinned: r.get(6)?,
        copied_at: r.get(7)?,
    })
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Storage {
        Storage::open_in_memory().unwrap()
    }

    fn texts(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.content.as_str()).collect()
    }

    #[test]
    fn insert_lists_newest_first() {
        let db = db();
        db.insert("text", "a", None, None, 1).unwrap();
        db.insert("text", "b", None, None, 2).unwrap();
        assert_eq!(texts(&db.list("", 10).unwrap()), ["b", "a"]);
    }

    #[test]
    fn duplicate_moves_to_top_without_new_row() {
        let db = db();
        let first = db.insert("text", "a", None, Some("old.exe"), 1).unwrap();
        db.insert("text", "b", None, None, 2).unwrap();
        let again = db.insert("text", "a", None, Some("new.exe"), 3).unwrap();

        assert_eq!(first, again);
        let items = db.list("", 10).unwrap();
        assert_eq!(texts(&items), ["a", "b"]);
        assert_eq!(items[0].copied_at, 3);
        assert_eq!(items[0].source_app.as_deref(), Some("new.exe"));
    }

    #[test]
    fn same_content_different_kind_is_not_a_duplicate() {
        let db = db();
        db.insert("text", "abc", None, None, 1).unwrap();
        db.insert("image", "abc", Some((1, 1)), None, 2).unwrap();
        assert_eq!(db.list("", 10).unwrap().len(), 2);
    }

    #[test]
    fn search_is_case_insensitive_and_treats_wildcards_literally() {
        let db = db();
        db.insert("text", "Hello World", None, None, 1).unwrap();
        db.insert("text", "100% done", None, None, 2).unwrap();
        db.insert("text", "1000 done", None, None, 3).unwrap();

        assert_eq!(texts(&db.list("hello", 10).unwrap()), ["Hello World"]);
        assert_eq!(texts(&db.list("0%", 10).unwrap()), ["100% done"]);
    }

    #[test]
    fn list_truncates_long_text() {
        let db = db();
        db.insert("text", &"x".repeat(5000), None, None, 1).unwrap();
        assert_eq!(db.list("", 10).unwrap()[0].content.len(), LIST_PREVIEW_CHARS as usize);
    }
}
