//! The icons of packages, kept on disk for good. An icon is downloaded once
//! and every later start reads the file, a frontend never asks the network
//! for an icon it already showed.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use tokio::{fs, sync::Semaphore};

use crate::{
    error::{Error, IoContext, Result},
    http::Client,
    ident::VersionedId,
    paths::DataDir,
    util::exists,
};

const EXTENSION: &str = "png";

/// A page of search results asks for many icons at once. A few at a time is
/// kind to the server and still fills a screen in a moment.
static DOWNLOADS: Semaphore = Semaphore::const_new(8);

/// Gives every unfinished download its own file name, so two requests for
/// the same icon never write into one file.
static PARTIAL: AtomicU64 = AtomicU64::new(0);

/// Thunderstore serves the icon of every version under a fixed address, so
/// the package list does not have to carry it.
pub fn icon_url(package: &VersionedId) -> String {
    format!("https://gcdn.thunderstore.io/live/repository/icons/{package}.png")
}

/// Downloaded icons, shared by every profile. One folder per package and one
/// file per version, the shape of the zip cache.
#[derive(Clone, Debug)]
pub struct IconCache {
    dir: PathBuf,
}

impl IconCache {
    pub fn new(data: &DataDir) -> Self {
        Self {
            dir: data.icons_dir(),
        }
    }

    pub fn path(&self, package: &VersionedId) -> PathBuf {
        self.folder(package)
            .join(format!("{}.{EXTENSION}", package.version))
    }

    fn folder(&self, package: &VersionedId) -> PathBuf {
        self.dir.join(package.id.to_string())
    }

    /// The icon file of `package`, downloaded first when the cache does not
    /// have it.
    pub async fn fetch(&self, client: &Client, package: &VersionedId) -> Result<PathBuf> {
        let path = self.path(package);
        if exists(&path).await {
            return Ok(path);
        }
        let permit = DOWNLOADS
            .acquire()
            .await
            .map_err(|error| Error::Invalid(format!("the icon downloads are closed: {error}")))?;

        let folder = self.folder(package);
        fs::create_dir_all(&folder).await.at(&folder)?;

        // A download that stops halfway must never look like a finished
        // icon, so it lands under another name and is renamed at the end.
        let partial = folder.join(format!(
            "{}.{}.part",
            package.version,
            PARTIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let bytes = client.get_bytes(&icon_url(package)).await?;
        fs::write(&partial, bytes).await.at(&partial)?;
        fs::rename(&partial, &path).await.at(&path)?;
        drop(permit);

        remove_other_versions(&folder, &path).await?;
        Ok(path)
    }
}

/// A mod that updates gets a new icon file. The old one is never asked for
/// again, so it goes, and the cache holds one icon per package.
async fn remove_other_versions(folder: &Path, keep: &Path) -> Result<()> {
    let mut entries = fs::read_dir(folder).await.at(folder)?;
    while let Some(entry) = entries.next_entry().await.at(folder)? {
        let path = entry.path();
        let is_icon = path
            .extension()
            .is_some_and(|extension| extension == EXTENSION);
        if is_icon && path != keep {
            fs::remove_file(&path).await.at(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn package(text: &str) -> Result<VersionedId> {
        text.parse()
    }

    #[test]
    fn the_address_follows_the_thunderstore_pattern() -> Result<()> {
        assert_eq!(
            icon_url(&package("denikson-BepInExPack_Valheim-5.4.2350")?),
            "https://gcdn.thunderstore.io/live/repository/icons/denikson-BepInExPack_Valheim-5.4.2350.png"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_cached_icon_needs_no_network() -> Result<()> {
        let home = tempdir().at(Path::new("tempdir"))?;
        let cache = IconCache::new(&DataDir::at(home.path().to_path_buf()));
        let package = package("Owner-Mod-1.2.3")?;

        let path = cache.path(&package);
        assert!(path.ends_with("icons/Owner-Mod/1.2.3.png"));
        let folder = cache.folder(&package);
        fs::create_dir_all(&folder).await.at(&folder)?;
        fs::write(&path, b"png").await.at(&path)?;

        // No server answers in a unit test, so a request would fail here.
        assert_eq!(cache.fetch(&Client::new()?, &package).await?, path);
        Ok(())
    }

    #[tokio::test]
    async fn a_new_version_replaces_the_old_icon() -> Result<()> {
        let home = tempdir().at(Path::new("tempdir"))?;
        let folder = home.path().join("Owner-Mod");
        fs::create_dir_all(&folder).await.at(&folder)?;
        let old = folder.join("1.0.0.png");
        let new = folder.join("1.1.0.png");
        let unfinished = folder.join("1.1.0.7.part");
        for path in [&old, &new, &unfinished] {
            fs::write(path, b"png").await.at(path)?;
        }

        remove_other_versions(&folder, &new).await?;

        assert!(!exists(&old).await);
        assert!(exists(&new).await);
        // Another request may still be writing it.
        assert!(exists(&unfinished).await);
        Ok(())
    }
}
