//! The card on top of the Mods page: the folder the game was found in, the
//! extra arguments, the achievements switch and the run button.

use hilen::{
    refs::Weak,
    ui::{Button, Label, Setup, Switch, TextField, ViewData, view},
};

use crate::{
    backend, launcher,
    ui::{style, toast},
};

pub const HEIGHT: f32 = 180.0;

const PAD: f32 = 16.0;
const RUN_W: f32 = 160.0;
const ARGS_T: f32 = 72.0;
const KEEP_T: f32 = ARGS_T + 20.0 + style::FIELD_H + 14.0;
const KEEP_LABEL_L: f32 = PAD + 56.0;
const KEEP_LABEL_W: f32 = 220.0;

#[view]
pub struct GamePanel {
    #[init]
    folder_title: Label,
    folder: Label,
    run: Button,
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
        self.folder_title.set_text("game folder");
        self.folder_title.place().t(PAD).l(PAD).size(300, 16);

        style::body(self.folder);
        self.folder.set_ellipsize_head(true);
        self.folder
            .place()
            .t(PAD + 20.0)
            .l(PAD)
            .r(PAD + RUN_W + PAD)
            .h(22);

        style::primary(self.run, "Run game");
        self.run.set_text_size(14);
        self.run.place().t(PAD).r(PAD).size(RUN_W, 40);
        self.run.on_tap(launcher::run_game);

        style::dim(self.args_title);
        self.args_title
            .set_text("extra arguments for the game, split on spaces");
        self.args_title.place().t(ARGS_T).l(PAD).size(400, 16);

        style::field(self.args, "-console");
        self.args.set_text(launcher::game_args());
        self.args
            .place()
            .t(ARGS_T + 20.0)
            .l(PAD)
            .r(PAD)
            .h(style::FIELD_H);
        self.args.changed.val(|text| launcher::set_game_args(&text));

        self.keep.place().t(KEEP_T).l(PAD).size(44, 24);
        self.keep.on_change(save_keep_achievements);

        style::body(self.keep_label);
        self.keep_label.set_text("keep achievements with mods");
        self.keep_label
            .place()
            .t(KEEP_T)
            .l(KEEP_LABEL_L)
            .size(KEEP_LABEL_W, 24);

        style::dim(self.keep_hint);
        self.keep_hint.set_ellipsize(true);
        self.keep_hint
            .set_text("the game blocks them when mods are loaded, real cheats still block them");
        self.keep_hint
            .place()
            .t(KEEP_T + 4.0)
            .l(KEEP_LABEL_L + KEEP_LABEL_W + 8.0)
            .r(PAD)
            .h(16);

        self.locate();
        self.load_keep_achievements();
    }
}

impl GamePanel {
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
                        self.folder.set_text("not found");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn load_keep_achievements(mut self: Weak<Self>) {
        backend::load(
            "reading the settings",
            |forge, _| async move { Ok(forge.keep_achievements().await?) },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(keep) => {
                        self.keep.set_on(keep);
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }
}

/// The plugin itself is put in place at the next start of the game.
fn save_keep_achievements(keep: bool) {
    backend::load(
        "saving the settings",
        move |forge, _| async move { Ok(forge.set_keep_achievements(keep).await?) },
        |result| {
            if let Err(error) = result {
                toast::failure(&error);
            }
        },
    );
}
