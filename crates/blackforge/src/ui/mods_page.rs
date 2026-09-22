//! The mods of the active profile: enable, disable, remove and update.

use std::cell::Cell;

use blackforge_core::{forge::LockChange, install::SyncReport};
use hilen::{
    dispatch::after,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, Question, Setup, Switch, TableData, TableView,
        TextAlignment, View, ViewData, ViewTooltip, view,
    },
};

use crate::{
    backend,
    ui::{
        busy, colors, hover,
        icon_button::{self, IconButton},
        mod_icon::ModIcon,
        mod_info::ModInfo,
        mod_pills::{ModPills, Note},
        names, pill, style,
        sync_panel::{self, SyncPanel},
        time, toast,
    },
    updates::{self, Newer},
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
const CHECK_W: f32 = 140.0;
const UPDATE_W: f32 = 100.0;
const HEADER_GAP: f32 = 8.0;
/// The age of the last check in the subtitle grows while the page is open.
const AGE_SECONDS: f32 = 60.0;

thread_local! {
    static PAGE: Cell<Weak<ModsPage>> = const { Cell::new(Weak::const_default()) };
}

/// The update state changed. Nothing happens while another page is open.
pub fn updates_changed() {
    let page = PAGE.with(Cell::get);
    if page.is_ok() {
        page.show_updates();
    }
}

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
    /// The game, the target and the count of mods, the first half of the
    /// subtitle.
    counts: String,

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
        self.subtitle.set_ellipsize(true);
        self.subtitle
            .place()
            .t(MODS_T + 56.0)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(16);

        style::ghost(self.update, "Update all");
        self.update
            .place()
            .t(MODS_T + 28.0)
            .r(style::PAGE_PAD)
            .size(UPDATE_W, style::BUTTON_H);
        self.update.on_tap(move || self.update_all());
        busy::track(self.update);

        style::ghost(self.check, "Check for updates");
        self.check
            .place()
            .t(MODS_T + 28.0)
            .r(style::PAGE_PAD + UPDATE_W + HEADER_GAP)
            .size(CHECK_W, style::BUTTON_H);
        self.check.on_tap(|| updates::check(true));

        self.cloud
            .place()
            .t(MODS_T + style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(sync_panel::HEIGHT);
        self.cloud.applied.sub(move || self.reload());

        style::dim(self.empty);
        self.empty
            .set_text("No mods yet, add some on the Browse page");
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

        PAGE.with(|slot| slot.set(self));
        self.show_updates();
        self.age();
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
                let counts = format!(
                    "{} {}, {} mods",
                    names::sentence(&manifest.game),
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
                Ok((counts, rows))
            },
            move |result: anyhow::Result<(String, Vec<ModRow>)>| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok((counts, rows)) => {
                        self.counts = counts;
                        self.empty.set_hidden(!rows.is_empty());
                        self.rows = rows;
                        self.show_updates();
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    /// Reads the age of the last check again once a minute while the page
    /// lives, the shell drops the page on a switch.
    fn age(self: Weak<Self>) {
        after(AGE_SECONDS, move || {
            if self.is_ok() {
                self.show_subtitle();
                self.age();
            }
        });
    }

    /// The subtitle, the update all button and the rows follow the update
    /// state the app keeps for the session.
    fn show_updates(mut self: Weak<Self>) {
        self.show_subtitle();
        self.refresh_update_button();
        self.table.reload_data();
    }

    fn show_subtitle(self: Weak<Self>) {
        let state = updates::get();
        let check = if state.checking {
            "looking for updates".to_owned()
        } else if let Some(checked) = state.checked {
            let found = match state.movable() {
                0 => "all up to date".to_owned(),
                1 => "1 update".to_owned(),
                count => format!("{count} updates"),
            };
            self.subtitle
                .set_tooltip(format!("Checked {}", time::full(checked)));
            format!("{found}, checked {}", time::ago(checked))
        } else {
            String::new()
        };
        let text = match (self.counts.is_empty(), check.is_empty()) {
            (true, _) => names::sentence(&check),
            (false, true) => self.counts.clone(),
            (false, false) => format!("{}, {check}", self.counts),
        };
        self.subtitle.set_text(text);
    }

    fn update_all(self: Weak<Self>) {
        busy::press(self.update, "Updating...");
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
                if result.is_ok() {
                    updates::all_moved();
                }
                self.report(result.map(|change| lock_change_summary(&change)));
            },
        );
    }

    /// Moves one mod to its newest version, the others stay where they are.
    fn update_mod(self: Weak<Self>, index: usize, button: Weak<Button>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let id = row.id.clone();
        busy::press(button, "Updating...");
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
                if let Ok((id, _)) = &result {
                    updates::moved(id);
                }
                self.report(result.map(|(_, change)| lock_change_summary(&change)));
            },
        );
    }

    /// Update all names how many mods it moves once a check ran, and is off
    /// when there is nothing to move. Pinned mods do not count, it leaves them.
    /// It is off during a change too, then the busy buttons decide.
    fn refresh_update_button(self: Weak<Self>) {
        let mut update = self.update;
        let state = updates::get();
        let count = state.movable();
        if count == 0 {
            update.set_text("Update all");
        } else {
            update.set_text(format!("Update {count}"));
        }
        if !backend::busy() {
            update.set_enabled(state.checked.is_none() || count > 0);
        }
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
                    Ok(id)
                },
                move |result| {
                    if let Ok(id) = &result {
                        updates::moved(&id.to_string());
                    }
                    self.report(result.map(|id| format!("Removed {id}")));
                },
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
                    if enabled { "Enabled" } else { "Disabled" }
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
        let newest = updates::get().newest.get(&row.id).cloned();
        cell.set_row(index, weak_from_ref(self), row, newest.as_ref());
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
            .size(150, style::BUTTON_H);
        self.update.on_tap(move || {
            if self.page.is_ok() {
                self.page.update_mod(self.index, self.update);
            }
        });
        busy::track(self.update);

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
        busy::track_icon(self.remove);

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
