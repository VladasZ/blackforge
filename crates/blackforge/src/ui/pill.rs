//! A small rounded chip for one fact of a row: an icon and a short text.

use hilen::{
    refs::Weak,
    ui::{ImageView, Label, Setup, TextAlignment, ViewData, view},
};

use crate::ui::colors;

pub const HEIGHT: f32 = 24.0;

const PAD: f32 = 9.0;
const ICON: f32 = 13.0;
const GAP: f32 = 5.0;

#[view]
pub struct Pill {
    #[init]
    icon: ImageView,
    label: Label,
    note: Label,
}

impl Setup for Pill {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::PILL_BG);
        self.set_corner_radius(HEIGHT / 2.0);

        self.icon.place().l(PAD).center_y().size(ICON, ICON);

        self.label.set_text_size(12).set_text_color(colors::FG);
        self.label.set_alignment(TextAlignment::Left);
        self.label.set_color(colors::CLEAR);

        self.note.set_text_size(12).set_text_color(colors::ACCENT);
        self.note.set_alignment(TextAlignment::Left);
        self.note.set_color(colors::CLEAR);
        self.note.set_hidden(true);
    }
}

impl Pill {
    /// The red look, for a fact the user must not miss.
    pub fn warn(self: Weak<Self>) {
        self.set_color(colors::BAD_BG);
        self.label.set_text_color(colors::BAD);
    }

    /// Sets the content and returns the width the pill needs. The text
    /// decides the width, so the owner places the pill after every call.
    pub fn set(self: Weak<Self>, icon: &str, text: &str) -> f32 {
        self.icon.set_image(icon);
        self.label.set_text(text);

        let text_left = PAD + ICON + GAP;
        let text_width = self.label.content_size().width;
        self.label
            .place()
            .clear()
            .l(text_left)
            .t(0)
            .b(0)
            .w(text_width);
        text_left + text_width + PAD
    }

    /// The orange look with a short note after the text, like `missing`, for
    /// a fact the user has to act on. Returns the width like `set`.
    pub fn set_marked(self: Weak<Self>, icon: &str, text: &str, note: &str) -> f32 {
        self.set_border_color(colors::ACCENT);
        self.set_border_width(1);
        let text_end = self.set(icon, text) - PAD;
        self.note.set_text(note);
        self.note.set_hidden(false);
        let note_width = self.note.content_size().width;
        self.note
            .place()
            .clear()
            .l(text_end + GAP)
            .t(0)
            .b(0)
            .w(note_width);
        text_end + GAP + note_width + PAD
    }
}

/// 10517074 reads as 10.5M and 923029 as 923K. A row is scanned, not read,
/// and the exact count is on the details of the package.
pub fn compact(count: u64) -> String {
    short(count, 1_000_000, "M")
        .or_else(|| short(count, 1_000, "K"))
        .unwrap_or_else(|| count.to_string())
}

fn short(count: u64, unit: u64, suffix: &str) -> Option<String> {
    if count < unit {
        return None;
    }
    let whole = count / unit;
    // Three digits already fill the pill, a decimal adds nothing there.
    if whole >= 100 {
        return Some(format!("{whole}{suffix}"));
    }
    let tenth = count % unit / (unit / 10);
    Some(format!("{whole}.{tenth}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::compact;

    #[test]
    fn shortens_counts() {
        assert_eq!(compact(436), "436");
        assert_eq!(compact(5_757), "5.7K");
        assert_eq!(compact(923_029), "923K");
        assert_eq!(compact(1_000_000), "1.0M");
        assert_eq!(compact(10_517_074), "10.5M");
        assert_eq!(compact(222_051_000), "222M");
    }
}
