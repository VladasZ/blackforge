//! Running commands. Every command is a shell string, the same way the make
//! targets and the old scripts wrote them, so a pipe or a quoted argument keeps
//! working without being taken apart into an argv.

use std::process::{Command, Stdio};

use anyhow::{Result, bail};

/// The shell that runs a command string, cmd on Windows and sh elsewhere.
fn shell(cmd: &str) -> Command {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c");
        c
    };
    c.arg(cmd);
    c
}

/// Echo the command, run it inheriting the terminal, and fail on a non zero exit.
pub fn run(cmd: &str) -> Result<()> {
    println!("{cmd}");
    let status = shell(cmd).status()?;
    if !status.success() {
        bail!("command failed: {cmd}");
    }
    Ok(())
}

/// Capture stdout. stdout is piped rather than inherited, so a captured secret
/// never reaches the log.
pub fn capture(cmd: &str) -> Result<String> {
    println!("{cmd}");
    let out = shell(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;
    if !out.status.success() {
        bail!("command failed: {cmd}");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Run a command containing credentials without echoing it.
pub fn run_secret(cmd: &str) -> Result<()> {
    let status = shell(cmd).status()?;
    if !status.success() {
        bail!("credentialed command failed with {status}");
    }
    Ok(())
}
