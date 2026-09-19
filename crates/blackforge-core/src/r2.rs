//! Reads and writes the profile exchange format of r2modman, which Gale and
//! Thunderstore Mod Manager share.
//!
//! An export is a zip, saved with the `.r2z` extension. It holds `export.r2x`,
//! a yaml list of mods with exact versions, the `BepInEx/config` folder under
//! `config/`, and other text config files by their path in the profile. A
//! profile code is the same zip as base64 text on the Thunderstore server.

use std::{
    fs::{File, create_dir_all, read},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    error::{Error, IoContext, Result},
    forge::Forge,
    game::{Target, VALHEIM},
    ident::PackageId,
    install::{STATE_FILE, rules::entry_parts},
    lock::{LOCK_FILE, LockedPackage, Lockfile},
    manifest::{MANIFEST_FILE, Manifest, ModSpec, VersionReq},
    profile::Profile,
    progress::Progress,
    thunderstore::{FRESH_ENOUGH, PackageIndex},
    util::walk_files,
};

const CREATE_URL: &str = "https://thunderstore.io/api/experimental/legacyprofile/create/";
const GET_URL: &str = "https://thunderstore.io/api/experimental/legacyprofile/get/";
const CODE_PREFIX: &str = "#r2modman";
const EXPORT_FILE: &str = "export.r2x";
/// The server refuses a bigger upload.
const MAX_CODE_BYTES: usize = 20_000_000;
const CONFIG_EXTENSIONS: [&str; 6] = [".cfg", ".txt", ".json", ".yml", ".yaml", ".ini"];
/// A shared profile is data from a stranger, so nothing that can run is ever
/// unpacked from it. The list is the one r2modman uses.
const BLOCKED_EXTENSIONS: [&str; 24] = [
    ".dll", ".exe", ".scr", ".com", ".pif", ".bat", ".cmd", ".ps1", ".vbs", ".vbe", ".js", ".jse",
    ".wsf", ".wsh", ".hta", ".msi", ".msix", ".sys", ".drv", ".cpl", ".ocx", ".lnk", ".reg",
    ".inf",
];

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportFile {
    profile_name: String,
    mods: Vec<ExportMod>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportMod {
    name: PackageId,
    version: ExportVersion,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct CreateResponse {
    key: String,
}

/// What an export holds, before it is written into a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
    pub profile_name: String,
    pub manifest: Manifest,
    pub lock: Lockfile,
    /// Config files as the path inside the profile and the content.
    pub files: Vec<(String, Vec<u8>)>,
}

/// Parses the zip of an export. `manifest` is the empty manifest of the new
/// profile, `index` fills in the dependency lists of the lock.
pub fn parse(zip_bytes: &[u8], mut manifest: Manifest, index: &PackageIndex) -> Result<Imported> {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))?;
    let mut text = String::new();
    archive
        .by_name(EXPORT_FILE)?
        .read_to_string(&mut text)
        .at(Path::new(EXPORT_FILE))?;
    let export: ExportFile = serde_yaml_ng::from_str(&text)?;

    // The export names every mod with its exact version and does not say
    // which ones the user picked, so all of them go into the manifest and
    // the lock mirrors the export without a resolve.
    let mut locked = Vec::new();
    for entry in export.mods {
        let version = Version::new(
            entry.version.major,
            entry.version.minor,
            entry.version.patch,
        );
        let dependencies = index
            .get(&entry.name)
            .and_then(|package| package.version(&version))
            .map(|release| {
                release
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        manifest.mods.insert(
            entry.name.clone(),
            ModSpec {
                version: VersionReq::Latest,
                enabled: entry.enabled,
            },
        );
        locked.push(LockedPackage {
            id: entry.name,
            version,
            dependencies,
        });
    }

    let mut files = Vec::new();
    let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
    for name in names {
        let Some(parts) = entry_parts(&name) else {
            continue;
        };
        let lower = name.to_lowercase();
        if BLOCKED_EXTENSIONS
            .iter()
            .any(|extension| lower.ends_with(extension))
        {
            continue;
        }
        let dest = match parts.as_slice() {
            [single] if is_reserved(single) => continue,
            [first, rest @ ..] if first == "config" && !rest.is_empty() => {
                format!("BepInEx/config/{}", rest.join("/"))
            }
            parts => parts.join("/"),
        };
        let mut content = Vec::new();
        archive
            .by_name(&name)?
            .read_to_end(&mut content)
            .at(Path::new(&name))?;
        files.push((dest, content));
    }

    Ok(Imported {
        profile_name: export.profile_name,
        manifest,
        lock: Lockfile::new(locked),
        files,
    })
}

fn is_reserved(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        EXPORT_FILE,
        "mods.yml",
        MANIFEST_FILE,
        LOCK_FILE,
        STATE_FILE,
    ]
    .contains(&lower.as_str())
}

/// Builds the export zip of a profile.
pub fn build(
    profile_name: &str,
    manifest: &Manifest,
    lock: &Lockfile,
    profile_dir: &Path,
) -> Result<Vec<u8>> {
    let export = ExportFile {
        profile_name: profile_name.to_owned(),
        mods: lock
            .packages
            .iter()
            .map(|package| ExportMod {
                name: package.id.clone(),
                version: ExportVersion {
                    major: package.version.major,
                    minor: package.version.minor,
                    patch: package.version.patch,
                },
                enabled: manifest.is_enabled(&package.id),
            })
            .collect(),
    };

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    writer.start_file(EXPORT_FILE, options)?;
    writer
        .write_all(serde_yaml_ng::to_string(&export)?.as_bytes())
        .at(Path::new(EXPORT_FILE))?;

    if profile_dir.is_dir() {
        for relative in walk_files(profile_dir)? {
            let parts: Vec<String> = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            let Some(entry) = export_entry(&parts) else {
                continue;
            };
            let path = profile_dir.join(&relative);
            writer.start_file(entry, options)?;
            writer.write_all(&read(&path).at(&path)?).at(&path)?;
        }
    }
    Ok(writer.finish()?.into_inner())
}

/// The name a profile file gets inside the export, `None` to leave it out.
fn export_entry(parts: &[String]) -> Option<String> {
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    match parts.as_slice() {
        [single] if is_reserved(single) => None,
        ["BepInEx", "config", rest @ ..] if !rest.is_empty() => {
            Some(format!("config/{}", rest.join("/")))
        }
        // r2modman leaves these out. The manifest of a mod is not a config.
        ["BepInEx", "plugins", _, "manifest.json"] | ["dotnet" | "_state", ..] => None,
        parts => {
            let last = parts.last()?.to_lowercase();
            CONFIG_EXTENSIONS
                .iter()
                .any(|extension| last.ends_with(extension))
                .then(|| parts.join("/"))
        }
    }
}

impl Forge {
    /// Imports an `.r2z` file, or a profile code when `source` is not a file.
    pub async fn import_r2(
        &self,
        source: &str,
        name: Option<&str>,
        target: Target,
        progress: &Progress,
    ) -> Result<Profile> {
        let path = PathBuf::from(source);
        let zip_bytes = if path.is_file() {
            spawn_blocking(move || read(&path).at(&path)).await??
        } else {
            self.download_code(source).await?
        };

        let manifest = Manifest::new(VALHEIM, target);
        let game = self.game(&manifest).await?;
        let index = self.index(&game, FRESH_ENOUGH, progress).await?;
        let imported = spawn_blocking(move || parse(&zip_bytes, manifest, &index)).await??;

        let name = name.map_or_else(|| profile_name_from(&imported.profile_name), str::to_owned);
        let profile = self
            .store()
            .create(&name, &imported.manifest.game, imported.manifest.target)
            .await?;
        profile.save(&imported.manifest, &imported.lock).await?;

        let dir = profile.dir().to_path_buf();
        spawn_blocking(move || {
            for (relative, content) in imported.files {
                let path = relative
                    .split('/')
                    .fold(dir.clone(), |path, part| path.join(part));
                if let Some(parent) = path.parent() {
                    create_dir_all(parent).at(parent)?;
                }
                File::create(&path)
                    .at(&path)?
                    .write_all(&content)
                    .at(&path)?;
            }
            Ok::<_, Error>(())
        })
        .await??;
        Ok(profile)
    }

    async fn download_code(&self, code: &str) -> Result<Vec<u8>> {
        let valid = !code.is_empty() && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !valid {
            return Err(Error::Invalid(format!(
                "'{code}' is not a file and not a profile code"
            )));
        }
        let body = self
            .client()
            .get_bytes(&format!("{GET_URL}{code}/"))
            .await?;
        let text = String::from_utf8(body)
            .map_err(|_| Error::Invalid("the profile code data is not text".to_owned()))?;
        let encoded = text.strip_prefix(CODE_PREFIX).ok_or_else(|| {
            Error::Invalid("the profile code does not hold an r2modman profile".to_owned())
        })?;
        let compact: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
        STANDARD
            .decode(compact)
            .map_err(|error| Error::Invalid(format!("bad base64 in the profile code: {error}")))
    }

    pub async fn export_r2_bytes(&self, profile: &Profile) -> Result<Vec<u8>> {
        let manifest = profile.manifest().await?;
        let lock = profile.lock().await?;
        let name = profile.name().to_owned();
        let dir = profile.dir().to_path_buf();
        spawn_blocking(move || build(&name, &manifest, &lock, &dir)).await?
    }

    /// Uploads the profile and returns the code. Codes are short lived.
    pub async fn export_r2_code(&self, profile: &Profile) -> Result<String> {
        let zip_bytes = self.export_r2_bytes(profile).await?;
        if zip_bytes.len() > MAX_CODE_BYTES {
            return Err(Error::Invalid(
                "the profile is over 20 MB, too large for a code, export it to a file instead"
                    .to_owned(),
            ));
        }
        let body = format!("{CODE_PREFIX}\n{}", STANDARD.encode(zip_bytes)).into_bytes();
        let answer = self
            .client()
            .post_bytes(CREATE_URL, "application/octet-stream", body)
            .await?;
        let created: CreateResponse = serde_json::from_slice(&answer)?;
        Ok(created.key)
    }
}

/// Turns any r2modman profile name into a valid blackforge one.
fn profile_name_from(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "imported".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::write;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn export_then_import_gives_the_same_set() -> Result<()> {
        let temp = tempdir().at(Path::new("tempdir"))?;
        let dir = temp.path();
        for folder in ["BepInEx/config", "BepInEx/plugins/Owner-Mod"] {
            create_dir_all(dir.join(folder)).at(dir)?;
        }
        for (file, content) in [
            ("BepInEx/config/owner.mod.cfg", "Enabled = true"),
            ("BepInEx/plugins/Owner-Mod/manifest.json", "{}"),
            ("BepInEx/plugins/Owner-Mod/Mod.dll", "binary"),
            ("BepInEx/plugins/Owner-Mod/translations.json", "{}"),
            (MANIFEST_FILE, "game = \"valheim\""),
        ] {
            write(dir.join(file), content).at(dir)?;
        }

        let mut manifest = Manifest::new(VALHEIM, Target::Client);
        manifest.mods.insert(
            "Owner-Mod".parse()?,
            ModSpec {
                version: VersionReq::Latest,
                enabled: false,
            },
        );
        let lock = Lockfile::new(vec![
            LockedPackage {
                id: "Owner-Mod".parse()?,
                version: Version::new(1, 2, 3),
                dependencies: vec![],
            },
            LockedPackage {
                id: "Owner-Lib".parse()?,
                version: Version::new(0, 4, 0),
                dependencies: vec![],
            },
        ]);

        let zip_bytes = build("My Profile!", &manifest, &lock, dir)?;
        let index = PackageIndex::from_packages(Vec::new());
        let imported = parse(&zip_bytes, Manifest::new(VALHEIM, Target::Client), &index)?;

        assert_eq!(imported.profile_name, "My Profile!");
        assert_eq!(profile_name_from(&imported.profile_name), "My-Profile");
        assert_eq!(imported.lock, lock);
        assert!(!imported.manifest.is_enabled(&"Owner-Mod".parse()?));
        assert!(imported.manifest.is_enabled(&"Owner-Lib".parse()?));

        let mut files: Vec<&str> = imported
            .files
            .iter()
            .map(|(path, _)| path.as_str())
            .collect();
        files.sort_unstable();
        assert_eq!(
            files,
            [
                "BepInEx/config/owner.mod.cfg",
                "BepInEx/plugins/Owner-Mod/translations.json"
            ]
        );
        Ok(())
    }

    #[test]
    fn reads_the_yaml_shape_of_r2modman() -> Result<()> {
        let yaml = r"profileName: Default
mods:
  - name: denikson-BepInExPack_Valheim
    version:
      major: 5
      minor: 4
      patch: 2350
    enabled: true
  - name: Owner-Mod
    version:
      major: 1
      minor: 0
      patch: 0
";
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.start_file(EXPORT_FILE, SimpleFileOptions::default())?;
        writer
            .write_all(yaml.as_bytes())
            .at(Path::new(EXPORT_FILE))?;
        writer.start_file("config/evil.dll", SimpleFileOptions::default())?;
        writer.start_file("../outside.cfg", SimpleFileOptions::default())?;
        let zip_bytes = writer.finish()?.into_inner();

        let index = PackageIndex::from_packages(Vec::new());
        let imported = parse(&zip_bytes, Manifest::new(VALHEIM, Target::Client), &index)?;
        assert_eq!(imported.lock.packages.len(), 2);
        assert_eq!(imported.lock.packages[1].version, Version::new(5, 4, 2350));
        assert!(imported.manifest.is_enabled(&"Owner-Mod".parse()?));
        assert!(imported.files.is_empty());
        Ok(())
    }
}
