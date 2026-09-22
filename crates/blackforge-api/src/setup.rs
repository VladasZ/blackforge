//! Private setup storage with its full history. Never exposed through the
//! friends API.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display, Formatter},
};

use serde::{Deserialize, Serialize};

pub type Settings = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setup {
    pub mods: BTreeMap<String, Mod>,
    pub configs: Settings,
}

/// Every pin written before pins named their server was made for Durka.
pub const LEGACY_PIN_SERVER: &str = "Durka";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "ModRepr")]
pub struct Mod {
    pub version: String,
    /// None for a dependency; `*` or an exact version for an explicitly chosen mod.
    pub requested: Option<String>,
    pub enabled: bool,
    pub dependencies: Vec<String>,
    /// The server whose install pinned the mod, set exactly when `requested`
    /// is an exact version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
}

#[derive(Deserialize)]
struct ModRepr {
    version: String,
    requested: Option<String>,
    enabled: bool,
    dependencies: Vec<String>,
    #[serde(default)]
    server: Option<String>,
}

/// An old app still sends and stores pins without a server, so every read
/// gives them `LEGACY_PIN_SERVER`.
impl From<ModRepr> for Mod {
    fn from(repr: ModRepr) -> Self {
        let pinned = repr
            .requested
            .as_deref()
            .is_some_and(|requested| requested != "*");
        let server = match (pinned, repr.server) {
            (true, None) => Some(LEGACY_PIN_SERVER.to_owned()),
            (true, server) => server,
            (false, _) => None,
        };
        Self {
            version: repr.version,
            requested: repr.requested,
            enabled: repr.enabled,
            dependencies: repr.dependencies,
            server,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub revision: i64,
    /// Host name of the machine that saved it.
    pub machine: String,
    /// Seconds since the Unix epoch.
    pub created: i64,
    pub setup: Setup,
}

/// `GET /api/sync`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Stable account ID, also namespaces the local sync state.
    pub account: String,
    /// The newest applied revision, the one every machine installs.
    pub head: Option<Revision>,
}

/// `POST /api/sync`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    /// The head this machine has installed, zero when the account has none.
    /// A save on top of an outdated head never writes.
    pub base: i64,
    pub machine: String,
    pub setup: Setup,
    /// False keeps the setup in the history only. It never becomes the head,
    /// so no machine installs it unless the user restores it.
    pub applied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Saved {
    Saved { revision: i64 },
    Conflict,
}

/// `POST /api/sync/restore`. The old setup becomes a new head revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Restore {
    pub revision: i64,
    pub base: i64,
    pub machine: String,
}

/// One row of `GET /api/sync/history`, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRow {
    pub revision: i64,
    pub machine: String,
    pub created: i64,
    pub applied: bool,
    pub restored_from: Option<i64>,
    /// Against the head at the time this revision was saved.
    pub summary: Summary,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Summary {
    pub mods_added: usize,
    pub mods_removed: usize,
    pub mods_changed: usize,
    pub settings_changed: usize,
}

impl Summary {
    pub fn between(before: &Setup, after: &Setup) -> Self {
        let old = settings(before);
        let new = settings(after);
        let keys: BTreeSet<_> = old.keys().chain(new.keys()).collect();
        Self {
            mods_added: after
                .mods
                .keys()
                .filter(|id| !before.mods.contains_key(*id))
                .count(),
            mods_removed: before
                .mods
                .keys()
                .filter(|id| !after.mods.contains_key(*id))
                .count(),
            mods_changed: after
                .mods
                .iter()
                .filter(|(id, value)| before.mods.get(*id).is_some_and(|old| old != *value))
                .count(),
            settings_changed: keys
                .into_iter()
                .filter(|key| old.get(*key) != new.get(*key))
                .count(),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl Display for Summary {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let parts: Vec<_> = [
            (self.mods_added, "mod added", "mods added"),
            (self.mods_removed, "mod removed", "mods removed"),
            (self.mods_changed, "mod changed", "mods changed"),
            (self.settings_changed, "setting changed", "settings changed"),
        ]
        .into_iter()
        .filter(|(count, ..)| *count > 0)
        .map(|(count, one, many)| format!("{count} {}", if count == 1 { one } else { many }))
        .collect();
        if parts.is_empty() {
            f.write_str("no changes")
        } else {
            f.write_str(&parts.join(", "))
        }
    }
}

fn settings(setup: &Setup) -> BTreeMap<(&str, &str, &str), &str> {
    let mut all = BTreeMap::new();
    for (file, sections) in &setup.configs {
        for (section, values) in sections {
            for (key, value) in values {
                all.insert(
                    (file.as_str(), section.as_str(), key.as_str()),
                    value.as_str(),
                );
            }
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Mod, Setup, Summary};

    fn setup(mods: &[(&str, &str)], count: &str) -> Setup {
        Setup {
            mods: mods
                .iter()
                .map(|(id, version)| {
                    (
                        (*id).to_owned(),
                        Mod {
                            version: (*version).to_owned(),
                            requested: Some("*".to_owned()),
                            enabled: true,
                            dependencies: Vec::new(),
                            server: None,
                        },
                    )
                })
                .collect(),
            configs: BTreeMap::from([(
                "example.cfg".to_owned(),
                BTreeMap::from([(
                    "general".to_owned(),
                    BTreeMap::from([("count".to_owned(), count.to_owned())]),
                )]),
            )]),
        }
    }

    #[test]
    fn a_summary_counts_every_kind_of_change() {
        let before = setup(&[("A-Kept", "1.0.0"), ("A-Gone", "1.0.0")], "1");
        let after = setup(&[("A-Kept", "2.0.0"), ("A-New", "1.0.0")], "2");
        let summary = Summary::between(&before, &after);
        assert_eq!(
            summary,
            Summary {
                mods_added: 1,
                mods_removed: 1,
                mods_changed: 1,
                settings_changed: 1,
            }
        );
        assert_eq!(
            summary.to_string(),
            "1 mod added, 1 mod removed, 1 mod changed, 1 setting changed"
        );
        assert!(Summary::between(&after, &after).is_empty());
        assert_eq!(Summary::default().to_string(), "no changes");
    }

    #[test]
    fn a_pin_without_a_server_reads_as_a_durka_pin() {
        let old: Mod = serde_json::from_str(
            r#"{"version":"1.3.1","requested":"1.3.1","enabled":true,"dependencies":[]}"#,
        )
        .unwrap();
        assert_eq!(old.server.as_deref(), Some(super::LEGACY_PIN_SERVER));

        let named: Mod = serde_json::from_str(
            r#"{"version":"1.3.1","requested":"1.3.1","enabled":true,"dependencies":[],"server":"Arena"}"#,
        )
        .unwrap();
        assert_eq!(named.server.as_deref(), Some("Arena"));

        let newest: Mod = serde_json::from_str(
            r#"{"version":"1.3.1","requested":"*","enabled":true,"dependencies":[],"server":"Arena"}"#,
        )
        .unwrap();
        assert_eq!(newest.server, None);
    }
}
