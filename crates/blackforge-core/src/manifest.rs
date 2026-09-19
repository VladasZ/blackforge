use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    path::Path,
    str::FromStr,
};

use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    error::{Error, IoContext, Result},
    game::Target,
    ident::{PackageId, parse_version},
};

pub const MANIFEST_FILE: &str = "blackforge.toml";

/// `blackforge.toml`, the mods the user asked for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub game: String,
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub mods: BTreeMap<PackageId, ModSpec>,
}

impl Manifest {
    pub fn new(game: &str, target: Target) -> Self {
        Self {
            game: game.to_owned(),
            target,
            mods: BTreeMap::new(),
        }
    }

    pub async fn read(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).await.at(path)?;
        Ok(toml::from_str(&text)?)
    }

    pub async fn write(&self, path: &Path) -> Result<()> {
        fs::write(path, toml::to_string(self)?).await.at(path)
    }

    pub fn require_mut(&mut self, id: &PackageId) -> Result<&mut ModSpec> {
        self.mods
            .get_mut(id)
            .ok_or_else(|| Error::NotInManifest(id.to_string()))
    }

    pub fn is_enabled(&self, id: &PackageId) -> bool {
        self.mods.get(id).is_none_or(|spec| spec.enabled)
    }
}

/// One entry of `[mods]`. It is written as a plain version string while the
/// mod is enabled, and as a table once it is disabled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "ModSpecRepr", into = "ModSpecRepr")]
pub struct ModSpec {
    pub version: VersionReq,
    pub enabled: bool,
}

impl ModSpec {
    pub fn new(version: VersionReq) -> Self {
        Self {
            version,
            enabled: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ModSpecRepr {
    Short(VersionReq),
    Full {
        version: VersionReq,
        #[serde(default = "enabled_by_default")]
        enabled: bool,
    },
}

fn enabled_by_default() -> bool {
    true
}

impl From<ModSpecRepr> for ModSpec {
    fn from(repr: ModSpecRepr) -> Self {
        match repr {
            ModSpecRepr::Short(version) => Self {
                version,
                enabled: true,
            },
            ModSpecRepr::Full { version, enabled } => Self { version, enabled },
        }
    }
}

impl From<ModSpec> for ModSpecRepr {
    fn from(spec: ModSpec) -> Self {
        if spec.enabled {
            Self::Short(spec.version)
        } else {
            Self::Full {
                version: spec.version,
                enabled: false,
            }
        }
    }
}

/// Thunderstore has no version ranges, so a mod is either pinned to one
/// version or follows the newest one through the lock file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum VersionReq {
    #[default]
    Latest,
    Exact(Version),
}

impl FromStr for VersionReq {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s == "*" {
            Ok(Self::Latest)
        } else {
            Ok(Self::Exact(parse_version(s)?))
        }
    }
}

impl TryFrom<String> for VersionReq {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<VersionReq> for String {
    fn from(req: VersionReq) -> Self {
        req.to_string()
    }
}

impl Display for VersionReq {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Latest => f.write_str("*"),
            Self::Exact(version) => write!(f, "{version}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_both_mod_forms() -> Result<()> {
        let text = r#"game = "valheim"
target = "server"

[mods]
Azumatt-AzuCraftyBoxes = "*"
denikson-BepInExPack_Valheim = "5.4.2350"

[mods.RandyKnapp-EpicLoot]
version = "0.9.1"
enabled = false
"#;
        let manifest: Manifest = toml::from_str(text)?;
        assert_eq!(manifest.target, Target::Server);
        assert_eq!(manifest.mods.len(), 3);
        let epic: PackageId = "RandyKnapp-EpicLoot".parse()?;
        assert!(!manifest.is_enabled(&epic));
        assert_eq!(
            manifest.mods[&epic].version,
            VersionReq::Exact(Version::new(0, 9, 1))
        );

        let again: Manifest = toml::from_str(&toml::to_string(&manifest)?)?;
        assert_eq!(again, manifest);
        Ok(())
    }

    #[test]
    fn target_defaults_to_client() -> Result<()> {
        let manifest: Manifest = toml::from_str("game = \"valheim\"\n")?;
        assert_eq!(manifest.target, Target::Client);
        assert!(manifest.mods.is_empty());
        Ok(())
    }
}
