//! The "copy config" dialog. One row per setting that differs, my value on
//! the left, the friend's on the right, and a tap picks the side that wins.
//! Nothing is written here, the caller gets the rows back with the picks.

use blackforge_core::social::picker::{Pick, PickerRow};
use hilen::{
    OnceEvent,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, ModalView, Setup, Size, TableData, TableView,
        UIColor, View, ViewData, view,
    },
};

use crate::ui::{colors, style, toast};

const PAD: f32 = 24.0;
const ROW_HEIGHT: f32 = 64.0;
const VALUE_WIDTH: f32 = 170.0;

/// The rows of one config file.
#[derive(Clone, Debug, Default)]
pub struct PickerFile {
    pub file: String,
    pub rows: Vec<PickerRow>,
}

#[derive(Clone, Debug, Default)]
pub struct PickerInput {
    pub friend: String,
    pub mod_name: String,
    pub files: Vec<PickerFile>,
}

#[view]
pub struct ConfigPicker {
    event: OnceEvent<Option<Vec<PickerFile>>>,

    files: Vec<PickerFile>,
    /// The file and the row of every line of the table.
    lines: Vec<(usize, usize)>,

    #[init]
    title: Label,
    subtitle: Label,
    mine_head: Label,
    friend_head: Label,
    table: TableView,
    cancel: Button,
    apply: Button,
}

impl ModalView<PickerInput, Option<Vec<PickerFile>>> for ConfigPicker {
    fn modal_event(&self) -> &OnceEvent<Option<Vec<PickerFile>>> {
        &self.event
    }

    fn modal_size() -> Size {
        (760, 560).into()
    }

    fn modal_scrim_color() -> UIColor {
        colors::SCRIM.into()
    }

    fn setup_input(mut self: Weak<Self>, input: PickerInput) {
        self.title
            .set_text(format!("{} settings of {}", input.mod_name, input.friend));
        self.friend_head.set_text(input.friend);

        self.lines = input
            .files
            .iter()
            .enumerate()
            .flat_map(|(file, picked)| (0..picked.rows.len()).map(move |row| (file, row)))
            .collect();
        self.files = input.files;

        self.table.reload_data();
        self.count();
    }
}

impl Setup for ConfigPicker {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::title(self.title, "");
        self.title.set_text_size(18);
        self.title.place().t(PAD).l(PAD).r(PAD).h(24);

        style::dim(self.subtitle);
        self.subtitle.place().t(PAD + 28.0).l(PAD).r(PAD).h(16);

        style::dim(self.mine_head);
        self.mine_head.set_text("Mine");
        self.mine_head
            .place()
            .t(PAD + 58.0)
            .r(PAD + 16.0 + VALUE_WIDTH + 8.0)
            .size(VALUE_WIDTH, 16);
        style::dim(self.friend_head);
        self.friend_head
            .place()
            .t(PAD + 58.0)
            .r(PAD + 16.0)
            .size(VALUE_WIDTH, 16);

        self.table.set_data_source(self).register_cell::<PickCell>();
        self.table.place().t(PAD + 80.0).l(PAD).r(PAD).b(PAD + 48.0);

        style::primary(self.apply, "Apply");
        self.apply.place().r(PAD).b(PAD).size(120, style::BUTTON_H);
        self.apply.on_tap(move || {
            if self
                .files
                .iter()
                .flat_map(|file| &file.rows)
                .any(|row| row.pick == Pick::Pending)
            {
                toast::info("choose a side for every conflict first");
            } else {
                self.hide_modal(Some(self.files.clone()));
            }
        });

        style::ghost(self.cancel, "Cancel");
        self.cancel
            .place()
            .r(PAD + 130.0)
            .b(PAD)
            .size(90, style::BUTTON_H);
        self.cancel.on_tap(move || self.hide_modal(None));
    }
}

impl ConfigPicker {
    fn pick(mut self: Weak<Self>, line: usize, pick: Pick) {
        let Some(&(file, row)) = self.lines.get(line) else {
            return;
        };
        self.files[file].rows[row].pick = pick;
        self.table.reload_data();
        self.count();
    }

    fn count(self: Weak<Self>) {
        let taken = self
            .files
            .iter()
            .flat_map(|file| &file.rows)
            .filter(|row| row.pick == Pick::Friend)
            .count();

        self.subtitle.set_text(format!(
            "{} settings differ. Tap a value to pick it, nothing of yours is lost unless you pick the other side",
            self.lines.len()
        ));
        self.apply.set_text(match taken {
            0 => "Apply nothing".to_owned(),
            1 => "Apply 1 value".to_owned(),
            _ => format!("Apply {taken} values"),
        });
    }
}

impl TableData for ConfigPicker {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.lines.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<PickCell>();
        let (file, row) = self.lines[index];
        cell.set_row(
            index,
            weak_from_ref(self),
            &self.files[file].file,
            &self.files[file].rows[row],
        );
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct PickCell {
    index: usize,
    picker: Weak<ConfigPicker>,

    #[init]
    key: Label,
    place_label: Label,
    mine: Button,
    friend: Button,
    line: Container,
}

impl Setup for PickCell {
    fn setup(self: Weak<Self>) {
        style::body(self.key);
        self.key.set_ellipsize(true);
        self.key
            .place()
            .t(12)
            .l(4)
            .r(16.0 + 2.0 * VALUE_WIDTH + 24.0)
            .h(20);

        style::dim(self.place_label);
        self.place_label.set_ellipsize(true);
        self.place_label
            .place()
            .t(34)
            .l(4)
            .r(16.0 + 2.0 * VALUE_WIDTH + 24.0)
            .h(16);

        self.friend.place().r(16).t(14).size(VALUE_WIDTH, 36);
        self.friend.on_tap(move || {
            if self.picker.is_ok() {
                self.picker.pick(self.index, Pick::Friend);
            }
        });

        self.mine
            .place()
            .r(16.0 + VALUE_WIDTH + 8.0)
            .t(14)
            .size(VALUE_WIDTH, 36);
        self.mine.on_tap(move || {
            if self.picker.is_ok() {
                self.picker.pick(self.index, Pick::Mine);
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl PickCell {
    fn set_row(
        mut self: Weak<Self>,
        index: usize,
        picker: Weak<ConfigPicker>,
        file: &str,
        row: &PickerRow,
    ) {
        self.index = index;
        self.picker = picker;

        self.key.set_text(&row.key);
        self.place_label
            .set_text(format!("{file}, [{}]", row.section));

        value_button(self.mine, &row.mine, row.pick == Pick::Mine);
        let theirs = if row.friend_has_default {
            format!("{} (default)", shown(&row.friend))
        } else {
            shown(&row.friend)
        };
        value_button(self.friend, &theirs, row.pick == Pick::Friend);
    }
}

/// An empty value is a real value, a blank button would read as broken.
fn shown(value: &str) -> String {
    if value.is_empty() {
        "Empty".to_owned()
    } else {
        value.to_owned()
    }
}

fn value_button(button: Weak<Button>, text: &str, picked: bool) {
    if picked {
        style::primary(button, &shown(text));
        button.set_border_width(0);
    } else {
        style::ghost(button, &shown(text));
    }
}
