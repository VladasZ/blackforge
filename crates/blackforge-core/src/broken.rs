//! The mods known to break the game. The server keeps the list by hand and
//! serves it to anybody. This module reads it and resolves it against the
//! game installed on this machine.

use std::{collections::BTreeMap, time::SystemTime};

use blackforge_api::broken::BrokenList;
use chrono::{DateTime, NaiveDate, Utc};
use semver::Version;

use crate::{
    error::{Error, Result},
    http::Client,
    ident::{PackageId, parse_version},
    paths::DataDir,
    served_list,
};

/// The list as the server has it now, see `served_list::load`.
pub async fn load_list(client: &Client, data: &DataDir) -> Result<BrokenList> {
    served_list::load(client, data, "broken.json", "/api/broken").await
}

/// The broken mods of one game as it is installed on this machine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Broken {
    version: Option<Version>,
    /// Each flagged package with the game version it broke on.
    mods: BTreeMap<PackageId, Version>,
}

impl Broken {
    /// `updated` is the update time of the Steam manifest. The game is on the
    /// newest release that came out on or before that day. `None`, for a
    /// folder given by hand, flags nothing, and so does a game older than
    /// every release the list knows.
    pub fn resolve(list: &BrokenList, game: &str, updated: Option<SystemTime>) -> Result<Self> {
        let (Some(breakage), Some(updated)) = (list.games.get(game), updated) else {
            return Ok(Self::default());
        };
        let day = DateTime::<Utc>::from(updated).date_naive();

        let mut version: Option<Version> = None;
        for release in &breakage.releases {
            let date = NaiveDate::parse_from_str(&release.date, "%Y-%m-%d").map_err(|_| {
                Error::Invalid(format!(
                    "the broken list has a bad release day '{}'",
                    release.date
                ))
            })?;
            let candidate = parse_version(&release.version)?;
            if date <= day && version.as_ref().is_none_or(|best| candidate > *best) {
                version = Some(candidate);
            }
        }
        let Some(version) = version else {
            return Ok(Self::default());
        };

        let mut mods = BTreeMap::new();
        for entry in &breakage.mods {
            let since = parse_version(&entry.since)?;
            if since <= version {
                mods.insert(entry.package.parse()?, since);
            }
        }
        Ok(Self {
            version: Some(version),
            mods,
        })
    }

    /// The game version this machine is on, when known.
    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    pub fn contains(&self, id: &PackageId) -> bool {
        self.mods.contains_key(id)
    }

    /// The game version `id` broke on, when it is flagged.
    pub fn since(&self, id: &PackageId) -> Option<&Version> {
        self.mods.get(id)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, SystemTime},
    };

    use blackforge_api::broken::{BrokenList, BrokenMod, GameBreakage, GameRelease};
    use semver::Version;

    use super::Broken;
    use crate::{error::Result, ident::PackageId};

    const CFC: &str = "rendl0449-CraftFromContainers";

    fn release(version: &str, date: &str) -> GameRelease {
        GameRelease {
            version: version.to_owned(),
            date: date.to_owned(),
        }
    }

    fn list(releases: Vec<GameRelease>, mods: Vec<(&str, &str)>) -> BrokenList {
        let mods = mods
            .into_iter()
            .map(|(package, since)| BrokenMod {
                package: package.to_owned(),
                since: since.to_owned(),
            })
            .collect();
        BrokenList {
            games: BTreeMap::from([("valheim".to_owned(), GameBreakage { releases, mods })]),
        }
    }

    fn valheim() -> BrokenList {
        list(
            vec![
                release("0.221.12", "2026-02-19"),
                release("1.0.0", "2026-09-09"),
                release("1.0.15", "2026-09-18"),
            ],
            vec![(CFC, "1.0.0"), ("Someone-Later", "1.0.15")],
        )
    }

    /// Noon UTC of a day of September 2026.
    fn september(day: u64) -> SystemTime {
        let first_noon = 1_788_220_800 + 12 * 3600;
        SystemTime::UNIX_EPOCH + Duration::from_secs(first_noon + (day - 1) * 86_400)
    }

    fn cfc() -> PackageId {
        CFC.parse().unwrap()
    }

    #[test]
    fn the_newest_release_on_or_before_the_update_day_is_the_version() -> Result<()> {
        let broken = Broken::resolve(&valheim(), "valheim", Some(september(9)))?;
        assert_eq!(broken.version(), Some(&Version::new(1, 0, 0)));
        let broken = Broken::resolve(&valheim(), "valheim", Some(september(20)))?;
        assert_eq!(broken.version(), Some(&Version::new(1, 0, 15)));
        Ok(())
    }

    #[test]
    fn a_mod_is_flagged_from_its_game_version_on() -> Result<()> {
        let later: PackageId = "Someone-Later".parse()?;
        let broken = Broken::resolve(&valheim(), "valheim", Some(september(9)))?;
        assert_eq!(broken.since(&cfc()), Some(&Version::new(1, 0, 0)));
        assert!(!broken.contains(&later));

        let broken = Broken::resolve(&valheim(), "valheim", Some(september(20)))?;
        assert!(broken.contains(&cfc()));
        assert!(broken.contains(&later));
        Ok(())
    }

    #[test]
    fn a_game_older_than_the_mod_needs_is_fine() -> Result<()> {
        let broken = Broken::resolve(&valheim(), "valheim", Some(september(1)))?;
        assert_eq!(broken.version(), Some(&Version::new(0, 221, 12)));
        assert!(!broken.contains(&cfc()));
        Ok(())
    }

    #[test]
    fn no_version_flags_nothing() -> Result<()> {
        let older_than_any = list(vec![release("1.0.0", "2026-09-09")], vec![(CFC, "1.0.0")]);
        assert_eq!(
            Broken::resolve(&older_than_any, "valheim", Some(september(1)))?,
            Broken::default()
        );
        assert_eq!(
            Broken::resolve(&valheim(), "valheim", None)?,
            Broken::default()
        );
        assert_eq!(
            Broken::resolve(&valheim(), "lethal_company", Some(september(20)))?,
            Broken::default()
        );
        Ok(())
    }

    #[test]
    fn a_bad_entry_is_an_error_not_a_guess() {
        let bad_day = list(vec![release("1.0.0", "September 9")], vec![]);
        assert!(Broken::resolve(&bad_day, "valheim", Some(september(20))).is_err());
        let bad_version = list(vec![release("1.0", "2026-09-09")], vec![]);
        assert!(Broken::resolve(&bad_version, "valheim", Some(september(20))).is_err());
        let bad_package = list(
            vec![release("1.0.0", "2026-09-09")],
            vec![("nodash", "1.0.0")],
        );
        assert!(Broken::resolve(&bad_package, "valheim", Some(september(20))).is_err());
    }
}
