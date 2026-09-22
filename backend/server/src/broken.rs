//! The mods known to break the game, from `broken.toml` next to this crate.
//! Anybody may read the list, the app asks for it before any login.

use std::sync::Arc;

use anyhow::{Context, Result};
use blackforge_api::broken::BrokenList;
use hilen_server::axum::{Json, Router, routing::get};

const LIST: &str = include_str!("../broken.toml");

/// The list as the file says. A bad file stops the server at start, not the
/// first reader.
pub fn load() -> Result<BrokenList> {
    toml::from_str(LIST).context("broken.toml does not parse")
}

pub fn routes<S: Clone + Send + Sync + 'static>(list: BrokenList) -> Router<S> {
    let list = Arc::new(list);
    Router::new().route(
        "/api/broken",
        get(move || {
            let list = Arc::clone(&list);
            async move { Json(list.as_ref().clone()) }
        }),
    )
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::load;

    #[test]
    fn the_list_parses_and_names_the_valheim_mod() {
        let list = load().unwrap();
        let valheim = &list.games["valheim"];
        assert!(
            valheim
                .mods
                .iter()
                .any(|entry| entry.package == "rendl0449-CraftFromContainers")
        );
    }

    /// The app parses these with its own rules. A typo here would reach
    /// every user as a doctor warning, so the file is checked at build time.
    #[test]
    fn every_entry_is_well_formed() {
        let list = load().unwrap();
        for (game, breakage) in &list.games {
            let releases: Vec<&str> = breakage
                .releases
                .iter()
                .map(|release| release.version.as_str())
                .collect();
            for release in &breakage.releases {
                assert!(
                    is_version(&release.version),
                    "{game}: bad version {}",
                    release.version
                );
                assert!(
                    NaiveDate::parse_from_str(&release.date, "%Y-%m-%d").is_ok(),
                    "{game}: bad day {}",
                    release.date
                );
            }
            for entry in &breakage.mods {
                assert!(
                    entry.package.split_once('-').is_some(),
                    "{game}: {} is not Owner-Name",
                    entry.package
                );
                assert!(
                    releases.contains(&entry.since.as_str()),
                    "{game}: {} names the release {} which has no row",
                    entry.package,
                    entry.since
                );
            }
        }
    }

    fn is_version(text: &str) -> bool {
        let parts: Vec<&str> = text.split('.').collect();
        parts.len() == 3 && parts.iter().all(|part| part.parse::<u64>().is_ok())
    }
}
