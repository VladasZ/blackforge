//! Shared by linux.rs and win.rs. Builds the tool image and runs one shell
//! string inside it with the repo mounted, plus named volumes for the cargo
//! caches so a second run does not start from zero.

use anyhow::{Result, bail};
use shared::run::{capture, run};
use std::env::current_dir;

pub fn build_image(name: &str, dockerfile: &str, platform: &str) -> Result<()> {
    run(&format!(
        "docker build --platform {platform} -f build/release/{dockerfile} -t {name} build/release"
    ))
}

pub fn run_in(image: &str, platform: &str, lane: &str, script: &str) -> Result<()> {
    let stage = match lane {
        "release-linux" => "linux-stage",
        "release-win" => "win-stage",
        _ => bail!("unknown release lane {lane}"),
    };
    let uid = capture("id -u")?;
    let gid = capture("id -g")?;
    let cwd = current_dir()?;
    let cwd = cwd.display();
    let volume = format!("blackforge-{lane}");
    let arch = platform.replace('/', "-");
    run(&format!(
        r#"docker run --rm --platform {platform} \
  -v "{cwd}:/work/apps/app" \
  -v {volume}-{arch}-target:/work/apps/app/target/{lane} \
  -v {volume}-{arch}-cargo-registry:/usr/local/cargo/registry \
  -v {volume}-{arch}-cargo-git:/usr/local/cargo/git \
  -v {volume}-{arch}-rustup:/usr/local/rustup \
  -e CARGO_TARGET_DIR=/work/apps/app/target/{lane} \
  {image} bash -c 'trap "chown -R {uid}:{gid} /work/apps/app/target/{stage}" EXIT; {script}'"#
    ))
}
