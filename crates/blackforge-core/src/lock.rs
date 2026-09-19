use std::path::Path;

use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    error::{Error, IoContext, Result},
    ident::{PackageId, VersionedId},
    util::exists,
};

pub const LOCK_FILE: &str = "blackforge.lock";
const LOCK_VERSION: u32 = 1;

/// `blackforge.lock`, the exact version of every mod, dependencies included.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    #[serde(default, rename = "package")]
    pub packages: Vec<LockedPackage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedPackage {
    pub id: PackageId,
    pub version: Version,
    #[serde(default)]
    pub dependencies: Vec<PackageId>,
}

impl LockedPackage {
    pub fn versioned(&self) -> VersionedId {
        self.id.with_version(self.version.clone())
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: LOCK_VERSION,
            packages: Vec::new(),
        }
    }
}

impl Lockfile {
    /// Packages are kept sorted by id so the file gives clean diffs.
    pub fn new(mut packages: Vec<LockedPackage>) -> Self {
        packages.sort_by(|a, b| a.id.cmp(&b.id));
        Self {
            version: LOCK_VERSION,
            packages,
        }
    }

    /// An empty lock when the file does not exist yet.
    pub async fn read(path: &Path) -> Result<Self> {
        if !exists(path).await {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path).await.at(path)?;
        let lock: Self = toml::from_str(&text)?;
        if lock.version > LOCK_VERSION {
            return Err(Error::Unsupported(format!(
                "lock file version {} is newer than this blackforge understands",
                lock.version
            )));
        }
        Ok(lock)
    }

    pub async fn write(&self, path: &Path) -> Result<()> {
        fs::write(path, toml::to_string(self)?).await.at(path)
    }

    pub fn get(&self, id: &PackageId) -> Option<&LockedPackage> {
        self.packages.iter().find(|package| &package.id == id)
    }
}
