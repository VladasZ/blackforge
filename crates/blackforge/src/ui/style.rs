//! The shared look of labels, buttons and fields, so every page reads the same.

use hilen::{
    refs::Weak,
    ui::{Button, Label, TableView, TextAlignment, TextField, ViewData},
};

use crate::ui::colors;

pub const PAGE_PAD: f32 = 28.0;
pub const HEADER: f32 = 84.0;
pub const BUTTON_H: f32 = 32.0;
pub const FIELD_H: f32 = 34.0;
const TABLE_FOOTER: f32 = 12.0;

pub fn title(label: Weak<Label>, text: &str) {
    label
        .set_text(text)
        .set_text_size(24)
        .set_text_color(colors::FG);
    label.set_alignment(TextAlignment::Left);
    label.set_color(colors::CLEAR);
}

pub fn body(label: Weak<Label>) {
    label.set_text_size(14).set_text_color(colors::FG);
    label.set_alignment(TextAlignment::Left);
    label.set_color(colors::CLEAR);
}

pub fn dim(label: Weak<Label>) {
    label.set_text_size(12).set_text_color(colors::DIM);
    label.set_alignment(TextAlignment::Left);
    label.set_color(colors::CLEAR);
}

pub fn primary(button: Weak<Button>, text: &str) {
    button.set_text(text);
    button.set_text_size(13);
    button.set_text_color(colors::ON_ACCENT);
    button.set_color(colors::ACCENT);
    button.set_corner_radius(7);
}

pub fn ghost(button: Weak<Button>, text: &str) {
    button.set_text(text);
    button.set_text_size(13);
    button.set_text_color(colors::FG);
    button.set_color(colors::CLEAR);
    button.set_border_color(colors::BORDER);
    button.set_border_width(1);
    button.set_corner_radius(7);
}

pub fn danger(button: Weak<Button>, text: &str) {
    button.set_text(text);
    button.set_text_size(13);
    button.set_text_color(colors::BAD);
    button.set_color(colors::BAD_BG);
    button.set_corner_radius(7);
}

pub fn field(field: Weak<TextField>, placeholder: &str) {
    let mut field = field;
    field.set_placeholder(placeholder);
    field.set_text_size(14);
    field.set_text_color(colors::FG);
    field.set_placeholder_color(colors::DIM);
    field.set_selected_color(colors::FIELD_BG);
    field.set_alignment(TextAlignment::Left);
    field.set_color(colors::FIELD_BG);
    field.set_border_color(colors::BORDER);
    field.set_border_width(1);
    field.set_corner_radius(7);
}

/// A table runs down to the status bar and clips its rows there. The footer
/// lets the last row scroll clear of that edge.
pub fn table(table: Weak<TableView>) {
    let mut table = table;
    table.set_footer_height(TABLE_FOOTER);
}

pub fn card(view: Weak<impl ViewData + ?Sized>) {
    view.set_color(colors::CARD_BG);
    view.set_border_color(colors::BORDER);
    view.set_border_width(1);
    view.set_corner_radius(10);
}
