//! One scheduler per database across CLI, desktop daemon and MCP processes.
use anyhow::{Context, Result};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub struct DatabaseOwner(File);
impl DatabaseOwner {
    pub fn acquire(database: &Path) -> Result<Self> {
        let parent = database
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let parent = parent.canonicalize()?;
        let database = if database.exists() {
            database.canonicalize()?
        } else {
            parent.join(database.file_name().context("database filename required")?)
        };
        let parent = database.parent().context("database parent required")?;
        let name = database.file_name().context("database filename required")?;
        let mut lock = name.to_os_string();
        lock.push(".owner.lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(parent.join(lock))?;
        file.try_lock().context(
            "database is in use by another Iris process; close it or select another data directory",
        )?;
        Ok(Self(file))
    }
}
impl Drop for DatabaseOwner {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excludes_second_owner_and_releases_on_drop() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite3");
        let owner = DatabaseOwner::acquire(&path).unwrap();
        assert!(DatabaseOwner::acquire(&path).is_err());
        drop(owner);
        assert!(DatabaseOwner::acquire(&path).is_ok());
    }
}
