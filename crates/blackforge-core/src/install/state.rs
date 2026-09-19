use std::{collections::BTreeMap, path::Path};

use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    error::{IoContext, Result},
    ident::PackageId,
    util::exists,
};

pub const STATE_FILE: &str = "installed.json";

/// What `sync` wrote into a tree, so the next run can remove exactly that.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    #[serde(default)]
    pub packages: BTreeMap<PackageId, InstalledPackage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub version: Version,
    /// Tracked files, relative to the tree root, with `/` on every system.
    pub files: Vec<String>,
}

impl Installed {
    pub async fn read(tree: &Path) -> Result<Self> {
        let path = tree.join(STATE_FILE);
        if !exists(&path).await {
            return Ok(Self::default());
        }
        let bytes = fs::read(&path).await.at(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn write(&self, tree: &Path) -> Result<()> {
        let path = tree.join(STATE_FILE);
        fs::write(&path, serde_json::to_vec_pretty(self)?)
            .await
            .at(&path)
    }
}
