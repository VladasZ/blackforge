use std::{io::ErrorKind, path::Path};

use blackforge_api::setup::Setup;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, to_vec};
use tokio::fs;

use crate::error::{Error, IoContext, Result};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct History {
    pub local: Setup,
    pub cloud: Setup,
}

impl History {
    pub async fn read(root: &Path, account: &str) -> Result<Self> {
        let path = root.join(file_name(account)?);
        match fs::read(&path).await {
            Ok(bytes) => Ok(from_slice(&bytes)?),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(cause) => Err(Error::Io { path, cause }),
        }
    }

    pub async fn save(&self, root: &Path, account: &str) -> Result<()> {
        let path = root.join(file_name(account)?);
        let temp = path.with_extension("tmp");
        fs::write(&temp, to_vec(self)?).await.at(&temp)?;
        fs::rename(&temp, &path).await.at(&path)
    }
}

fn file_name(account: &str) -> Result<String> {
    if account.is_empty()
        || !account
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Error::Invalid("invalid cloud account id".to_owned()));
    }
    Ok(format!("cloud-{account}.json"))
}
