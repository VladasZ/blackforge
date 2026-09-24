//! Who may join my game servers. One list covers all of them, and I am always
//! let in without a row. A removal stops the next join of that person.
//!
//! The field searches people by the start of their username, like the Friends
//! page. While it holds text the table shows the people found, each with an
//! Add button, an empty field shows the members.

use blackforge_api::{FoundUser, gate::Member, username};
use hilen::{
    OnceEvent,
    dispatch::after,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, ModalView, Setup, Size, TableData, TableView,
        TextAlignment, TextField, UIColor, View, ViewData, view,
    },
};

use crate::{
    backend, social,
    ui::{
        avatar::{self, Avatar},
        colors, style, toast,
    },
};

const PAD: f32 = 24.0;
const ROW_HEIGHT: f32 = 58.0;
const BUTTON_WIDTH: f32 = 90.0;
/// The server is asked this long after the last key, not on every key.
const TYPING_PAUSE: f32 = 0.3;
const NO_MEMBERS: &str = "Nobody but you may join yet. Search above to add someone.";
const NOBODY_FOUND: &str = "No username starts with that.";

#[view]
pub struct MembersModal {
    event: OnceEvent<()>,
    members: Vec<Member>,
    found: Vec<FoundUser>,
    /// The field holds a search, so the table shows the people found.
    searching: bool,
    /// Counts the changes of the field, so an old pause or a slow old answer
    /// cannot replace a newer one.
    typed: u64,

    #[init]
    title: Label,
    hint: Label,
    field: TextField,
    note: Label,
    table: TableView,
    close: Button,
}

impl ModalView<(), ()> for MembersModal {
    fn modal_event(&self) -> &OnceEvent<()> {
        &self.event
    }

    fn modal_size() -> Size {
        (520, 560).into()
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

        style::field(self.field, "Search by username to add someone");
        self.field
            .place()
            .t(PAD + 76.0)
            .l(PAD)
            .r(PAD)
            .h(style::FIELD_H);
        self.field.changed.val(move |text| self.text_changed(&text));

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
            move |result| self.show_members(result),
        );
    }

    fn text_changed(mut self: Weak<Self>, text: &str) {
        self.typed += 1;
        let typed = self.typed;

        // An empty field shows the members, and so does text no username
        // can hold.
        let Ok(Some(start)) = username::normalize_start(text) else {
            self.searching = false;
            self.found.clear();
            return self.refresh();
        };

        after(TYPING_PAUSE, move || {
            if self.is_ok() && self.typed == typed {
                self.find(start, typed);
            }
        });
    }

    fn find(self: Weak<Self>, start: String, typed: u64) {
        backend::load(
            "searching people",
            move |_, _| async move { Ok(social::client()?.search_users(&start).await?) },
            move |result| {
                if !self.is_ok() || self.typed != typed {
                    return;
                }
                match result {
                    Ok(found) => self.show_found(found),
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn show_found(mut self: Weak<Self>, found: Vec<FoundUser>) {
        self.searching = true;
        self.found = found;
        self.refresh();
    }

    fn is_member(&self, name: &str) -> bool {
        self.members.iter().any(|member| member.username == name)
    }

    fn act(self: Weak<Self>, index: usize) {
        if self.searching {
            if let Some(found) = self.found.get(index) {
                self.add(found.username.clone());
            }
        } else if let Some(member) = self.members.get(index) {
            self.remove(member.username.clone());
        }
    }

    fn add(self: Weak<Self>, name: String) {
        backend::load(
            "adding a member",
            |_, _| async move { Ok(social::client()?.add_member(&name).await?) },
            move |result| {
                // Back to the list, which now shows the new member.
                if result.is_ok() && self.is_ok() {
                    self.field.set_text("");
                    self.clear_search();
                }
                self.show_members(result);
            },
        );
    }

    fn clear_search(mut self: Weak<Self>) {
        self.typed += 1;
        self.searching = false;
        self.found.clear();
    }

    fn remove(self: Weak<Self>, name: String) {
        backend::load(
            "removing a member",
            |_, _| async move { Ok(social::client()?.remove_member(&name).await?) },
            move |result| self.show_members(result),
        );
    }

    fn show_members(mut self: Weak<Self>, result: anyhow::Result<Vec<Member>>) {
        if !self.is_ok() {
            return;
        }
        match result {
            Ok(members) => {
                self.members = members;
                self.refresh();
            }
            Err(error) => toast::failure(&error),
        }
    }

    fn refresh(mut self: Weak<Self>) {
        let empty = self.number_of_cells() == 0;
        self.note.set_text(if self.searching {
            NOBODY_FOUND
        } else {
            NO_MEMBERS
        });
        self.note.set_hidden(!empty);
        self.table.reload_data();
    }
}

impl TableData for MembersModal {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        if self.searching {
            self.found.len()
        } else {
            self.members.len()
        }
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<MemberCell>();
        let owner = weak_from_ref(self);
        if self.searching {
            let found = &self.found[index];
            cell.set_row(
                index,
                owner,
                &found.username,
                found.picture.as_deref(),
                if self.is_member(&found.username) {
                    Action::AlreadyMember
                } else {
                    Action::Add
                },
            );
        } else {
            let member = &self.members[index];
            cell.set_row(
                index,
                owner,
                &member.username,
                member.picture.as_deref(),
                Action::Remove,
            );
        }
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

/// What the button of a row does.
#[derive(Clone, Copy)]
enum Action {
    Add,
    AlreadyMember,
    Remove,
}

#[view]
struct MemberCell {
    index: usize,
    owner: Weak<MembersModal>,

    #[init]
    avatar: Avatar,
    name: Label,
    detail: Label,
    action: Button,
    line: Container,
}

impl Setup for MemberCell {
    fn setup(self: Weak<Self>) {
        self.avatar
            .place()
            .l(6)
            .center_y()
            .size(avatar::SIZE, avatar::SIZE);

        style::body(self.name);
        self.name.set_ellipsize(true);
        self.name.place().t(10).l(54).r(BUTTON_WIDTH + 24.0).h(20);

        style::dim(self.detail);
        self.detail.place().t(32).l(54).r(BUTTON_WIDTH + 24.0).h(16);

        self.action
            .place()
            .r(4)
            .t(13)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.action.on_tap(move || {
            if self.owner.is_ok() {
                self.owner.act(self.index);
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl MemberCell {
    fn set_row(
        mut self: Weak<Self>,
        index: usize,
        owner: Weak<MembersModal>,
        name: &str,
        picture: Option<&str>,
        action: Action,
    ) {
        self.index = index;
        self.owner = owner;
        self.avatar.show(name, picture);
        self.name.set_text(name);
        self.action
            .set_hidden(matches!(action, Action::AlreadyMember));
        match action {
            Action::Add => {
                self.detail.set_text("");
                style::primary(self.action, "Add");
                // A reused row may come from Remove, which has a border.
                self.action.set_border_width(0);
            }
            Action::AlreadyMember => {
                self.detail.set_text("Already a member");
            }
            Action::Remove => {
                self.detail.set_text("Member");
                style::danger(self.action, "Remove");
            }
        }
    }
}
