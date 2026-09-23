//! The game card of the Settings page: the folder the game was found in, the
//! extra arguments and the achievements switch.

use blackforge_core::profile::LaunchSettings;
use hilen::{
    refs::Weak,
    ui::{Label, Setup, Switch, TextField, VerticalAlignment, ViewData, view},
};

use crate::{
    backend, cloud,
    ui::{style, toast},
};

const PAD: f32 = 16.0;
const ARGS_T: f32 = 72.0;
const KEEP_T: f32 = ARGS_T + 20.0 + style::FIELD_H + 14.0;
const KEEP_LABEL_L: f32 = PAD + 56.0;
const KEEP_LABEL_W: f32 = 220.0;
/// The hint starts under the switch label.
const HINT_T: f32 = KEEP_T + 26.0;

#[view]
pub struct GamePanel {
    #[init]
    folder_title: Label,
    folder: Label,
    args_title: Label,
    args: TextField,
    keep: Switch,
    keep_label: Label,
    keep_hint: Label,
}

impl Setup for GamePanel {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::dim(self.folder_title);
        self.folder_title.set_text("Game folder");
        self.folder_title.place().t(PAD).l(PAD).size(300, 16);

        style::body(self.folder);
        self.folder.set_ellipsize_head(true);
        self.folder.place().t(PAD + 20.0).l(PAD).r(PAD).h(22);

        style::dim(self.args_title);
        self.args_title
            .set_text("Extra arguments for the game, split on spaces");
        self.args_title.place().t(ARGS_T).l(PAD).size(400, 16);

        style::field(self.args, "-console");
        self.args
            .place()
            .t(ARGS_T + 20.0)
            .l(PAD)
            .r(PAD)
            .h(style::FIELD_H);
        // Saved when the field loses the focus, every save is a cloud sync.
        self.args.editing_ended.val(|text| {
            save(move |settings| settings.game_args = text);
        });

        self.keep.place().t(KEEP_T).l(PAD).size(44, 24);
        self.keep
            .on_change(|keep| save(move |settings| settings.keep_achievements = keep));

        style::body(self.keep_label);
        self.keep_label.set_text("Keep achievements with mods");
        self.keep_label
            .place()
            .t(KEEP_T)
            .l(KEEP_LABEL_L)
            .size(KEEP_LABEL_W, 24);

        style::dim(self.keep_hint);
        self.keep_hint.set_multiline(true);
        self.keep_hint
            .set_vertical_alignment(VerticalAlignment::Top);
        self.keep_hint
            .set_text("The game blocks them when mods are loaded, real cheats still block them");

        self.locate();
        self.load_settings();
    }
}

impl GamePanel {
    /// Wraps the hint at `width` and returns the height the card needs.
    pub fn fit(self: Weak<Self>, width: f32) -> f32 {
        let hint = self
            .keep_hint
            .size_for_width(width - KEEP_LABEL_L - PAD)
            .height;
        self.keep_hint
            .place()
            .clear()
            .t(HINT_T)
            .l(KEEP_LABEL_L)
            .r(PAD)
            .h(hint);
        HINT_T + hint + PAD
    }

    fn locate(self: Weak<Self>) {
        backend::load(
            "looking for the game",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let game = forge.game(&profile.manifest().await?).await?;
                let install = forge.locate_game(&game, None).await?;
                Ok(install.dir.display().to_string())
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(folder) => {
                        self.folder.set_text(folder);
                    }
                    Err(error) => {
                        self.folder.set_text("Not found");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn load_settings(mut self: Weak<Self>) {
        backend::load(
            "reading the settings",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                Ok(forge.launch_settings(&profile).await?)
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(settings) => {
                        self.args.set_text(settings.game_args);
                        self.keep.set_on(settings.keep_achievements);
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }
}

/// The settings live in the profile, so cloud sync carries them to the other
/// machines. The achievements plugin itself is put in place at the next start.
fn save(edit: impl FnOnce(&mut LaunchSettings) + Send + 'static) {
    backend::load(
        "saving the settings",
        move |forge, progress| async move {
            let profile = backend::profile(forge, &progress).await?;
            Ok(forge.edit_launch_settings(&profile, edit).await?)
        },
        |result| match result {
            Ok(()) => cloud::schedule(),
            Err(error) => toast::failure(&error),
        },
    );
}
