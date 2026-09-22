//! The pills of one mod in a row: its version, and a note when the mod is
//! a dependency, pinned or disabled. Every page that lists mods shows the
//! same two, so they are one view.

use blackforge_core::manifest::{ModSpec, VersionReq};
use hilen::{
    refs::Weak,
    ui::{Setup, ViewData, view},
};

use crate::ui::pill::Pill;

const GAP: f32 = 6.0;

/// What a row says about a mod next to its version.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Note {
    /// Asked for by the user, on its newest version, enabled.
    #[default]
    None,
    /// Pulled in by another mod.
    Dependency,
    /// Held at one version.
    Pinned,
    Disabled,
}

impl Note {
    /// The note of a locked mod, from its entry in the manifest. A mod the
    /// manifest does not name is there for another mod.
    pub fn of(spec: Option<&ModSpec>) -> Self {
        match spec {
            Some(spec) if !spec.enabled => Self::Disabled,
            Some(spec) if spec.version != VersionReq::Latest => Self::Pinned,
            Some(_) => Self::None,
            None => Self::Dependency,
        }
    }

    fn icon_and_text(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::None => None,
            Self::Dependency => Some(("pill_dependency.svg", "dependency")),
            Self::Pinned => Some(("pill_pinned.svg", "pinned")),
            Self::Disabled => Some(("pill_disabled.svg", "disabled")),
        }
    }
}

#[view]
pub struct ModPills {
    #[init]
    version: Pill,
    note: Pill,
}

impl Setup for ModPills {
    fn setup(self: Weak<Self>) {
        self.version.place().l(0).t(0).b(0);
        self.note.place().t(0).b(0);
    }
}

impl ModPills {
    /// Sets the pills and returns the width they take together. The text
    /// decides the width, so the owner places the view after every call.
    pub fn set(self: Weak<Self>, version: &str, note: Note) -> f32 {
        let version = self.version.set("pill_version.svg", version);
        self.version.place().w(version);

        let Some((icon, text)) = note.icon_and_text() else {
            self.note.set_hidden(true);
            return version;
        };
        self.note.set_hidden(false);
        let note = self.note.set(icon, text);
        self.note.place().l(version + GAP).w(note);
        version + GAP + note
    }
}

#[cfg(test)]
mod tests {
    use blackforge_core::manifest::{ModSpec, VersionReq};

    use super::Note;

    #[test]
    fn note_of_a_manifest_entry() {
        assert_eq!(Note::of(None), Note::Dependency);
        assert_eq!(
            Note::of(Some(&ModSpec::new(VersionReq::Latest))),
            Note::None
        );
        let pinned = ModSpec::new("1.2.3".parse().unwrap());
        assert_eq!(Note::of(Some(&pinned)), Note::Pinned);
        let mut disabled = ModSpec::new(VersionReq::Latest);
        disabled.enabled = false;
        assert_eq!(Note::of(Some(&disabled)), Note::Disabled);
    }
}
