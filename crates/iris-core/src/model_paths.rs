//! Paths beneath an explicitly selected model/import directory, never an arbitrary
//! HTTP-supplied child path. Reject links/reparse points, including dangling links.
use anyhow::{bail, Context, Result};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The selected root must already exist. Missing children are allowed for an
/// installation, but every existing component must be an ordinary file/directory.
/// A deliberately selected root may itself be a link; its canonical location is
/// the boundary. This is not a sandbox against a process concurrently replacing
/// the root or its parents; model/import directories must remain user-owned.
pub fn checked_model_path(directory: &Path, relative: &str) -> Result<PathBuf> {
    if relative.is_empty() || relative.contains(['\\', ':', '\0']) {
        bail!("model artifact path must be relative and portable");
    }
    for component in relative.split('/') {
        let stem = component
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            bail!("model artifact path contains an unsafe component");
        }
    }
    let root = directory
        .canonicalize()
        .context("model directory missing or unreadable")?;
    let path = root.join(relative);
    if !path.starts_with(&root) {
        return Err(anyhow::anyhow!("model artifact path escapes its directory"));
    }
    let mut current = root.clone();
    for component in relative.split('/') {
        current.push(component);
        if !current.starts_with(&root) {
            return Err(anyhow::anyhow!("model artifact path escapes its directory"));
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_link(&metadata) {
                    bail!("model artifact path contains a symlink or reparse point");
                }
                if current != path && !metadata.is_dir() {
                    bail!("model artifact parent is not a directory");
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("cannot inspect model artifact path"),
        }
    }
    // Keep this final containment guard at the returned value, as well as at
    // each metadata operation. Callers can only obtain a checked child path.
    if !path.starts_with(&root) {
        return Err(anyhow::anyhow!("model artifact path escapes its directory"));
    }
    Ok(path)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and other reparse points must not bypass is_symlink().
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub fn model_path_exists(directory: &Path, relative: &str) -> Result<bool> {
    match checked_model_path(directory, relative) {
        Ok(path) => Ok(path.try_exists()?),
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_existing_and_missing_children_stay_under_selected_root() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("optional")).unwrap();
        fs::write(root.path().join("optional/model.onnx"), b"fixture").unwrap();
        let path = checked_model_path(root.path(), "optional/model.onnx").unwrap();
        assert!(path.starts_with(root.path().canonicalize().unwrap()));
        assert!(model_path_exists(root.path(), "optional/model.onnx").unwrap());
        assert!(!model_path_exists(root.path(), "optional/missing.onnx").unwrap());
        assert!(!model_path_exists(&root.path().join("missing"), "model.onnx").unwrap());
        assert!(checked_model_path(root.path(), "optional/.install-123/model.previous").is_ok());
    }

    #[test]
    fn traversal_absolute_device_and_nonportable_names_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        for name in [
            "",
            "..",
            "../outside",
            "/absolute",
            "optional/../outside",
            "optional/./model",
            "optional//model",
            "optional/",
            "C:/outside",
            "C:relative",
            "\\\\server\\share",
            "optional\\model",
            "optional/model:stream",
            "optional/NUL.onnx",
            "COM1",
            "LPT9.txt",
            "optional/name.",
            "optional/name ",
            "optional/\0",
        ] {
            assert!(
                checked_model_path(root.path(), name).is_err(),
                "accepted {name:?}"
            );
        }
    }

    #[test]
    fn a_file_cannot_be_used_as_a_parent_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("optional"), b"not a directory").unwrap();
        assert!(checked_model_path(root.path(), "optional/model.onnx").is_err());
    }

    #[test]
    fn linked_directory_cannot_redirect_reads_or_creates() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(
            outside.path().join("model.onnx"),
            b"outside remains unchanged",
        )
        .unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), root.path().join("optional")).unwrap();
        #[cfg(windows)]
        assert!(std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(root.path().join("optional"))
            .arg(outside.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success());
        for name in [
            "optional/model.onnx",
            "optional/new.onnx",
            "optional/.install-123/new.onnx",
        ] {
            assert!(checked_model_path(root.path(), name).is_err());
            assert!(model_path_exists(root.path(), name).is_err());
        }
        assert_eq!(
            fs::read(outside.path().join("model.onnx")).unwrap(),
            b"outside remains unchanged"
        );
        assert!(!outside.path().join("new.onnx").exists());
    }

    #[cfg(unix)]
    #[test]
    fn file_and_dangling_links_are_rejected_even_within_root() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("real.onnx"), b"fixture").unwrap();
        std::os::unix::fs::symlink(
            root.path().join("real.onnx"),
            root.path().join("model.onnx"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            root.path().join("missing"),
            root.path().join("dangling.onnx"),
        )
        .unwrap();
        assert!(checked_model_path(root.path(), "model.onnx").is_err());
        assert!(checked_model_path(root.path(), "dangling.onnx").is_err());
    }
}
