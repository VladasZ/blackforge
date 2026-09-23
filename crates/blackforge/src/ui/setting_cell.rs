//! One setting of a config file: its key, what it does, and the control
//! that edits its value. The control stands right of the text while the
//! column has room for both, and moves under the text when it does not.

use blackforge_core::config::split_values;
use hilen::{
    refs::Weak,
    ui::{
        Button, Container, DropDown, Label, Setup, Switch, TextAlignment, TextField,
        VerticalAlignment, ViewData, ViewSubviews, view,
    },
};

use crate::ui::{
    colors,
    config_settings::{ConfigSettings, Control, SettingRow, toggle},
    hover, style,
};

/// A setting with a detail of one line. A longer detail wraps and makes
/// its row taller.
pub const MIN_HEIGHT: f32 = 62.0;

const KEY_T: f32 = 10.0;
const DETAIL_T: f32 = 34.0;
const DETAIL_L: f32 = 4.0;
const PAD_B: f32 = 12.0;
/// The scroll bar draws over the right edge of the table.
const EDGE: f32 = 16.0;
/// Between the text and a control right of it.
const GAP: f32 = 20.0;
/// The dim "Default: true" line under the description.
const DEFAULT_H: f32 = 16.0;
const DEFAULT_GAP: f32 = 4.0;
const FIELD_W: f32 = 290.0;
/// A field right of the text is never narrower. Below that it moves under
/// the text.
const FIELD_MIN: f32 = 150.0;
/// The text right of which a field stands is never narrower either.
const TEXT_MIN: f32 = 240.0;
/// A field right of the text sits level with the key.
const FIELD_T: f32 = 6.0;
const SWITCH_T: f32 = 8.0;
const SWITCH_W: f32 = 44.0;
const SWITCH_H: f32 = 24.0;
/// Between the text and a control under it.
const STACK_GAP: f32 = 10.0;
const NOTE_GAP: f32 = 4.0;
const NOTE_H: f32 = 16.0;
const PILL_H: f32 = 26.0;
const PILL_PAD: f32 = 12.0;
const PILL_GAP: f32 = 6.0;

/// Where the parts of one setting row go at the width of the table.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// The width of the row. A reused cell still has its old size when it
    /// is set up.
    width: f32,
    /// The control stands under the text, not right of it.
    stacked: bool,
    /// Where the text of a label starts inside its frame. A control under
    /// the text starts there too.
    inset: f32,
    /// From the right edge of the row to the end of the text.
    text_right: f32,
    detail_h: f32,
    default_t: f32,
    control_t: f32,
    control_w: f32,
    /// Left, top and width of every pill, inside the pill row.
    pills: Vec<(f32, f32, f32)>,
    note_t: f32,
    pub height: f32,
}

impl Layout {
    pub fn new(
        row: &SettingRow,
        width: f32,
        refused: bool,
        probe: Weak<Label>,
        pill_probe: Weak<Label>,
    ) -> Self {
        let inset = probe.text_inset();
        let full = width - DETAIL_L - inset - EDGE;
        let (stacked, control_w) = match &row.control {
            Control::Switch(_) => (false, SWITCH_W),
            Control::Pills(_) => (true, full),
            Control::Field | Control::Choice { .. } => {
                let room = full - GAP - TEXT_MIN;
                if room >= FIELD_MIN {
                    (false, room.min(FIELD_W))
                } else {
                    (true, full.min(FIELD_W))
                }
            }
        };
        let text_right = if stacked {
            EDGE
        } else {
            EDGE + control_w + GAP
        };

        let detail_h = if row.detail.is_empty() {
            0.0
        } else {
            probe.set_text(&row.detail);
            probe.size_for_width(width - DETAIL_L - text_right).height
        };
        let mut y = DETAIL_T + detail_h;
        let mut default_t = y;
        if row.setting.default.is_some() {
            if detail_h > 0.0 {
                default_t += DEFAULT_GAP;
            }
            y = default_t + DEFAULT_H;
        }

        let pills = match &row.control {
            Control::Pills(values) => flow(values, control_w, pill_probe),
            _ => Vec::new(),
        };
        let control_h = match &row.control {
            Control::Switch(_) => SWITCH_H,
            Control::Field | Control::Choice { .. } => style::FIELD_H,
            Control::Pills(_) => pills.last().map_or(0.0, |(_, top, _)| top + PILL_H),
        };
        let control_t = if stacked {
            y += STACK_GAP;
            let top = y;
            y += control_h;
            top
        } else if matches!(row.control, Control::Switch(_)) {
            SWITCH_T
        } else {
            FIELD_T
        };
        let note_t = control_t + control_h + NOTE_GAP;
        let mut bottom = y;
        if refused {
            bottom = bottom.max(note_t + NOTE_H);
        }

        Self {
            width,
            stacked,
            inset,
            text_right,
            detail_h,
            default_t,
            control_t,
            control_w,
            pills,
            note_t,
            height: (bottom + PAD_B).max(MIN_HEIGHT),
        }
    }
}

/// The pills in rows as wide as `width`, one row after another.
fn flow(values: &[String], width: f32, probe: Weak<Label>) -> Vec<(f32, f32, f32)> {
    let (mut x, mut y) = (0.0, 0.0);
    values
        .iter()
        .map(|value| {
            probe.set_text(value);
            let pill = probe.content_size().width + 2.0 * PILL_PAD;
            if x > 0.0 && x + pill > width {
                x = 0.0;
                y += PILL_H + PILL_GAP;
            }
            let at = (x, y, pill);
            x += pill + PILL_GAP;
            at
        })
        .collect()
}

/// Right of the text, or under it at the left edge.
fn place_control(view: Weak<impl ViewData + ?Sized>, layout: &Layout, top: f32, height: f32) {
    let place = view.place();
    place.clear().t(top).size(layout.control_w, height);
    if layout.stacked {
        place.l(DETAIL_L + layout.inset);
    } else {
        place.r(EDGE);
    }
}

/// The text look of a pill, shared with the label that measures one.
pub fn pill_text(label: Weak<Label>) {
    label.set_text_size(12);
    label.set_alignment(TextAlignment::Center);
    label.set_color(colors::CLEAR);
}

#[view]
pub struct SettingCell {
    index: usize,
    page: Weak<ConfigSettings>,
    pill_buttons: Vec<Weak<Button>>,

    #[init]
    key: Label,
    changed: Container,
    reset: Button,
    saved: Label,
    detail: Label,
    default: Label,
    value: TextField,
    toggle: Switch,
    choice: DropDown<String>,
    pills: Container,
    note: Label,
    line: Container,
}

impl Setup for SettingCell {
    fn setup(self: Weak<Self>) {
        style::body(self.key);
        self.key.set_ellipsize(true);

        // An orange dot after the key says the value is not the default.
        self.changed.set_color(colors::ACCENT);
        self.changed.set_corner_radius(4);

        style::ghost(self.reset, "Reset");
        self.reset.set_border_width(0);
        self.reset.set_text_size(12);
        self.reset.set_text_color(colors::ACCENT);
        self.reset.on_tap(move || {
            if self.page.is_ok() {
                self.page.reset(self.index);
            }
        });

        style::dim(self.saved);
        self.saved.set_text("Saved");
        self.saved.set_text_color(colors::OK);

        // The whole text, wrapped. The row is as tall as the text needs.
        style::dim(self.detail);
        self.detail.set_multiline(true);
        self.detail.set_vertical_alignment(VerticalAlignment::Top);

        style::dim(self.default);

        style::field(self.value, "");
        self.value.editing_ended.val(move |text| {
            if self.page.is_ok() {
                self.page.save(self.index, text);
            }
        });

        self.toggle.on_change(move |on| {
            if self.page.is_ok() {
                self.page.save(self.index, on.to_string());
            }
        });

        let mut choice = self.choice;
        choice.set_text_size(14);
        choice.set_text_color(colors::FG);
        choice.set_accent_color(colors::ACCENT);
        choice.set_color(colors::FIELD_BG);
        choice.set_border_color(colors::BORDER);
        choice.set_border_width(1);
        choice.set_corner_radius(7);
        choice.on_changed(move |value| {
            if self.page.is_ok() {
                self.page.save(self.index, value);
            }
        });

        self.pills.set_color(colors::CLEAR);

        style::dim(self.note);
        self.note.set_text_color(colors::BAD);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl SettingCell {
    pub fn set_setting(
        mut self: Weak<Self>,
        index: usize,
        page: Weak<ConfigSettings>,
        row: &SettingRow,
        layout: &Layout,
        saved: bool,
        refused: Option<(&str, &str)>,
    ) {
        self.index = index;
        self.page = page;

        self.show_key(row, layout, saved);
        self.show_text(row, layout);
        self.show_control(row, layout, refused.map(|(text, _)| text));

        self.note.set_hidden(refused.is_none());
        if let Some((_, note)) = refused {
            self.note.set_text(note);
            place_control(self.note, layout, layout.note_t, NOTE_H);
        }
    }

    /// The key is as wide as its text, so the dot, the reset link and the
    /// saved mark sit right after it.
    fn show_key(self: Weak<Self>, row: &SettingRow, layout: &Layout, saved: bool) {
        self.key.set_text(&row.setting.key);
        let room = layout.width - DETAIL_L - layout.text_right - 90.0;
        let key = self.key.content_size().width.min(room.max(0.0));
        self.key.place().clear().t(KEY_T).l(DETAIL_L).w(key).h(20);

        let changed = row.changed();
        self.changed.set_hidden(!changed);
        self.changed
            .place()
            .clear()
            .t(KEY_T + 6.0)
            .l(DETAIL_L + key + 8.0)
            .size(8, 8);
        self.reset.set_hidden(!changed);
        self.reset
            .place()
            .clear()
            .t(KEY_T - 2.0)
            .l(DETAIL_L + key + 22.0)
            .size(56, 24);

        self.saved.set_hidden(!saved);
        let after_key = if changed { 22.0 + 56.0 } else { 8.0 };
        self.saved
            .place()
            .clear()
            .t(KEY_T + 2.0)
            .l(DETAIL_L + key + after_key)
            .size(50, 16);
    }

    fn show_text(self: Weak<Self>, row: &SettingRow, layout: &Layout) {
        self.detail.set_text(&row.detail);
        self.detail.set_hidden(row.detail.is_empty());
        self.detail
            .place()
            .clear()
            .t(DETAIL_T)
            .l(DETAIL_L)
            .r(layout.text_right)
            .h(layout.detail_h);

        self.default.set_hidden(row.setting.default.is_none());
        if let Some(default) = &row.setting.default {
            self.default.set_text(format!("Default: {default}"));
            self.default
                .place()
                .clear()
                .t(layout.default_t)
                .l(DETAIL_L)
                .r(layout.text_right)
                .h(DEFAULT_H);
        }
    }

    fn show_control(mut self: Weak<Self>, row: &SettingRow, layout: &Layout, typed: Option<&str>) {
        self.value
            .set_hidden(!matches!(row.control, Control::Field));
        self.toggle
            .set_hidden(!matches!(row.control, Control::Switch(_)));
        self.choice
            .set_hidden(!matches!(row.control, Control::Choice { .. }));
        self.pills
            .set_hidden(!matches!(row.control, Control::Pills(_)));

        match &row.control {
            Control::Switch(on) => {
                self.toggle.set_on(*on);
                place_control(self.toggle, layout, layout.control_t, SWITCH_H);
            }
            Control::Field => {
                self.value.set_text(typed.unwrap_or(&row.setting.value));
                place_control(self.value, layout, layout.control_t, style::FIELD_H);
            }
            Control::Choice { values, picked } => {
                self.choice.set_values(values.clone());
                self.choice.set_value(picked);
                place_control(self.choice, layout, layout.control_t, style::FIELD_H);
            }
            Control::Pills(values) => {
                self.show_pills(values, &row.setting.value, layout);
            }
        }
    }

    /// One button per value, the ones the value names in the accent.
    fn show_pills(mut self: Weak<Self>, values: &[String], current: &str, layout: &Layout) {
        for mut button in self.pill_buttons.drain(..) {
            button.remove_from_superview();
        }
        let height = layout.pills.last().map_or(0.0, |(_, top, _)| top + PILL_H);
        self.pills
            .place()
            .clear()
            .t(layout.control_t)
            .l(DETAIL_L + layout.inset)
            .size(layout.control_w, height);

        let picked = split_values(current);
        for (value, (left, top, width)) in values.iter().zip(&layout.pills) {
            let on = picked.iter().any(|part| part.eq_ignore_ascii_case(value));
            let button = self.pills.add_view::<Button>();
            button.set_text(value);
            button.set_text_size(12);
            button.set_corner_radius(PILL_H / 2.0);
            button.set_border_width(1);
            if on {
                button.set_color(colors::ACCENT_BG);
                button.set_border_color(colors::ACCENT);
                button.set_text_color(colors::ACCENT);
                hover::clickable(button);
            } else {
                button.set_color(colors::PILL_BG);
                button.set_border_color(colors::CLEAR);
                button.set_text_color(colors::FG);
                hover::button(button, colors::NAV_HOVER_BG);
            }
            button.place().l(*left).t(*top).size(*width, PILL_H);
            let value = value.clone();
            let current = current.to_owned();
            button.on_tap(move || {
                if self.page.is_ok()
                    && let Some(next) = toggle(&current, &value)
                {
                    self.page.save(self.index, next);
                }
            });
            self.pill_buttons.push(button);
        }
    }
}
