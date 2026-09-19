use std::{
    env::var_os,
    path::{Path, PathBuf},
};

use crate::error::{Error, Result};

pub const HOME_ENV: &str = "BLACKFORGE_HOME";

/// The one folder where blackforge keeps everything: the global config, all
/// profiles, the download cache and the package list.
#[derive(Clone, Debug)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// `BLACKFORGE_HOME` wins when set, otherwise `~/.config/blackforge`, the
    /// same path on every system.
    pub fn locate() -> Result<Self> {
        if let Some(home) = var_os(HOME_ENV) {
            return Ok(Self::at(PathBuf::from(home)));
        }
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Invalid("this system has no home folder".to_owned()))?;
        Ok(Self::at(home.join(".config").join("blackforge")))
    }

    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn index_dir(&self) -> PathBuf {
        self.root.join("index")
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.root.join("profiles")
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }
}
