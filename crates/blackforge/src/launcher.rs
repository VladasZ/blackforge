//! Starts the game with the mods of the active profile.

use std::{
    path::PathBuf,
    sync::{
        LazyLock,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

use blackforge_core::{
    Error,
    game::Target,
    join,
    launch::{LaunchInput, Os, doorstop_major, inherited_env, plan, spawn as spawn_game},
    progress::Progress,
    steam,
};
use hilen::{
    dispatch::{on_main, sleep, spawn},
    filesystem::Paths,
    store::OnDisk,
    ui::Question,
};
use tokio::process::Child;

use crate::{
    backend, bridge,
    cloud::{self, Launch},
    social,
    ui::{doctor_page, sidebar, toast},
};

/// Steam is looked for this often on a Mac.
const STEAM_SECONDS: f32 = 3.0;

/// Where a start of the game is, shown on the run button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Run {
    Idle,
    Syncing,
    Starting,
    Running,
}

impl Run {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Syncing,
            2 => Self::Starting,
            3 => Self::Running,
            _ => Self::Idle,
        }
    }
}

static RUN: AtomicU8 = AtomicU8::new(Run::Idle as u8);

pub fn run() -> Run {
    Run::from_u8(RUN.load(Ordering::SeqCst))
}

/// A start is under way or the game runs.
pub fn running() -> bool {
    run() != Run::Idle
}

fn set_run(run: Run) {
    RUN.store(run as u8, Ordering::SeqCst);
    on_main(move || sidebar::show_run(run));
}

/// The game needs the Steam app and it does not run, see `watch_steam`.
static STEAM_MISSING: AtomicBool = AtomicBool::new(false);

pub fn steam_missing() -> bool {
    STEAM_MISSING.load(Ordering::SeqCst)
}

/// On a Mac the game client signs in through the Steam app, and the game is
/// started without it. The run button waits until Steam runs, and the
/// Doctor badge follows. Runs for the whole session.
pub fn watch_steam() {
    let Ok(os) = Os::current() else {
        return;
    };
    if !steam::needed(os, Target::Client) {
        return;
    }
    spawn(async move {
        loop {
            let missing = steam_missing_now(os).await;
            if STEAM_MISSING.swap(missing, Ordering::SeqCst) != missing {
                on_main(|| {
                    sidebar::show_run(run());
                    doctor_page::check_in_background();
                });
            }
            sleep(STEAM_SECONDS).await;
        }
    });
}

async fn steam_missing_now(os: Os) -> bool {
    let target = match profile_target().await {
        Ok(target) => target,
        Err(error) => {
            log::warn!("the profile did not load for the Steam check: {error:#}");
            return false;
        }
    };
    if !steam::needed(os, target) {
        return false;
    }
    match steam::running().await {
        Ok(running) => !running,
        Err(error) => {
            log::warn!("the Steam check did not run: {error:#}");
            false
        }
    }
}

async fn profile_target() -> anyhow::Result<Target> {
    let forge = backend::forge()?;
    let profile = backend::profile(forge, &Progress::silent()).await?;
    Ok(profile.manifest().await?.target)
}

/// Where the game arguments lived before they moved into the launch file of
/// the profile, see `move_old_game_args`.
static OLD_GAME_ARGS: LazyLock<OnDisk<String>> =
    LazyLock::new(|| OnDisk::new("gui-game-args.json"));

/// Moves the game arguments of an app before 0.1.16 into the profile, so
/// cloud sync carries them. Runs once, the old file goes away after.
pub fn move_old_game_args() {
    let Some(args) = OLD_GAME_ARGS.get() else {
        return;
    };
    if args.trim().is_empty() {
        OLD_GAME_ARGS.reset();
        return;
    }
    backend::load(
        "moving the game arguments",
        |forge, progress| async move {
            let profile = backend::profile(forge, &progress).await?;
            if profile.launch_settings().await?.is_none() {
                forge
                    .edit_launch_settings(&profile, |settings| settings.game_args = args)
                    .await?;
            }
            Ok(())
        },
        |result| match result {
            Ok(()) => {
                OLD_GAME_ARGS.reset();
                cloud::schedule();
            }
            Err(error) => log::warn!("the game arguments did not move: {error:#}"),
        },
    );
}

enum Started {
    Running {
        child: Box<Child>,
        label: String,
    },
    /// Steam does not know the game, the user has to point at the folder.
    NotFound {
        game: String,
    },
}

pub fn run_game() {
    start(None);
}

/// A folder given here wins over the Steam lookup and is remembered.
pub fn run_game_from(game_dir: PathBuf) {
    start(Some(game_dir));
}

fn start(game_dir: Option<PathBuf>) {
    let first = if social::signed_in() {
        Run::Syncing
    } else {
        Run::Starting
    };
    // The button is off while a start runs, a second start can only come
    // from the folder picker of a failed one.
    if RUN
        .compare_exchange(
            Run::Idle as u8,
            first as u8,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_err()
    {
        return;
    }
    sidebar::show_run(first);
    if first == Run::Starting {
        launch(game_dir);
        return;
    }
    // Like Steam, the game waits for the newest setup. A machine that plays
    // on an old one writes settings that conflict with the cloud later.
    backend::load(
        "syncing with the cloud",
        |_, progress| async move { Ok(cloud::before_launch(&progress).await) },
        move |result| match result {
            Ok(Launch::Ready) => launch(game_dir),
            Ok(Launch::Conflict(pending)) => cloud::ask(*pending, move |settled| {
                if settled {
                    launch(game_dir);
                } else {
                    set_run(Run::Idle);
                }
            }),
            Err(error) => {
                set_run(Run::Idle);
                toast::failure(&error);
            }
        },
    );
}

/// `RUN` is set by now, every way out that starts no game sets it back to idle.
fn launch(game_dir: Option<PathBuf>) {
    set_run(Run::Starting);

    backend::load(
        "starting the game",
        move |forge, progress| async move {
            let held = backend::PROFILE_IO.lock().await;
            let profile = backend::profile(forge, &progress).await?;
            // The command line can change the lock without installing, and the
            // install after a new profile can fail. When nothing is missing
            // this only reads the install state, no network.
            forge.sync(&profile, &progress).await?;
            let manifest = profile.manifest().await?;
            let game = forge.game(&manifest).await?;
            let install = match forge.locate_game(&game, game_dir).await {
                Ok(install) => install,
                Err(Error::GameNotInstalled(game)) => return Ok(Started::NotFound { game }),
                Err(error) => return Err(error.into()),
            };
            let doorstop_major = doorstop_major(profile.dir()).await;
            let settings = forge.launch_settings(&profile).await?;
            let mut game_args: Vec<String> = settings
                .game_args
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            // The join plugin asks this app for a code at a click on a join
            // button, the servers let nobody in without one.
            if join::applies_to(&game) {
                game_args.extend(join::bridge_args(bridge::open()?, &bridge::new_key()?));
            }
            let plan = plan(&LaunchInput {
                os: Os::current()?,
                game: &game,
                install: &install,
                profile_dir: profile.dir(),
                doorstop_major,
                game_args: &game_args,
                keep_achievements: settings.keep_achievements,
                inherited: &inherited_env,
            })?;
            let child = spawn_game(&plan, profile.dir()).await?;
            drop(held);
            let label = format!("{} {}", game.display_name, manifest.target);
            Ok(Started::Running {
                child: Box::new(child),
                label,
            })
        },
        |result| match result {
            Ok(Started::Running { child, label }) => {
                set_run(Run::Running);
                toast::success(format!("Started {label}"));
                social::game_started();
                wait_for_exit(*child);
            }
            Ok(Started::NotFound { game }) => {
                set_run(Run::Idle);
                ask_for_folder(&game);
            }
            Err(error) => {
                set_run(Run::Idle);
                toast::failure(&error);
            }
        },
    );
}

fn wait_for_exit(mut child: Child) {
    spawn(async move {
        // The key stays. Steam often hands the game to a new process, so the
        // child exits at once while the game runs on and still needs its key.
        // The next start replaces the key.
        let status = child.wait().await;
        on_main(move || {
            set_run(Run::Idle);
            social::game_exited();
            match status {
                Ok(status) if status.success() => log::info!("the game exited with {status}"),
                Ok(status) => toast::error(format!("the game exited with {status}")),
                Err(error) => toast::error(format!("cannot wait for the game: {error}")),
            }
        });
    });
}

fn ask_for_folder(game: &str) {
    Question::ask(format!(
        "{game} was not found through Steam. Pick the game folder by hand?"
    ))
    .on_yes(pick_folder_and_run);
}

fn pick_folder_and_run() {
    spawn(async {
        let picked = Paths::pick_folder().await;
        on_main(move || {
            if let Some(game_dir) = picked {
                run_game_from(game_dir);
            }
        });
    });
}
