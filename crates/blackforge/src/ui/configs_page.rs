//! The config files of the mods: pick a file on the left, edit its settings
//! on the right. A value is saved when its field loses the focus.

use blackforge_core::config::{Setting, find, list, read, write};
use hilen::{
    Event,
    refs::{Weak, weak_from_ref},
    ui::{
        CellRegistry, Container, Label, Setup, TableData, TableView, TextAlignment, TextField,
        View, ViewData, view,
    },
};

use crate::{
    backend,
    ui::{colors, style, toast},
};

const FILES_WIDTH: f32 = 290.0;
const FILE_HEIGHT: f32 = 36.0;
const SETTING_HEIGHT: f32 = 62.0;

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
            .set_text("a value is saved when its field loses the focus");
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

    names: Vec<String>,
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
                Ok(list(&profile).await?)
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(names) => {
                        self.loaded.trigger(names.len());
                        self.names = names;
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
        self.names.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<FileCell>();
        cell.set_file(&self.names[index], self.selected == Some(index));
        cell
    }

    fn cell_selected(&mut self, index: usize) {
        let Some(name) = self.names.get(index) else {
            return;
        };
        self.selected = Some(index);
        self.picked.trigger(name.clone());
        self.table.reload_data();
    }
}

#[view]
struct FileCell {
    #[init]
    name: Label,
}

impl Setup for FileCell {
    fn setup(self: Weak<Self>) {
        self.set_corner_radius(7);
        style::body(self.name);
        self.name.set_text_size(13);
        self.name.set_ellipsize(true);
        self.name.place().l(10).r(8).t(0).b(0);
    }
}

impl FileCell {
    fn set_file(self: Weak<Self>, name: &str, selected: bool) {
        self.name.set_text(name.trim_end_matches(".cfg"));
        if selected {
            self.set_color(colors::NAV_ACTIVE_BG);
            self.name.set_text_color(colors::ACCENT);
        } else {
            self.set_color(colors::CLEAR);
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
        self.value.set_text(&setting.value);
    }
}
