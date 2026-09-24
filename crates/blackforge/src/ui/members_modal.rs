//! Who may join my game servers. One list covers all of them, and I am always
//! let in without a row. A removal stops the next join of that person.

use blackforge_api::gate::Member;
use hilen::{
    OnceEvent,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, ModalView, Setup, Size, TableData, TableView,
        TextAlignment, TextField, UIColor, View, ViewData, view,
    },
};

use crate::{
    backend, social,
    ui::{colors, style, toast},
};

const PAD: f32 = 24.0;
const ROW_HEIGHT: f32 = 44.0;
const BUTTON_WIDTH: f32 = 90.0;

#[view]
pub struct MembersModal {
    event: OnceEvent<()>,
    members: Vec<Member>,

    #[init]
    title: Label,
    hint: Label,
    username: TextField,
    add: Button,
    note: Label,
    table: TableView,
    close: Button,
}

impl ModalView<(), ()> for MembersModal {
    fn modal_event(&self) -> &OnceEvent<()> {
        &self.event
    }

    fn modal_size() -> Size {
        (520, 520).into()
    }

    fn modal_scrim_color() -> UIColor {
        colors::SCRIM.into()
    }

    fn setup_input(self: Weak<Self>, (): ()) {
        self.load();
    }
}

impl Setup for MembersModal {
    fn setup(self: Weak<Self>) {
        style::card(self);

        style::title(self.title, "Members");
        self.title.set_text_size(18);
        self.title.place().t(PAD).l(PAD).r(PAD).h(24);

        style::dim(self.hint);
        self.hint.set_multiline(true);
        self.hint.set_text(
            "The people who may join all of your servers. There is no password, blackforge lets them in. You are always let in.",
        );
        self.hint.place().t(PAD + 30.0).l(PAD).r(PAD).h(34);

        style::field(self.username, "Username");
        self.username
            .place()
            .t(PAD + 76.0)
            .l(PAD)
            .r(PAD + BUTTON_WIDTH + 8.0)
            .h(style::FIELD_H);

        style::primary(self.add, "Add");
        self.add
            .place()
            .t(PAD + 76.0)
            .r(PAD)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.add.on_tap(move || self.add());

        style::dim(self.note);
        self.note.set_alignment(TextAlignment::Center);
        self.note.set_text("Loading");
        self.note.place().t(PAD + 180.0).l(PAD).r(PAD).h(20);

        self.table
            .set_data_source(self)
            .register_cell::<MemberCell>();
        self.table
            .place()
            .t(PAD + 124.0)
            .l(PAD)
            .r(PAD)
            .b(PAD + 48.0);

        style::ghost(self.close, "Close");
        self.close
            .place()
            .r(PAD)
            .b(PAD)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.close.on_tap(move || self.hide_modal(()));
    }
}

impl MembersModal {
    fn load(self: Weak<Self>) {
        backend::load(
            "loading the members",
            |_, _| async move { Ok(social::client()?.members().await?) },
            move |result| self.show(result),
        );
    }

    fn add(self: Weak<Self>) {
        let username = self.username.text().trim().to_owned();
        if username.is_empty() {
            return toast::error("type the username of who may join");
        }
        backend::load(
            "adding a member",
            |_, _| async move { Ok(social::client()?.add_member(&username).await?) },
            move |result| {
                if result.is_ok() && self.is_ok() {
                    self.username.set_text("");
                }
                self.show(result);
            },
        );
    }

    fn remove(self: Weak<Self>, index: usize) {
        let Some(member) = self.members.get(index) else {
            return;
        };
        let username = member.username.clone();
        backend::load(
            "removing a member",
            |_, _| async move { Ok(social::client()?.remove_member(&username).await?) },
            move |result| self.show(result),
        );
    }

    fn show(mut self: Weak<Self>, result: anyhow::Result<Vec<Member>>) {
        if !self.is_ok() {
            return;
        }
        match result {
            Ok(members) => {
                self.note.set_text("Nobody but you may join yet.");
                self.note.set_hidden(!members.is_empty());
                self.members = members;
                self.table.reload_data();
            }
            Err(error) => toast::failure(&error),
        }
    }
}

impl TableData for MembersModal {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.members.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<MemberCell>();
        cell.set_row(index, weak_from_ref(self), &self.members[index]);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct MemberCell {
    index: usize,
    owner: Weak<MembersModal>,

    #[init]
    name: Label,
    remove: Button,
    line: Container,
}

impl Setup for MemberCell {
    fn setup(self: Weak<Self>) {
        style::body(self.name);
        self.name.set_ellipsize(true);
        self.name.place().t(12).l(4).r(BUTTON_WIDTH + 16.0).h(20);

        style::danger(self.remove, "Remove");
        self.remove
            .place()
            .r(4)
            .t(6)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.remove.on_tap(move || {
            if self.owner.is_ok() {
                self.owner.remove(self.index);
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl MemberCell {
    fn set_row(mut self: Weak<Self>, index: usize, owner: Weak<MembersModal>, member: &Member) {
        self.index = index;
        self.owner = owner;
        self.name.set_text(&member.username);
    }
}
