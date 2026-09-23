//! The Steam app on a Mac. The game client logs in and plays online through
//! it. On Windows `run` starts the game through `steam.exe`, which opens
//! Steam first. On a Mac the game binary is started directly, and with no
//! Steam running `SteamAPI_Init` fails and the game has no login.

use std::path::Path;

use tokio::process::Command;

use crate::{
    error::{Error, IoContext, Result},
    game::Target,
    launch::Os,
};

/// The name of the Steam process inside `Steam.app`.
const PROCESS: &str = "steam_osx";
const PGREP: &str = "/usr/bin/pgrep";
const OPEN: &str = "/usr/bin/open";

/// A start of the `target` of the game on `os` needs a running Steam app.
pub fn needed(os: Os, target: Target) -> bool {
    matches!(os, Os::MacArm | Os::MacIntel) && target == Target::Client
}

/// The Steam app runs now.
pub async fn running() -> Result<bool> {
    let output = Command::new(PGREP)
        .args(["-x", PROCESS])
        .output()
        .await
        .at(Path::new(PGREP))?;
    Ok(output.status.success())
}

/// Opens the Steam app. It takes a few seconds to start.
pub async fn open() -> Result<()> {
    let status = Command::new(OPEN)
        .args(["-a", "Steam"])
        .status()
        .await
        .at(Path::new(OPEN))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Invalid(
            "Steam could not be opened, is it installed?".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::needed;
    use crate::{game::Target, launch::Os};

    #[test]
    fn only_a_mac_client_needs_steam() {
        assert!(needed(Os::MacArm, Target::Client));
        assert!(needed(Os::MacIntel, Target::Client));
        assert!(!needed(Os::MacArm, Target::Server));
        assert!(!needed(Os::Windows, Target::Client));
        assert!(!needed(Os::Linux, Target::Client));
    }
}
