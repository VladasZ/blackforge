//! The pills of one mod in a row: its version, a note when the mod is a
//! dependency, pinned or disabled, and a red one when the server lists it as
//! broken on this game version. Every page that lists mods shows the same
//! three, so they are one view.

use blackforge_core::{manifest::ModSpec, required::is_required};
use hilen::{
    refs::Weak,
    ui::{Setup, ViewData, view},
};

use crate::ui::pill::Pill;

const GAP: f32 = 6.0;

/// What a row says about a mod next to its version.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Note {
    /// Asked for by the user, on its newest version, enabled.
    #[default]
    None,
    /// Pulled in by another mod.
    Dependency,
    /// Held at the version its server needs.
    Pinned(String),
    /// On the required list, every profile has it.
    Required,
    Disabled,
}

impl Note {
    /// The note of a locked mod, from its entry in the manifest. A mod the
    /// manifest does not name is there for another mod.
    pub fn of(spec: Option<&ModSpec>) -> Self {
        match spec {
            Some(spec) if is_required(spec) => Self::Required,
            Some(spec) if !spec.enabled => Self::Disabled,
            Some(spec) if let Some(server) = spec.version.server() => {
                Self::Pinned(server.to_owned())
            }
            Some(_) => Self::None,
            None => Self::Dependency,
        }
    }

    fn icon_and_text(&self) -> Option<(&'static str, String)> {
        match self {
            Self::None => None,
            Self::Dependency => Some(("pill_dependency.svg", "dependency".to_owned())),
            Self::Pinned(server) => Some(("pill_pinned.svg", format!("pinned for {server}"))),
            Self::Required => Some(("pill_pinned.svg", "required".to_owned())),
            Self::Disabled => Some(("pill_disabled.svg", "disabled".to_owned())),
        }
    }
}

#[view]
pub struct ModPills {
    #[init]
    version: Pill,
    note: Pill,
    broken: Pill,
}

impl Setup for ModPills {
    fn setup(self: Weak<Self>) {
        self.version.place().l(0).t(0).b(0);
        self.note.place().t(0).b(0);
        self.broken.warn();
        self.broken.place().t(0).b(0);
    }
}

impl ModPills {
    /// Sets the pills and returns the width they take together. The text
    /// decides the width, so the owner places the view after every call.
    pub fn set(self: Weak<Self>, version: &str, note: &Note, broken: bool) -> f32 {
        let mut width = self.version.set("pill_version.svg", version);
        self.version.place().w(width);

        match note.icon_and_text() {
            Some((icon, text)) => {
                self.note.set_hidden(false);
                let note = self.note.set(icon, &text);
                self.note.place().l(width + GAP).w(note);
                width += GAP + note;
            }
            None => {
                self.note.set_hidden(true);
            }
        }

        self.broken.set_hidden(!broken);
        if broken {
            let broken = self.broken.set("pill_broken.svg", "broken");
            self.broken.place().l(width + GAP).w(broken);
            width += GAP + broken;
        }
        width
    }
}
