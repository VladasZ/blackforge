//! Competitive servers, see `docs/competitive.md`. On such a server a player
//! may bring nothing from a tier whose boss the server has not killed yet.
//!
//! The backend keeps the tiers by hand, each a boss key of the game and the
//! materials that boss unlocks. The game server reports its dead bosses at
//! `POST /api/gate/progress` and gets back which materials are forbidden now.
//! A join code of such a server carries the same list for the join plugin.
//! The plugins find every item made from a forbidden material by themselves,
//! from the recipes of the game.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The tiers of every game, the file `tiers.toml` of the backend.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tiers {
    /// By the game label of the Thunderstore schema, `valheim` for example.
    pub games: BTreeMap<String, Vec<Tier>>,
}

/// One boss and the materials its kill unlocks, in the order of the game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tier {
    /// The global key the game sets when the boss dies, `defeated_eikthyr`.
    pub key: String,
    /// The name a player reads, `Eikthyr`.
    pub boss: String,
    /// Prefab names of the game, `CopperOre`.
    pub items: Vec<String>,
}

/// One material a player may not bring yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forbidden {
    /// The prefab name.
    pub item: String,
    /// The boss that unlocks it.
    pub boss: String,
    /// The place of that boss in the game, the first boss is 0. An item made
    /// from several forbidden materials names the latest boss.
    pub tier: u32,
}

/// What a server of a game forbids while these bosses are dead.
pub fn forbidden(tiers: &[Tier], dead: &[String]) -> Vec<Forbidden> {
    tiers
        .iter()
        .zip(0u32..)
        .filter(|(tier, _)| !dead.contains(&tier.key))
        .flat_map(|(tier, index)| {
            tier.items.iter().map(move |item| Forbidden {
                item: item.clone(),
                boss: tier.boss.clone(),
                tier: index,
            })
        })
        .collect()
}

/// The body of `POST /api/gate/progress`. The route wants
/// `Authorization: Bearer <gate secret>` like `/api/gate/verify`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// The name the game server runs with.
    pub server: String,
    /// The id of the world, items found in it carry it as their tag.
    pub world: String,
    /// The boss keys set in the world.
    pub keys: Vec<String>,
}

/// The rules of one server, the answer of `POST /api/gate/progress`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    pub competitive: bool,
    /// Empty unless the server is competitive.
    pub forbidden: Vec<Forbidden>,
}

#[cfg(test)]
mod tests {
    use serde_json::to_string;

    use super::{Forbidden, Progress, Rules, Tier, forbidden};

    fn tiers() -> Vec<Tier> {
        vec![
            Tier {
                key: "defeated_eikthyr".to_owned(),
                boss: "Eikthyr".to_owned(),
                items: vec!["CopperOre".to_owned(), "TinOre".to_owned()],
            },
            Tier {
                key: "defeated_gdking".to_owned(),
                boss: "The Elder".to_owned(),
                items: vec!["IronScrap".to_owned()],
            },
        ]
    }

    #[test]
    fn a_dead_boss_frees_its_tier_only() {
        let all = forbidden(&tiers(), &[]);
        assert_eq!(all.len(), 3);
        assert_eq!(all[2].tier, 1);

        let after_eikthyr = forbidden(&tiers(), &["defeated_eikthyr".to_owned()]);
        assert_eq!(
            after_eikthyr,
            [Forbidden {
                item: "IronScrap".to_owned(),
                boss: "The Elder".to_owned(),
                tier: 1,
            }]
        );
    }

    // The server plugin writes and reads these in C#, so the shape is fixed here.
    #[test]
    fn progress_and_rules_shape() {
        let progress = Progress {
            server: "Arkham Asylum".to_owned(),
            world: "-123".to_owned(),
            keys: vec!["defeated_eikthyr".to_owned()],
        };
        assert_eq!(
            to_string(&progress).unwrap(),
            r#"{"server":"Arkham Asylum","world":"-123","keys":["defeated_eikthyr"]}"#
        );
        let rules = Rules {
            competitive: true,
            forbidden: forbidden(&tiers(), &["defeated_eikthyr".to_owned()]),
        };
        assert_eq!(
            to_string(&rules).unwrap(),
            r#"{"competitive":true,"forbidden":[{"item":"IronScrap","boss":"The Elder","tier":1}]}"#
        );
    }
}
