//! The `.dl-menu` popover from studio's `AppView.vue`. Windows shows one row
//! of architectures, Linux shows a labelled group per architecture plus the
//! apt hint. Rebuilt on every open, so it never holds stale file names.

use hilen::{
    refs::Weak,
    ui::{
        Button, Container, Label, Setup, Shadow, TextAlignment, UIEvent, ViewData, ViewFrame,
        ViewSubviews, view,
    },
};

use crate::{
    fonts,
    model::Choice,
    ui::{colors, text_margin},
};

const PAD: f32 = 5.6;
const GAP: f32 = 3.2;
const GROUP_GAP: f32 = 5.6;
const HEAD_HEIGHT: f32 = 14.0;
const ITEM_HEIGHT: f32 = 33.0;
const ITEM_GAP: f32 = 4.8;

/// How far forward the open menu is pushed. The engine steps depth by 1e-5 per
/// nesting level, so ten steps clear every sibling of this menu and everything
/// nested inside them, while staying far behind the modal layer at 0.4.
const RAISE: f32 = 0.000_1;

#[view]
pub struct DropMenu {
    /// The picked file name and the architecture it belongs to.
    pub on_pick: UIEvent<(String, String)>,

    raised: bool,
}

impl Setup for DropMenu {
    fn setup(self: Weak<Self>) {
        self.set_color(colors::CARD_BG);
        self.set_border_color(colors::BORDER);
        self.set_border_width(1);
        self.set_corner_radius(10);
        self.set_shadow(Shadow {
            offset: (0, 12).into(),
            radius: 30.0,
            color: colors::MENU_SHADOW,
        });
    }
}

impl DropMenu {
    /// Builds the menu and returns the height it needs. `heads` draws the
    /// per-architecture captions, which only the Linux menu has. `hint` adds
    /// the apt note under the items.
    pub fn set_groups(
        mut self: Weak<Self>,
        groups: &[(String, Vec<Choice>)],
        width: f32,
        heads: bool,
        hint: Option<&str>,
    ) -> f32 {
        // A subview sits in front of its parent, so a plain later sibling still
        // draws behind the labels of an earlier one. Without this the menu
        // opens under the text of the button below it and under the cards. One
        // bump is enough, every item added after it inherits the raised depth.
        if !self.raised {
            self.bump_z_position(RAISE);
            self.raised = true;
        }

        self.remove_all_subviews();

        let inner = width - PAD * 2.0;
        let mut y = PAD;

        for (index, (arch, choices)) in groups.iter().filter(|(_, c)| !c.is_empty()).enumerate() {
            if index > 0 {
                y += GROUP_GAP;
            }

            if heads {
                let head = self.add_view::<Label>();
                head.set_color(colors::CLEAR);
                head.set_alignment(TextAlignment::Left);
                head.set_font(fonts::mono(600.0));
                head.set_text_size(10.9);
                head.set_text_color(colors::FG_DIM);
                head.set_letter_spacing(0.87);
                head.set_text(arch.to_uppercase());
                head.set_frame((PAD + 8.8 - text_margin(), y, inner, HEAD_HEIGHT));
                y += HEAD_HEIGHT + GAP;
            }

            let count = choices.len() as f32;
            let item_width = (inner - ITEM_GAP * (count - 1.0)) / count;

            for (column, choice) in choices.iter().enumerate() {
                let item = self.add_view::<Button>();
                item.set_text(&choice.label);
                item.set_font(fonts::mono(400.0));
                item.set_text_size(13.6);
                item.set_text_color(colors::FG);
                item.set_color(colors::CLEAR);
                item.set_corner_radius(6);
                item.set_frame((
                    PAD + column as f32 * (item_width + ITEM_GAP),
                    y,
                    item_width,
                    ITEM_HEIGHT,
                ));

                let file = choice.file.clone();
                let arch = choice.arch.clone();
                item.on_tap(move || self.on_pick.trigger((file.clone(), arch.clone())));
            }

            y += ITEM_HEIGHT;
        }

        if let Some(hint) = hint {
            y += 6.4;

            let line = self.add_view::<Container>();
            line.set_color(colors::BORDER);
            line.set_frame((0.0, y, width, 1.0));
            y += 1.0 + 8.8;

            let note = self.add_view::<Label>();
            note.set_color(colors::CLEAR);
            note.set_alignment(TextAlignment::Left);
            note.set_font(fonts::inter(400.0));
            note.set_text_size(11.5);
            note.set_text_color(colors::FG_DIM);
            note.set_multiline(true);
            note.set_text(
                "Ubuntu 23.10+ can't install .deb files from the App Center. Use the terminal:",
            );

            let note_width = width - 12.8 * 2.0;
            let note_height = note.size_for_width(note_width).height;
            note.set_frame((12.8 - text_margin(), y, note_width, note_height));
            y += note_height + 6.4;

            let code = self.add_view::<Label>();
            code.set_alignment(TextAlignment::Left);
            code.set_font(fonts::mono(400.0));
            code.set_text_size(11.5);
            code.set_text_color(colors::FG);
            code.set_color(colors::BORDER_SOFT);
            code.set_corner_radius(5);
            code.set_text(format!("  {hint}"));
            code.set_frame((12.8, y, note_width, 24.0));
            y += 24.0 + 8.8;
        } else {
            y += PAD;
        }

        y
    }
}
