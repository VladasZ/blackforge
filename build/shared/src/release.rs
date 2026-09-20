use std::fs::read_to_string;

use anyhow::{Context, Result};
use serde::Deserialize;

pub struct Release {
    pub name: String,
    pub version: String,
    pub bundle_id: String,
    pub download_url: String,
}

impl Release {
    pub fn artifact(&self, suffix: &str) -> String {
        format!("{}-{}-{suffix}", self.name, self.version)
    }
}

#[derive(Deserialize)]
struct Cargo {
    workspace: Workspace,
}

#[derive(Deserialize)]
struct Workspace {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    version: String,
}

#[derive(Deserialize)]
struct Hilen {
    project_name: String,
    bundle_id: String,
    release: Distribution,
}

#[derive(Deserialize)]
struct Distribution {
    download_url: String,
}

pub fn read() -> Result<Release> {
    let cargo: Cargo =
        toml::from_str(&read_to_string("Cargo.toml").context("run from the repository root")?)?;
    let config: Hilen = toml::from_str(&read_to_string("hilen.toml")?)?;
    Ok(Release {
        name: config.project_name,
        version: cargo.workspace.package.version,
        bundle_id: config.bundle_id,
        download_url: config.release.download_url.trim_end_matches('/').to_owned(),
    })
}
