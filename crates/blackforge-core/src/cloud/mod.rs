//! Private setup snapshots, three-way review, and staged installation.
mod merge;
mod restore;
mod snapshot;
mod state;

pub use merge::{Change, Entry, Key, Review, review};
pub use restore::{recover, restore};
pub use snapshot::{capture, portable, snapshot, validate};
pub use state::History;

#[cfg(test)]
mod tests;
