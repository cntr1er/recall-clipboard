//! SQLite-backed clipboard history.
//!
//! Every clip is one row. `(kind, content)` is unique, so copying the same thing
//! again is an upsert that bumps `copied_at` instead of adding a duplicate.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

/// Longest prefix of a text clip sent to list views; `get` returns the full text.
const LIST_PREVIEW_CHARS: i64 = 1000;
const DAY_MS: i64 = 86_400_000;

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
    /// Lowercase executable name of the app the copy came from, e.g. `chrome.exe`.
    pub source_app: Option<String>,
    /// Friendly name of that app, e.g. `Google Chrome`.
    pub source_name: Option<String>,
    pub pinned: bool,
    /// Unix milliseconds of the most recent copy.
    pub copied_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub exe: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Filter {
    /// Case-insensitive substring of text clips. Empty matches everything.
    pub query: String,
    /// `"text"` or `"image"`; `None` for both.
    pub kind: Option<String>,
    pub pinned_only: bool,
    pub source_app: Option<String>,
    pub limit: u32,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            query: String::new(),
            kind: None,
            pinned_only: false,
            source_app: None,
            limit: 500,
        }
    }
}

pub struct Storage {
    conn: Connection,
}

const SELECT_ITEM: &str = "SELECT i.id, i.kind, substr(i.content, 1, ?1), i.width, i.height,
        i.source_app, a.name, i.pinned, i.copied_at
     FROM items i LEFT JOIN apps a ON a.exe = i.source_app";

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
             CREATE INDEX IF NOT EXISTS items_order ON items (pinned DESC, copied_at DESC);
             CREATE TABLE IF NOT EXISTS apps (
                 exe  TEXT PRIMARY KEY,
                 name TEXT NOT NULL
             );",
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
            params![
                kind,
                content,
                size.map(|s| s.0),
                size.map(|s| s.1),
                source_app,
                now
            ],
            |r| r.get(0),
        )
    }

    /// Pinned first, then newest first.
    pub fn list(&self, f: &Filter) -> rusqlite::Result<Vec<Item>> {
        let pattern = format!("%{}%", escape_like(&f.query));
        let mut stmt = self.conn.prepare_cached(&format!(
            "{SELECT_ITEM}
             WHERE (?2 = '' OR (i.kind = 'text' AND i.content LIKE ?3 ESCAPE '\\'))
               AND (?4 IS NULL OR i.kind = ?4)
               AND (?5 = 0 OR i.pinned = 1)
               AND (?6 IS NULL OR i.source_app = ?6)
             ORDER BY i.pinned DESC, i.copied_at DESC
             LIMIT ?7"
        ))?;
        stmt.query_map(
            params![
                LIST_PREVIEW_CHARS,
                f.query,
                pattern,
                f.kind,
                f.pinned_only,
                f.source_app,
                f.limit
            ],
            row_to_item,
        )?
        .collect()
    }

    /// One item with its full, untruncated content.
    pub fn get(&self, id: i64) -> rusqlite::Result<Option<Item>> {
        self.conn
            .query_row(
                &format!("{SELECT_ITEM} WHERE i.id = ?2"),
                params![i64::MAX, id],
                row_to_item,
            )
            .optional()
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET pinned = ?2 WHERE id = ?1",
            params![id, pinned],
        )?;
        Ok(())
    }

    /// Moves an item to the top, as if it had just been copied.
    pub fn touch(&self, id: i64, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET copied_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// Deletes one item (pinned or not). Returns its image hash if it was an image.
    pub fn delete(&self, id: i64) -> rusqlite::Result<Option<String>> {
        let deleted: Option<(String, String)> = self
            .conn
            .query_row(
                "DELETE FROM items WHERE id = ?1 RETURNING kind, content",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(deleted.filter(|(k, _)| k == "image").map(|(_, c)| c))
    }

    /// Deletes every unpinned item. Returns the image hashes removed.
    pub fn clear_unpinned(&self) -> rusqlite::Result<Vec<String>> {
        self.delete_returning_images(
            "DELETE FROM items WHERE pinned = 0 RETURNING kind, content",
            [],
        )
    }

    /// Deletes unpinned items older than `retention_days`, and unpinned items beyond
    /// the newest `max_items`. Returns the image hashes removed.
    pub fn cleanup(
        &self,
        now: i64,
        retention_days: u32,
        max_items: u32,
    ) -> rusqlite::Result<Vec<String>> {
        self.delete_returning_images(
            "DELETE FROM items WHERE pinned = 0 AND (
                 copied_at < ?1 OR id NOT IN (
                     SELECT id FROM items WHERE pinned = 0 ORDER BY copied_at DESC LIMIT ?2))
             RETURNING kind, content",
            params![now - retention_days as i64 * DAY_MS, max_items],
        )
    }

    fn delete_returning_images(
        &self,
        sql: &str,
        p: impl rusqlite::Params,
    ) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(p, |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut hashes = Vec::new();
        for row in rows {
            let (kind, content) = row?;
            if kind == "image" {
                hashes.push(content);
            }
        }
        Ok(hashes)
    }

    pub fn has_app(&self, exe: &str) -> rusqlite::Result<bool> {
        self.conn
            .query_row("SELECT 1 FROM apps WHERE exe = ?1", [exe], |_| Ok(()))
            .optional()
            .map(|r| r.is_some())
    }

    pub fn upsert_app(&self, exe: &str, name: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO apps (exe, name) VALUES (?1, ?2)
             ON CONFLICT (exe) DO UPDATE SET name = excluded.name",
            [exe, name],
        )?;
        Ok(())
    }

    /// Every app seen as a copy source, by name.
    pub fn apps(&self) -> rusqlite::Result<Vec<App>> {
        let mut stmt = self
            .conn
            .prepare("SELECT exe, name FROM apps ORDER BY name COLLATE NOCASE")?;
        stmt.query_map([], |r| {
            Ok(App {
                exe: r.get(0)?,
                name: r.get(1)?,
            })
        })?
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
        source_name: r.get(6)?,
        pinned: r.get(7)?,
        copied_at: r.get(8)?,
    })
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Storage {
        Storage::open_in_memory().unwrap()
    }

    fn all(db: &Storage) -> Vec<Item> {
        db.list(&Filter::default()).unwrap()
    }

    fn texts(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.content.as_str()).collect()
    }

    fn query(q: &str) -> Filter {
        Filter {
            query: q.into(),
            ..Default::default()
        }
    }

    #[test]
    fn insert_lists_newest_first() {
        let db = db();
        db.insert("text", "a", None, None, 1).unwrap();
        db.insert("text", "b", None, None, 2).unwrap();
        assert_eq!(texts(&all(&db)), ["b", "a"]);
    }

    #[test]
    fn duplicate_moves_to_top_without_new_row() {
        let db = db();
        let first = db.insert("text", "a", None, Some("old.exe"), 1).unwrap();
        db.insert("text", "b", None, None, 2).unwrap();
        let again = db.insert("text", "a", None, Some("new.exe"), 3).unwrap();

        assert_eq!(first, again);
        let items = all(&db);
        assert_eq!(texts(&items), ["a", "b"]);
        assert_eq!(items[0].copied_at, 3);
        assert_eq!(items[0].source_app.as_deref(), Some("new.exe"));
    }

    #[test]
    fn same_content_different_kind_is_not_a_duplicate() {
        let db = db();
        db.insert("text", "abc", None, None, 1).unwrap();
        db.insert("image", "abc", Some((1, 1)), None, 2).unwrap();
        assert_eq!(all(&db).len(), 2);
    }

    #[test]
    fn search_is_case_insensitive_and_treats_wildcards_literally() {
        let db = db();
        db.insert("text", "Hello World", None, None, 1).unwrap();
        db.insert("text", "100% done", None, None, 2).unwrap();
        db.insert("text", "1000 done", None, None, 3).unwrap();

        assert_eq!(texts(&db.list(&query("hello")).unwrap()), ["Hello World"]);
        assert_eq!(texts(&db.list(&query("0%")).unwrap()), ["100% done"]);
    }

    #[test]
    fn list_truncates_long_text_but_get_returns_all_of_it() {
        let db = db();
        let id = db.insert("text", &"x".repeat(5000), None, None, 1).unwrap();
        assert_eq!(all(&db)[0].content.len(), LIST_PREVIEW_CHARS as usize);
        assert_eq!(db.get(id).unwrap().unwrap().content.len(), 5000);
    }

    #[test]
    fn filters_by_kind_pin_and_source() {
        let db = db();
        db.insert("text", "t", None, Some("a.exe"), 1).unwrap();
        let img = db
            .insert("image", "h", Some((2, 2)), Some("b.exe"), 2)
            .unwrap();
        db.set_pinned(img, true).unwrap();

        let f = |kind: Option<&str>, pinned_only, source: Option<&str>| Filter {
            kind: kind.map(Into::into),
            pinned_only,
            source_app: source.map(Into::into),
            ..Default::default()
        };
        assert_eq!(
            texts(&db.list(&f(Some("text"), false, None)).unwrap()),
            ["t"]
        );
        assert_eq!(texts(&db.list(&f(None, true, None)).unwrap()), ["h"]);
        assert_eq!(
            texts(&db.list(&f(None, false, Some("a.exe"))).unwrap()),
            ["t"]
        );
    }

    #[test]
    fn pinned_items_stay_on_top() {
        let db = db();
        let old = db.insert("text", "old", None, None, 1).unwrap();
        db.insert("text", "new", None, None, 2).unwrap();
        db.set_pinned(old, true).unwrap();
        assert_eq!(texts(&all(&db)), ["old", "new"]);
        assert!(all(&db)[0].pinned);

        db.set_pinned(old, false).unwrap();
        assert_eq!(texts(&all(&db)), ["new", "old"]);
    }

    #[test]
    fn cleanup_removes_old_unpinned_items_only() {
        let db = db();
        let now = 100 * DAY_MS;
        let pinned = db.insert("text", "pinned-old", None, None, 0).unwrap();
        db.set_pinned(pinned, true).unwrap();
        db.insert("image", "old-image", Some((1, 1)), None, now - 31 * DAY_MS)
            .unwrap();
        db.insert("text", "recent", None, None, now - DAY_MS)
            .unwrap();

        let removed = db.cleanup(now, 30, 1000).unwrap();
        assert_eq!(removed, ["old-image"]);
        assert_eq!(texts(&all(&db)), ["pinned-old", "recent"]);
    }

    #[test]
    fn cleanup_enforces_max_items_without_counting_pins() {
        let db = db();
        for i in 0..5 {
            db.insert("text", &format!("t{i}"), None, None, i).unwrap();
        }
        let pinned = db.insert("text", "pin", None, None, -1).unwrap();
        db.set_pinned(pinned, true).unwrap();

        db.cleanup(10, 30, 2).unwrap();
        assert_eq!(texts(&all(&db)), ["pin", "t4", "t3"]);
    }

    #[test]
    fn delete_and_clear() {
        let db = db();
        let img = db.insert("image", "h1", Some((1, 1)), None, 1).unwrap();
        let txt = db.insert("text", "t", None, None, 2).unwrap();
        let keep = db.insert("image", "h2", Some((1, 1)), None, 3).unwrap();
        db.set_pinned(keep, true).unwrap();

        assert_eq!(db.delete(txt).unwrap(), None);
        assert_eq!(db.delete(txt).unwrap(), None, "deleting twice is a no-op");
        assert_eq!(db.clear_unpinned().unwrap(), ["h1"]);
        assert!(db.get(img).unwrap().is_none());
        assert_eq!(texts(&all(&db)), ["h2"]);
        assert_eq!(db.delete(keep).unwrap().as_deref(), Some("h2"));
    }

    #[test]
    fn touch_moves_item_to_top() {
        let db = db();
        let a = db.insert("text", "a", None, None, 1).unwrap();
        db.insert("text", "b", None, None, 2).unwrap();
        db.touch(a, 3).unwrap();
        assert_eq!(texts(&all(&db)), ["a", "b"]);
    }

    #[test]
    fn apps_are_joined_by_exe() {
        let db = db();
        db.upsert_app("code.exe", "Visual Studio Code").unwrap();
        db.insert("text", "x", None, Some("code.exe"), 1).unwrap();
        assert!(db.has_app("code.exe").unwrap());
        assert!(!db.has_app("other.exe").unwrap());
        assert_eq!(
            all(&db)[0].source_name.as_deref(),
            Some("Visual Studio Code")
        );
        assert_eq!(db.apps().unwrap()[0].name, "Visual Studio Code");
    }
}
