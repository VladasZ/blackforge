//! The form that registers a game server or edits one of mine: a name and a
//! switch per mod of my profile. A ticked mod is required at the version my
//! lock has, so editing after an update moves the server to the new version.

use std::collections::HashMap;

use blackforge_api::servers::{SaveServer, Server, ServerMod, normalize_name, validate};
use hilen::{
    OnceEvent,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, ModalView, Setup, Size, Switch, TableData,
        TableView, TextAlignment, TextField, UIColor, View, ViewData, view,
    },
};

use crate::{
    backend, social,
    ui::{colors, style, toast},
};

const PAD: f32 = 24.0;
const ROW_HEIGHT: f32 = 44.0;

#[derive(Clone, Debug)]
struct PickRow {
    id: String,
    version: String,
    /// In my lock only because another mod needs it.
    dependency: bool,
    /// The server lists it, my profile does not have it.
    foreign: bool,
    picked: bool,
}

#[view]
pub struct ServerModal {
    /// True when a server was saved.
    event: OnceEvent<bool>,
    /// The id of the server being edited, `None` for a new one.
    editing: Option<String>,
    game: String,
    rows: Vec<PickRow>,

    #[init]
    title: Label,
    hint: Label,
    name: TextField,
    note: Label,
    table: TableView,
    cancel: Button,
    save: Button,
}

impl ModalView<Option<Server>, bool> for ServerModal {
    fn modal_event(&self) -> &OnceEvent<bool> {
        &self.event
    }

    fn modal_size() -> Size {
        (640, 600).into()
    }

    fn modal_scrim_color() -> UIColor {
        colors::SCRIM.into()
    }

    fn setup_input(mut self: Weak<Self>, server: Option<Server>) {
        if let Some(server) = &server {
            self.title.set_text(format!("Edit {}", server.name));
            // The name stays, pins of players name the server. The title shows it.
            self.name.set_text(&server.name);
            self.name.set_hidden(true);
            self.editing = Some(server.id.clone());
        }
        self.load(server);
    }
}

impl Setup for ServerModal {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::title(self.title, "Register a server");
        self.title.set_text_size(18);
        self.title.place().t(PAD).l(PAD).r(PAD).h(24);

        style::dim(self.hint);
        self.hint.set_multiline(true);
        self.hint.set_text(
            "Switch on the mods a player must have to join. The versions are the ones in your profile. The address and the password stay out of blackforge, players join through the game.",
        );
        self.hint.place().t(PAD + 30.0).l(PAD).r(PAD).h(34);

        style::field(self.name, "Server name");
        self.name
            .place()
            .t(PAD + 76.0)
            .l(PAD)
            .r(PAD)
            .h(style::FIELD_H);

        style::dim(self.note);
        self.note.set_alignment(TextAlignment::Center);
        self.note.set_text("Reading your mods...");
        self.note.place().t(PAD + 180.0).l(PAD).r(PAD).h(20);

        self.table.set_data_source(self).register_cell::<PickCell>();
        self.table
            .place()
            .t(PAD + 124.0)
            .l(PAD)
            .r(PAD)
            .b(PAD + 48.0);

        style::ghost(self.cancel, "Cancel");
        self.cancel
            .place()
            .r(PAD + 100.0)
            .b(PAD)
            .size(90, style::BUTTON_H);
        self.cancel.on_tap(move || self.hide_modal(false));

        style::primary(self.save, "Save");
        self.save.place().r(PAD).b(PAD).size(90, style::BUTTON_H);
        self.save.on_tap(move || self.save());
    }
}

impl ServerModal {
    fn load(self: Weak<Self>, server: Option<Server>) {
        backend::load(
            "reading the profile",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let manifest = profile.manifest().await?;
                let lock = profile.lock().await?;
                let required: HashMap<String, String> = server
                    .as_ref()
                    .map(|server| {
                        server
                            .mods
                            .iter()
                            .map(|server_mod| (server_mod.id.clone(), server_mod.version.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut rows: Vec<PickRow> = lock
                    .packages
                    .iter()
                    .map(|package| {
                        let id = package.id.to_string();
                        PickRow {
                            picked: required.contains_key(&id),
                            dependency: !manifest.mods.contains_key(&package.id),
                            foreign: false,
                            version: package.version.to_string(),
                            id,
                        }
                    })
                    .collect();
                // A required mod that left my profile stays on the server
                // until I switch it off, at the version the server has.
                let mine: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
                for (id, version) in required {
                    if !mine.contains(&id) {
                        rows.push(PickRow {
                            id,
                            version,
                            dependency: false,
                            foreign: true,
                            picked: true,
                        });
                    }
                }
                rows.sort_by_key(|row| row.id.to_lowercase());
                Ok((manifest.game, rows))
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok((game, rows)) => self.set_rows(game, rows),
                    Err(error) => {
                        self.note.set_text("Your mods could not be read.");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn set_rows(mut self: Weak<Self>, game: String, rows: Vec<PickRow>) {
        self.game = game;
        self.note.set_hidden(!rows.is_empty());
        self.note.set_text("Your profile has no mods yet.");
        self.rows = rows;
        self.table.reload_data();
    }

    fn toggle(mut self: Weak<Self>, index: usize, on: bool) {
        if let Some(row) = self.rows.get_mut(index) {
            row.picked = on;
        }
    }

    fn save(self: Weak<Self>) {
        let name = match normalize_name(self.name.text()) {
            Ok(name) => name,
            Err(error) => return toast::error(error.to_string()),
        };
        let save = SaveServer {
            name,
            game: self.game.clone(),
            mods: self
                .rows
                .iter()
                .filter(|row| row.picked)
                .map(|row| ServerMod {
                    id: row.id.clone(),
                    version: row.version.clone(),
                })
                .collect(),
        };
        if let Err(error) = validate(&save) {
            return toast::error(error.to_string());
        }
        let editing = self.editing.clone();

        backend::load(
            "saving the server",
            |_, _| async move {
                let client = social::client()?;
                let saved = match editing {
                    Some(id) => client.update_server(&id, &save).await?,
                    None => client.create_server(&save).await?,
                };
                Ok(saved)
            },
            move |result| match result {
                Ok(saved) => {
                    toast::success(format!(
                        "{} is registered with {} mods",
                        saved.name,
                        saved.mods.len()
                    ));
                    if self.is_ok() {
                        self.hide_modal(true);
                    }
                }
                Err(error) => toast::failure(&error),
            },
        );
    }
}

impl TableData for ServerModal {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<PickCell>();
        cell.set_row(index, weak_from_ref(self), &self.rows[index]);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct PickCell {
    index: usize,
    owner: Weak<ServerModal>,

    #[init]
    required: Switch,
    name: Label,
    detail: Label,
    line: Container,
}

impl Setup for PickCell {
    fn setup(self: Weak<Self>) {
        self.required.place().l(4).t(10).size(44, 24);
        self.required.on_change(move |on| {
            if self.owner.is_ok() {
                self.owner.toggle(self.index, on);
            }
        });

        style::body(self.name);
        self.name.set_ellipsize(true);
        self.name.place().t(6).l(60).r(16).h(18);

        style::dim(self.detail);
        self.detail.set_ellipsize(true);
        self.detail.place().t(24).l(60).r(16).h(14);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl PickCell {
    fn set_row(mut self: Weak<Self>, index: usize, owner: Weak<ServerModal>, row: &PickRow) {
        self.index = index;
        self.owner = owner;
        self.required.set_on(row.picked);
        self.name.set_text(&row.id);
        self.detail.set_text(match (row.foreign, row.dependency) {
            (true, _) => format!("{}, not in your profile", row.version),
            (false, true) => format!("{}, a dependency", row.version),
            (false, false) => row.version.clone(),
        });
    }
}
