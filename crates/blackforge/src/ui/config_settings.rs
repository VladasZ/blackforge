//! The right side of the Configs page: the settings of one file in a table.
//! A value is saved when its field loses the focus, a switch, a list or a
//! pill saves at once.

use blackforge_core::config::{Accepted, Setting, find, read, split_values, write};
use hilen::{
    dispatch::after,
    refs::{Weak, weak_from_ref},
    ui::{CellRegistry, Label, Setup, TableData, TableView, View, ViewData, ViewFrame, view},
};

use crate::{
    backend,
    ui::{
        colors,
        setting_cell::{self, Layout, SettingCell},
        style, toast,
    },
};

/// Seconds the saved mark stays next to a field.
const SAVED_FOR: f32 = 2.0;
/// A longer list does not fit on screen as an open drop down, and a longer
/// row of pills is no quicker to read than a text field.
const MAX_CHOICES: usize = 12;

#[derive(Clone, Debug)]
enum Row {
    Section(String),
    /// Boxed, a setting is ten times the size of a section name.
    Setting(Box<SettingRow>),
}

#[derive(Clone, Debug)]
pub struct SettingRow {
    pub setting: Setting,
    /// The description, the text under the key.
    pub detail: String,
    pub control: Control,
    accepted: Option<Accepted>,
}

/// What edits the value of a setting.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Switch(bool),
    Field,
    /// One value of a short list, in the spelling of the list.
    Choice {
        values: Vec<String>,
        picked: String,
    },
    /// Any of a short list at once, every value a pill that turns on or off.
    Pills(Vec<String>),
}

impl SettingRow {
    fn new(setting: Setting) -> Self {
        let detail = setting.description.join(" ");
        let accepted = setting.accepted();
        let control = control(&setting, accepted.clone());
        Self {
            setting,
            detail,
            control,
            accepted,
        }
    }

    /// The value differs from the default the file names. A setting with no
    /// default is never marked, there is nothing to go back to.
    pub fn changed(&self) -> bool {
        self.setting
            .default
            .as_ref()
            .is_some_and(|default| !default.eq_ignore_ascii_case(&self.setting.value))
    }

    /// Why the value cannot be saved, `None` when it can.
    fn refuse(&self, value: &str) -> Option<String> {
        let Some(accepted @ Accepted::Range { min, max }) = &self.accepted else {
            return None;
        };
        (!accepted.allows(value)).then(|| format!("Must be from {min} to {max}"))
    }
}

fn control(setting: &Setting, accepted: Option<Accepted>) -> Control {
    if let Some(on) = setting.as_bool() {
        return Control::Switch(on);
    }
    let Some(Accepted::Values { values, multiple }) = accepted else {
        return Control::Field;
    };
    if values.len() > MAX_CHOICES {
        return Control::Field;
    }
    if multiple {
        return Control::Pills(values);
    }
    // A value the list does not name stays editable as text, a drop down
    // would show another value than the file has.
    match values
        .iter()
        .find(|value| value.eq_ignore_ascii_case(&setting.value))
    {
        Some(picked) => Control::Choice {
            picked: picked.clone(),
            values,
        },
        None => Control::Field,
    }
}

/// The values of a pill row after a tap on `value`. The last value on
/// cannot be turned off, the mod reads an empty value as an error.
pub fn toggle(current: &str, value: &str) -> Option<String> {
    let mut picked: Vec<&str> = split_values(current);
    if let Some(at) = picked
        .iter()
        .position(|part| part.eq_ignore_ascii_case(value))
    {
        if picked.len() == 1 {
            return None;
        }
        picked.remove(at);
    } else {
        picked.push(value);
    }
    Some(picked.join(", "))
}

#[view]
pub struct ConfigSettings {
    file: String,
    rows: Vec<Row>,
    /// The row that shows the saved mark right now.
    saved: Option<usize>,
    /// A value outside the range of its row, with the note that says so. The
    /// field keeps the typed text, the file keeps the old value.
    refused: Option<(usize, String, String)>,

    #[init]
    table: TableView,
    /// Never shown. It has the look of a detail label, so it can say how
    /// tall a detail is at the width the table has now.
    probe: Label,
    /// Never shown, it measures the text of a pill.
    pill_probe: Label,
}

impl Setup for ConfigSettings {
    fn setup(mut self: Weak<Self>) {
        self.table
            .set_data_source(self)
            .register_cell::<SectionCell>()
            .register_cell::<SettingCell>();
        style::table(self.table);
        self.table.set_variable_heights(true);
        self.table.place().back();
        // A detail wraps at the width of the panel, so a new width means new
        // row heights. The table listens to its own size event itself and an
        // event takes one listener, so the panel listens to its own. The
        // panel is laid out before the table in it, so its width is the
        // fresh one, and the table fills it.
        self.size_changed().sub(move || self.table.reload_data());

        style::dim(self.probe);
        self.probe.set_multiline(true);
        self.probe.set_hidden(true);

        setting_cell::pill_text(self.pill_probe);
        self.pill_probe.set_hidden(true);
    }
}

impl ConfigSettings {
    pub fn open(mut self: Weak<Self>, file: String) {
        self.file.clone_from(&file);
        self.refused = None;
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
                    rows.push(Row::Setting(Box::new(SettingRow::new(setting))));
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

    pub fn save(mut self: Weak<Self>, index: usize, value: String) {
        let Some(Row::Setting(row)) = self.rows.get_mut(index) else {
            return;
        };
        if let Some(note) = row.refuse(&value) {
            self.refused = Some((index, value, note));
            self.table.reload_data();
            return;
        }
        let was_refused = self
            .refused
            .as_ref()
            .is_some_and(|(refused, ..)| *refused == index);
        if was_refused {
            self.refused = None;
            self.table.reload_data();
        }
        let Some(Row::Setting(row)) = self.rows.get_mut(index) else {
            return;
        };
        if row.setting.value == value {
            return;
        }
        row.setting.value.clone_from(&value);
        row.control = control(&row.setting, row.accepted.clone());

        let section = row.setting.section.clone();
        let key = row.setting.key.clone();
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
                Ok(_) => {
                    if self.is_ok() {
                        self.show_saved(index);
                    }
                }
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

    /// The mark goes away by itself. A later save moves it to its own row.
    fn show_saved(mut self: Weak<Self>, index: usize) {
        self.saved = Some(index);
        self.table.reload_data();
        after(SAVED_FOR, move || {
            if self.is_ok() && self.saved == Some(index) {
                let mut page = self;
                page.saved = None;
                page.table.reload_data();
            }
        });
    }

    pub fn reset(self: Weak<Self>, index: usize) {
        let Some(Row::Setting(row)) = self.rows.get(index) else {
            return;
        };
        if let Some(default) = row.setting.default.clone() {
            self.save(index, default);
        }
    }

    fn refused_at(&self, index: usize) -> Option<(&str, &str)> {
        self.refused
            .as_ref()
            .filter(|(refused, ..)| *refused == index)
            .map(|(_, text, note)| (text.as_str(), note.as_str()))
    }

    fn layout(&self, row: &SettingRow, refused: bool) -> Layout {
        Layout::new(row, self.width(), refused, self.probe, self.pill_probe)
    }
}

impl TableData for ConfigSettings {
    fn cell_height(&self, index: usize) -> f32 {
        match &self.rows[index] {
            Row::Section(_) => setting_cell::MIN_HEIGHT,
            Row::Setting(row) => self.layout(row, self.refused_at(index).is_some()).height,
        }
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
            Row::Setting(row) => {
                let cell = registry.cell::<SettingCell>();
                let refused = self.refused_at(index);
                let layout = self.layout(row, refused.is_some());
                cell.set_setting(
                    index,
                    weak_from_ref(self),
                    row,
                    &layout,
                    self.saved == Some(index),
                    refused,
                );
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

#[cfg(test)]
mod tests {
    use super::toggle;

    #[test]
    fn a_pill_turns_its_value_on_and_off() {
        assert_eq!(
            toggle("Warning, Error", "error").as_deref(),
            Some("Warning")
        );
        assert_eq!(
            toggle("Warning", "Error").as_deref(),
            Some("Warning, Error")
        );
        assert_eq!(toggle("Warning", "Warning"), None);
    }
}
