//! Installing the mod list of a registered game server into a profile. Only
//! what the server needs changes. A mod the player lacks is added, a mod at
//! another version is moved to the server's version and pinned, a disabled
//! one is switched on. The rest of the profile stays as it is.

use blackforge_api::servers::{Server, ServerMod};

use crate::{
    error::Result,
    forge::Forge,
    http::Client,
    ident::{PackageId, parse_version},
    install::SyncReport,
    lock::Lockfile,
    manifest::{Manifest, VersionReq},
    profile::Profile,
    progress::Progress,
    social::client::SERVER,
};

/// Every registered server. The list is public, so it goes through the plain
/// HTTP client and works before the user ever signs in.
pub async fn fetch(client: &Client) -> Result<Vec<Server>> {
    client.get_json(&format!("{SERVER}/api/servers")).await
}

/// What a profile lacks to join a server.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    /// Not in the lock at all.
    pub missing: Vec<ServerMod>,
    /// In the lock at another version, with the version the profile has.
    pub other_version: Vec<(ServerMod, String)>,
    /// At the right version but switched off in the manifest.
    pub disabled: Vec<ServerMod>,
}

impl Needs {
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    pub fn count(&self) -> usize {
        self.missing.len() + self.other_version.len() + self.disabled.len()
    }

    /// Every mod that has to be added or moved, the disabled ones only need
    /// the switch.
    fn to_add(&self) -> impl Iterator<Item = &ServerMod> {
        self.missing
            .iter()
            .chain(self.other_version.iter().map(|(server_mod, _)| server_mod))
    }
}

/// Compares a profile with the list of a server. A version the server wrote
/// in a way the profile cannot parse counts as missing, the add that follows
/// then says what is wrong with it.
pub fn needs(manifest: &Manifest, lock: &Lockfile, required: &[ServerMod]) -> Needs {
    let mut needs = Needs::default();
    for server_mod in required {
        let id: Option<PackageId> = server_mod.id.parse().ok();
        let wanted = parse_version(&server_mod.version).ok();
        let locked = id
            .as_ref()
            .and_then(|id| lock.packages.iter().find(|package| &package.id == id));
        match (id, wanted, locked) {
            (Some(id), Some(wanted), Some(package)) if package.version == wanted => {
                if !manifest.is_enabled(&id) {
                    needs.disabled.push(server_mod.clone());
                }
            }
            (Some(_), Some(_), Some(package)) => needs
                .other_version
                .push((server_mod.clone(), package.version.to_string())),
            _ => needs.missing.push(server_mod.clone()),
        }
    }
    needs
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerInstall {
    /// What the profile lacked before the install.
    pub needs: Needs,
    pub sync: SyncReport,
}

impl Forge {
    /// Makes the profile able to join the server and puts the files on disk.
    pub async fn install_server(
        &self,
        profile: &Profile,
        required: &[ServerMod],
        progress: &Progress,
    ) -> Result<ServerInstall> {
        let manifest = profile.manifest().await?;
        let lock = profile.lock().await?;
        let needs = needs(&manifest, &lock, required);
        for server_mod in needs.to_add() {
            let version = VersionReq::Exact(parse_version(&server_mod.version)?);
            self.add(profile, &server_mod.id, version, progress).await?;
        }
        // A mod that was added keeps its enabled flag, so a disabled one at
        // the wrong version needs the switch too. The fresh manifest knows.
        let manifest = profile.manifest().await?;
        for server_mod in required {
            if let Ok(id) = server_mod.id.parse::<PackageId>()
                && !manifest.is_enabled(&id)
            {
                self.set_enabled(profile, &server_mod.id, true).await?;
            }
        }
        let sync = self.sync(profile, progress).await?;
        Ok(ServerInstall { needs, sync })
    }
}

#[cfg(test)]
mod tests {
    use blackforge_api::servers::ServerMod;

    use super::needs;
    use crate::{
        game::{Target, VALHEIM},
        ident::{PackageId, parse_version},
        lock::{LockedPackage, Lockfile},
        manifest::{Manifest, ModSpec, VersionReq},
    };

    fn required(id: &str, version: &str) -> ServerMod {
        ServerMod {
            id: id.to_owned(),
            version: version.to_owned(),
        }
    }

    fn locked(id: &str, version: &str) -> LockedPackage {
        LockedPackage {
            id: id.parse().unwrap(),
            version: parse_version(version).unwrap(),
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn a_profile_is_compared_mod_by_mod() {
        let mut manifest = Manifest::new(VALHEIM, Target::Client);
        let off: PackageId = "Crystal-DigDeeper".parse().unwrap();
        manifest.mods.insert(
            off.clone(),
            ModSpec {
                version: VersionReq::Latest,
                enabled: false,
            },
        );
        let lock = Lockfile::new(vec![
            locked("denikson-BepInExPack_Valheim", "5.4.2350"),
            locked("shudnal-ConditionalConfigSync", "1.0.9"),
            locked("Crystal-DigDeeper", "1.3.1"),
        ]);
        let needs = needs(
            &manifest,
            &lock,
            &[
                required("denikson-BepInExPack_Valheim", "5.4.2350"),
                required("shudnal-ConditionalConfigSync", "1.0.6"),
                required("Crystal-DigDeeper", "1.3.1"),
                required("Grantapher-ValheimPlus_Grantapher_Temporary", "10.2.0"),
                required("Broken-Version", "not.a.version"),
            ],
        );

        assert_eq!(
            needs.missing,
            [
                required("Grantapher-ValheimPlus_Grantapher_Temporary", "10.2.0"),
                required("Broken-Version", "not.a.version"),
            ]
        );
        assert_eq!(
            needs.other_version,
            [(
                required("shudnal-ConditionalConfigSync", "1.0.6"),
                "1.0.9".to_owned()
            )]
        );
        assert_eq!(needs.disabled, [required("Crystal-DigDeeper", "1.3.1")]);
        assert_eq!(needs.count(), 4);
    }

    #[test]
    fn a_matching_profile_needs_nothing() {
        let manifest = Manifest::new(VALHEIM, Target::Client);
        let lock = Lockfile::new(vec![locked("Crystal-DigDeeper", "1.3.1")]);
        let needs = needs(&manifest, &lock, &[required("Crystal-DigDeeper", "1.3.1")]);
        assert!(needs.is_empty());
    }
}
