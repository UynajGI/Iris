use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};

#[derive(Clone)]
pub struct Policy {
    roots: Vec<PathBuf>,
}
impl Policy {
    pub fn new(roots: &[PathBuf]) -> Result<Self> {
        anyhow::ensure!(!roots.is_empty(), "at least one --allow-root is required");
        let roots = roots
            .iter()
            .map(|p| {
                let p = p.canonicalize()?;
                anyhow::ensure!(p.is_dir(), "allowed root must be a directory");
                Ok(p)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { roots })
    }
    pub fn check(&self, path: &Path) -> Result<PathBuf> {
        anyhow::ensure!(path.is_absolute(), "absolute paths are required");
        anyhow::ensure!(
            !path.components().any(|c| matches!(c, Component::ParentDir)),
            "parent traversal is forbidden"
        );
        let mut existing = path;
        let mut suffix = Vec::new();
        while !existing.exists() {
            suffix.push(
                existing
                    .file_name()
                    .context("path has no existing ancestor")?
                    .to_owned(),
            );
            existing = existing.parent().context("path has no existing ancestor")?;
        }
        let mut resolved = existing.canonicalize()?;
        for part in suffix.into_iter().rev() {
            resolved.push(part);
        }
        if !self.roots.iter().any(|r| resolved.starts_with(r)) {
            bail!("path is outside --allow-root directories");
        }
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contains_future_destinations_but_not_traversal_or_prefix_siblings() {
        let temp = tempfile::tempdir().unwrap();
        let allowed = temp.path().join("photos");
        std::fs::create_dir(&allowed).unwrap();
        let policy = Policy::new(&[allowed.clone()]).unwrap();
        assert!(policy.check(&allowed.join("new/sub/file.csv")).is_ok());
        assert!(policy
            .check(&temp.path().join("photos-other/file.csv"))
            .is_err());
        assert!(policy.check(&allowed.join("../secret")).is_err());
        assert!(policy.check(Path::new("relative.jpg")).is_err());
    }
}
