//! The config files of the mods: pick a file on the left, edit its settings
//! on the right, see `config_settings`.

use blackforge_core::config::{list, package_of};
use hilen::{
    Event,
    refs::{Weak, weak_from_ref},
    ui::{
        CellRegistry, Label, Setup, TableData, TableView, TextAlignment, VerticalAlignment, View,
        ViewData, ViewTouch, view,
    },
};

use crate::{
    backend,
    ui::{
        colors,
        config_settings::ConfigSettings,
        hover,
        mod_icon::ModIcon,
        mod_pills::{ModPills, Note},
        names, pill, style, toast,
    },
};

const FILES_WIDTH: f32 = 290.0;
/// The scroll bar draws over the right edge of the table, a card ends
/// before it.
const BAR_CLEAR: f32 = 12.0;
const CARD_GAP: f32 = 8.0;
const FILE_ICON: f32 = 36.0;
/// The name, the file and the rest start right of the icon.
const FILE_TEXT_LEFT: f32 = 10.0 + FILE_ICON + 12.0;
const DESCRIPTION_T: f32 = 49.0;
/// Two lines, the card is narrow. The details of the mod have the rest.
const DESCRIPTION_H: f32 = 34.0;
/// The card is as wide as the column less the bar, so the text width is
/// known before the card is laid out.
const DESCRIPTION_W: f32 = FILES_WIDTH - BAR_CLEAR - FILE_TEXT_LEFT - 10.0;
const PILLS_T: f32 = DESCRIPTION_T + DESCRIPTION_H + 4.0;
const CARD_HEIGHT: f32 = PILLS_T + pill::HEIGHT + 10.0;
const FILE_HEIGHT: f32 = CARD_HEIGHT + CARD_GAP;

/// One config file and the mod it most likely belongs to.
#[derive(Clone, Debug)]
struct FileRow {
    file: String,
    package: Option<FilePackage>,
}

#[derive(Clone, Debug)]
struct FilePackage {
    id: String,
    version: String,
    note: Note,
    /// The server lists it as broken on this game version.
    broken: bool,
    /// Empty when the package list does not know the mod.
    description: String,
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
        self.subtitle.set_text(
            "A value is saved when its field loses the focus, a switch or a list at once",
        );
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        style::dim(self.empty);
        self.empty.set_text(
            "No config files yet, a mod writes its config on the first start of the game",
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
        self.table.set_cell_margins(0, BAR_CLEAR);
        self.table.place().back();

        backend::load(
            "reading the configs",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let manifest = profile.manifest().await?;
                let lock = profile.lock().await?;
                let files = list(&profile).await?;
                let ids = lock.packages.iter().map(|package| &package.id);
                let descriptions = backend::descriptions(forge, &progress, ids).await;
                let broken = backend::broken_known(forge, &progress).await;
                let rows = files
                    .into_iter()
                    .map(|file| {
                        let id = package_of(&file, lock.packages.iter().map(|package| &package.id));
                        let package = lock
                            .packages
                            .iter()
                            .find(|package| Some(&package.id) == id)
                            .map(|package| FilePackage {
                                id: package.id.to_string(),
                                version: package.version.to_string(),
                                note: Note::of(manifest.mods.get(&package.id)),
                                broken: broken.contains(&package.id),
                                description: descriptions
                                    .get(&package.id.to_string())
                                    .cloned()
                                    .unwrap_or_default(),
                            });
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
                        // The page opens on the first file, an empty right
                        // side says nothing.
                        self.pick(0);
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }
}

impl ConfigFiles {
    fn pick(mut self: Weak<Self>, index: usize) {
        if let Some(file) = self.rows.get(index).map(|row| row.file.clone()) {
            self.selected = Some(index);
            self.picked.trigger(file);
        }
        self.table.reload_data();
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
        weak_from_ref(self).pick(index);
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
    description: Label,
    pills: ModPills,
}

impl Setup for FileCard {
    fn setup(self: Weak<Self>) {
        style::card(self);

        self.icon.place().l(10).t(10).size(FILE_ICON, FILE_ICON);

        style::body(self.name);
        self.name.set_ellipsize(true);
        self.name.place().t(9).l(FILE_TEXT_LEFT).r(10).h(20);

        style::dim(self.file);
        self.file.set_ellipsize(true);
        self.file.place().t(31).l(FILE_TEXT_LEFT).r(10).h(16);

        style::dim(self.description);
        self.description.set_multiline(true);
        self.description
            .set_vertical_alignment(VerticalAlignment::Top);
        self.description
            .place()
            .t(DESCRIPTION_T)
            .l(FILE_TEXT_LEFT)
            .r(10)
            .h(DESCRIPTION_H);

        // The wash says the card can be clicked, the click opens the file.
        hover::clickable(self);
        self.enable_hover();
        self.touch()
            .hovered
            .val(self, move |hovered| self.refresh(hovered));
    }
}

impl FileCard {
    fn set_file(mut self: Weak<Self>, row: &FileRow, selected: bool) {
        let stem = row.file.trim_end_matches(".cfg");
        self.pills.set_hidden(row.package.is_none());
        if let Some(package) = &row.package {
            self.icon.show(&package.id, &package.version);
            self.name.set_text(names::title(&package.id));
            fit_description(self.description, &package.description);
            let pills = self
                .pills
                .set(&package.version, &package.note, package.broken);
            self.pills
                .place()
                .clear()
                .l(FILE_TEXT_LEFT)
                .t(PILLS_T)
                .size(pills, pill::HEIGHT);
        } else {
            self.icon.clear();
            self.name.set_text(stem);
            self.description.set_text("");
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

/// The description cut to the lines the card has room for. A multiline
/// label clips a line that does not fit, and half a line of text reads as
/// a mistake, so whole words go until the rest fits.
fn fit_description(label: Weak<Label>, text: &str) {
    label.set_text(text);
    let mut words: Vec<&str> = text.split(' ').collect();
    while label.size_for_width(DESCRIPTION_W).height > DESCRIPTION_H && words.pop().is_some() {
        label.set_text(format!("{}...", words.join(" ")));
    }
}
