use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "blackforge", version, about = "Mod manager for Valheim")]
pub struct Cli {
    /// Work on this profile and not on the active one
    #[arg(long, global = true)]
    pub profile: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add mods to the manifest and the lock, then sync
    Add {
        /// Owner-Name or a bare Name, always at the newest version
        #[arg(required = true)]
        mods: Vec<String>,
    },
    /// Remove mods, and the dependencies nothing else needs
    Remove {
        #[arg(required = true)]
        mods: Vec<String>,
    },
    /// Make the profile folder match the lock
    Sync,
    /// Move the lock to the newest versions, all mods when none is named
    Update { mods: Vec<String> },
    /// Show which locked mods have a newer version
    Outdated,
    /// Start the game or the server with the mods of the profile
    Run {
        /// Folder of the game, for a copy that Steam does not know. It is remembered
        #[arg(long)]
        game_dir: Option<PathBuf>,
        /// Arguments for the game itself, after `--`
        #[arg(last = true)]
        game_args: Vec<String>,
    },
    /// Search Thunderstore
    Search {
        #[arg(required = true)]
        words: Vec<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Show the details of one mod
    Info { name: String },
    /// Show the mods of the profile
    List,
    /// Put the files of a mod back without changing the lock
    Enable { name: String },
    /// Leave the files of a mod out without removing it from the manifest
    Disable { name: String },
    /// Manage profiles
    #[command(subcommand)]
    Profile(ProfileCommand),
    /// Create a profile from an r2modman code or .r2z file
    Import {
        source: String,
        /// Name of the new profile, taken from the export when left out
        #[arg(long)]
        name: Option<String>,
        /// Make it a dedicated server profile
        #[arg(long)]
        server: bool,
    },
    /// Share the profile as an r2modman code, or as a file with --file
    Export {
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Write the profile as a plain folder, for a rented or Docker server
    Deploy {
        path: PathBuf,
        /// Replace configs that already exist at the target
        #[arg(long)]
        overwrite_configs: bool,
    },
    /// Check everything that run depends on
    Doctor,
    /// Let a modded game earn achievements, shows the setting when no word is given
    Achievements {
        #[arg(value_enum)]
        state: Option<OnOff>,
    },
    /// List the portals of a world save
    Portals {
        /// A world .db file, or the world folder of a newer save
        world: PathBuf,
        /// Print only the names that have no second portal
        #[arg(long)]
        unpaired: bool,
    },
    /// Read and edit the config files of the mods
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OnOff {
    On,
    Off,
}

#[derive(Debug, Args)]
pub struct NewProfile {
    pub name: String,
    /// Make it a dedicated server profile
    #[arg(long)]
    pub server: bool,
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// List the profiles, the active one is marked
    List,
    /// Create a profile without switching to it
    Create(NewProfile),
    /// Make a profile the active one
    Switch { name: String },
    /// Delete a profile with its mods and configs
    Delete { name: String },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// List the config files of the profile
    List,
    /// Show every setting of one config file
    Show { file: String },
    /// Print one value
    Get {
        file: String,
        section: String,
        key: String,
    },
    /// Change one value
    Set {
        file: String,
        section: String,
        key: String,
        value: String,
    },
}
