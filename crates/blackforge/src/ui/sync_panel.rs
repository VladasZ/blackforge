use blackforge_core::{
    cloud::{Entry, Key},
    social::picker::{Pick, PickerRow},
};
use hilen::{
    Event,
    login::GoogleLoginButton,
    refs::Weak,
    ui::{Button, Label, ModalView, Setup, ViewData, view},
};

use crate::{
    backend,
    cloud::{self, Preview},
    social,
    ui::{
        config_picker::{ConfigPicker, PickerFile, PickerInput},
        style, toast,
    },
};

#[view]
pub struct SyncPanel {
    pub applied: Event,
    preview: Option<Preview>,
    busy: bool,

    #[init]
    title: Label,
    status: Label,
    login: GoogleLoginButton,
    check: Button,
    review: Button,
}

impl Setup for SyncPanel {
    fn setup(self: Weak<Self>) {
        style::body(self.title);
        self.title.set_text("Cloud sync");
        self.title.place().t(0).l(0).size(100, 32);
        style::dim(self.status);
        self.status.set_multiline(true);
        self.status.place().t(0).l(108).r(0).h(32);
        self.login.place().t(38).l(0).size(220, 36);
        self.login.logged_in.val(move |_| self.load());
        self.login.failed.val(toast::error);
        style::ghost(self.check, "Check for changes");
        self.check.place().t(38).l(0).size(170, style::BUTTON_H);
        self.check.on_tap(move || self.load());
        style::primary(self.review, "Review changes");
        self.review.place().t(38).l(186).size(170, style::BUTTON_H);
        self.review.on_tap(move || self.review_changes());
        self.load();
    }
}

impl SyncPanel {
    pub fn load(mut self: Weak<Self>) {
        if self.busy {
            return;
        }
        let signed_in = social::signed_in();
        self.login.set_hidden(signed_in);
        self.check.set_hidden(!signed_in);
        self.review.set_hidden(true);
        self.preview = None;
        if !signed_in {
            self.status
                .set_text("Sign in to save your mods and settings across machines.");
            return;
        }
        self.busy = true;
        self.status.set_text("Checking your saved setup...");
        backend::load(
            "checking cloud setup",
            |_, _| cloud::check(),
            move |result| {
                if !self.is_ok() {
                    return;
                }
                self.busy = false;
                match result {
                    Ok(preview) => {
                        self.status.set_text(preview.message());
                        self.review.set_hidden(preview.review.changes.is_empty());
                        self.preview = Some(preview);
                    }
                    Err(error) => {
                        self.status
                            .set_text(format!("Not synced: {error:#}. Check again to retry."));
                    }
                }
            },
        );
    }

    fn review_changes(self: Weak<Self>) {
        let Some(mut preview) = self.preview.clone() else {
            return;
        };
        let files = preview
            .review
            .changes
            .iter()
            .map(|change| {
                let (file, section, key) = match &change.key {
                    Key::Mod(id) => (
                        "Mods".to_owned(),
                        "version and state".to_owned(),
                        id.clone(),
                    ),
                    Key::Setting { file, section, key } => {
                        (file.clone(), section.clone(), key.clone())
                    }
                };
                PickerFile {
                    file,
                    rows: vec![PickerRow {
                        section,
                        key: if change.take_remote.is_none() {
                            format!("Conflict: {key}")
                        } else {
                            key
                        },
                        mine: change
                            .local
                            .as_ref()
                            .map_or_else(|| "removed".to_owned(), Entry::label),
                        friend: change
                            .remote
                            .as_ref()
                            .map_or_else(|| "removed".to_owned(), Entry::label),
                        friend_has_default: false,
                        pick: match change.take_remote {
                            Some(true) => Pick::Friend,
                            Some(false) => Pick::Mine,
                            None => Pick::Pending,
                        },
                    }],
                }
            })
            .collect();
        ConfigPicker::show_modally_with_input(
            PickerInput {
                sync: true,
                friend: "cloud".to_owned(),
                mod_name: String::new(),
                files,
            },
            move |picked| {
                let Some(files) = picked else {
                    return;
                };
                for (change, row) in preview
                    .review
                    .changes
                    .iter_mut()
                    .zip(files.iter().flat_map(|file| &file.rows))
                {
                    change.take_remote = match row.pick {
                        Pick::Mine => Some(false),
                        Pick::Friend => Some(true),
                        Pick::Pending => None,
                    };
                }
                backend::change(
                    "applying cloud setup",
                    move |_, progress| async move { cloud::apply(preview, &progress).await },
                    move |result| {
                        match result {
                            Ok(()) => {
                                toast::success("Your saved setup is installed.");
                                if self.is_ok() {
                                    self.applied.trigger(());
                                }
                            }
                            Err(error) => toast::failure(&error),
                        }
                        if self.is_ok() {
                            self.load();
                        }
                    },
                );
            },
        );
    }
}
