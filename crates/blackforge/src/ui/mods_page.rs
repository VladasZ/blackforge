//! The game card with the run button, and under it the mods of the active
//! profile: enable, disable, remove, update and sync.

use std::collections::HashMap;

use blackforge_core::{forge::LockChange, install::SyncReport, manifest::VersionReq};
use hilen::{
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, Question, Setup, Switch, TableData, TableView,
        TextAlignment, View, ViewData, view,
    },
};

use crate::{
    backend,
    ui::{
        colors,
        game_panel::{self, GamePanel},
        mod_icon::ModIcon,
        mod_info::ModInfo,
        style,
        sync_panel::SyncPanel,
        toast,
    },
};

const GAME_T: f32 = 24.0;
/// The mods header and the table sit this far under the top of the page.
const MODS_T: f32 = GAME_T + game_panel::HEIGHT;
const ROW_HEIGHT: f32 = 58.0;
const ICON: f32 = 36.0;
/// The name and the detail start right of the icon.
const TEXT_LEFT: f32 = 4.0 + ICON + 12.0;

#[derive(Clone, Debug)]
struct ModRow {
    id: String,
    version: String,
    note: &'static str,
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
    /// The newest version of every locked mod that has one, by id.
    newest: HashMap<String, String>,

    #[init]
    game: GamePanel,
    title: Label,
    subtitle: Label,
    check: Button,
    update: Button,
    sync: Button,
    cloud: SyncPanel,
    empty: Label,
    table: TableView,
}

impl Setup for ModsPage {
    fn setup(self: Weak<Self>) {
        self.game
            .place()
            .t(GAME_T)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(game_panel::HEIGHT);

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

        style::primary(self.sync, "install files");
        self.sync
            .place()
            .t(MODS_T + 28.0)
            .r(style::PAGE_PAD)
            .size(104, style::BUTTON_H);
        self.sync.on_tap(move || self.sync_profile());

        style::ghost(self.update, "update all");
        self.update
            .place()
            .t(MODS_T + 28.0)
            .r(style::PAGE_PAD + 112.0)
            .size(100, style::BUTTON_H);
        self.update.on_tap(move || self.update_all());

        style::ghost(self.check, "check for updates");
        self.check
            .place()
            .t(MODS_T + 28.0)
            .r(style::PAGE_PAD + 220.0)
            .size(140, style::BUTTON_H);
        self.check.on_tap(move || self.check_updates());

        self.cloud
            .place()
            .t(MODS_T + style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(84);
        self.cloud.applied.sub(move || self.reload());

        style::dim(self.empty);
        self.empty
            .set_text("no mods yet, add some on the Browse page");
        self.empty.set_alignment(TextAlignment::Center);
        self.empty.set_hidden(true);
        self.empty
            .place()
            .t(MODS_T + style::HEADER + 136.0)
            .l(0)
            .r(0)
            .h(20);

        self.table.set_data_source(self).register_cell::<ModCell>();
        style::table(self.table);
        self.table
            .place()
            .t(MODS_T + style::HEADER + 96.0)
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
                        let note = match spec {
                            Some(spec) if !spec.enabled => "disabled",
                            Some(spec) if spec.version != VersionReq::Latest => "pinned",
                            Some(_) => "",
                            None => "dependency",
                        };
                        ModRow {
                            id: package.id.to_string(),
                            version: package.version.to_string(),
                            note,
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

    fn sync_profile(self: Weak<Self>) {
        backend::change(
            "syncing the mods",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                Ok(forge.sync(&profile, &progress).await?)
            },
            move |result| self.report(result.map(|report| sync_summary(&report))),
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
                    page.newest.clear();
                }
                self.report(result.map(|change| lock_change_summary(&change)));
            },
        );
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
                            .map(|entry| (entry.id.to_string(), entry.latest.to_string()))
                            .collect();
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
        Question::ask(format!("Remove {id}?")).on_yes(move || {
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
    detail: Label,
    newer: Label,
    enabled: Switch,
    remove: Button,
    line: Container,
}

impl Setup for ModCell {
    fn setup(self: Weak<Self>) {
        self.icon.place().l(4).center_y().size(ICON, ICON);

        style::body(self.name);
        self.name.set_ellipsize(true);
        self.name.place().t(10).l(TEXT_LEFT).r(330).h(20);

        style::dim(self.detail);
        self.detail.place().t(32).l(TEXT_LEFT).r(330).h(16);

        style::dim(self.newer);
        self.newer.set_text_color(colors::ACCENT);
        self.newer.set_alignment(TextAlignment::Right);
        self.newer.place().r(182).center_y().size(150, 16);

        self.enabled.place().r(116).center_y().size(44, 24);
        self.enabled.on_change(move |on| {
            if self.page.is_ok() {
                self.page.set_mod_enabled(self.index, on);
            }
        });

        style::danger(self.remove, "remove");
        self.remove.place().r(16).center_y().size(84, 28);
        self.remove.on_tap(move || {
            if self.page.is_ok() {
                self.page.remove_mod(self.index);
            }
        });

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
        newest: Option<&String>,
    ) {
        self.index = index;
        self.page = page;

        self.icon.show(&row.id, &row.version);
        self.name.set_text(&row.id);
        self.name
            .set_text_color(if row.enabled { colors::FG } else { colors::DIM });

        let detail = if row.note.is_empty() {
            row.version.clone()
        } else {
            format!("{}, {}", row.version, row.note)
        };
        self.detail.set_text(detail);

        self.newer.set_hidden(newest.is_none());
        if let Some(newest) = newest {
            self.newer.set_text(format!("{newest} is out"));
        }

        self.enabled.set_hidden(!row.direct);
        self.enabled.set_on(row.enabled);
        self.remove.set_hidden(!row.direct);
    }
}
