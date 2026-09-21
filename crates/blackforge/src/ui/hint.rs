//! How a fix that the core names is done in the window.

use anyhow::Error;
use blackforge_core::{Error as CoreError, fix::Fix};

/// The window has no profiles, it always works on the default one and makes
/// it by itself, so the two profile fixes have nothing to point at here.
pub fn hint(fix: Fix) -> Option<&'static str> {
    match fix {
        Fix::CreateProfile | Fix::PickProfile => None,
        Fix::GiveGameFolder => Some("press Run game to pick the folder by hand"),
        Fix::Sync => Some("press Run game, it installs the missing files first"),
    }
}

/// What was found, followed by the way out when there is one.
pub fn with_hint(text: &str, fix: Option<Fix>) -> String {
    match fix.and_then(hint) {
        Some(hint) => format!("{text}, {hint}"),
        None => text.to_owned(),
    }
}

/// The whole error chain, followed by the way out when the core knows one.
pub fn describe(error: &Error) -> String {
    let fix = error.downcast_ref::<CoreError>().and_then(CoreError::fix);
    with_hint(&format!("{error:#}"), fix)
}
