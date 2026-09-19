//! Checks everything `run` depends on and says what to do about each problem.

use std::path::Path;

use tokio::process::Command;

use crate::{
    error::Result,
    forge::Forge,
    install::{Installed, wanted},
    launch::{Os, preloader_path},
    profile::Profile,
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
    pub detail: String,
}

impl Check {
    fn new(name: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
        }
    }
}

impl Forge {
    pub async fn doctor(&self) -> Result<Vec<Check>> {
        let mut checks = vec![Check::new(
            "data folder",
            Status::Ok,
            self.data().root().display().to_string(),
        )];

        let profile = match self.store().active().await {
            Ok(profile) => profile,
            Err(error) => {
                checks.push(if self.store().list().await?.is_empty() {
                    Check::new(
                        "active profile",
                        Status::Warning,
                        "no profile yet, the first 'blackforge add <mod>' or 'blackforge run' creates it",
                    )
                } else {
                    Check::new("active profile", Status::Problem, error.to_string())
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

        let game = self.game(&manifest).await?;
        let os = Os::current()?;
        match self.locate_game(&game, None).await {
            Ok(install) => {
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
            Err(error) => checks.push(Check::new(
                "game",
                Status::Problem,
                format!("{error}. Pass the folder once with 'blackforge run --game-dir <path>'"),
            )),
        }

        checks.push(self.sync_check(&profile).await?);
        let loader_ready = exists(&preloader_path(profile.dir())).await;
        checks.push(if loader_ready {
            Check::new("mod loader", Status::Ok, "BepInEx is in the profile")
        } else {
            Check::new(
                "mod loader",
                Status::Problem,
                "BepInEx is not in the profile, run 'blackforge sync'",
            )
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
            checks.push(quarantine_check(&profile).await);
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
                format!("{} installed, the profile matches the lock", wanted.len()),
            )
        } else {
            Check::new(
                "mods",
                Status::Warning,
                "the profile does not match the lock, run 'blackforge sync'",
            )
        })
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
