use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Thunderstore package id in the form `Owner-Name`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PackageId {
    owner: String,
    name: String,
}

impl PackageId {
    pub fn new(owner: &str, name: &str) -> Result<Self> {
        if !valid_owner(owner) || !valid_name(name) {
            return Err(Error::InvalidPackageId(format!("{owner}-{name}")));
        }
        Ok(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn with_version(&self, version: Version) -> VersionedId {
        VersionedId {
            id: self.clone(),
            version,
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A team name may hold `-`, for example `LVH-IT`. A package name may not,
/// which is what lets `Owner-Name` be split from the right.
fn valid_owner(owner: &str) -> bool {
    !owner.is_empty()
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && owner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl FromStr for PackageId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (owner, name) = s
            .rsplit_once('-')
            .ok_or_else(|| Error::InvalidPackageId(s.to_owned()))?;
        Self::new(owner, name).map_err(|_| Error::InvalidPackageId(s.to_owned()))
    }
}

impl TryFrom<String> for PackageId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<PackageId> for String {
    fn from(id: PackageId) -> Self {
        id.to_string()
    }
}

impl Display for PackageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.owner, self.name)
    }
}

/// Thunderstore dependency string in the form `Owner-Name-1.2.3`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VersionedId {
    pub id: PackageId,
    pub version: Version,
}

impl FromStr for VersionedId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (id, version) = s
            .rsplit_once('-')
            .ok_or_else(|| Error::InvalidPackageId(s.to_owned()))?;
        Ok(Self {
            id: id.parse()?,
            version: parse_version(version)?,
        })
    }
}

impl TryFrom<String> for VersionedId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<VersionedId> for String {
    fn from(id: VersionedId) -> Self {
        id.to_string()
    }
}

impl Display for VersionedId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.id, self.version)
    }
}

/// Thunderstore versions are 3 plain numbers. Unlike strict semver they may
/// carry leading zeros, so `semver::Version::parse` would reject some of them.
pub fn parse_version(s: &str) -> Result<Version> {
    let bad = || Error::InvalidVersion(s.to_owned());
    let mut parts = s
        .split('.')
        .map(|part| part.parse::<u64>().map_err(|_| bad()));
    let major = parts.next().ok_or_else(bad)??;
    let minor = parts.next().ok_or_else(bad)??;
    let patch = parts.next().ok_or_else(bad)??;
    if parts.next().is_some() {
        return Err(bad());
    }
    Ok(Version::new(major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_id() -> Result<()> {
        let id: PackageId = "denikson-BepInExPack_Valheim".parse()?;
        assert_eq!(id.owner(), "denikson");
        assert_eq!(id.name(), "BepInExPack_Valheim");
        assert_eq!(id.to_string(), "denikson-BepInExPack_Valheim");
        Ok(())
    }

    #[test]
    fn parses_versioned_id() -> Result<()> {
        let dep: VersionedId = "denikson-BepInExPack_Valheim-5.4.2350".parse()?;
        assert_eq!(dep.id.to_string(), "denikson-BepInExPack_Valheim");
        assert_eq!(dep.version, Version::new(5, 4, 2350));
        Ok(())
    }

    #[test]
    fn owner_may_hold_a_dash() -> Result<()> {
        let dep: VersionedId = "LVH-IT-UseEquipmentInWater-0.2.3".parse()?;
        assert_eq!(dep.id.owner(), "LVH-IT");
        assert_eq!(dep.id.name(), "UseEquipmentInWater");
        assert_eq!(dep.to_string(), "LVH-IT-UseEquipmentInWater-0.2.3");
        Ok(())
    }

    #[test]
    fn rejects_bad_ids() {
        assert!("nodash".parse::<PackageId>().is_err());
        assert!("owner-bad.name".parse::<PackageId>().is_err());
        assert!("-name".parse::<PackageId>().is_err());
        assert!("owner-name-1.2".parse::<VersionedId>().is_err());
    }

    #[test]
    fn accepts_leading_zeros() -> Result<()> {
        assert_eq!(parse_version("1.02.3")?, Version::new(1, 2, 3));
        Ok(())
    }
}
