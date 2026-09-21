use blackforge_api::setup::Setup;

use super::{
    merge::{first_review, review},
    snapshot::{Snapshot, validate},
};
use crate::error::Result;

/// What one sync has to do, decided without touching the disk or the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// This machine and the cloud already agree.
    Settled,
    /// Every difference merges by itself. The cloud side is installed before
    /// anything is uploaded, so a machine never saves on top of a setup it
    /// could not install.
    Merge {
        merged: Setup,
        install: bool,
        upload: bool,
    },
    /// Both sides changed the same thing, the user picks a whole side.
    /// `local` is what picking this machine uploads.
    Conflict { local: Setup },
}

/// `baseline` is None on a machine that never synced this account, `head` is
/// None for an account with nothing saved. `loaders` are the mod loader ids a
/// new profile gets by itself.
pub fn plan(
    baseline: Option<&Setup>,
    captured: &Snapshot,
    head: Option<&Setup>,
    loaders: &[String],
) -> Result<Step> {
    let local = &captured.setup;
    let Some(remote) = head else {
        return Ok(Step::Merge {
            merged: local.clone(),
            install: false,
            upload: true,
        });
    };
    validate(remote)?;
    let mut result = match baseline {
        Some(base) => review(base, local, remote),
        None => first_review(local, remote, loaders),
    };
    result.keep_local_only(remote, &captured.local_only);
    if result.is_empty() {
        return Ok(Step::Settled);
    }
    // A merge can pair a mod with a dependency the other side removed. That
    // is a conflict too, only a whole side is known to work.
    Ok(
        match result.merged().filter(|merged| validate(merged).is_ok()) {
            Some(merged) => Step::Merge {
                install: result.incoming(),
                upload: merged != *remote,
                merged,
            },
            None => Step::Conflict {
                local: result.local_side(),
            },
        },
    )
}
