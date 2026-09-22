//! `GET /api/broken`, the mods known to break the game. The list is kept by
//! hand on the server, blackforge guesses nothing from the dates or versions
//! of the mods themselves. Anybody may read it, it is the one route with no
//! login.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokenList {
    /// By the game label of the Thunderstore schema, `valheim` for example.
    pub games: BTreeMap<String, GameBreakage>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameBreakage {
    /// The releases of the game with the day each came out. The Steam
    /// manifest of an installed game has an update time and no version, so
    /// this table turns the one into the other.
    pub releases: Vec<GameRelease>,
    pub mods: Vec<BrokenMod>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRelease {
    /// Three numbers, `1.0.15`.
    pub version: String,
    /// A calendar day, `2026-09-09`.
    pub date: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokenMod {
    /// `Owner-Name`. Every version of it is flagged.
    pub package: String,
    /// The game version it broke on. It stays flagged on every later one.
    pub since: String,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{from_str, to_string};

    use super::{BrokenList, BrokenMod, GameBreakage, GameRelease};

    #[test]
    fn broken_list_shape() {
        let list = BrokenList {
            games: BTreeMap::from([(
                "valheim".to_owned(),
                GameBreakage {
                    releases: vec![GameRelease {
                        version: "1.0.0".to_owned(),
                        date: "2026-09-09".to_owned(),
                    }],
                    mods: vec![BrokenMod {
                        package: "rendl0449-CraftFromContainers".to_owned(),
                        since: "1.0.0".to_owned(),
                    }],
                },
            )]),
        };
        let json = to_string(&list).unwrap();
        assert_eq!(
            json,
            r#"{"games":{"valheim":{"releases":[{"version":"1.0.0","date":"2026-09-09"}],"mods":[{"package":"rendl0449-CraftFromContainers","since":"1.0.0"}]}}}"#
        );
        assert_eq!(from_str::<BrokenList>(&json).unwrap(), list);
    }
}
