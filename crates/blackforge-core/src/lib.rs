//! The library behind the blackforge mod manager.
//!
//! It never writes to the terminal. Long operations are async and report
//! through [`progress::Progress`], so a command line and a graphical frontend
//! can both sit on top of it. [`forge::Forge`] is the entry point.

pub mod achievements;
pub mod broken;
pub mod cloud;
pub mod config;
pub mod deploy;
pub mod doctor;
pub mod emoji;
pub mod error;
pub mod fix;
pub mod forge;
pub mod game;
pub mod http;
pub mod hugin;
pub mod ident;
pub mod install;
pub mod join;
pub mod launch;
pub mod lock;
pub mod manifest;
pub mod paths;
pub mod profile;
pub mod progress;
pub mod quit;
pub mod r2;
pub mod rail;
pub mod required;
pub mod resolve;
mod served_list;
pub mod servers;
pub mod social;
pub mod steam;
#[cfg(test)]
mod testing;
pub mod thunderstore;
mod util;
pub mod world;

pub use error::{Error, Result};
