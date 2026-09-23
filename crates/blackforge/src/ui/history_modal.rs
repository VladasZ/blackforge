//! Every setup the account ever saved, newest first. Restoring one makes it
//! the newest setup again, nothing leaves the list.

use blackforge_api::setup::HistoryRow;
use hilen::{
    OnceEvent,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, ModalView, Question, Setup, Size, TableData,
        TableView, TextAlignment, UIColor, View, ViewData, ViewTooltip, view,
    },
};

use crate::{
    backend, cloud,
    ui::{colors, style, time, toast},
};

const PAD: f32 = 24.0;
const ROW_HEIGHT: f32 = 58.0;
const RESTORE_W: f32 = 90.0;

#[view]
pub struct HistoryModal {
    /// True when a setup was restored.
    event: OnceEvent<bool>,
    rows: Vec<HistoryRow>,
    /// The revision every machine has installed now, it cannot be restored.
    head: i64,

    #[init]
    title: Label,
    subtitle: Label,
    note: Label,
    table: TableView,
    close: Button,
}

impl ModalView<(), bool> for HistoryModal {
    fn modal_event(&self) -> &OnceEvent<bool> {
        &self.event
    }

    fn modal_size() -> Size {
        (680, 560).into()
    }

    fn modal_scrim_color() -> UIColor {
        colors::SCRIM.into()
    }
}

impl Setup for HistoryModal {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::title(self.title, "Sync history");
        self.title.set_text_size(18);
        self.title.place().t(PAD).l(PAD).r(PAD).h(24);

        style::dim(self.subtitle);
        self.subtitle.set_text(
            "Restoring a setup makes it the newest one on every machine. Nothing is removed from this list.",
        );
        self.subtitle.place().t(PAD + 28.0).l(PAD).r(PAD).h(16);

        style::dim(self.note);
        self.note.set_alignment(TextAlignment::Center);
        self.note.set_text("Reading the history...");
        self.note.place().t(PAD + 120.0).l(PAD).r(PAD).h(20);

        self.table
            .set_data_source(self)
            .register_cell::<HistoryCell>();
        self.table.place().t(PAD + 60.0).l(PAD).r(PAD).b(PAD + 48.0);

        style::ghost(self.close, "Close");
        self.close.place().r(PAD).b(PAD).size(90, style::BUTTON_H);
        self.close.on_tap(move || self.hide_modal(false));

        self.load();
    }
}

impl HistoryModal {
    fn load(mut self: Weak<Self>) {
        backend::load(
            "reading the sync history",
            |_, _| cloud::history(),
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(rows) => {
                        self.head = rows
                            .iter()
                            .find(|row| row.applied)
                            .map_or(0, |row| row.revision);
                        self.note.set_text("Nothing is saved yet.");
                        self.note.set_hidden(!rows.is_empty());
                        self.rows = rows;
                        self.table.reload_data();
                    }
                    Err(error) => {
                        self.note.set_text("The history could not be read.");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn ask_restore(self: Weak<Self>, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let revision = row.revision;
        Question::ask(format!(
            "Restore revision {revision}, saved {}? It becomes the newest setup on every machine.",
            time::ago(row.created)
        ))
        .on_yes(move || self.restore(revision));
    }

    fn restore(self: Weak<Self>, revision: i64) {
        backend::load(
            "restoring the setup",
            move |_, _| cloud::rollback(revision),
            move |result| match result {
                Ok(()) => {
                    toast::success(format!("Revision {revision} is restored, it installs now."));
                    cloud::schedule();
                    if self.is_ok() {
                        self.hide_modal(true);
                    }
                }
                Err(error) => toast::failure(&error),
            },
        );
    }
}

impl TableData for HistoryModal {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<HistoryCell>();
        cell.set_row(index, weak_from_ref(self), &self.rows[index], self.head);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct HistoryCell {
    index: usize,
    owner: Weak<HistoryModal>,

    #[init]
    when: Label,
    detail: Label,
    current: Label,
    restore: Button,
    line: Container,
}

impl Setup for HistoryCell {
    fn setup(self: Weak<Self>) {
        style::body(self.when);
        self.when.set_ellipsize(true);
        self.when
            .place()
            .t(10)
            .l(4)
            .r(16.0 + RESTORE_W + 12.0)
            .h(20);

        style::dim(self.detail);
        self.detail.set_ellipsize(true);
        self.detail
            .place()
            .t(32)
            .l(4)
            .r(16.0 + RESTORE_W + 12.0)
            .h(16);

        style::dim(self.current);
        self.current.set_alignment(TextAlignment::Right);
        self.current.set_text("Installed now");
        self.current.place().r(16).t(20).size(RESTORE_W, 16);

        style::ghost(self.restore, "Restore");
        self.restore
            .place()
            .r(16)
            .t(13)
            .size(RESTORE_W, style::BUTTON_H);
        self.restore.on_tap(move || {
            if self.owner.is_ok() {
                self.owner.ask_restore(self.index);
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl HistoryCell {
    fn set_row(
        mut self: Weak<Self>,
        index: usize,
        owner: Weak<HistoryModal>,
        row: &HistoryRow,
        head: i64,
    ) {
        self.index = index;
        self.owner = owner;

        self.when.set_text(format!(
            "Revision {}, {}, {}",
            row.revision,
            time::ago(row.created),
            cloud::machine_label(&row.machine)
        ));
        self.when.set_tooltip(time::full(row.created));
        self.detail
            .set_text(match (row.restored_from, row.applied) {
                (Some(from), _) => format!("Restored revision {from}: {}", row.summary),
                (None, false) => format!(
                    "Kept from a conflict, never installed by itself: {}",
                    row.summary
                ),
                (None, true) => row.summary.to_string(),
            });
        let is_head = row.revision == head;
        self.current.set_hidden(!is_head);
        self.restore.set_hidden(is_head);
    }
}
