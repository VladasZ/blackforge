//! Starts the game with the mods of the active profile.

use std::{
    path::PathBuf,
    sync::{
        LazyLock,
        atomic::{AtomicU8, Ordering},
    },
};

use blackforge_core::{
    Error,
    launch::{LaunchInput, Os, doorstop_major, inherited_env, plan, spawn as spawn_game},
};
use hilen::{
    dispatch::{on_main, spawn},
    filesystem::Paths,
    store::OnDisk,
    ui::Question,
};
use tokio::process::Child;

use crate::{
    backend,
    cloud::{self, Launch},
    social,
    ui::{sidebar, toast},
};

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

static GAME_ARGS: LazyLock<OnDisk<String>> = LazyLock::new(|| OnDisk::new("gui-game-args.json"));

/// Extra arguments for the game itself, split on spaces at launch.
pub fn game_args() -> String {
    GAME_ARGS.get().unwrap_or_default()
}

pub fn set_game_args(args: &str) {
    GAME_ARGS.set(args.to_owned());
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
    let game_args: Vec<String> = game_args().split_whitespace().map(str::to_owned).collect();

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
            let plan = plan(&LaunchInput {
                os: Os::current()?,
                game: &game,
                install: &install,
                profile_dir: profile.dir(),
                doorstop_major,
                game_args: &game_args,
                keep_achievements: forge.keep_achievements().await?,
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
