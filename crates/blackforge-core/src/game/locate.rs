use std::{
    fs::read_dir,
    path::{Path, PathBuf},
    time::SystemTime,
};

use steamlocate::SteamDir;
use tokio::task::spawn_blocking;

use crate::{
    error::{Error, IoContext, Result},
    game::GameDef,
};

/// An installed copy of the game on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameInstall {
    pub dir: PathBuf,
    pub executable: PathBuf,
    /// `None` when the folder was given by hand and not found through Steam.
    pub steam_dir: Option<PathBuf>,
    /// When Steam last updated the game, from its manifest. `None` for a
    /// folder given by hand, and when the manifest does not say.
    pub updated: Option<SystemTime>,
}

/// Finds the game. `game_dir` skips the Steam lookup, for a server installed
/// with `SteamCMD` or any copy outside the Steam libraries.
pub async fn locate(game: &GameDef, game_dir: Option<PathBuf>) -> Result<GameInstall> {
    let game = game.clone();
    spawn_blocking(move || {
        let (dir, steam_dir, updated) = if let Some(dir) = game_dir {
            (dir, None, None)
        } else {
            let (dir, steam_dir, updated) = steam_app_dir(&game)?;
            (dir, Some(steam_dir), updated)
        };
        let executable = find_executable(&game, &dir)?;
        Ok(GameInstall {
            dir,
            executable,
            steam_dir,
            updated,
        })
    })
    .await?
}

/// The game folder, the Steam folder, and when Steam last updated the game.
fn steam_app_dir(game: &GameDef) -> Result<(PathBuf, PathBuf, Option<SystemTime>)> {
    let not_installed = || Error::GameNotInstalled(game.display_name.clone());
    let app_id = game.steam_app_id.ok_or_else(not_installed)?;
    let steam = SteamDir::locate().map_err(|_| not_installed())?;
    let (app, library) = steam
        .find_app(app_id)
        .map_err(|_| not_installed())?
        .ok_or_else(not_installed)?;
    Ok((
        library.resolve_app_dir(&app),
        steam.path().to_path_buf(),
        app.last_updated,
    ))
}

fn find_executable(game: &GameDef, dir: &Path) -> Result<PathBuf> {
    let found = if cfg!(target_os = "macos") {
        mac_executable(dir)?
    } else {
        listed_executable(game, dir)
    };
    found.ok_or_else(|| Error::Invalid(format!("no game executable found in {}", dir.display())))
}

/// The schema lists the Windows and the Linux binary. A Linux machine that has
/// only the `.exe` runs the game through Proton, and the native name is tried
/// first so a native build wins.
fn listed_executable(game: &GameDef, dir: &Path) -> Option<PathBuf> {
    let windows = cfg!(target_os = "windows");
    let mut names: Vec<&String> = game.exe_names.iter().collect();
    names.sort_by_key(|name| name.to_lowercase().ends_with(".exe") != windows);
    names
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

/// The schema has no macOS names. The game ships as one `.app` bundle with a
/// single binary inside `Contents/MacOS`.
fn mac_executable(dir: &Path) -> Result<Option<PathBuf>> {
    for entry in read_dir(dir).at(dir)? {
        let bundle = entry.at(dir)?.path();
        if bundle
            .extension()
            .is_none_or(|extension| extension != "app")
        {
            continue;
        }
        let binaries = bundle.join("Contents").join("MacOS");
        let Ok(entries) = read_dir(&binaries) else {
            continue;
        };
        let stem = bundle
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase());
        let mut files: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.is_file())
            .collect();
        files.sort();
        let by_name = files
            .iter()
            .find(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_lowercase())
                    == stem
            })
            .cloned();
        if let Some(found) = by_name.or_else(|| files.into_iter().next()) {
            return Ok(Some(found));
        }
    }
    Ok(None)
}
