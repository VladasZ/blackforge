//! The tiers of competitive servers, from `tiers.toml` next to this crate,
//! see `docs/competitive.md`.

use anyhow::{Context, Result};
use blackforge_api::competitive::{Rules, Tiers, forbidden};

const FILE: &str = include_str!("../tiers.toml");

/// The tiers as the file says. A bad file stops the server at start.
pub fn load() -> Result<Tiers> {
    toml::from_str(FILE).context("tiers.toml does not parse")
}

/// What a server forbids now. A server that is not competitive forbids
/// nothing, and a game without tiers has nothing to forbid.
pub fn rules(tiers: &Tiers, game: &str, competitive: bool, dead: &[String]) -> Rules {
    match tiers.games.get(game) {
        Some(game) if competitive => Rules {
            competitive,
            forbidden: forbidden(&game.tiers, dead),
            allowed: game.allow.clone(),
        },
        _ => Rules {
            competitive,
            ..Rules::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{load, rules};

    /// A typo here reaches every player as a wrong block, so the file is
    /// checked at build time. The plugins check the names against the game.
    #[test]
    fn every_tier_is_well_formed() {
        let tiers = load().unwrap();
        let valheim = &tiers.games["valheim"];
        assert!(!valheim.tiers.is_empty());
        let mut keys = HashSet::new();
        let mut items = HashSet::new();
        for tier in &valheim.tiers {
            assert!(
                tier.key.starts_with("defeated_"),
                "{} is not a boss key",
                tier.key
            );
            assert!(keys.insert(&tier.key), "{} is listed twice", tier.key);
            assert!(!tier.boss.is_empty() && !tier.items.is_empty());
            for item in &tier.items {
                assert!(
                    !item.is_empty() && !item.contains(char::is_whitespace),
                    "{item:?} is not a prefab name"
                );
                assert!(items.insert(item), "{item} sits in two tiers");
            }
        }
        for item in &valheim.allow {
            assert!(
                !items.contains(item),
                "{item} is both forbidden and allowed"
            );
        }
    }

    #[test]
    fn only_a_competitive_server_forbids() {
        let tiers = load().unwrap();
        let open = rules(&tiers, "valheim", false, &[]);
        assert!(!open.competitive && open.forbidden.is_empty() && open.allowed.is_empty());

        let fresh = rules(&tiers, "valheim", true, &[]);
        assert!(fresh.forbidden.iter().any(|item| item.item == "DragonTear"));
        // Anybody can kill a troll, only boss drops and the like are listed.
        assert!(!fresh.forbidden.iter().any(|item| item.item == "TrollHide"));
        assert!(fresh.allowed.iter().any(|item| item == "Bread"));

        let late = rules(&tiers, "valheim", true, &["defeated_eikthyr".to_owned()]);
        assert!(!late.forbidden.iter().any(|item| item.item == "HardAntler"));

        let unknown = rules(&tiers, "stardew", true, &[]);
        assert!(unknown.competitive && unknown.forbidden.is_empty());
    }
}
