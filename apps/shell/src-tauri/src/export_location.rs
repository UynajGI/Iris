use std::path::{Path, PathBuf};

/// Resolve an existing export directory; a file target only ever opens its parent.
pub fn export_folder(path: &Path, file: bool) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("导出位置须为本地绝对路径".into());
    }
    let resolved = path
        .canonicalize()
        .map_err(|_| "导出位置已不存在或无法访问，请检查磁盘连接".to_string())?;
    let directory = if file {
        if !resolved.is_file() {
            return Err("导出的文件已不存在".into());
        }
        resolved.parent().ok_or("无法确定导出文件所在目录")?
    } else {
        &resolved
    };
    if !directory.is_dir() {
        return Err("导出位置不是文件夹".into());
    }
    // Explorer does not consistently accept the extended-length prefix returned by canonicalize.
    #[cfg(windows)]
    {
        let text = directory.to_string_lossy();
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            return Ok(PathBuf::from(format!(r"\\{unc}")));
        }
        if let Some(disk) = text.strip_prefix(r"\\?\") {
            return Ok(PathBuf::from(disk));
        }
    }
    Ok(directory.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_only_directories_and_file_parents() {
        let root =
            std::env::temp_dir().join(format!("iris-export-location-{}", std::process::id()));
        let folder = root.join("导出 folder");
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("selection.csv");
        std::fs::write(&file, b"synthetic fixture").unwrap();
        let direct = export_folder(&folder, false).unwrap();
        assert_eq!(direct, export_folder(&file, true).unwrap());
        assert!(direct.is_absolute() && direct.is_dir());
        assert!(export_folder(&file, false).is_err());
        assert!(export_folder(&folder, true).is_err());
        assert!(export_folder(&folder.join("missing"), false).is_err());
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(folder).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn rejects_urls_relative_paths_and_launcher_options() {
        for value in [
            "",
            "..",
            "selection.csv",
            "https://example.com",
            "file:///tmp",
            "--help",
        ] {
            assert!(export_folder(Path::new(value), false).is_err());
        }
    }
}
