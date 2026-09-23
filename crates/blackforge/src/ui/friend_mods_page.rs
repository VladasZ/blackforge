//! The mods of one friend, the ones they asked for. A dependency comes
//! along with the mod that needs it, so it is not listed. Every row adds
//! that one mod the way Browse does, the newest version with what it needs.
//! A mod both of us have gets a second button that opens the config picker.

use std::collections::{HashMap, HashSet};

use blackforge_api::SharedProfile;
use blackforge_core::{
    broken::Broken,
    config::{self, package_of},
    ident::PackageId,
    social::picker::{self, Pick},
};
use hilen::{
    Event,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, ModalView, Setup, TableData, TableView,
        TextAlignment, VerticalAlignment, View, ViewData, view,
    },
};

use crate::{
    backend, social,
    ui::{
        busy, colors,
        config_picker::{ConfigPicker, PickerFile, PickerInput},
        hover,
        mod_icon::ModIcon,
        mod_info::ModInfo,
        mod_pills::{ModPills, Note},
        mods_page::lock_change_summary,
        names, pill, style, toast,
    },
};

const ROW_HEIGHT: f32 = 104.0;
const BUTTON_WIDTH: f32 = 110.0;
/// The pills end where the buttons start, with a gap.
const PILLS_RIGHT: f32 = 16.0 + 2.0 * BUTTON_WIDTH + 8.0 + 16.0;
/// Where the first line of a row starts: the name, the pills and the buttons.
const TOP_LINE: f32 = 12.0;
const ICON: f32 = 40.0;
/// The name and the description start right of the icon.
const TEXT_LEFT: f32 = 4.0 + ICON + 12.0;

#[derive(Clone, Debug)]
struct ModRow {
    id: String,
    version: String,
    /// Off in the friend's profile.
    enabled: bool,
    installed: bool,
    /// The server lists it as broken on the game version of this machine.
    broken: bool,
    /// Empty when the package list does not know the mod.
    description: String,
    /// The friend's shared config files of this mod that I have as well.
    configs: Vec<String>,
}

#[view]
pub struct FriendModsPage {
    pub back: Event,

    friend: String,
    profile: SharedProfile,
    rows: Vec<ModRow>,

    #[init]
    back_button: Button,
    title: Label,
    subtitle: Label,
    table: TableView,
}

impl Setup for FriendModsPage {
    fn setup(self: Weak<Self>) {
        style::ghost(self.back_button, "Back");
        self.back_button
            .place()
            .t(24)
            .l(style::PAGE_PAD)
            .size(70, style::BUTTON_H);
        self.back_button.on_tap(move || self.back.trigger(()));

        style::title(self.title, "");
        self.title
            .place()
            .t(24)
            .l(style::PAGE_PAD + 84.0)
            .size(500, 30);

        style::dim(self.subtitle);
        self.subtitle
            .place()
            .t(56)
            .l(style::PAGE_PAD + 84.0)
            .size(600, 16);

        self.table.set_data_source(self).register_cell::<ModCell>();
        style::table(self.table);
        self.table
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);
    }
}

impl FriendModsPage {
    pub fn set_friend(mut self: Weak<Self>, friend: String) {
        self.title.set_text(format!("Mods of {friend}"));
        self.friend = friend;
        self.refresh();
    }

    fn refresh(self: Weak<Self>) {
        self.subtitle.set_text("Loading");
        let friend = self.friend.clone();

        backend::load(
            "loading the mods of a friend",
            move |forge, progress| async move {
                let shared = social::client()?.friend_profile(&friend).await?;

                let profile = backend::profile(forge, &progress).await?;
                let installed: HashSet<String> = profile
                    .lock()
                    .await?
                    .packages
                    .iter()
                    .map(|package| package.id.to_string())
                    .collect();
                let my_configs: HashSet<String> = config::list(&profile)
                    .await?
                    .into_iter()
                    .map(|file| file.to_lowercase())
                    .collect();
                let ids: Vec<PackageId> = shared
                    .mods
                    .iter()
                    .filter_map(|shared| shared.id.parse().ok())
                    .collect();
                let descriptions = backend::descriptions(forge, &progress, ids.iter()).await;
                let broken = backend::broken_known(forge, &progress).await;

                let rows = rows_of(
                    &shared,
                    &ids,
                    &installed,
                    &my_configs,
                    &descriptions,
                    &broken,
                );
                Ok((shared, rows))
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok((shared, rows)) => self.set_rows(shared, rows),
                    Err(error) => {
                        self.subtitle.set_text("The mods did not load");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn set_rows(mut self: Weak<Self>, shared: SharedProfile, rows: Vec<ModRow>) {
        let missing = rows.iter().filter(|row| !row.installed).count();
        self.subtitle.set_text(match (rows.len(), missing) {
            (0, _) => "Nothing shared yet".to_owned(),
            (count, 0) => format!("{count} mods, you have all of them"),
            (count, missing) => format!("{count} mods, you do not have {missing} of them"),
        });
        self.profile = shared;
        self.rows = rows;
        self.table.reload_data();
    }

    fn add(self: Weak<Self>, index: usize, button: Weak<Button>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let id = row.id.clone();
        busy::press(button, "Adding...");
        backend::change(
            "adding the mod",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let (id, change) = forge.add(&profile, &id, &progress).await?;
                forge.sync(&profile, &progress).await?;
                Ok(format!("added {id}, {}", lock_change_summary(&change)))
            },
            move |result| {
                match result {
                    Ok(text) => toast::success(text),
                    Err(error) => toast::failure(&error),
                }
                if self.is_ok() {
                    self.refresh();
                }
            },
        );
    }

    /// Reads my side of every shared file of the mod and opens the picker.
    fn copy_config(self: Weak<Self>, index: usize, button: Weak<Button>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let mod_name = row.id.clone();
        let wanted: Vec<_> = self
            .profile
            .configs
            .iter()
            .filter(|shared| row.configs.contains(&shared.file))
            .cloned()
            .collect();

        backend::load(
            "comparing the settings",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let mut files = Vec::new();
                for shared in wanted {
                    let path = config::find(&profile, &shared.file).await?;
                    let mine = config::read(&path).await?;
                    let rows = picker::rows(&mine, &shared.settings);
                    if !rows.is_empty() {
                        files.push(PickerFile {
                            file: shared.file,
                            rows,
                        });
                    }
                }
                Ok(files)
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(files) if files.is_empty() => toast::info("your settings already match"),
                    Ok(files) => self.pick(mod_name, files, button),
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn pick(self: Weak<Self>, mod_name: String, files: Vec<PickerFile>, button: Weak<Button>) {
        let input = PickerInput {
            friend: self.friend.clone(),
            mod_name,
            files,
        };
        ConfigPicker::show_modally_with_input(input, move |picked| {
            if let Some(files) = picked {
                write_picks(files, button);
            }
        });
    }
}

fn write_picks(files: Vec<PickerFile>, button: Weak<Button>) {
    if !files
        .iter()
        .flat_map(|file| &file.rows)
        .any(|row| row.pick == Pick::Friend)
    {
        return toast::info("nothing was picked, nothing changed");
    }

    busy::press(button, "Copying...");
    backend::change(
        "copying the settings",
        |forge, progress| async move {
            let profile = backend::profile(forge, &progress).await?;
            let mut changed = 0;
            for file in files {
                let path = config::find(&profile, &file.file).await?;
                let mut mine = config::read(&path).await?;
                changed += picker::apply(&mut mine, &file.rows)?;
                config::write(&path, &mine).await?;
            }
            Ok(changed)
        },
        |result| match result {
            Ok(1) => toast::success("1 setting copied"),
            Ok(changed) => toast::success(format!("{changed} settings copied")),
            Err(error) => toast::failure(&error),
        },
    );
}

/// The mods the friend asked for, dependencies left out. A config file has
/// no field that names its mod, so the file names are matched against all
/// of the friend's mods the way the Configs page does it, `ids` holds the
/// dependencies too so a file of one is not given to another mod.
fn rows_of(
    shared: &SharedProfile,
    ids: &[PackageId],
    installed: &HashSet<String>,
    my_configs: &HashSet<String>,
    descriptions: &HashMap<String, String>,
    broken: &Broken,
) -> Vec<ModRow> {
    shared
        .mods
        .iter()
        .filter(|shared_mod| !shared_mod.dependency)
        .map(|shared_mod| {
            let installed = installed.contains(&shared_mod.id);
            let configs = shared
                .configs
                .iter()
                .filter(|config| installed && my_configs.contains(&config.file.to_lowercase()))
                .filter(|config| {
                    package_of(&config.file, ids).is_some_and(|id| id.to_string() == shared_mod.id)
                })
                .map(|config| config.file.clone())
                .collect();

            ModRow {
                id: shared_mod.id.clone(),
                version: shared_mod.version.clone(),
                enabled: shared_mod.enabled,
                installed,
                broken: shared_mod
                    .id
                    .parse()
                    .is_ok_and(|id: PackageId| broken.contains(&id)),
                description: descriptions
                    .get(&shared_mod.id)
                    .cloned()
                    .unwrap_or_default(),
                configs,
            }
        })
        .collect()
}

impl TableData for FriendModsPage {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<ModCell>();
        cell.set_row(index, weak_from_ref(self), &self.rows[index]);
        cell
    }

    fn cell_selected(&mut self, index: usize) {
        let page = weak_from_ref(self);
        if let Some(row) = self.rows.get(index) {
            ModInfo::open(&row.id, move |changed| {
                if changed && page.is_ok() {
                    page.refresh();
                }
            });
        }
    }
}

#[view]
struct ModCell {
    index: usize,
    page: Weak<FriendModsPage>,

    #[init]
    icon: ModIcon,
    name: Label,
    author: Label,
    description: Label,
    pills: ModPills,
    installed: Label,
    add: Button,
    copy_config: Button,
    line: Container,
}

impl Setup for ModCell {
    fn setup(self: Weak<Self>) {
        self.icon.place().l(4).t(TOP_LINE).size(ICON, ICON);

        style::body(self.name);
        self.name.set_ellipsize(true);

        style::dim(self.author);
        self.author.set_ellipsize(true);
        self.author
            .place()
            .t(TOP_LINE + 24.0)
            .l(TEXT_LEFT)
            .r(16)
            .h(16);

        // The whole row width and 2 lines, like a row of Browse. A click on
        // the row opens the details with the full text.
        style::dim(self.description);
        self.description.set_multiline(true);
        self.description
            .set_vertical_alignment(VerticalAlignment::Top);
        self.description
            .place()
            .t(TOP_LINE + 46.0)
            .l(TEXT_LEFT)
            .r(16)
            .h(36);

        // The same rectangle as the add button, so the column reads as one.
        self.installed.set_text("Installed");
        self.installed.set_text_size(13).set_text_color(colors::OK);
        self.installed.set_alignment(TextAlignment::Center);
        self.installed.set_color(colors::OK_BG);
        self.installed.set_corner_radius(7);
        self.installed
            .place()
            .r(16)
            .t(TOP_LINE - 4.0)
            .size(BUTTON_WIDTH, style::BUTTON_H);

        style::primary(self.add, "Add");
        self.add
            .place()
            .r(16)
            .t(TOP_LINE - 4.0)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.add.on_tap(move || {
            if self.page.is_ok() {
                self.page.add(self.index, self.add);
            }
        });
        busy::track(self.add);

        style::ghost(self.copy_config, "Copy config");
        self.copy_config
            .place()
            .r(16.0 + BUTTON_WIDTH + 8.0)
            .t(TOP_LINE - 4.0)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.copy_config.on_tap(move || {
            if self.page.is_ok() {
                self.page.copy_config(self.index, self.copy_config);
            }
        });
        busy::track(self.copy_config);

        // The wash says the row can be clicked, the click opens the details.
        hover::row(self);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl ModCell {
    fn set_row(mut self: Weak<Self>, index: usize, page: Weak<FriendModsPage>, row: &ModRow) {
        self.index = index;
        self.page = page;

        self.icon.show(&row.id, &row.version);
        self.name.set_text(names::title(&row.id));
        self.author.set_text(names::author(&row.id));
        self.description.set_text(&row.description);

        // The pills sit right of the name, before the buttons. The name ends
        // where they start.
        let note = if row.enabled {
            Note::None
        } else {
            Note::Disabled
        };
        let pills = self.pills.set(&row.version, &note, row.broken);
        self.pills
            .place()
            .clear()
            .r(PILLS_RIGHT)
            .t(TOP_LINE)
            .size(pills, pill::HEIGHT);
        self.name
            .place()
            .clear()
            .t(TOP_LINE + 2.0)
            .l(TEXT_LEFT)
            .r(PILLS_RIGHT + pills + 8.0)
            .h(20);

        self.installed.set_hidden(!row.installed);
        self.add.set_hidden(row.installed);
        self.copy_config.set_hidden(row.configs.is_empty());
    }
}
