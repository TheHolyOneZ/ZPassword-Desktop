use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = parent_dir(path);
    fs::create_dir_all(&dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".zpass-")
        .suffix(".tmp")
        .tempfile_in(&dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| Error::Io(e.error))?;
    #[cfg(unix)]
    if let Ok(d) = File::open(&dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

fn parent_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

pub fn backup_dir(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vault".into());
    parent_dir(path).join(format!("{name}.backups"))
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: PathBuf,
    pub modified: i64,
    pub size: u64,
}

pub fn rotate_backup(path: &Path, keep: usize) -> Result<()> {
    if keep == 0 || !path.exists() {
        return Ok(());
    }
    let dir = backup_dir(path);
    fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let target = dir.join(format!("{stamp:015}.zpv"));
    let bytes = fs::read(path)?;
    atomic_write(&target, &bytes)?;
    let backups = list_backups(path)?;
    for old in backups.into_iter().skip(keep) {
        let _ = fs::remove_file(old.path);
    }
    Ok(())
}

pub fn list_backups(path: &Path) -> Result<Vec<BackupInfo>> {
    let dir = backup_dir(path);
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(out);
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "zpv") {
            let meta = e.metadata()?;
            let modified = meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.push(BackupInfo {
                path: p,
                modified,
                size: meta.len(),
            });
        }
    }
    out.sort_by(|a, b| b.path.cmp(&a.path));
    Ok(out)
}

#[derive(Debug)]
pub struct VaultLock {
    _file: File,
}

impl VaultLock {
    pub fn acquire(vault_path: &Path) -> Result<Self> {
        let mut name = vault_path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        name.push(".lock");
        let lock_path = parent_dir(vault_path).join(name);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(Error::Locked),
            Err(TryLockError::Error(e)) => Err(Error::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_and_backups() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("v.zpv");
        atomic_write(&p, b"one").unwrap();
        for i in 0..5 {
            rotate_backup(&p, 3).unwrap();
            atomic_write(&p, format!("v{i}").as_bytes()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(fs::read(&p).unwrap(), b"v4");
        let b = list_backups(&p).unwrap();
        assert_eq!(b.len(), 3);
        assert_eq!(fs::read(&b[0].path).unwrap(), b"v3");
    }

    #[test]
    fn lock_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("v.zpv");
        let _a = VaultLock::acquire(&p).unwrap();
        assert!(matches!(VaultLock::acquire(&p), Err(Error::Locked)));
    }
}
