use std::{
    fs::{read_dir, remove_dir},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use tokio::fs;

use crate::error::{IoContext, Result};

/// How long ago the file was written. `None` when it does not exist.
pub async fn file_age(path: &Path) -> Option<Duration> {
    let modified = fs::metadata(path).await.ok()?.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

pub async fn exists(path: &Path) -> bool {
    fs::try_exists(path).await.unwrap_or(false)
}

/// Every file under `root`, as paths relative to `root`, sorted.
pub fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in read_dir(&dir).at(&dir)? {
            let entry = entry.at(&dir)?;
            let path = entry.path();
            if entry.file_type().at(&path)?.is_dir() {
                pending.push(path);
            } else if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_path_buf());
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Removes `dir` and then every parent that became empty, up to `stop`.
pub fn remove_empty_parents(dir: &Path, stop: &Path) -> Result<()> {
    let mut current = Some(dir);
    while let Some(dir) = current {
        if dir == stop || !dir.starts_with(stop) {
            break;
        }
        let is_empty = match read_dir(dir) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => break,
        };
        if !is_empty {
            break;
        }
        remove_dir(dir).at(dir)?;
        current = dir.parent();
    }
    Ok(())
}
