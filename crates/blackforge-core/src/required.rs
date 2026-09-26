//! The mods every profile of a game must have. The server keeps the list by
//! hand in `backend/server/required.toml`. A required mod sits in the manifest as a pin
//! for `REQUIRED_PIN_SERVER`, so the lock, cloud sync and deploy treat it
//! like any other pinned mod.

use blackforge_api::required::{REQUIRED_PIN_SERVER, RequiredList};
use semver::Version;

use crate::{
    error::Result,
    http::Client,
    ident::{PackageId, parse_version},
    manifest::{Manifest, ModSpec, Pin, VersionReq},
    paths::DataDir,
    served_list,
};

/// The list as the server has it now, see `served_list::load`.
pub async fn load_list(client: &Client, data: &DataDir) -> Result<RequiredList> {
    served_list::load(client, data, "required.json", "/api/required").await
}

/// The required mods of `game` with their versions. A bad entry is an error,
/// not a guess.
pub fn of_game(list: &RequiredList, game: &str) -> Result<Vec<(PackageId, Version)>> {
    list.games
        .get(game)
        .into_iter()
        .flatten()
        .map(|entry| Ok((entry.package.parse()?, parse_version(&entry.version)?)))
        .collect()
}

/// Whether the manifest entry is held by the required list.
pub fn is_required(spec: &ModSpec) -> bool {
    spec.version.server() == Some(REQUIRED_PIN_SERVER)
}

/// Brings `manifest` in line with `required` and names every mod it changed.
/// A required mod is added, moved to its version and switched on. A mod
/// that left the list keeps its place and follows the newest version again,
/// so the user can remove it.
pub fn apply(manifest: &mut Manifest, required: &[(PackageId, Version)]) -> Vec<PackageId> {
    let mut changed = Vec::new();
    for (id, version) in required {
        let wanted = ModSpec::new(VersionReq::Pinned(Pin {
            version: version.clone(),
            server: REQUIRED_PIN_SERVER.to_owned(),
        }));
        if manifest.mods.get(id) != Some(&wanted) {
            manifest.mods.insert(id.clone(), wanted);
            changed.push(id.clone());
        }
    }
    for (id, spec) in &mut manifest.mods {
        if is_required(spec) && !required.iter().any(|(wanted, _)| wanted == id) {
            spec.version = VersionReq::Latest;
            changed.push(id.clone());
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use blackforge_api::required::{RequiredList, RequiredMod};
    use semver::Version;

    use super::{apply, is_required, of_game};
    use crate::{
        error::Result,
        game::{Target, VALHEIM},
        ident::PackageId,
        manifest::{Manifest, ModSpec, Pin, VersionReq},
    };

    fn sailing() -> PackageId {
        "Smoothbrain-Sailing".parse().unwrap()
    }

    fn required() -> Vec<(PackageId, Version)> {
        vec![(sailing(), Version::new(1, 1, 8))]
    }

    #[test]
    fn a_missing_mod_is_added_as_a_required_pin() {
        let mut manifest = Manifest::new(VALHEIM, Target::Client);
        assert_eq!(apply(&mut manifest, &required()), [sailing()]);
        let spec = &manifest.mods[&sailing()];
        assert!(is_required(spec));
        assert!(spec.enabled);
        assert_eq!(spec.version.pinned_version(), Some(&Version::new(1, 1, 8)));
        assert!(apply(&mut manifest, &required()).is_empty());
    }

    #[test]
    fn a_disabled_or_other_version_is_put_back() {
        let mut manifest = Manifest::new(VALHEIM, Target::Client);
        manifest.mods.insert(
            sailing(),
            ModSpec {
                version: VersionReq::Pinned(Pin {
                    version: Version::new(1, 1, 7),
                    server: "Durka".to_owned(),
                }),
                enabled: false,
            },
        );
        assert_eq!(apply(&mut manifest, &required()), [sailing()]);
        let spec = &manifest.mods[&sailing()];
        assert!(is_required(spec) && spec.enabled);
        assert_eq!(spec.version.pinned_version(), Some(&Version::new(1, 1, 8)));
    }

    #[test]
    fn a_mod_off_the_list_follows_the_newest_again() {
        let mut manifest = Manifest::new(VALHEIM, Target::Client);
        apply(&mut manifest, &required());
        assert_eq!(apply(&mut manifest, &[]), [sailing()]);
        let spec = &manifest.mods[&sailing()];
        assert!(!is_required(spec));
        assert_eq!(spec.version, VersionReq::Latest);
    }

    #[test]
    fn the_list_is_read_per_game() -> Result<()> {
        let list = RequiredList {
            games: [(
                VALHEIM.to_owned(),
                vec![RequiredMod {
                    package: "Smoothbrain-Sailing".to_owned(),
                    version: "1.1.8".to_owned(),
                }],
            )]
            .into(),
        };
        assert_eq!(of_game(&list, VALHEIM)?, required());
        assert!(of_game(&list, "lethal_company")?.is_empty());

        let mut bad = list;
        bad.games.get_mut(VALHEIM).unwrap()[0].version = "1.1".to_owned();
        assert!(of_game(&bad, VALHEIM).is_err());
        Ok(())
    }
}
