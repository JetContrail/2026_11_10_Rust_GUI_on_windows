use std::fs;
use std::io::Write;
use std::path::Path;

/// Writes `data` to a sibling temporary file and renames it over `path`,
/// so an interrupted save never leaves a truncated file behind.
pub(crate) fn write_atomically(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{file_name}.tmp"));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        write_atomically(&path, b"one").unwrap();
        write_atomically(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn fails_for_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(write_atomically(&dir.path().join("no/such/file"), b"x").is_err());
    }
}
