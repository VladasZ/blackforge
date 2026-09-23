//! Starts the game with the mod loader of a profile.
//!
//! `BepInEx` is loaded by Unity Doorstop. On Linux and macOS Doorstop is a
//! library that is preloaded through the environment, so the game folder is
//! never touched. On Windows it is a proxy `winhttp.dll` that only works
//! from the game folder, so that one file and its config are copied there.

use std::{
    env::var,
    path::{Path, PathBuf},
};

use tokio::{
    fs,
    process::{Child, Command},
};

use crate::{
    achievements,
    error::{Error, IoContext, Result},
    game::{GameDef, GameInstall, Target},
    steam,
    util::exists,
};

const PRELOADER: [&str; 3] = ["BepInEx", "core", "BepInEx.Preloader.dll"];
const DOORSTOP_LIBS: &str = "doorstop_libs";
const WINDOWS_PROXY: &str = "winhttp.dll";
const DOORSTOP_CONFIG: &str = "doorstop_config.ini";
const DOORSTOP_VERSION: &str = ".doorstop_version";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    MacIntel,
    MacArm,
}

impl Os {
    pub fn current() -> Result<Self> {
        if cfg!(target_os = "windows") {
            Ok(Self::Windows)
        } else if cfg!(target_os = "linux") {
            Ok(Self::Linux)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Ok(Self::MacArm)
        } else if cfg!(target_os = "macos") {
            Ok(Self::MacIntel)
        } else {
            Err(Error::Unsupported("this operating system".to_owned()))
        }
    }
}

/// The exact process to start. Built without side effects so every system
/// can be tested on any machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
    /// Files to copy from the profile into the game folder first.
    pub game_files: Vec<String>,
    /// Whether the achievements plugin goes into the profile first.
    pub keep_achievements: bool,
    /// The Steam app has to run before the start, see `steam`.
    pub needs_steam: bool,
}

pub fn preloader_path(profile_dir: &Path) -> PathBuf {
    PRELOADER
        .iter()
        .fold(profile_dir.to_path_buf(), |path, part| path.join(part))
}

/// Doorstop 3 and 4 name their options differently. The loader pack says
/// which one it ships in `.doorstop_version`, a missing file means 3.
pub async fn doorstop_major(profile_dir: &Path) -> u32 {
    let path = profile_dir.join(DOORSTOP_VERSION);
    let Ok(text) = fs::read_to_string(&path).await else {
        return 3;
    };
    text.trim()
        .split('.')
        .next()
        .and_then(|major| major.parse().ok())
        .filter(|major| *major > 3)
        .unwrap_or(3)
}

pub struct LaunchInput<'a> {
    pub os: Os,
    pub game: &'a GameDef,
    pub install: &'a GameInstall,
    pub profile_dir: &'a Path,
    pub doorstop_major: u32,
    pub game_args: &'a [String],
    /// The setting of the user, it only has an effect where the game has
    /// achievements.
    pub keep_achievements: bool,
    /// Inherited values of the variables that the plan extends.
    pub inherited: &'a dyn Fn(&str) -> Option<String>,
}

pub fn plan(input: &LaunchInput<'_>) -> Result<LaunchPlan> {
    let preloader = preloader_path(input.profile_dir)
        .to_string_lossy()
        .into_owned();
    let exe = &input.install.executable;
    let is_windows_binary = exe
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));

    match input.os {
        Os::Windows => Ok(windows_plan(input, preloader)),
        Os::Linux if is_windows_binary => Err(Error::Unsupported(
            "this copy of the game is the Windows build, running it through Proton is not supported yet".to_owned(),
        )),
        Os::Linux | Os::MacIntel | Os::MacArm => Ok(unix_plan(input, preloader)),
    }
}

fn keeps_achievements(input: &LaunchInput<'_>) -> bool {
    input.keep_achievements && achievements::applies_to(input.game)
}

fn doorstop_args(major: u32, preloader: String) -> Vec<String> {
    let (enabled, target) = if major >= 4 {
        ("--doorstop-enabled", "--doorstop-target-assembly")
    } else {
        ("--doorstop-enable", "--doorstop-target")
    };
    vec![
        enabled.to_owned(),
        "true".to_owned(),
        target.to_owned(),
        preloader,
    ]
}

fn windows_plan(input: &LaunchInput<'_>, preloader: String) -> LaunchPlan {
    let mut args = Vec::new();
    // The client goes through Steam so the overlay and the Steam login work.
    // A server, or a copy outside Steam, is started as a plain process.
    let steam = input
        .install
        .steam_dir
        .as_ref()
        .filter(|_| input.game.target == Target::Client);
    let program = match (steam, input.game.steam_app_id) {
        (Some(steam), Some(app_id)) => {
            args.push("-applaunch".to_owned());
            args.push(app_id.to_string());
            steam.join("steam.exe")
        }
        _ => input.install.executable.clone(),
    };
    args.extend(doorstop_args(input.doorstop_major, preloader));
    args.extend(input.game_args.iter().cloned());
    LaunchPlan {
        program,
        args,
        env: Vec::new(),
        cwd: input.install.dir.clone(),
        game_files: vec![
            WINDOWS_PROXY.to_owned(),
            DOORSTOP_CONFIG.to_owned(),
            DOORSTOP_VERSION.to_owned(),
        ],
        keep_achievements: keeps_achievements(input),
        // Steam opens itself, the start goes through it.
        needs_steam: false,
    }
}

fn unix_plan(input: &LaunchInput<'_>, preloader: String) -> LaunchPlan {
    let libs = input.profile_dir.join(DOORSTOP_LIBS);
    let (library, preload_var, path_var) = match input.os {
        Os::Linux => ("libdoorstop_x64.so", "LD_PRELOAD", "LD_LIBRARY_PATH"),
        _ => (
            "libdoorstop_x64.dylib",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
        ),
    };
    let join = |first: String, variable: &str| match (input.inherited)(variable) {
        Some(rest) if !rest.is_empty() => format!("{first}:{rest}"),
        _ => first,
    };

    let mut library_path = libs.to_string_lossy().into_owned();
    if input.os == Os::Linux && input.game.target == Target::Server {
        // The server start script of the game adds this folder, it holds the
        // Steam libraries the server binary links against.
        library_path = format!(
            "{library_path}:{}",
            input.install.dir.join("linux64").to_string_lossy()
        );
    }

    let (enabled, target) = if input.doorstop_major >= 4 {
        ("DOORSTOP_ENABLED", "DOORSTOP_TARGET_ASSEMBLY")
    } else {
        ("DOORSTOP_ENABLE", "DOORSTOP_INVOKE_DLL_PATH")
    };
    let enabled_value = if input.doorstop_major >= 4 {
        "1"
    } else {
        "TRUE"
    };
    let mut env = vec![
        (enabled.to_owned(), enabled_value.to_owned()),
        (target.to_owned(), preloader),
        (path_var.to_owned(), join(library_path, path_var)),
        (
            preload_var.to_owned(),
            join(
                libs.join(library).to_string_lossy().into_owned(),
                preload_var,
            ),
        ),
    ];
    if let Some(app_id) = input.game.client_steam_app_id {
        env.push(("SteamAppId".to_owned(), app_id.to_string()));
    }

    let exe = input.install.executable.to_string_lossy().into_owned();
    let cwd = input.install.dir.clone();
    let game_args = input.game_args.iter().cloned();

    if input.os == Os::MacArm {
        // The Doorstop library is built for Intel only, so the game must run
        // under Rosetta. macOS strips every `DYLD_` variable when a system
        // binary like `arch` starts, so they are handed over with `-e`.
        let mut args = vec!["-x86_64".to_owned()];
        for (name, value) in env {
            args.push("-e".to_owned());
            args.push(format!("{name}={value}"));
        }
        args.push(exe);
        args.extend(game_args);
        return LaunchPlan {
            program: PathBuf::from("/usr/bin/arch"),
            args,
            env: Vec::new(),
            cwd,
            game_files: Vec::new(),
            keep_achievements: keeps_achievements(input),
            needs_steam: steam::needed(input.os, input.game.target),
        };
    }
    LaunchPlan {
        program: PathBuf::from(exe),
        args: game_args.collect(),
        env,
        cwd,
        game_files: Vec::new(),
        keep_achievements: keeps_achievements(input),
        needs_steam: steam::needed(input.os, input.game.target),
    }
}

/// Puts the files of a start in place. The achievements plugin goes into the
/// profile or comes out of it. The Windows proxy files are copied into the game
/// folder, the config goes in switched off, so a plain start from Steam still
/// gives the game without mods, and `run` switches it on through the command
/// line.
pub async fn prepare(plan: &LaunchPlan, profile_dir: &Path) -> Result<()> {
    achievements::apply(profile_dir, plan.keep_achievements).await?;
    for name in &plan.game_files {
        let source = profile_dir.join(name);
        if !exists(&source).await {
            continue;
        }
        let dest = plan.cwd.join(name);
        if name == DOORSTOP_CONFIG {
            let text = fs::read_to_string(&source).await.at(&source)?;
            fs::write(&dest, disable_doorstop(&text)).await.at(&dest)?;
        } else {
            fs::copy(&source, &dest).await.at(&dest)?;
        }
    }
    Ok(())
}

fn disable_doorstop(config: &str) -> String {
    let mut out = String::with_capacity(config.len());
    for line in config.lines() {
        if let Some(key @ ("enabled" | "enable")) = line.split('=').next().map(str::trim) {
            out.push_str(&format!("{key} = false"));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

pub async fn spawn(plan: &LaunchPlan, profile_dir: &Path) -> Result<Child> {
    if !exists(&preloader_path(profile_dir)).await {
        return Err(Error::LoaderMissing);
    }
    // A failed process lookup blocks nothing, the start goes on as before.
    if plan.needs_steam && matches!(steam::running().await, Ok(false)) {
        return Err(Error::SteamNotRunning);
    }
    prepare(plan, profile_dir).await?;
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .envs(plan.env.iter().cloned())
        .current_dir(&plan.cwd);
    command.spawn().at(&plan.program)
}

pub fn inherited_env(name: &str) -> Option<String> {
    var(name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::valheim;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    /// A plan for Linux or macOS is only ever built on that system. These tests
    /// build it on any host, and on Windows `Path::join` writes `\`.
    fn unix(text: &str) -> String {
        text.replace('\\', "/")
    }

    fn input<'a>(
        os: Os,
        game: &'a GameDef,
        install: &'a GameInstall,
        profile: &'a Path,
        args: &'a [String],
    ) -> LaunchInput<'a> {
        LaunchInput {
            os,
            game,
            install,
            profile_dir: profile,
            doorstop_major: 4,
            game_args: args,
            keep_achievements: true,
            inherited: &no_env,
        }
    }

    #[test]
    fn mac_arm_runs_under_rosetta_and_passes_dyld_through_arch() -> Result<()> {
        let game = valheim(Target::Client)?;
        let install = GameInstall {
            dir: PathBuf::from("/games/Valheim"),
            executable: PathBuf::from("/games/Valheim/valheim.app/Contents/MacOS/Valheim"),
            steam_dir: None,
            updated: None,
        };
        let args = ["-console".to_owned()];
        let plan = plan(&input(
            Os::MacArm,
            &game,
            &install,
            Path::new("/data/profiles/main"),
            &args,
        ))?;
        assert_eq!(plan.program, PathBuf::from("/usr/bin/arch"));
        let args: Vec<String> = plan.args.iter().map(|arg| unix(arg)).collect();
        assert_eq!(
            args,
            [
                "-x86_64",
                "-e",
                "DOORSTOP_ENABLED=1",
                "-e",
                "DOORSTOP_TARGET_ASSEMBLY=/data/profiles/main/BepInEx/core/BepInEx.Preloader.dll",
                "-e",
                "DYLD_LIBRARY_PATH=/data/profiles/main/doorstop_libs",
                "-e",
                "DYLD_INSERT_LIBRARIES=/data/profiles/main/doorstop_libs/libdoorstop_x64.dylib",
                "-e",
                "SteamAppId=892970",
                "/games/Valheim/valheim.app/Contents/MacOS/Valheim",
                "-console",
            ]
        );
        assert!(plan.env.is_empty());
        assert!(plan.game_files.is_empty());
        assert!(plan.keep_achievements);
        assert!(plan.needs_steam);
        Ok(())
    }

    #[test]
    fn linux_server_preloads_doorstop_and_keeps_inherited_paths() -> Result<()> {
        let game = valheim(Target::Server)?;
        let install = GameInstall {
            dir: PathBuf::from("/srv/valheim"),
            executable: PathBuf::from("/srv/valheim/valheim_server.x86_64"),
            steam_dir: None,
            updated: None,
        };
        let inherited = |name: &str| (name == "LD_LIBRARY_PATH").then(|| "/usr/lib".to_owned());
        let plan = plan(&LaunchInput {
            os: Os::Linux,
            game: &game,
            install: &install,
            profile_dir: Path::new("/data/p"),
            doorstop_major: 4,
            game_args: &[],
            keep_achievements: true,
            inherited: &inherited,
        })?;
        assert_eq!(
            plan.program,
            PathBuf::from("/srv/valheim/valheim_server.x86_64")
        );
        let env: Vec<(String, String)> = plan
            .env
            .iter()
            .map(|(name, value)| (name.clone(), unix(value)))
            .collect();
        assert_eq!(
            env,
            [
                ("DOORSTOP_ENABLED".to_owned(), "1".to_owned()),
                (
                    "DOORSTOP_TARGET_ASSEMBLY".to_owned(),
                    "/data/p/BepInEx/core/BepInEx.Preloader.dll".to_owned()
                ),
                (
                    "LD_LIBRARY_PATH".to_owned(),
                    "/data/p/doorstop_libs:/srv/valheim/linux64:/usr/lib".to_owned()
                ),
                (
                    "LD_PRELOAD".to_owned(),
                    "/data/p/doorstop_libs/libdoorstop_x64.so".to_owned()
                ),
                ("SteamAppId".to_owned(), "892970".to_owned()),
            ]
        );
        // A server has no achievements, the setting of the user is dropped.
        assert!(!plan.keep_achievements);
        Ok(())
    }

    #[test]
    fn linux_refuses_the_windows_build() -> Result<()> {
        let game = valheim(Target::Client)?;
        let install = GameInstall {
            dir: PathBuf::from("/games/Valheim"),
            executable: PathBuf::from("/games/Valheim/valheim.exe"),
            steam_dir: None,
            updated: None,
        };
        let result = plan(&input(
            Os::Linux,
            &game,
            &install,
            Path::new("/data/p"),
            &[],
        ));
        assert!(matches!(result, Err(Error::Unsupported(_))));
        Ok(())
    }

    #[test]
    fn windows_client_goes_through_steam() -> Result<()> {
        let game = valheim(Target::Client)?;
        let install = GameInstall {
            dir: PathBuf::from("C:/Steam/steamapps/common/Valheim"),
            executable: PathBuf::from("C:/Steam/steamapps/common/Valheim/valheim.exe"),
            steam_dir: Some(PathBuf::from("C:/Steam")),
            updated: None,
        };
        let plan = plan(&input(
            Os::Windows,
            &game,
            &install,
            Path::new("C:/data/p"),
            &[],
        ))?;
        assert_eq!(plan.program, PathBuf::from("C:/Steam/steam.exe"));
        assert_eq!(
            plan.args[..4],
            ["-applaunch", "892970", "--doorstop-enabled", "true"]
        );
        assert_eq!(plan.args[4], "--doorstop-target-assembly");
        assert_eq!(
            plan.game_files,
            ["winhttp.dll", "doorstop_config.ini", ".doorstop_version"]
        );
        Ok(())
    }

    #[test]
    fn windows_server_starts_the_binary_itself() -> Result<()> {
        let game = valheim(Target::Server)?;
        let install = GameInstall {
            dir: PathBuf::from("C:/server"),
            executable: PathBuf::from("C:/server/valheim_server.exe"),
            steam_dir: Some(PathBuf::from("C:/Steam")),
            updated: None,
        };
        let plan = plan(&input(
            Os::Windows,
            &game,
            &install,
            Path::new("C:/data/p"),
            &[],
        ))?;
        assert_eq!(plan.program, PathBuf::from("C:/server/valheim_server.exe"));
        assert_eq!(plan.args[0], "--doorstop-enabled");
        Ok(())
    }

    #[test]
    fn copied_doorstop_config_is_switched_off() {
        let config =
            "[General]\nenabled = true\ntarget_assembly=BepInEx\\core\\BepInEx.Preloader.dll\n";
        assert_eq!(
            disable_doorstop(config),
            "[General]\nenabled = false\ntarget_assembly=BepInEx\\core\\BepInEx.Preloader.dll\n"
        );
    }
}
