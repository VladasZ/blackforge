//! The release manifest the CI writes next to the binaries. Mirrors the shape
//! studio's `AppView.vue` reads, see a live copy at
//! `https://gebling-studio.vladas.xyz/blackforge/download/manifest.json`.

use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct Manifest {
    pub mac: Option<String>,
    pub win: Option<WinArches>,
    pub linux: Option<LinuxArches>,
}

#[derive(Default, Deserialize)]
pub struct WinArches {
    pub x64: Option<String>,
    pub arm64: Option<String>,
}

#[derive(Default, Deserialize)]
pub struct LinuxArches {
    pub x64: Option<LinuxArch>,
    pub arm64: Option<LinuxArch>,
}

#[derive(Default, Deserialize)]
pub struct LinuxArch {
    pub deb: Option<String>,
    pub appimage: Option<String>,
}

/// The version shown on a button comes out of the file name, not the
/// manifest's own `version` field. A file
/// is named `blackforge-0.4.9-mac-universal.dmg`.
pub fn version_of(file: &str) -> String {
    let mut parts = file.split('-').skip(1);

    parts
        .find(|part| {
            part.split('.').count() == 3 && part.split('.').all(|n| n.parse::<u32>().is_ok())
        })
        .unwrap_or_default()
        .to_string()
}

/// One entry in a platform dropdown, an architecture or a package format.
pub struct Choice {
    pub label: String,
    pub file: String,
    pub arch: String,
}

impl Manifest {
    pub fn win_choices(&self) -> Vec<Choice> {
        let win = self.win.as_ref();

        [
            ("x64", win.and_then(|w| w.x64.as_ref())),
            ("arm64", win.and_then(|w| w.arm64.as_ref())),
        ]
        .into_iter()
        .filter_map(|(arch, file)| {
            file.map(|file| Choice {
                label: arch.to_string(),
                file: file.clone(),
                arch: arch.to_string(),
            })
        })
        .collect()
    }

    /// Linux groups by architecture, and each architecture offers the package
    /// formats the release actually produced.
    pub fn linux_groups(&self) -> Vec<(String, Vec<Choice>)> {
        let linux = self.linux.as_ref();

        [
            ("x64", linux.and_then(|l| l.x64.as_ref())),
            ("arm64", linux.and_then(|l| l.arm64.as_ref())),
        ]
        .into_iter()
        .filter_map(|(arch, entry)| {
            let entry = entry?;

            let choices: Vec<Choice> = [
                (".deb", entry.deb.as_ref()),
                (".AppImage", entry.appimage.as_ref()),
            ]
            .into_iter()
            .filter_map(|(label, file)| {
                file.map(|file| Choice {
                    label: label.to_string(),
                    file: file.clone(),
                    arch: arch.to_string(),
                })
            })
            .collect();

            (!choices.is_empty()).then(|| (arch.to_string(), choices))
        })
        .collect()
    }
}
