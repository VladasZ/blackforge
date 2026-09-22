//! Newer versions of the locked mods. The app looks for them at start and
//! when the user asks, and keeps the answer for the session, so the Mods page
//! and the sidebar badge show it on any page.

use std::{cell::RefCell, collections::HashMap};

use chrono::Utc;

use crate::{
    backend,
    ui::{mods_page, nav_item::Badge, page::Page, sidebar, toast},
};

/// A newer version of a locked mod.
#[derive(Clone, Debug)]
pub struct Newer {
    pub version: String,
    /// Held at its version, so update all leaves it and the row offers no
    /// update of its own.
    pub pinned: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Updates {
    /// When the last check ended, seconds since the epoch. `None` before the
    /// first one, `newest` is only complete after it.
    pub checked: Option<i64>,
    /// A check runs now.
    pub checking: bool,
    /// The newest version of every locked mod that has one, by id.
    pub newest: HashMap<String, Newer>,
}

impl Updates {
    /// The mods update all would move, pinned ones left out.
    pub fn movable(&self) -> usize {
        self.newest.values().filter(|newer| !newer.pinned).count()
    }
}

thread_local! {
    static UPDATES: RefCell<Updates> = RefCell::new(Updates::default());
}

pub fn get() -> Updates {
    UPDATES.with(|updates| updates.borrow().clone())
}

fn change(edit: impl FnOnce(&mut Updates)) {
    UPDATES.with(|updates| edit(&mut updates.borrow_mut()));
    let movable = get().movable();
    sidebar::set_badge(Page::Mods, Badge::Count(movable));
    mods_page::updates_changed();
}

/// Looks for newer versions. `announce` shows the result as a toast, the
/// check at start stays quiet and only fills the badge.
pub fn check(announce: bool) {
    if get().checking {
        return;
    }
    change(|updates| updates.checking = true);
    backend::load(
        "looking for newer versions",
        |forge, progress| async move {
            let profile = backend::profile(forge, &progress).await?;
            let outdated = forge.outdated(&profile, &progress).await?;
            backend::forget_index().await;
            Ok(outdated)
        },
        move |result| match result {
            Ok(outdated) => {
                if announce {
                    if outdated.is_empty() {
                        toast::success("Every mod is on its newest version");
                    } else {
                        toast::info(format!("{} mods have a newer version", outdated.len()));
                    }
                }
                change(|updates| {
                    updates.checking = false;
                    updates.checked = Some(Utc::now().timestamp());
                    updates.newest = outdated
                        .into_iter()
                        .map(|entry| {
                            (
                                entry.id.to_string(),
                                Newer {
                                    version: entry.latest.to_string(),
                                    pinned: entry.pinned,
                                },
                            )
                        })
                        .collect();
                });
            }
            Err(error) => {
                change(|updates| updates.checking = false);
                if announce {
                    toast::failure(&error);
                } else {
                    log::warn!("the update check at start failed: {error:#}");
                }
            }
        },
    );
}

/// Update all moved every mod that is not pinned.
pub fn all_moved() {
    change(|updates| updates.newest.retain(|_, newer| newer.pinned));
}

/// One mod moved to its newest version.
pub fn moved(id: &str) {
    change(|updates| {
        updates.newest.remove(id);
    });
}
