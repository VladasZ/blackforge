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
pub enum Entry {
    Mod(Mod),
    Setting(String),
}

impl Entry {
    pub fn label(&self) -> String {
        match self {
            Self::Mod(value) => format!(
                "{} · {} · {}",
                value.version,
                if value.enabled { "on" } else { "off" },
                match value.requested.as_deref() {
                    None => "dependency",
                    Some("*") => "auto",
                    Some(_) => "pinned",
                }
            ),
            Self::Setting(value) => value.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Change {
    pub key: Key,
    pub local: Option<Entry>,
    pub remote: Option<Entry>,
    /// None requires an explicit decision from the user.
    pub take_remote: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Review {
    pub local: Setup,
    pub changes: Vec<Change>,
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
    pub fn conflicts(&self) -> usize {
        self.changes
            .iter()
            .filter(|change| change.take_remote.is_none())
            .count()
    }

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
}

/// Keep separate local and cloud baselines: saving local changes must not
/// acknowledge remote changes which this machine has not installed yet.
pub fn review(local_base: &Setup, cloud_base: &Setup, local: &Setup, remote: &Setup) -> Review {
    let lb = flatten(local_base);
    let cb = flatten(cloud_base);
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
            let local_changed = mine != lb.get(&key);
            let remote_changed = theirs != cb.get(&key) || lb.get(&key) != cb.get(&key);
            let take_remote = match (local_changed, remote_changed) {
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
