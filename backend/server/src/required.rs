//! The mods every profile of a game must have, from `required.toml` next to
//! this crate. Anybody may read the list, the app asks for it before any
//! login.

use std::sync::Arc;

use anyhow::{Context, Result};
use blackforge_api::required::RequiredList;
use hilen_server::axum::{Json, Router, routing::get};

const LIST: &str = include_str!("../required.toml");

/// The list as the file says. A bad file stops the server at start, not the
/// first reader.
pub fn load() -> Result<RequiredList> {
    toml::from_str(LIST).context("required.toml does not parse")
}

pub fn routes<S: Clone + Send + Sync + 'static>(list: RequiredList) -> Router<S> {
    let list = Arc::new(list);
    Router::new().route(
        "/api/required",
        get(move || {
            let list = Arc::clone(&list);
            async move { Json(list.as_ref().clone()) }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::load;

    /// The app refuses an entry it cannot read and then adds nothing, so the
    /// file is checked at build time.
    #[test]
    fn every_entry_is_well_formed() {
        let list = load().unwrap();
        for (game, mods) in &list.games {
            for entry in mods {
                assert!(
                    entry.package.split_once('-').is_some(),
                    "{game}: {} is not Owner-Name",
                    entry.package
                );
                let parts: Vec<&str> = entry.version.split('.').collect();
                assert!(
                    parts.len() == 3 && parts.iter().all(|part| part.parse::<u64>().is_ok()),
                    "{game}: {} has the bad version {}",
                    entry.package,
                    entry.version
                );
            }
        }
    }
}
