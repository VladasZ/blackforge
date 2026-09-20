//! The config files of the mods: pick a file on the left, edit its settings
//! on the right. A value is saved when its field loses the focus, an on or
//! off value has a switch and is saved at once.

use blackforge_core::config::{Setting, find, list, package_of, read, write};
use hilen::{
    Event,
    refs::{Weak, weak_from_ref},
    ui::{
        CellRegistry, Container, Label, Setup, Switch, TableData, TableView, TextAlignment,
        TextField, View, ViewData, ViewTouch, view,
    },
};

use crate::{
    backend,
    ui::{colors, mod_icon::ModIcon, style, toast},
};

const FILES_WIDTH: f32 = 290.0;
const CARD_HEIGHT: f32 = 56.0;
const CARD_GAP: f32 = 8.0;
const FILE_HEIGHT: f32 = CARD_HEIGHT + CARD_GAP;
const FILE_ICON: f32 = 36.0;
/// The name and the file start right of the icon.
const FILE_TEXT_LEFT: f32 = 10.0 + FILE_ICON + 12.0;
const SETTING_HEIGHT: f32 = 62.0;

/// One config file and the mod it most likely belongs to.
#[derive(Clone, Debug)]
struct FileRow {
    file: String,
    /// The id and the version of the mod, for the name and the icon.
    package: Option<(String, String)>,
}

#[view]
pub struct ConfigsPage {
    #[init]
    title: Label,
    subtitle: Label,
    empty: Label,
    files: ConfigFiles,
    settings: ConfigSettings,
}

impl Setup for ConfigsPage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Configs");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle
            .set_text("a value is saved when its field loses the focus, a switch at once");
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        style::dim(self.empty);
        self.empty.set_text(
            "no config files yet, a mod writes its config on the first start of the game",
        );
        self.empty.set_alignment(TextAlignment::Center);
        self.empty.set_hidden(true);
        self.empty.place().t(style::HEADER + 40.0).l(0).r(0).h(20);

        self.files
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .b(0)
            .w(FILES_WIDTH);
        self.files.picked.val(move |file| self.settings.open(file));
        self.files.loaded.val(move |count| {
            self.empty.set_hidden(count > 0);
        });

        self.settings
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD + FILES_WIDTH + 20.0)
            .r(style::PAGE_PAD)
            .b(0);
    }
}

#[view]
struct ConfigFiles {
    picked: Event<String>,
    /// How many files the profile has.
    loaded: Event<usize>,

    rows: Vec<FileRow>,
    selected: Option<usize>,

    #[init]
    table: TableView,
}

impl Setup for ConfigFiles {
    fn setup(mut self: Weak<Self>) {
        self.table.set_data_source(self).register_cell::<FileCell>();
        style::table(self.table);
        self.table.place().back();

        backend::load(
            "reading the configs",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let lock = profile.lock().await?;
                let rows = list(&profile)
                    .await?
                    .into_iter()
                    .map(|file| {
                        let id = package_of(&file, lock.packages.iter().map(|package| &package.id));
                        let package = lock
                            .packages
                            .iter()
                            .find(|package| Some(&package.id) == id)
                            .map(|package| (package.id.to_string(), package.version.to_string()));
                        FileRow { file, package }
                    })
                    .collect();
                Ok(rows)
            },
            move |result: anyhow::Result<Vec<FileRow>>| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(rows) => {
                        self.loaded.trigger(rows.len());
                        self.rows = rows;
                        self.table.reload_data();
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }
}

impl TableData for ConfigFiles {
    fn cell_height(&self, _: usize) -> f32 {
        FILE_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<FileCell>();
        cell.card
            .set_file(&self.rows[index], self.selected == Some(index));
        cell
    }

    fn cell_selected(&mut self, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        self.selected = Some(index);
        self.picked.trigger(row.file.clone());
        self.table.reload_data();
    }
}

/// A row of the table. The card is shorter than the row, that is the gap
/// between two cards.
#[view]
struct FileCell {
    #[init]
    card: FileCard,
}

impl Setup for FileCell {
    fn setup(self: Weak<Self>) {
        self.card.place().t(0).l(0).r(0).h(CARD_HEIGHT);
    }
}

#[view]
struct FileCard {
    selected: bool,

    #[init]
    icon: ModIcon,
    name: Label,
    file: Label,
}

impl Setup for FileCard {
    fn setup(self: Weak<Self>) {
        style::card(self);

        self.icon
            .place()
            .l(10)
            .center_y()
            .size(FILE_ICON, FILE_ICON);

        style::body(self.name);
        self.name.set_ellipsize(true);
        self.name.place().t(9).l(FILE_TEXT_LEFT).r(10).h(20);

        style::dim(self.file);
        self.file.set_ellipsize(true);
        self.file.place().t(31).l(FILE_TEXT_LEFT).r(10).h(16);

        // The wash says the card can be clicked, the click opens the file.
        self.enable_hover();
        self.touch()
            .hovered
            .val(self, move |hovered| self.refresh(hovered));
    }
}

impl FileCard {
    fn set_file(mut self: Weak<Self>, row: &FileRow, selected: bool) {
        let stem = row.file.trim_end_matches(".cfg");
        if let Some((id, version)) = &row.package {
            self.icon.show(id, version);
            // `Owner-Name`, the name alone is the title of the card.
            self.name
                .set_text(id.split_once('-').map_or(stem, |(_, name)| name));
        } else {
            self.icon.clear();
            self.name.set_text(stem);
        }
        self.file.set_text(&row.file);

        self.selected = selected;
        self.refresh(self.is_hovered());
    }

    fn refresh(self: Weak<Self>, hovered: bool) {
        if self.selected {
            self.set_color(colors::NAV_ACTIVE_BG);
            self.set_border_color(colors::ACCENT);
            self.name.set_text_color(colors::ACCENT);
        } else {
            self.set_color(if hovered {
                colors::NAV_HOVER_BG
            } else {
                colors::CARD_BG
            });
            self.set_border_color(colors::BORDER);
            self.name.set_text_color(colors::FG);
        }
    }
}

#[derive(Clone, Debug)]
enum Row {
    Section(String),
    Setting(Setting),
}

#[view]
struct ConfigSettings {
    file: String,
    rows: Vec<Row>,

    #[init]
    table: TableView,
}

impl Setup for ConfigSettings {
    fn setup(self: Weak<Self>) {
        self.table
            .set_data_source(self)
            .register_cell::<SectionCell>()
            .register_cell::<SettingCell>();
        style::table(self.table);
        self.table.place().back();
    }
}

impl ConfigSettings {
    fn open(mut self: Weak<Self>, file: String) {
        self.file.clone_from(&file);
        backend::load(
            "reading the config",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let config = read(&find(&profile, &file).await?).await?;
                let mut rows = Vec::new();
                let mut section = None;
                for setting in config.settings() {
                    if section.as_ref() != Some(&setting.section) {
                        section = Some(setting.section.clone());
                        rows.push(Row::Section(setting.section.clone()));
                    }
                    rows.push(Row::Setting(setting));
                }
                Ok((file, rows))
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    // The user can pick another file while this one loads.
                    Ok((file, rows)) if file == self.file => {
                        self.rows = rows;
                        self.table.reload_data();
                    }
                    Ok(_) => {}
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn save(mut self: Weak<Self>, index: usize, value: String) {
        let Some(Row::Setting(setting)) = self.rows.get_mut(index) else {
            return;
        };
        if setting.value == value {
            return;
        }
        setting.value.clone_from(&value);

        let section = setting.section.clone();
        let key = setting.key.clone();
        let file = self.file.clone();
        backend::change(
            "saving the config",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let path = find(&profile, &file).await?;
                let mut config = read(&path).await?;
                config.set(&section, &key, &value)?;
                write(&path, &config).await?;
                Ok(format!("{section}.{key} = {value}"))
            },
            move |result: anyhow::Result<String>| match result {
                Ok(text) => toast::success(text),
                Err(error) => {
                    toast::failure(&error);
                    // The row already shows the new value, the file does not.
                    if self.is_ok() {
                        self.open(self.file.clone());
                    }
                }
            },
        );
    }
}

impl TableData for ConfigSettings {
    fn cell_height(&self, _: usize) -> f32 {
        SETTING_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        match &self.rows[index] {
            Row::Section(name) => {
                let cell = registry.cell::<SectionCell>();
                cell.name.set_text(name);
                cell
            }
            Row::Setting(setting) => {
                let cell = registry.cell::<SettingCell>();
                cell.set_setting(index, weak_from_ref(self), setting);
                cell
            }
        }
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct SectionCell {
    #[init]
    name: Label,
}

impl Setup for SectionCell {
    fn setup(self: Weak<Self>) {
        style::body(self.name);
        self.name.set_text_color(colors::ACCENT);
        self.name.place().l(4).r(4).b(8).h(20);
    }
}

#[view]
struct SettingCell {
    index: usize,
    page: Weak<ConfigSettings>,

    #[init]
    key: Label,
    detail: Label,
    value: TextField,
    toggle: Switch,
    line: Container,
}

impl Setup for SettingCell {
    fn setup(self: Weak<Self>) {
        style::body(self.key);
        self.key.set_ellipsize(true);
        self.key.place().t(10).l(4).r(310).h(20);

        style::dim(self.detail);
        self.detail.set_ellipsize(true);
        self.detail.place().t(34).l(4).r(310).h(16);

        style::field(self.value, "");
        self.value
            .place()
            .r(16)
            .center_y()
            .size(290, style::FIELD_H);
        self.value.editing_ended.val(move |text| {
            if self.page.is_ok() {
                self.page.save(self.index, text);
            }
        });

        self.toggle.place().r(16).center_y().size(44, 24);
        self.toggle.on_change(move |on| {
            if self.page.is_ok() {
                self.page.save(self.index, on.to_string());
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl SettingCell {
    fn set_setting(
        mut self: Weak<Self>,
        index: usize,
        page: Weak<ConfigSettings>,
        setting: &Setting,
    ) {
        self.index = index;
        self.page = page;
        self.key.set_text(&setting.key);

        let mut detail = setting.description.first().cloned().unwrap_or_default();
        if let Some(default) = &setting.default {
            if !detail.is_empty() {
                detail.push_str(", ");
            }
            detail.push_str(&format!("default {default}"));
        }
        self.detail.set_text(detail);

        let on = setting.as_bool();
        self.value.set_hidden(on.is_some());
        self.toggle.set_hidden(on.is_none());
        match on {
            Some(on) => {
                self.toggle.set_on(on);
            }
            None => {
                self.value.set_text(&setting.value);
            }
        }
    }
}
