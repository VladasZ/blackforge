use std::path::PathBuf;

use tokio::fs;

use crate::{
    error::{IoContext, Result},
    http::Client,
    ident::VersionedId,
    paths::DataDir,
    progress::{Event, Progress},
    thunderstore::download_url,
    util::exists,
};

/// Downloaded mod zips, shared by every profile.
#[derive(Clone, Debug)]
pub struct ZipCache {
    dir: PathBuf,
}

impl ZipCache {
    pub fn new(data: &DataDir) -> Self {
        Self {
            dir: data.cache_dir(),
        }
    }

    pub fn path(&self, package: &VersionedId) -> PathBuf {
        self.dir
            .join(package.id.to_string())
            .join(format!("{}.zip", package.version))
    }

    /// The zip of `package`, downloaded first when the cache does not have it.
    pub async fn fetch(
        &self,
        client: &Client,
        package: &VersionedId,
        progress: &Progress,
    ) -> Result<PathBuf> {
        let path = self.path(package);
        if exists(&path).await {
            return Ok(path);
        }
        let folder = self.dir.join(package.id.to_string());
        fs::create_dir_all(&folder).await.at(&folder)?;

        // A download that stops halfway must never look like a finished zip,
        // so it lands under another name and is renamed at the end.
        let partial = folder.join(format!("{}.zip.part", package.version));
        client
            .download(
                &download_url(package),
                &partial,
                |total_bytes| {
                    progress.send(Event::DownloadStarted {
                        package: package.clone(),
                        total_bytes,
                    })
                },
                |bytes| {
                    progress.send(Event::DownloadProgress {
                        package: package.clone(),
                        bytes,
                    })
                },
            )
            .await?;
        fs::rename(&partial, &path).await.at(&path)?;
        progress.send(Event::DownloadFinished {
            package: package.clone(),
        })?;
        Ok(path)
    }
}
