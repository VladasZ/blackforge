//! The mods of the active profile: enable, disable, remove and update.

use std::collections::HashMap;

use blackforge_core::{forge::LockChange, install::SyncReport};
use hilen::{
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, Question, Setup, Switch, TableData, TableView,
        TextAlignment, View, ViewData, ViewTooltip, view,
    },
};

use crate::{
    backend,
    ui::{
        colors, hover,
        icon_button::{self, IconButton},
        mod_icon::ModIcon,
        mod_info::ModInfo,
        mod_pills::{ModPills, Note},
        names, pill, style,
        sync_panel::{self, SyncPanel},
        toast,
    },
};

/// The mods header and the table sit this far under the top of the page.
const MODS_T: f32 = 0.0;
const ROW_HEIGHT: f32 = 84.0;
/// The table starts this far under the cloud sync line.
const LIST_GAP: f32 = 12.0;
const AUTHOR_T: f32 = 28.0;
const PILLS_T: f32 = 50.0;
const ICON: f32 = 36.0;
/// The name and the detail start right of the icon.
const TEXT_LEFT: f32 = 4.0 + ICON + 12.0;
/// The update button and the newer note end left of the switch.
const RIGHT_OF_SWITCH: f32 = 16.0 + icon_button::SIZE + 16.0 + 44.0 + 16.0;

#[derive(Clone, Debug)]
struct ModRow {
    id: String,
    version: String,
    note: Note,
    /// The server lists it as broken on this game version.
    broken: bool,
    /// The user asked for it, so it can be disabled and removed. A
    /// dependency leaves by itself when nothing needs it.
    direct: bool,
    enabled: bool,
}

/// A newer version of a locked mod.
#[derive(Clone, Debug)]
struct Newer {
    version: String,
    /// Held at its version, so update all leaves it and the row offers no
    /// update of its own.
    pinned: bool,
}

pub fn lock_change_summary(change: &LockChange) -> String {
    if change.is_empty() {
        return "the lock did not change".to_owned();
    }
    let mut text = format!(
        "{} added, {} changed, {} removed",
        change.added.len(),
        change.changed.len(),
        change.removed.len()
    );
    if !change.missing.is_empty() {
        text.push_str(&format!(
            ", {} dependencies are no longer on Thunderstore",
            change.missing.len()
        ));
    }
    text
}

pub fn sync_summary(report: &SyncReport) -> String {
    format!(
        "{} installed, {} removed, {} unchanged",
        report.installed.len(),
        report.removed.len(),
        report.unchanged
    )
}

#[view]
pub struct ModsPage {
    rows: Vec<ModRow>,
    /// The newest version of every locked mod that has one, by id.
    newest: HashMap<String, Newer>,
    /// A check for updates ran, so `newest` is complete.
    checked: bool,

    #[init]
    title: Label,
    subtitle: Label,
    check: Button,
    update: Button,
    cloud: SyncPanel,
    empty: Label,
    table: TableView,
}

impl Setup for ModsPage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Mods");
        self.title
            .place()
            .t(MODS_T + 24.0)
            .l(style::PAGE_PAD)
            .size(300, 30);

        style::dim(self.subtitle);
        self.subtitle
            .place()
            .t(MODS_T + 56.0)
            .l(style::PAGE_PAD)
            .size(500, 16);

        style::ghost(self.update, "Update all");
        self.update
            .place()
            .t(MODS_T + 28.0)
            .r(style::PAGE_PAD)
            .size(100, style::BUTTON_H);
        self.update.on_tap(move || self.update_all());

        style::ghost(self.check, "Check for updates");
        self.check
            .place()
            .t(MODS_T + 28.0)
            .r(style::PAGE_PAD + 108.0)
            .size(140, style::BUTTON_H);
        self.check.on_tap(move || self.check_updates());

        self.cloud
            .place()
            .t(MODS_T + style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(sync_panel::HEIGHT);
        self.cloud.applied.sub(move || self.reload());

        style::dim(self.empty);
        self.empty
            .set_text("no mods yet, add some on the Browse page");
        self.empty.set_alignment(TextAlignment::Center);
        self.empty.set_hidden(true);
        self.empty
            .place()
            .t(MODS_T + style::HEADER + sync_panel::HEIGHT + LIST_GAP + 40.0)
            .l(0)
            .r(0)
            .h(20);

        self.table.set_data_source(self).register_cell::<ModCell>();
        style::table(self.table);
        self.table
            .place()
            .t(MODS_T + style::HEADER + sync_panel::HEIGHT + LIST_GAP)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);

        self.reload();
    }
}

impl ModsPage {
    fn reload(mut self: Weak<Self>) {
        backend::load(
            "reading the mods",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let manifest = profile.manifest().await?;
                let lock = profile.lock().await?;
                let broken = backend::broken_known(forge, &progress).await;
                let subtitle = format!(
                    "{} {}, {} mods",
                    manifest.game,
                    manifest.target,
                    lock.packages.len()
                );
                let rows = lock
                    .packages
                    .iter()
                    .map(|package| {
                        let spec = manifest.mods.get(&package.id);
                        ModRow {
                            id: package.id.to_string(),
                            version: package.version.to_string(),
                            note: Note::of(spec),
                            broken: broken.contains(&package.id),
                            direct: spec.is_some(),
                            enabled: spec.is_none_or(|spec| spec.enabled),
                        }
                    })
                    .collect();
                Ok((subtitle, rows))
            },
            move |result: anyhow::Result<(String, Vec<ModRow>)>| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok((subtitle, rows)) => {
                        self.subtitle.set_text(subtitle);
                        self.empty.set_hidden(!rows.is_empty());
                        self.rows = rows;
                        self.table.reload_data();
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn update_all(self: Weak<Self>) {
        backend::change(
            "updating the mods",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let change = forge.update(&profile, &[], &progress).await?;
                backend::forget_index().await;
                forge.sync(&profile, &progress).await?;
                Ok(change)
            },
            move |result: anyhow::Result<LockChange>| {
                if result.is_ok() && self.is_ok() {
                    let mut page = self;
                    page.newest.retain(|_, newer| newer.pinned);
                    page.refresh_update_button();
                }
                self.report(result.map(|change| lock_change_summary(&change)));
            },
        );
    }

    /// Moves one mod to its newest version, the others stay where they are.
    fn update_mod(self: Weak<Self>, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let id = row.id.clone();
        backend::change(
            "updating the mod",
            move |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let change = forge
                    .update(&profile, std::slice::from_ref(&id), &progress)
                    .await?;
                forge.sync(&profile, &progress).await?;
                Ok((id, change))
            },
            move |result: anyhow::Result<(String, LockChange)>| {
                if let Ok((id, _)) = &result
                    && self.is_ok()
                {
                    let mut page = self;
                    page.newest.remove(id);
                    page.refresh_update_button();
                }
                self.report(result.map(|(_, change)| lock_change_summary(&change)));
            },
        );
    }

    /// Update all names how many mods it moves once a check ran, and is off
    /// when there is nothing to move. Pinned mods do not count, it leaves them.
    fn refresh_update_button(self: Weak<Self>) {
        let mut update = self.update;
        if !self.checked {
            update.set_text("Update all");
            update.set_enabled(true);
            return;
        }
        let count = self.newest.values().filter(|newer| !newer.pinned).count();
        if count == 0 {
            update.set_text("Update all");
        } else {
            update.set_text(format!("Update {count}"));
        }
        update.set_enabled(count > 0);
    }

    fn check_updates(mut self: Weak<Self>) {
        backend::load(
            "looking for newer versions",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let outdated = forge.outdated(&profile, &progress).await?;
                backend::forget_index().await;
                Ok(outdated)
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(outdated) => {
                        if outdated.is_empty() {
                            toast::success("every mod is on its newest version");
                        } else {
                            toast::info(format!("{} mods have a newer version", outdated.len()));
                        }
                        self.newest = outdated
                            .into_iter()
                            .map(|entry| {
                                (
                                    entry.id.to_string(),
                                    Newer {
                                        version: entry.latest.to_string(),
                                        pinned: entry.pinned,
                                    },
                                )
                            })
                            .collect();
                        self.checked = true;
                        self.refresh_update_button();
                        self.table.reload_data();
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn remove_mod(self: Weak<Self>, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let id = row.id.clone();
        Question::ask(format!("Remove {}?", names::title(&id))).on_yes(move || {
            backend::change(
                "removing the mod",
                |forge, progress| async move {
                    let profile = backend::profile(forge, &progress).await?;
                    let (id, _) = forge.remove(&profile, &id).await?;
                    forge.sync(&profile, &progress).await?;
                    Ok(format!("removed {id}"))
                },
                move |result| self.report(result),
            );
        });
    }

    fn set_mod_enabled(self: Weak<Self>, index: usize, enabled: bool) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let id = row.id.clone();
        backend::change(
            if enabled {
                "enabling the mod"
            } else {
                "disabling the mod"
            },
            move |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let id = forge.set_enabled(&profile, &id, enabled).await?;
                forge.sync(&profile, &progress).await?;
                Ok(format!(
                    "{} {id}",
                    if enabled { "enabled" } else { "disabled" }
                ))
            },
            move |result| self.report(result),
        );
    }

    /// Shows how a change went and reads the mods again. After a failure too,
    /// a switch the user flipped has to go back to what is on disk.
    fn report(self: Weak<Self>, result: anyhow::Result<String>) {
        match result {
            Ok(text) => toast::success(text),
            Err(error) => toast::failure(&error),
        }
        if self.is_ok() {
            self.reload();
        }
    }
}

impl TableData for ModsPage {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<ModCell>();
        let row = &self.rows[index];
        cell.set_row(index, weak_from_ref(self), row, self.newest.get(&row.id));
        cell
    }

    fn cell_selected(&mut self, index: usize) {
        let page = weak_from_ref(self);
        if let Some(row) = self.rows.get(index) {
            ModInfo::open(&row.id, move |changed| {
                if changed && page.is_ok() {
                    page.reload();
                }
            });
        }
    }
}

#[view]
struct ModCell {
    index: usize,
    page: Weak<ModsPage>,

    #[init]
    icon: ModIcon,
    name: Label,
    author: Label,
    pills: ModPills,
    newer: Label,
    update: Button,
    enabled: Switch,
    remove: IconButton,
    line: Container,
}

impl Setup for ModCell {
    fn setup(self: Weak<Self>) {
        self.icon.place().l(4).center_y().size(ICON, ICON);

        style::body(self.name);
        self.name.set_ellipsize(true);
        self.name.place().t(8).l(TEXT_LEFT).r(330).h(20);

        style::dim(self.author);
        self.author.set_ellipsize(true);
        self.author.place().t(AUTHOR_T).l(TEXT_LEFT).r(330).h(16);

        // A pinned mod only says a newer version exists, updating it would
        // break the match with its server.
        style::dim(self.newer);
        self.newer.set_text_color(colors::ACCENT);
        self.newer.set_alignment(TextAlignment::Right);
        self.newer
            .place()
            .r(RIGHT_OF_SWITCH)
            .center_y()
            .size(150, 16);

        style::ghost(self.update, "");
        self.update
            .place()
            .r(RIGHT_OF_SWITCH)
            .center_y()
            .size(130, style::BUTTON_H);
        self.update.on_tap(move || {
            if self.page.is_ok() {
                self.page.update_mod(self.index);
            }
        });

        self.enabled
            .place()
            .r(16.0 + icon_button::SIZE + 16.0)
            .center_y()
            .size(44, 24);
        self.enabled.on_change(move |on| {
            if self.page.is_ok() {
                self.page.set_mod_enabled(self.index, on);
            }
        });

        self.remove.set_icon("trash.svg");
        self.remove.set_tooltip("Remove");
        self.remove
            .place()
            .r(16)
            .center_y()
            .size(icon_button::SIZE, icon_button::SIZE);
        self.remove.tapped.sub(move || {
            if self.page.is_ok() {
                self.page.remove_mod(self.index);
            }
        });

        // The wash says the row can be clicked, the click opens the details.
        hover::row(self);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl ModCell {
    fn set_row(
        mut self: Weak<Self>,
        index: usize,
        page: Weak<ModsPage>,
        row: &ModRow,
        newest: Option<&Newer>,
    ) {
        self.index = index;
        self.page = page;

        self.icon.show(&row.id, &row.version);
        self.name.set_text(names::title(&row.id));
        self.name
            .set_text_color(if row.enabled { colors::FG } else { colors::DIM });
        self.author.set_text(names::author(&row.id));

        // The pills sit under the name, as wide as their text.
        let pills = self.pills.set(&row.version, &row.note, row.broken);
        self.pills
            .place()
            .clear()
            .l(TEXT_LEFT)
            .t(PILLS_T)
            .size(pills, pill::HEIGHT);

        let pinned = newest.filter(|newer| newer.pinned);
        let movable = newest.filter(|newer| !newer.pinned);
        self.newer.set_hidden(pinned.is_none());
        if let Some(newer) = pinned {
            self.newer.set_text(format!("{} is out", newer.version));
        }
        self.update.set_hidden(movable.is_none());
        if let Some(newer) = movable {
            self.update.set_text(format!("Update to {}", newer.version));
        }

        self.enabled.set_hidden(!row.direct);
        self.enabled.set_on(row.enabled);
        self.remove.set_hidden(!row.direct);
    }
}
