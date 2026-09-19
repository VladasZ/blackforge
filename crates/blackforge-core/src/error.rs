use std::{
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("network request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("bad json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bad toml: {0}")]
    TomlRead(#[from] toml::de::Error),
    #[error("cannot write toml: {0}")]
    TomlWrite(#[from] toml::ser::Error),
    #[error("bad yaml: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("bad zip archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("a background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
    #[error("'{0}' is not a valid package id, expected Owner-Name")]
    InvalidPackageId(String),
    #[error("'{0}' is not a valid version, expected 1.2.3")]
    InvalidVersion(String),
    #[error("package '{0}' is not on Thunderstore")]
    PackageNotFound(String),
    #[error("package '{id}' has no version {version}")]
    VersionNotFound { id: String, version: String },
    #[error("game '{0}' is not in the Thunderstore schema")]
    UnknownGame(String),
    #[error("game '{game}' has no {target} entry in the Thunderstore schema")]
    UnknownTarget { game: String, target: String },
    #[error("{0} is not installed, or Steam could not be found")]
    GameNotInstalled(String),
    #[error("profile '{0}' does not exist")]
    ProfileNotFound(String),
    #[error("profile '{0}' already exists")]
    ProfileExists(String),
    #[error("'{0}' is not a valid profile name, use letters, digits, '-' and '_'")]
    InvalidProfileName(String),
    #[error("no active profile, pick one with 'blackforge profile switch <name>'")]
    NoActiveProfile,
    #[error("mod '{0}' is not in the manifest")]
    NotInManifest(String),
    #[error("the package list is not downloaded yet")]
    IndexMissing,
    #[error("the profile has no mod loader, run 'blackforge sync' first")]
    LoaderMissing,
    #[error("the progress receiver was dropped, the operation was cancelled")]
    Cancelled,
    #[error("not supported: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Invalid(String),
}

pub trait IoContext<T> {
    fn at(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn at(self, path: &Path) -> Result<T> {
        self.map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}
