//! Checks everything `run` depends on and says what to do about each problem.

use std::{path::Path, time::SystemTime};

use tokio::process::Command;

use crate::{
    broken::{Broken, load_list},
    error::Result,
    fix::Fix,
    forge::Forge,
    game::GameDef,
    install::{Installed, wanted},
    launch::{Os, preloader_path},
    profile::Profile,
    steam,
    util::exists,
};

const ROSETTA: &str = "/Library/Apple/usr/share/rosetta/rosetta";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warning,
    Problem,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    /// What was found. It never says how to solve it inside blackforge, that
    /// is `fix`, which each frontend words its own way.
    pub detail: String,
    pub fix: Option<Fix>,
}

impl Check {
    fn new(name: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
            fix: None,
        }
    }

    fn with_fix(mut self, fix: Option<Fix>) -> Self {
        self.fix = fix;
        self
    }
}

impl Forge {
    /// The checks of the active profile.
    pub async fn doctor(&self) -> Result<Vec<Check>> {
        let mut checks = vec![self.data_folder_check()];

        let profile = match self.store().active().await {
            Ok(profile) => profile,
            Err(error) => {
                checks.push(if self.store().list().await?.is_empty() {
                    Check::new("active profile", Status::Warning, "no profile yet")
                        .with_fix(Some(Fix::CreateProfile))
                } else {
                    Check::new("active profile", Status::Problem, error.to_string())
                        .with_fix(error.fix())
                });
                return Ok(checks);
            }
        };
        let manifest = profile.manifest().await?;
        checks.push(Check::new(
            "active profile",
            Status::Ok,
            format!("{}, {} {}", profile.name(), manifest.game, manifest.target),
        ));
        checks.extend(self.run_checks(&profile).await?);
        Ok(checks)
    }

    /// The checks of one given profile, for a frontend that does not follow
    /// the active one.
    pub async fn doctor_of(&self, profile: &Profile) -> Result<Vec<Check>> {
        let mut checks = vec![self.data_folder_check()];
        checks.extend(self.run_checks(profile).await?);
        Ok(checks)
    }

    fn data_folder_check(&self) -> Check {
        Check::new(
            "data folder",
            Status::Ok,
            self.data().root().display().to_string(),
        )
    }

    /// Everything a run of this profile depends on.
    async fn run_checks(&self, profile: &Profile) -> Result<Vec<Check>> {
        let mut checks = Vec::new();
        let manifest = profile.manifest().await?;
        let game = self.game(&manifest).await?;
        let os = Os::current()?;
        let mut updated = None;
        match self.locate_game(&game, None).await {
            Ok(install) => {
                updated = install.updated;
                checks.push(Check::new(
                    "game",
                    Status::Ok,
                    install.executable.display().to_string(),
                ));
                let is_windows_build = install
                    .executable
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
                if os == Os::Linux && is_windows_build {
                    checks.push(Check::new(
                        "game build",
                        Status::Problem,
                        "this is the Windows build, running through Proton is not supported yet",
                    ));
                }
            }
            Err(error) => checks.push(
                Check::new("game", Status::Problem, error.to_string())
                    .with_fix(Some(Fix::GiveGameFolder)),
            ),
        }

        checks.push(self.sync_check(profile).await?);
        checks.extend(self.broken_checks(profile, &game, updated).await?);
        let loader_ready = exists(&preloader_path(profile.dir())).await;
        checks.push(if loader_ready {
            Check::new("mod loader", Status::Ok, "BepInEx is installed")
        } else {
            Check::new("mod loader", Status::Problem, "BepInEx is not installed")
                .with_fix(Some(Fix::Sync))
        });

        if os == Os::MacArm {
            checks.push(if exists(Path::new(ROSETTA)).await {
                Check::new(
                    "rosetta",
                    Status::Ok,
                    "installed, the mod loader needs the Intel build of the game",
                )
            } else {
                Check::new(
                    "rosetta",
                    Status::Problem,
                    "missing, install it with 'softwareupdate --install-rosetta'",
                )
            });
        }
        if matches!(os, Os::MacArm | Os::MacIntel) && loader_ready {
            checks.push(quarantine_check(profile).await);
        }
        if steam::needed(os, game.target) {
            checks.push(steam_check().await);
        }
        Ok(checks)
    }

    /// One warning per mod of the lock that the server lists as broken on
    /// the game version of this machine. A list that cannot be read or
    /// understood is a warning too, not a failed doctor.
    async fn broken_checks(
        &self,
        profile: &Profile,
        game: &GameDef,
        updated: Option<SystemTime>,
    ) -> Result<Vec<Check>> {
        let resolved = load_list(self.client(), self.data())
            .await
            .and_then(|list| Broken::resolve(&list, &game.label, updated));
        let broken = match resolved {
            Ok(broken) => broken,
            Err(error) => {
                return Ok(vec![Check::new(
                    "broken mods",
                    Status::Warning,
                    format!("the list of broken mods could not be read: {error}"),
                )]);
            }
        };
        let Some(version) = broken.version() else {
            return Ok(vec![Check::new(
                "broken mods",
                Status::Ok,
                "the game version is not known, so no mod is flagged",
            )]);
        };

        let checks: Vec<Check> = profile
            .lock()
            .await?
            .packages
            .iter()
            .filter_map(|package| {
                let since = broken.since(&package.id)?;
                Some(Check::new(
                    "broken mod",
                    Status::Warning,
                    format!(
                        "{} broke on {} {since}, this game is {version}",
                        package.id, game.display_name
                    ),
                ))
            })
            .collect();
        if checks.is_empty() {
            return Ok(vec![Check::new(
                "broken mods",
                Status::Ok,
                format!("none known for {} {version}", game.display_name),
            )]);
        }
        Ok(checks)
    }

    async fn sync_check(&self, profile: &Profile) -> Result<Check> {
        let manifest = profile.manifest().await?;
        let lock = profile.lock().await?;
        let installed = Installed::read(profile.dir()).await?;
        let wanted = wanted(&manifest, &lock);
        let in_sync = wanted.len() == installed.packages.len()
            && wanted.iter().all(|want| {
                installed
                    .packages
                    .get(&want.id)
                    .is_some_and(|package| package.version == want.version)
            });
        Ok(if in_sync {
            Check::new(
                "mods",
                Status::Ok,
                format!("{} installed, all match the lock", wanted.len()),
            )
        } else {
            Check::new(
                "mods",
                Status::Warning,
                "the installed mods do not match the lock",
            )
            .with_fix(Some(Fix::Sync))
        })
    }
}

async fn steam_check() -> Check {
    match steam::running().await {
        Ok(true) => Check::new("steam", Status::Ok, "running"),
        Ok(false) => {
            Check::new("steam", Status::Problem, "not running").with_fix(Some(Fix::StartSteam))
        }
        Err(error) => Check::new(
            "steam",
            Status::Warning,
            format!("could not look for the Steam app: {error}"),
        ),
    }
}

/// macOS refuses to load a library that carries the quarantine flag. A file
/// blackforge downloads never gets it, a profile copied over from a browser
/// download or an archive can.
async fn quarantine_check(profile: &Profile) -> Check {
    let libs = profile.dir().join("doorstop_libs");
    let output = Command::new("/usr/bin/xattr")
        .arg("-r")
        .arg(&libs)
        .output()
        .await;
    match output {
        Ok(output) if String::from_utf8_lossy(&output.stdout).contains("com.apple.quarantine") => {
            Check::new(
                "quarantine",
                Status::Problem,
                format!(
                    "macOS blocks the loader, clear it with 'xattr -dr com.apple.quarantine \"{}\"'",
                    profile.dir().display()
                ),
            )
        }
        Ok(_) => Check::new(
            "quarantine",
            Status::Ok,
            "the loader libraries are not blocked by macOS",
        ),
        Err(error) => Check::new(
            "quarantine",
            Status::Warning,
            format!("could not run xattr: {error}"),
        ),
    }
}
