//! Where the game is and how it starts: the folder, the extra arguments, the
//! achievements switch and the run button.

use std::path::PathBuf;

use hilen::{
    dispatch::{on_main, spawn},
    filesystem::Paths,
    refs::Weak,
    ui::{Button, Label, Setup, Switch, TextField, ViewData, view},
};

use crate::{
    backend, launcher,
    ui::{style, toast},
};

const PAD: f32 = 20.0;
const ACHIEVEMENTS_T: f32 = style::HEADER + 106.0 + style::FIELD_H + 24.0;

#[view]
pub struct GamePage {
    #[init]
    title: Label,
    subtitle: Label,
    folder_title: Label,
    folder: Label,
    pick: Button,
    args_title: Label,
    args: TextField,
    achievements_title: Label,
    keep: Switch,
    keep_label: Label,
    keep_hint: Label,
    run: Button,
}

impl Setup for GamePage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Game");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle.set_text("the game starts with your mods");
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        style::dim(self.folder_title);
        self.folder_title.set_text("game folder");
        self.folder_title
            .place()
            .t(style::HEADER + 8.0)
            .l(style::PAGE_PAD)
            .size(300, 16);

        style::body(self.folder);
        self.folder.set_ellipsize_head(true);
        self.folder
            .place()
            .t(style::HEADER + 30.0)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD + 170.0)
            .h(22);

        style::ghost(self.pick, "pick the folder");
        self.pick
            .place()
            .t(style::HEADER + 24.0)
            .r(style::PAGE_PAD)
            .size(150, style::BUTTON_H);
        self.pick.on_tap(move || self.pick_folder());

        style::dim(self.args_title);
        self.args_title
            .set_text("extra arguments for the game, split on spaces");
        self.args_title
            .place()
            .t(style::HEADER + 84.0)
            .l(style::PAGE_PAD)
            .size(400, 16);

        style::field(self.args, "-console");
        self.args.set_text(launcher::game_args());
        self.args
            .place()
            .t(style::HEADER + 106.0)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(style::FIELD_H);
        self.args.changed.val(|text| launcher::set_game_args(&text));

        style::dim(self.achievements_title);
        self.achievements_title.set_text("achievements");
        self.achievements_title
            .place()
            .t(ACHIEVEMENTS_T)
            .l(style::PAGE_PAD)
            .size(300, 16);

        self.keep
            .place()
            .t(ACHIEVEMENTS_T + 22.0)
            .l(style::PAGE_PAD)
            .size(44, 24);
        self.keep.on_change(save_keep_achievements);

        style::body(self.keep_label);
        self.keep_label.set_text("keep achievements with mods");
        self.keep_label
            .place()
            .t(ACHIEVEMENTS_T + 22.0)
            .l(style::PAGE_PAD + 56.0)
            .size(400, 24);

        style::dim(self.keep_hint);
        self.keep_hint
            .set_text("the game blocks them when mods are loaded, real cheats still block them");
        self.keep_hint
            .place()
            .t(ACHIEVEMENTS_T + 52.0)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(16);

        style::primary(self.run, "Run game");
        self.run.set_text_size(14);
        self.run
            .place()
            .t(ACHIEVEMENTS_T + 68.0 + PAD)
            .l(style::PAGE_PAD)
            .size(160, 40);
        self.run.on_tap(launcher::run_game);

        self.locate(None);
        self.load_keep_achievements();
    }
}

impl GamePage {
    fn pick_folder(self: Weak<Self>) {
        spawn(async move {
            let picked = Paths::pick_folder().await;
            on_main(move || {
                if picked.is_some() && self.is_ok() {
                    self.locate(picked);
                }
            });
        });
    }

    /// A folder given here wins over the Steam lookup and is remembered.
    fn locate(self: Weak<Self>, game_dir: Option<PathBuf>) {
        backend::load(
            "looking for the game",
            move |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let game = forge.game(&profile.manifest().await?).await?;
                let install = forge.locate_game(&game, game_dir).await?;
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
