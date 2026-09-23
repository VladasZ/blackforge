//! Private setup snapshots, automatic merging, and staged installation.
mod merge;
mod plan;
mod restore;
mod snapshot;
mod state;

pub use merge::{Key, LaunchKey};
pub use plan::{Step, plan};
pub use restore::{recover, restore};
pub use snapshot::{Snapshot, capture, portable, portable_args, snapshot, validate};
pub use state::Baseline;

#[cfg(test)]
mod tests;
