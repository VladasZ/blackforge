use std::{io::ErrorKind, path::Path};

use blackforge_api::setup::Setup;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, to_vec};
use tokio::fs;

use crate::error::{Error, IoContext, Result};

/// The setup this machine and the cloud last agreed on, per account.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Baseline {
    pub setup: Setup,
}

impl Baseline {
    /// None on a machine that never synced this account.
    pub async fn read(root: &Path, account: &str) -> Result<Option<Self>> {
        let path = root.join(file_name("sync", account)?);
        match fs::read(&path).await {
            Ok(bytes) => Ok(Some(from_slice(&bytes)?)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(cause) => Err(Error::Io { path, cause }),
        }
    }

    pub async fn save(&self, root: &Path, account: &str) -> Result<()> {
        let path = root.join(file_name("sync", account)?);
        let temp = path.with_extension("tmp");
        fs::write(&temp, to_vec(self)?).await.at(&temp)?;
        fs::rename(&temp, &path).await.at(&path)?;
        // Releases up to 0.1.8 kept two snapshots under this name.
        let old = root.join(file_name("cloud", account)?);
        match fs::remove_file(&old).await {
            Err(cause) if cause.kind() != ErrorKind::NotFound => {
                Err(Error::Io { path: old, cause })
            }
            _ => Ok(()),
        }
    }
}

fn file_name(prefix: &str, account: &str) -> Result<String> {
    if account.is_empty()
        || !account
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Error::Invalid("invalid cloud account id".to_owned()));
    }
    Ok(format!("{prefix}-{account}.json"))
}
