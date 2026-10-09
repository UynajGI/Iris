use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 5;
pub struct Store {
    pub conn: Connection,
    pub path: PathBuf,
    pub recovery_notice: Option<String>,
}
impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path).context("open project database")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            bail!("database version {version} is newer than supported {SCHEMA_VERSION}");
        }
        let mut recovery_notice = None;
        if version < SCHEMA_VERSION {
            let has_tables: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table')",
                [],
                |r| r.get(0),
            )?;
            if has_tables {
                let backup =
                    path.with_extension(format!("v{version}.{}.bak", uuid::Uuid::new_v4()));
                conn.backup("main", &backup, None)?;
                recovery_notice = Some(format!(
                    "Database upgraded; recovery backup: {}",
                    backup.display()
                ));
            }
            if version < 1 {
                conn.execute_batch(SCHEMA)?;
            }
            if version < 2 {
                conn.execute_batch(MIGRATION_2)?;
            }
            if version < 3 {
                conn.execute_batch(MIGRATION_3)?;
            }
            if version < 4 {
                conn.execute_batch(MIGRATION_4)?;
            }
            if version < 5 {
                conn.execute_batch(MIGRATION_5)?;
            }
        }
        let integrity: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if integrity != "ok" {
            bail!("database integrity check failed: {integrity}");
        }
        Ok(Self {
            conn,
            path,
            recovery_notice,
        })
    }
    pub fn backup(&self, destination: impl AsRef<Path>) -> Result<()> {
        if destination.as_ref().exists() {
            bail!("backup destination already exists");
        }
        self.conn.backup("main", destination, None)?;
        Ok(())
    }
}
const SCHEMA: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS projects(id INTEGER PRIMARY KEY,root TEXT NOT NULL UNIQUE,name TEXT NOT NULL,created_at TEXT NOT NULL,cache_root TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS photos(id INTEGER PRIMARY KEY,project_id INTEGER NOT NULL REFERENCES projects(id),path TEXT NOT NULL,filename TEXT NOT NULL,format TEXT NOT NULL,mtime INTEGER NOT NULL,size_bytes INTEGER NOT NULL,width INTEGER NOT NULL,height INTEGER NOT NULL,taken_at TEXT,capture_variant_id TEXT,missing INTEGER NOT NULL DEFAULT 0,quarantined INTEGER NOT NULL DEFAULT 0,UNIQUE(project_id,path));
CREATE TABLE IF NOT EXISTS analyses(photo_id INTEGER PRIMARY KEY REFERENCES photos(id),data TEXT NOT NULL,version TEXT,analyzed_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS faces(photo_id INTEGER NOT NULL REFERENCES photos(id),face_index INTEGER NOT NULL,data TEXT NOT NULL,PRIMARY KEY(photo_id,face_index));
CREATE TABLE IF NOT EXISTS burst_groups(id TEXT PRIMARY KEY,project_id INTEGER NOT NULL REFERENCES projects(id),kind TEXT NOT NULL,members TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,project_id INTEGER NOT NULL REFERENCES projects(id),created_at TEXT NOT NULL,undone_at TEXT);
CREATE TABLE IF NOT EXISTS decisions(id INTEGER PRIMARY KEY,photo_id INTEGER NOT NULL REFERENCES photos(id),session_id TEXT NOT NULL REFERENCES sessions(id),action TEXT NOT NULL,previous_action TEXT NOT NULL,source TEXT NOT NULL,created_at TEXT NOT NULL,undone_at TEXT);
CREATE INDEX IF NOT EXISTS decisions_photo ON decisions(photo_id,id DESC);
CREATE TABLE IF NOT EXISTS settings(project_id INTEGER PRIMARY KEY REFERENCES projects(id),data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS profiles(name TEXT PRIMARY KEY,data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS quarantine(id TEXT PRIMARY KEY,project_id INTEGER NOT NULL REFERENCES projects(id),state TEXT NOT NULL,data TEXT NOT NULL,created_at TEXT NOT NULL);
PRAGMA user_version=1;
COMMIT;
"#;

// Invalidation is atomic with source changes, including quarantine recovery
// and scans using a separate database connection.
const MIGRATION_2: &str = r#"
BEGIN IMMEDIATE;
ALTER TABLE projects ADD COLUMN groups_dirty INTEGER NOT NULL DEFAULT 1;
CREATE TRIGGER group_photo_insert AFTER INSERT ON photos BEGIN
  UPDATE projects SET groups_dirty=1 WHERE id=NEW.project_id;
  DELETE FROM burst_groups WHERE project_id=NEW.project_id;
END;
CREATE TRIGGER group_photo_update AFTER UPDATE ON photos
WHEN OLD.mtime!=NEW.mtime OR OLD.size_bytes!=NEW.size_bytes OR OLD.missing!=NEW.missing
  OR OLD.quarantined!=NEW.quarantined OR OLD.taken_at IS NOT NEW.taken_at
  OR OLD.capture_variant_id IS NOT NEW.capture_variant_id BEGIN
  UPDATE projects SET groups_dirty=1 WHERE id=NEW.project_id;
  DELETE FROM burst_groups WHERE project_id=NEW.project_id;
END;
CREATE TRIGGER group_analysis_insert AFTER INSERT ON analyses BEGIN
  UPDATE projects SET groups_dirty=1 WHERE id=(SELECT project_id FROM photos WHERE id=NEW.photo_id);
  DELETE FROM burst_groups WHERE project_id=(SELECT project_id FROM photos WHERE id=NEW.photo_id);
END;
CREATE TRIGGER group_analysis_update AFTER UPDATE ON analyses BEGIN
  UPDATE projects SET groups_dirty=1 WHERE id=(SELECT project_id FROM photos WHERE id=NEW.photo_id);
  DELETE FROM burst_groups WHERE project_id=(SELECT project_id FROM photos WHERE id=NEW.photo_id);
END;
CREATE TRIGGER group_analysis_delete AFTER DELETE ON analyses BEGIN
  UPDATE projects SET groups_dirty=1 WHERE id=(SELECT project_id FROM photos WHERE id=OLD.photo_id);
  DELETE FROM burst_groups WHERE project_id=(SELECT project_id FROM photos WHERE id=OLD.photo_id);
END;
CREATE TRIGGER group_settings_insert AFTER INSERT ON settings BEGIN
  UPDATE projects SET groups_dirty=1 WHERE id=NEW.project_id;
  DELETE FROM burst_groups WHERE project_id=NEW.project_id;
END;
CREATE TRIGGER group_settings_update AFTER UPDATE ON settings WHEN OLD.data!=NEW.data BEGIN
  UPDATE projects SET groups_dirty=1 WHERE id=NEW.project_id;
  DELETE FROM burst_groups WHERE project_id=NEW.project_id;
END;
PRAGMA user_version=2;
COMMIT;
"#;

const MIGRATION_3: &str = r#"
BEGIN IMMEDIATE;
ALTER TABLE projects ADD COLUMN last_opened_at TEXT NOT NULL DEFAULT '';
ALTER TABLE projects ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;
UPDATE projects SET last_opened_at=created_at;
CREATE TABLE cache_migrations(id TEXT PRIMARY KEY,project_id INTEGER NOT NULL REFERENCES projects(id),state TEXT NOT NULL,data TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX cache_migrations_project ON cache_migrations(project_id,created_at DESC);
PRAGMA user_version=3;
COMMIT;
"#;

const MIGRATION_5: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS analysis_runs(project_id INTEGER PRIMARY KEY REFERENCES projects(id),data TEXT NOT NULL,completed_at TEXT NOT NULL);
PRAGMA user_version=5;
COMMIT;
"#;

const MIGRATION_4: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE photo_marks(
  id INTEGER PRIMARY KEY,
  photo_id INTEGER NOT NULL REFERENCES photos(id),
  session_id TEXT NOT NULL REFERENCES sessions(id),
  rating INTEGER NOT NULL CHECK(rating BETWEEN 0 AND 5),
  color_label TEXT NOT NULL CHECK(color_label IN ('none','red','yellow','green','blue','purple')),
  undone_at TEXT
);
CREATE INDEX photo_marks_photo ON photo_marks(photo_id,id DESC);
CREATE INDEX photo_marks_session ON photo_marks(session_id);
PRAGMA user_version=4;
COMMIT;
"#;
