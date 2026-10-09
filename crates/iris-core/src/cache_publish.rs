//! Publish a completed cache file without replacing another publisher's result.
use std::{io, path::Path};

/// The caller must finish and close `temp` before publishing it beside `dest`.
/// On success `dest` names the complete file; an existing destination is never
/// replaced. Windows consumes `temp`, whereas the hard-link fallback retains it.
/// The caller may therefore attempt to remove `temp` after either result.
/// Errors retain the operating system's diagnostic and leave cleanup to the caller.
pub(crate) fn publish_noclobber(temp: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(existing: *const u16, destination: *const u16, flags: u32) -> i32;
        }

        fn wide(path: &Path) -> io::Result<Vec<u16>> {
            // Canonicalizing the existing parent supplies a verbatim absolute
            // Windows path, preserving std::fs support for long cache paths.
            let filename = path.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "cache path needs a filename")
            })?;
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
            let absolute = parent
                .unwrap_or(Path::new("."))
                .canonicalize()?
                .join(filename);
            let mut encoded: Vec<u16> = OsStr::new(&absolute).encode_wide().collect();
            if encoded.contains(&0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "cache path contains a NUL character",
                ));
            }
            encoded.push(0);
            Ok(encoded)
        }

        let source = wide(temp)?;
        let destination = wide(dest)?;
        // No REPLACE_EXISTING and no COPY_ALLOWED: publication is a same-volume
        // rename, never an overwrite or a copy exposing an incomplete target.
        // SAFETY: both buffers are NUL-terminated and live throughout the call.
        if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::hard_link(temp, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Barrier,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "iris-cache-publish-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            if let (Ok(path), Ok(root)) =
                (self.0.canonicalize(), std::env::temp_dir().canonicalize())
            {
                if path.parent() == Some(root.as_path())
                    && path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with("iris-cache-publish-")
                    })
                {
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
    }

    #[test]
    fn publishes_complete_file_and_allows_temporary_cleanup() {
        let dir = Directory::new();
        let temp = dir.file("complete.tmp");
        let dest = dir.file("preview.jpg");
        fs::write(&temp, b"complete cache contents").unwrap();
        publish_noclobber(&temp, &dest).unwrap();
        let _ = fs::remove_file(&temp);
        assert_eq!(fs::read(&dest).unwrap(), b"complete cache contents");
        assert!(!temp.exists());
    }

    #[test]
    fn existing_destination_is_unchanged_and_source_remains() {
        let dir = Directory::new();
        let temp = dir.file("loser.tmp");
        let dest = dir.file("preview.jpg");
        fs::write(&temp, b"new").unwrap();
        fs::write(&dest, b"winner").unwrap();
        let error = publish_noclobber(&temp, &dest).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(error.raw_os_error().is_some());
        assert_eq!(fs::read(&dest).unwrap(), b"winner");
        assert_eq!(fs::read(&temp).unwrap(), b"new");
    }

    #[test]
    fn concurrent_publishers_expose_only_one_complete_result() {
        let dir = Directory::new();
        let dest = dir.file("preview.jpg");
        let barrier = Arc::new(Barrier::new(9));
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for index in 0..8u8 {
                let temp = dir.file(&format!("{index}.tmp"));
                fs::write(&temp, vec![index; 128 * 1024]).unwrap();
                let dest = &dest;
                let barrier = barrier.clone();
                handles.push(scope.spawn(move || {
                    barrier.wait();
                    let result = publish_noclobber(&temp, dest);
                    if let Err(error) = &result {
                        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
                    }
                    let _ = fs::remove_file(temp);
                    result.is_ok()
                }));
            }
            barrier.wait();
            // Read concurrently with publication, not only after all writers exit.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                assert!(
                    std::time::Instant::now() < deadline,
                    "no publisher produced a destination"
                );
                match fs::read(&dest) {
                    Ok(bytes) => {
                        assert_eq!(bytes.len(), 128 * 1024);
                        assert!(bytes[0] < 8);
                        assert!(bytes.iter().all(|byte| *byte == bytes[0]));
                        break;
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        std::thread::yield_now()
                    }
                    Err(error) => panic!("unexpected destination read error: {error}"),
                }
            }
            assert_eq!(
                handles
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .filter(|ok| *ok)
                    .count(),
                1
            );
        });
    }

    #[test]
    fn missing_source_reports_os_error_without_creating_destination() {
        let dir = Directory::new();
        let dest = dir.file("preview.jpg");
        let error = publish_noclobber(&dir.file("missing.tmp"), &dest).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.raw_os_error().is_some());
        assert!(!dest.exists());
    }

    #[test]
    fn missing_destination_parent_keeps_completed_source() {
        let dir = Directory::new();
        let temp = dir.file("complete.tmp");
        fs::write(&temp, b"complete").unwrap();
        let error = publish_noclobber(&temp, &dir.file("missing/preview.jpg")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.raw_os_error().is_some());
        assert_eq!(fs::read(&temp).unwrap(), b"complete");
    }

    #[cfg(windows)]
    #[test]
    fn nul_paths_are_rejected_without_publishing() {
        let dir = Directory::new();
        let temp = dir.file("complete.tmp");
        let dest = dir.file("preview.jpg");
        fs::write(&temp, b"complete").unwrap();
        let error = publish_noclobber(&dir.file("source\0.tmp"), &dest).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let error = publish_noclobber(&temp, &dir.file("preview\0.jpg")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&temp).unwrap(), b"complete");
        assert!(!dest.exists());
    }

    #[cfg(windows)]
    #[test]
    fn publishes_unicode_names_beyond_legacy_windows_path_limit() {
        let dir = Directory::new();
        let parent = dir
            .0
            .join("a".repeat(100))
            .join("b".repeat(100))
            .join("c".repeat(100));
        fs::create_dir_all(&parent).unwrap();
        let temp = parent.join("缓存.tmp");
        let dest = parent.join("预览.jpg");
        fs::write(&temp, b"complete long-path cache").unwrap();
        publish_noclobber(&temp, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"complete long-path cache");
        assert!(!temp.exists());
    }
}
