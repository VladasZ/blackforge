//! What the user can do inside blackforge about a problem. The core only
//! names the action. Each frontend says how to do it there, the command line
//! as a command and the window as a button, so no text of the core ever names
//! a frontend.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fix {
    /// No profile exists yet. Adding a mod or starting the game makes one.
    CreateProfile,
    /// Profiles exist but none is the active one.
    PickProfile,
    /// Steam does not know the game, the folder has to be given by hand.
    GiveGameFolder,
    /// The profile folder does not hold what the lock asks for.
    Sync,
    /// The game client on a Mac needs the Steam app running.
    StartSteam,
}
