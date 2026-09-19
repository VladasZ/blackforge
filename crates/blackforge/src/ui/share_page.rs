//! Ways out of the app for the active profile: an r2modman code, an export
//! file, and a plain folder for a rented or Docker server.

use hilen::{
    dispatch::{on_main, spawn},
    filesystem::Paths,
    refs::Weak,
    system::Clipboard,
    ui::{Button, Label, Setup, Switch, ViewData, view},
};
use tokio::fs;

use crate::{
    backend,
    ui::{mods_page::sync_summary, style, toast},
};

const PAD: f32 = 20.0;
const GAP: f32 = 16.0;
const CODE_HEIGHT: f32 = 156.0;
const FILE_HEIGHT: f32 = 116.0;
const DEPLOY_HEIGHT: f32 = 160.0;

fn card_title(label: Weak<Label>, text: &str) {
    style::body(label);
    label.set_text(text).set_text_size(16);
    label.place().t(16).l(PAD).r(PAD).h(22);
}

fn card_text(label: Weak<Label>, text: &str) {
    style::dim(label);
    label.set_text(text);
    label.place().t(42).l(PAD).r(PAD).h(16);
}

#[view]
pub struct SharePage {
    #[init]
    title: Label,
    subtitle: Label,
    code: CodeCard,
    file: FileCard,
    deploy: DeployCard,
}

impl Setup for SharePage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Share");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle
            .set_text("give your mods to a friend or to a server");
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        let mut y = style::HEADER;
        self.code
            .place()
            .t(y)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(CODE_HEIGHT);
        y += CODE_HEIGHT + GAP;
        self.file
            .place()
            .t(y)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(FILE_HEIGHT);
        y += FILE_HEIGHT + GAP;
        self.deploy
            .place()
            .t(y)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(DEPLOY_HEIGHT);
    }
}

#[view]
struct CodeCard {
    #[init]
    title: Label,
    text: Label,
    create: Button,
    code: Label,
    copy: Button,
}

impl Setup for CodeCard {
    fn setup(self: Weak<Self>) {
        style::card(self);
        card_title(self.title, "Share as a code");
        card_text(
            self.text,
            "the code works in r2modman, Gale and blackforge, and it expires within hours",
        );

        style::primary(self.create, "create a code");
        self.create.place().b(16).l(PAD).size(130, style::BUTTON_H);
        self.create.on_tap(move || self.create_code());

        style::body(self.code);
        self.code.set_text_size(18);
        self.code.place().t(70).l(PAD).r(PAD).h(26);

        style::ghost(self.copy, "copy");
        self.copy.set_hidden(true);
        self.copy
            .place()
            .b(16)
            .l(PAD + 138.0)
            .size(76, style::BUTTON_H);
        self.copy
            .on_tap(move || match Clipboard::set_text(self.code.text()) {
                Ok(()) => toast::success("the code is on the clipboard"),
                Err(error) => toast::error(format!("cannot copy: {error}")),
            });
    }
}

impl CodeCard {
    fn create_code(self: Weak<Self>) {
        backend::load(
            "uploading the mods",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                Ok(forge.export_r2_code(&profile).await?)
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(code) => {
                        self.code.set_text(code);
                        self.copy.set_hidden(false);
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }
}

#[view]
struct FileCard {
    #[init]
    title: Label,
    text: Label,
    export: Button,
}

impl Setup for FileCard {
    fn setup(self: Weak<Self>) {
        style::card(self);
        card_title(self.title, "Export to a file");
        card_text(
            self.text,
            "writes default.r2z into the folder you pick, for a mod set too large for a code",
        );

        style::primary(self.export, "pick a folder");
        self.export.place().b(16).l(PAD).size(130, style::BUTTON_H);
        self.export.on_tap(|| {
            spawn(async {
                let picked = Paths::pick_folder().await;
                on_main(move || {
                    let Some(folder) = picked else {
                        return;
                    };
                    backend::load(
                        "exporting the mods",
                        |forge, progress| async move {
                            let profile = backend::profile(forge, &progress).await?;
                            let zip_bytes = forge.export_r2_bytes(&profile).await?;
                            let path = folder.join(format!("{}.r2z", profile.name()));
                            fs::write(&path, zip_bytes).await?;
                            Ok(path)
                        },
                        |result| match result {
                            Ok(path) => toast::success(format!("wrote {}", path.display())),
                            Err(error) => toast::failure(&error),
                        },
                    );
                });
            });
        });
    }
}

#[view]
struct DeployCard {
    #[init]
    title: Label,
    text: Label,
    overwrite: Switch,
    overwrite_label: Label,
    deploy: Button,
}

impl Setup for DeployCard {
    fn setup(self: Weak<Self>) {
        style::card(self);
        card_title(self.title, "Deploy to a folder");
        card_text(
            self.text,
            "writes the mods and their configs as a plain folder, for a rented or Docker server",
        );

        self.overwrite.place().t(72).l(PAD).size(44, 24);

        style::body(self.overwrite_label);
        self.overwrite_label
            .set_text("replace configs that already exist at the target");
        self.overwrite_label
            .place()
            .t(74)
            .l(PAD + 56.0)
            .r(PAD)
            .h(20);

        style::primary(self.deploy, "pick a folder");
        self.deploy.place().b(16).l(PAD).size(130, style::BUTTON_H);
        self.deploy.on_tap(move || {
            let overwrite_configs = self.overwrite.on();
            spawn(async move {
                let picked = Paths::pick_folder().await;
                on_main(move || {
                    let Some(folder) = picked else {
                        return;
                    };
                    backend::change(
                        "deploying the mods",
                        move |forge, progress| async move {
                            let profile = backend::profile(forge, &progress).await?;
                            // The lock decides what is deployed, so the profile
                            // itself is synced first and a broken lock shows up
                            // here and not on the server.
                            forge.sync(&profile, &progress).await?;
                            let report = forge
                                .deploy(&profile, &folder, overwrite_configs, &progress)
                                .await?;
                            Ok(format!(
                                "deployed to {}, {}, {} configs written, {} kept",
                                folder.display(),
                                sync_summary(&report.sync),
                                report.configs_written.len(),
                                report.configs_kept.len()
                            ))
                        },
                        |result: anyhow::Result<String>| match result {
                            Ok(text) => toast::success(text),
                            Err(error) => toast::failure(&error),
                        },
                    );
                });
            });
        });
    }
}
