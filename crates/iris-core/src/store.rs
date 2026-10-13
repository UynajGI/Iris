use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 9;
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
        register_catalog_functions(&conn)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            bail!("database version {version} is newer than supported {SCHEMA_VERSION}");
        }
        let recovery_notice = upgrade_database(&conn, &path, version)?;
        let integrity: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if integrity != "ok" {
            bail!("database integrity check failed: {integrity}");
        }
        invalidate_old_groups(&conn)?;
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

fn invalidate_old_groups(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let previous: String = tx.query_row(
        "SELECT COALESCE((SELECT version FROM engine_cache_versions WHERE name='groups'), '')",
        [],
        |row| row.get(0),
    )?;
    if previous == crate::vision::ANALYSIS_VERSION {
        return Ok(());
    }
    tx.execute(
        "UPDATE projects SET groups_dirty=1 WHERE id IN (SELECT p.project_id FROM photos p JOIN analyses a ON a.photo_id=p.id WHERE a.version IS NOT ?1)",
        [crate::vision::ANALYSIS_VERSION],
    )?;
    tx.execute("DELETE FROM burst_groups WHERE project_id IN (SELECT id FROM projects WHERE groups_dirty=1)", [])?;
    tx.execute(
        "INSERT INTO engine_cache_versions(name,version) VALUES('groups',?1) ON CONFLICT(name) DO UPDATE SET version=excluded.version",
        [crate::vision::ANALYSIS_VERSION],
    )?;
    tx.commit()?;
    Ok(())
}

fn upgrade_database(conn: &Connection, path: &Path, version: i64) -> Result<Option<String>> {
    if version >= SCHEMA_VERSION {
        return Ok(None);
    }
    let recovery_notice = backup_before_upgrade(conn, path, version)?;
    for (target, migration) in [
        (1, SCHEMA),
        (2, MIGRATION_2),
        (3, MIGRATION_3),
        (4, MIGRATION_4),
        (5, MIGRATION_5),
        (6, MIGRATION_6),
        (7, MIGRATION_7),
        (8, MIGRATION_8),
        (9, MIGRATION_9),
    ] {
        if version < target {
            conn.execute_batch(migration)?;
        }
    }
    Ok(recovery_notice)
}

fn backup_before_upgrade(conn: &Connection, path: &Path, version: i64) -> Result<Option<String>> {
    let has_tables: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table')",
        [],
        |r| r.get(0),
    )?;
    if !has_tables {
        return Ok(None);
    }
    let backup = path.with_extension(format!("v{version}.{}.bak", uuid::Uuid::new_v4()));
    conn.backup("main", &backup, None)?;
    Ok(Some(format!(
        "Database upgraded; recovery backup: {}",
        backup.display()
    )))
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

const MIGRATION_6: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE engine_cache_versions(name TEXT PRIMARY KEY,version TEXT NOT NULL);
PRAGMA user_version=6;
COMMIT;
"#;

// Normalize the small settings object at write time, never the complete analysis
// (faces/vectors) while paging. This is the same validation/defaulting gate as
// services::analysis_matches_settings, including historical partial settings.
fn register_catalog_functions(conn: &Connection) -> Result<()> {
    use rusqlite::{functions::FunctionFlags, types::ValueRef};
    conn.create_scalar_function(
        "iris_catalog_settings_v1",
        1,
        FunctionFlags::SQLITE_UTF8
            | FunctionFlags::SQLITE_DETERMINISTIC
            | FunctionFlags::SQLITE_INNOCUOUS,
        |context| {
            let ValueRef::Text(bytes) = context.get_raw(0) else {
                return Ok(None::<String>);
            };
            let parsed = serde_json::from_slice::<crate::vision::AnalysisSettings>(bytes)
                .ok()
                .filter(|settings| settings.validate().is_ok());
            Ok(parsed.and_then(|settings| serde_json::to_string(&settings).ok()))
        },
    )?;
    Ok(())
}

const MIGRATION_7: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE analysis_catalog(
    photo_id INTEGER PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
    version TEXT,
    settings_key TEXT,
    source_invalid INTEGER NOT NULL,
    score REAL NOT NULL,
    verdict TEXT
);
CREATE VIEW analysis_catalog_source AS
SELECT photo_id,
    CASE WHEN json_type(safe,'$.version')='text' THEN json_extract(safe,'$.version') END,
    iris_catalog_settings_v1(json_extract(safe,'$.settings')),
    json_type(safe,'$.source_invalid') IS 'true',
    CASE WHEN json_type(safe,'$.composite_score') IN ('integer','real')
        THEN CAST(json_extract(safe,'$.composite_score') AS REAL) ELSE -1.0 END,
    CASE WHEN json_type(safe,'$.verdict')='text' THEN json_extract(safe,'$.verdict') END
FROM (SELECT photo_id,CASE WHEN json_valid(data) THEN data ELSE '{}' END AS safe FROM analyses);
INSERT INTO analysis_catalog SELECT * FROM analysis_catalog_source;
CREATE TRIGGER catalog_analysis_insert AFTER INSERT ON analyses BEGIN
    INSERT OR REPLACE INTO analysis_catalog SELECT * FROM analysis_catalog_source WHERE photo_id=NEW.photo_id;
END;
CREATE TRIGGER catalog_analysis_update AFTER UPDATE OF data ON analyses BEGIN
    DELETE FROM analysis_catalog WHERE photo_id=NEW.photo_id;
    INSERT INTO analysis_catalog SELECT * FROM analysis_catalog_source WHERE photo_id=NEW.photo_id;
END;
CREATE TRIGGER catalog_analysis_delete AFTER DELETE ON analyses BEGIN
    DELETE FROM analysis_catalog WHERE photo_id=OLD.photo_id;
END;
CREATE INDEX catalog_score ON analysis_catalog(version,settings_key,source_invalid,score,photo_id);
CREATE INDEX catalog_verdict ON analysis_catalog(version,settings_key,source_invalid,verdict,photo_id);
CREATE INDEX photo_catalog_name ON photos(project_id,path COLLATE BINARY);
CREATE INDEX photo_catalog_size ON photos(project_id,size_bytes,id);
CREATE INDEX photo_catalog_date ON photos(project_id,COALESCE(taken_at,''),mtime,id);
CREATE TABLE IF NOT EXISTS scene_assignments(
    photo_id INTEGER PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    scene_id TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS scene_assignment_project ON scene_assignments(project_id,scene_id);
CREATE TABLE IF NOT EXISTS retrieval_embeddings(
    photo_id INTEGER PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
    space_key TEXT NOT NULL,
    mtime INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    dimensions INTEGER NOT NULL,
    vector BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS retrieval_space ON retrieval_embeddings(space_key,photo_id);
PRAGMA user_version=7;
COMMIT;
"#;

const MIGRATION_8: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE group_selection_intents(
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  group_id TEXT NOT NULL,
  members_key TEXT NOT NULL,
  data TEXT NOT NULL,
  PRIMARY KEY(project_id,group_id)
);
CREATE TABLE selection_locks(
  photo_id INTEGER PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE
);
PRAGMA user_version=8;
COMMIT;
"#;

const MIGRATION_9: &str = r#"
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS edit_documents(
 id TEXT PRIMARY KEY, photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
 name TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision>=0), cursor INTEGER NOT NULL CHECK(cursor>=0),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS edit_documents_photo ON edit_documents(photo_id,id);
CREATE TABLE IF NOT EXISTS edit_revisions(
 document_id TEXT NOT NULL REFERENCES edit_documents(id) ON DELETE CASCADE,
 revision INTEGER NOT NULL CHECK(revision>=0), document_json TEXT NOT NULL,
 action TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(document_id,revision)
);
CREATE TABLE IF NOT EXISTS edit_states(
 document_id TEXT NOT NULL REFERENCES edit_documents(id) ON DELETE CASCADE,
 position INTEGER NOT NULL CHECK(position>=0), snapshot_revision INTEGER NOT NULL,
 PRIMARY KEY(document_id,position),
 FOREIGN KEY(document_id,snapshot_revision) REFERENCES edit_revisions(document_id,revision)
);
CREATE TABLE IF NOT EXISTS edit_commands(
 document_id TEXT NOT NULL REFERENCES edit_documents(id) ON DELETE CASCADE,
 request_id TEXT NOT NULL, payload_hash TEXT NOT NULL, result_revision INTEGER NOT NULL,
 PRIMARY KEY(document_id,request_id),
 FOREIGN KEY(document_id,result_revision) REFERENCES edit_revisions(document_id,revision)
);
CREATE TABLE IF NOT EXISTS edit_resources(
 hash TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('lut','icc','lens_profile')),
 size_bytes INTEGER NOT NULL CHECK(size_bytes>0), created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS edit_resource_pins(
 document_id TEXT NOT NULL, revision INTEGER NOT NULL, hash TEXT NOT NULL REFERENCES edit_resources(hash),
 PRIMARY KEY(document_id,revision,hash),
 FOREIGN KEY(document_id,revision) REFERENCES edit_revisions(document_id,revision) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS edit_presets(
 id TEXT PRIMARY KEY, name TEXT NOT NULL, recipe_json TEXT NOT NULL,
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS edit_preset_resources(
 preset_id TEXT NOT NULL REFERENCES edit_presets(id) ON DELETE CASCADE,
 hash TEXT NOT NULL REFERENCES edit_resources(hash), PRIMARY KEY(preset_id,hash)
);
CREATE TABLE IF NOT EXISTS rendered_export_jobs(
 id TEXT PRIMARY KEY, batch_id TEXT NOT NULL, document_id TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision>=0), snapshot_json TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('queued','running','publishing','completed','failed','cancelled','interrupted')),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 result_hash TEXT, result_bytes INTEGER, error TEXT, temporary_path TEXT
);
CREATE INDEX IF NOT EXISTS rendered_export_state ON rendered_export_jobs(state,created_at,id);
CREATE TABLE IF NOT EXISTS rendered_export_resources(
 job_id TEXT NOT NULL REFERENCES rendered_export_jobs(id) ON DELETE CASCADE,
 hash TEXT NOT NULL, bytes BLOB NOT NULL, PRIMARY KEY(job_id,hash)
);
PRAGMA user_version=9;
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
