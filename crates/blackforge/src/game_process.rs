//! The game behind a start through Steam. On Windows the app starts
//! `steam.exe -applaunch`, which hands the game to the running Steam and exits
//! at once. So the app waits on the game itself, found by its program name.

use std::{ffi::OsStr, path::Path};

use hilen::dispatch::sleep;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Steam can ask about the start arguments first, or update itself.
const APPEAR_SECONDS: f32 = 120.0;
const LOOK_SECONDS: f32 = 2.0;

/// Waits until the game shows up and exits again. False when it never showed
/// up.
pub async fn wait(executable: &Path) -> bool {
    let Some(name) = executable.file_name() else {
        return false;
    };
    let mut system = System::new();
    let mut waited = 0.0;
    while !running(&mut system, name) {
        if waited >= APPEAR_SECONDS {
            return false;
        }
        sleep(LOOK_SECONDS).await;
        waited += LOOK_SECONDS;
    }
    log::info!("the game runs as {}", name.to_string_lossy());
    while running(&mut system, name) {
        sleep(LOOK_SECONDS).await;
    }
    true
}

fn running(system: &mut System, name: &OsStr) -> bool {
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .values()
        .any(|process| process.name().eq_ignore_ascii_case(name))
}
