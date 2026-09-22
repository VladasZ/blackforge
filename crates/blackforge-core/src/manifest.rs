use std::{collections::BTreeMap, path::Path};

use blackforge_api::setup::LEGACY_PIN_SERVER;
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

/// One entry of `[mods]`. A mod that follows the newest version and is
/// enabled is written as `"*"`, anything else as a table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ModSpecRepr", into = "ModSpecRepr")]
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
    Short(String),
    Full {
        version: String,
        #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
    },
}

fn enabled_by_default() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_true(value: &bool) -> bool {
    *value
}

impl TryFrom<ModSpecRepr> for ModSpec {
    type Error = Error;

    fn try_from(repr: ModSpecRepr) -> Result<Self> {
        Ok(match repr {
            ModSpecRepr::Short(version) => Self::new(VersionReq::from_parts(&version, None)?),
            ModSpecRepr::Full {
                version,
                enabled,
                server,
            } => Self {
                version: VersionReq::from_parts(&version, server)?,
                enabled,
            },
        })
    }
}

impl From<ModSpec> for ModSpecRepr {
    fn from(spec: ModSpec) -> Self {
        match (spec.version, spec.enabled) {
            (VersionReq::Latest, true) => Self::Short("*".to_owned()),
            (version, enabled) => Self::Full {
                version: version.version_text(),
                enabled,
                server: version.server().map(ToOwned::to_owned),
            },
        }
    }
}

/// Thunderstore has no version ranges, so a mod either follows the newest
/// version through the lock file or is pinned to the one a server needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum VersionReq {
    #[default]
    Latest,
    Pinned(Pin),
}

/// A mod held at the version a server needs. There is no pin without a
/// server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub version: Version,
    pub server: String,
}

impl VersionReq {
    /// From the version text, `*` or an exact version, and the server name
    /// that files and synced setups store next to it. A pin with no server
    /// is an old one and belongs to `LEGACY_PIN_SERVER`.
    pub fn from_parts(version: &str, server: Option<String>) -> Result<Self> {
        if version == "*" {
            return Ok(Self::Latest);
        }
        Ok(Self::Pinned(Pin {
            version: parse_version(version)?,
            server: server.unwrap_or_else(|| LEGACY_PIN_SERVER.to_owned()),
        }))
    }

    /// `*` or the pinned version, the text files and synced setups store.
    pub fn version_text(&self) -> String {
        match self {
            Self::Latest => "*".to_owned(),
            Self::Pinned(pin) => pin.version.to_string(),
        }
    }

    pub fn server(&self) -> Option<&str> {
        match self {
            Self::Latest => None,
            Self::Pinned(pin) => Some(&pin.server),
        }
    }

    pub fn pinned_version(&self) -> Option<&Version> {
        match self {
            Self::Latest => None,
            Self::Pinned(pin) => Some(&pin.version),
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
            VersionReq::Pinned(Pin {
                version: Version::new(0, 9, 1),
                server: LEGACY_PIN_SERVER.to_owned(),
            })
        );

        let again: Manifest = toml::from_str(&toml::to_string(&manifest)?)?;
        assert_eq!(again, manifest);
        Ok(())
    }

    #[test]
    fn a_server_pin_keeps_its_server() -> Result<()> {
        let text = r#"game = "valheim"

[mods]
Crystal-DigDeeper = { version = "1.3.1", server = "Durka" }
"#;
        let manifest: Manifest = toml::from_str(text)?;
        let dig: PackageId = "Crystal-DigDeeper".parse()?;
        assert_eq!(manifest.mods[&dig].version.server(), Some("Durka"));
        assert!(manifest.is_enabled(&dig));

        let written = toml::to_string(&manifest)?;
        assert!(!written.contains("enabled"), "{written}");
        let again: Manifest = toml::from_str(&written)?;
        assert_eq!(again, manifest);
        Ok(())
    }

    #[test]
    fn an_old_pin_becomes_a_durka_pin_and_is_written_with_it() -> Result<()> {
        let manifest: Manifest = toml::from_str(
            "game = \"valheim\"

[mods]
Crystal-DigDeeper = \"1.3.1\"
",
        )?;
        let dig: PackageId = "Crystal-DigDeeper".parse()?;
        assert_eq!(
            manifest.mods[&dig].version.server(),
            Some(LEGACY_PIN_SERVER)
        );
        let written = toml::to_string(&manifest)?;
        assert!(written.contains("server = \"Durka\""), "{written}");
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
