use std::collections::{BTreeMap, BTreeSet};

use blackforge_api::setup::{Mod, Setup};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Mod(String),
    Setting {
        file: String,
        section: String,
        key: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Mod(Mod),
    Setting(String),
}

#[derive(Clone, Debug)]
struct Change {
    key: Key,
    local: Option<Entry>,
    remote: Option<Entry>,
    /// None when both sides changed the key, only the user can settle that.
    take_remote: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Review {
    local: Setup,
    changes: Vec<Change>,
    preserved: BTreeMap<Key, Entry>,
}

impl Review {
    /// A local secret or path never deletes a portable value saved by a
    /// different machine, and never appears as a pending local change.
    pub fn keep_local_only(&mut self, remote: &Setup, keys: &[Key]) {
        self.changes.retain(|change| !keys.contains(&change.key));
        self.preserved = flatten(remote)
            .into_iter()
            .filter(|(key, _)| keys.contains(key))
            .collect();
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The cloud holds something this machine has not installed yet.
    pub fn incoming(&self) -> bool {
        self.changes
            .iter()
            .any(|change| change.take_remote == Some(true))
    }

    /// None while a conflict waits for the user.
    pub fn merged(&self) -> Option<Setup> {
        let mut entries = flatten(&self.local);
        for change in &self.changes {
            let value = if change.take_remote? {
                &change.remote
            } else {
                &change.local
            };
            if let Some(value) = value {
                entries.insert(change.key.clone(), value.clone());
            } else {
                entries.remove(&change.key);
            }
        }
        entries.extend(self.preserved.clone());
        Some(expand(entries))
    }

    /// The whole local side, what the user keeps by picking this machine.
    pub fn local_side(&self) -> Setup {
        let mut entries = flatten(&self.local);
        entries.extend(self.preserved.clone());
        expand(entries)
    }
}

/// `base` is the setup this machine and the cloud last agreed on. A machine
/// installs the cloud side before it uploads, so one baseline is enough.
pub fn review(base: &Setup, local: &Setup, remote: &Setup) -> Review {
    let b = flatten(base);
    let l = flatten(local);
    let r = flatten(remote);
    let keys: BTreeSet<_> = l.keys().chain(r.keys()).cloned().collect();
    let changes = keys
        .into_iter()
        .filter_map(|key| {
            let mine = l.get(&key);
            let theirs = r.get(&key);
            if mine == theirs {
                return None;
            }
            let take_remote = match (mine != b.get(&key), theirs != b.get(&key)) {
                (true, true) => None,
                (true, false) => Some(false),
                _ => Some(true),
            };
            Some(Change {
                key,
                local: mine.cloned(),
                remote: theirs.cloned(),
                take_remote,
            })
        })
        .collect();
    Review {
        local: local.clone(),
        changes,
        preserved: BTreeMap::new(),
    }
}

/// A machine that never synced this account has no baseline. Its setup and
/// the cloud setup share no history, so merging them key by key would build a
/// mix neither machine ever had. Any difference is one conflict for the user.
pub fn first_review(local: &Setup, remote: &Setup, loaders: &[String]) -> Review {
    let fresh = local
        .mods
        .iter()
        .all(|(id, value)| value.requested.is_none() || loaders.contains(id));
    if fresh {
        // A new installation only holds what the app created by itself.
        return review(local, local, remote);
    }
    let mut result = review(&Setup::default(), local, remote);
    for change in &mut result.changes {
        change.take_remote = None;
    }
    result
}

fn flatten(setup: &Setup) -> BTreeMap<Key, Entry> {
    let mut entries: BTreeMap<_, _> = setup
        .mods
        .iter()
        .map(|(id, value)| (Key::Mod(id.clone()), Entry::Mod(value.clone())))
        .collect();
    for (file, sections) in &setup.configs {
        for (section, settings) in sections {
            for (key, value) in settings {
                entries.insert(
                    Key::Setting {
                        file: file.clone(),
                        section: section.clone(),
                        key: key.clone(),
                    },
                    Entry::Setting(value.clone()),
                );
            }
        }
    }
    entries
}

fn expand(entries: BTreeMap<Key, Entry>) -> Setup {
    let mut setup = Setup::default();
    for (key, value) in entries {
        match (key, value) {
            (Key::Mod(id), Entry::Mod(value)) => {
                setup.mods.insert(id, value);
            }
            (Key::Setting { file, section, key }, Entry::Setting(value)) => {
                setup
                    .configs
                    .entry(file)
                    .or_default()
                    .entry(section)
                    .or_default()
                    .insert(key, value);
            }
            _ => unreachable!("setup keys and values are made together"),
        }
    }
    setup
}
