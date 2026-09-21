use std::{
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{fix::Fix, world::Malformed};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{path}: {cause}")]
    Io { path: PathBuf, cause: io::Error },
    #[error("network request failed: {0}")]
    Http(reqwest::Error),
    #[error("bad json: {0}")]
    Json(serde_json::Error),
    #[error("bad toml: {0}")]
    TomlRead(toml::de::Error),
    #[error("cannot write toml: {0}")]
    TomlWrite(toml::ser::Error),
    #[error("bad yaml: {0}")]
    Yaml(serde_yaml_ng::Error),
    #[error("{path}: not a readable Valheim world, {reason}")]
    World { path: PathBuf, reason: Malformed },
    #[error("bad zip archive: {0}")]
    Zip(zip::result::ZipError),
    #[error("a background task failed: {0}")]
    Task(tokio::task::JoinError),
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
    #[error("no profile is the active one")]
    NoActiveProfile,
    #[error("mod '{0}' is not in the manifest")]
    NotInManifest(String),
    #[error("the package list is not downloaded yet")]
    IndexMissing,
    #[error("the mod loader is not installed, the mods are not synced yet")]
    LoaderMissing,
    #[error("the progress receiver was dropped, the operation was cancelled")]
    Cancelled,
    #[error("not signed in, or the session ended. Sign in again")]
    NotSignedIn,
    /// The blackforge server said no, in words meant for the user.
    #[error("{0}")]
    Server(String),
    #[error("not supported: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Invalid(String),
}

// The messages above hold the cause. thiserror would also put a `#[from]`
// field or a field named `source` into the error chain, and both frontends
// print the whole chain, so the cause would show twice.
macro_rules! from_cause {
    ($($variant:ident($cause:ty),)*) => {
        $(impl From<$cause> for Error {
            fn from(cause: $cause) -> Self {
                Self::$variant(cause)
            }
        })*
    };
}

from_cause! {
    Http(reqwest::Error),
    Json(serde_json::Error),
    TomlRead(toml::de::Error),
    TomlWrite(toml::ser::Error),
    Yaml(serde_yaml_ng::Error),
    Zip(zip::result::ZipError),
    Task(tokio::task::JoinError),
}

impl Error {
    /// What the user can do inside blackforge about this error, when one
    /// action solves it.
    pub fn fix(&self) -> Option<Fix> {
        match self {
            Self::NoActiveProfile => Some(Fix::PickProfile),
            Self::GameNotInstalled(_) => Some(Fix::GiveGameFolder),
            Self::LoaderMissing => Some(Fix::Sync),
            _ => None,
        }
    }
}

pub trait IoContext<T> {
    fn at(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn at(self, path: &Path) -> Result<T> {
        self.map_err(|cause| Error::Io {
            path: path.to_path_buf(),
            cause,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use reqwest::Client;
    use zip::result::ZipError;

    use super::*;

    // Both frontends print the whole chain of an error. The messages here
    // already hold the cause, so a cause in the chain as well shows up twice.
    #[test]
    fn a_cause_is_in_the_message_and_not_in_the_chain() {
        let io: Result<()> = Err(io::Error::other("the disk is gone")).at(Path::new("file"));
        let errors = [
            io.err(),
            serde_json::from_str::<u32>("x").err().map(Error::from),
            toml::from_str::<toml::Table>("=").err().map(Error::from),
            serde_yaml_ng::from_str::<u32>("x").err().map(Error::from),
            Client::new()
                .get("no address")
                .build()
                .err()
                .map(Error::from),
            Some(Error::from(ZipError::FileNotFound)),
        ];
        for error in errors {
            let error = error.expect("every sample is an error");
            assert!(error.source().is_none(), "{error}");
        }
    }
}
