//! The "Find people" tab of the Friends page: a search by the start of a
//! username, and one button per found person to ask, accept or take a request
//! back.

use anyhow::Result;
use blackforge_api::{FoundUser, Relation, username};
use blackforge_core::social::client::SocialClient;
use hilen::{
    dispatch::after,
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, Setup, TableData, TableView, TextField, View,
        ViewData, view,
    },
};

use crate::ui::{
    avatar::{self, Avatar},
    colors,
    friends_page::FriendsPage,
    style,
};

const ROW_HEIGHT: f32 = 58.0;
const FIELD_WIDTH: f32 = 320.0;
/// The server is asked this long after the last key, not on every key.
const TYPING_PAUSE: f32 = 0.3;

#[view]
pub struct FriendSearch {
    page: Weak<FriendsPage>,
    found: Vec<FoundUser>,
    /// Counts the changes of the field, so an old pause or a slow old answer
    /// cannot replace a newer one.
    typed: u64,

    #[init]
    field: TextField,
    table: TableView,
}

impl Setup for FriendSearch {
    fn setup(self: Weak<Self>) {
        style::field(self.field, "Search by username");
        self.field
            .place()
            .t(0)
            .l(0)
            .size(FIELD_WIDTH, style::FIELD_H);
        self.field.changed.val(move |text| self.text_changed(&text));

        self.table
            .set_data_source(self)
            .register_cell::<FoundCell>();
        style::table(self.table);
        self.table.place().t(style::FIELD_H + 16.0).l(0).r(0).b(0);
    }
}

impl FriendSearch {
    /// The page owns the calls to the server, it knows what to do when the
    /// session ended.
    pub fn set_page(mut self: Weak<Self>, page: Weak<FriendsPage>) {
        self.page = page;
    }

    fn text_changed(mut self: Weak<Self>, text: &str) {
        self.typed += 1;
        let typed = self.typed;

        // An empty field finds nobody, and so does text no username can hold.
        let Ok(Some(start)) = username::normalize_start(text) else {
            return self.show_found(Vec::new());
        };

        after(TYPING_PAUSE, move || {
            if self.is_ok() && self.typed == typed {
                self.find(start, typed);
            }
        });
    }

    fn find(self: Weak<Self>, start: String, typed: u64) {
        self.page.call(
            "searching people",
            move |client| async move { Ok(client.search_users(&start).await?) },
            move |found: Vec<FoundUser>| {
                if self.typed == typed {
                    self.show_found(found);
                }
            },
        );
    }

    /// Asks again for what the field holds now, a request changed what the
    /// found people are to me.
    fn find_again(mut self: Weak<Self>) {
        if let Ok(Some(start)) = username::normalize_start(self.field.text()) {
            self.typed += 1;
            self.find(start, self.typed);
        }
    }

    fn show_found(mut self: Weak<Self>, found: Vec<FoundUser>) {
        self.found = found;
        self.table.reload_data();
    }

    fn act(self: Weak<Self>, index: usize) {
        let Some(found) = self.found.get(index).cloned() else {
            return;
        };
        let name = found.username;

        match found.relation {
            Relation::Stranger => {
                self.request("sending the request", name, |client, name| async move {
                    Ok(client.request_friend(&name).await?)
                });
            }
            Relation::AskedMe => {
                self.request("accepting", name, |client, name| async move {
                    Ok(client.accept_friend(&name).await?)
                });
            }
            Relation::Asked => {
                self.request("taking the request back", name, |client, name| async move {
                    Ok(client.remove_friend(&name).await?)
                });
            }
            Relation::Friend => {}
        }
    }

    fn request<Fut>(
        self: Weak<Self>,
        title: &str,
        name: String,
        work: impl FnOnce(SocialClient, String) -> Fut + Send + 'static,
    ) where
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.page.call(
            title,
            move |client| work(client, name),
            move |()| self.find_again(),
        );
    }
}

impl TableData for FriendSearch {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.found.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<FoundCell>();
        cell.set_row(index, weak_from_ref(self), &self.found[index]);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct FoundCell {
    index: usize,
    search: Weak<FriendSearch>,

    #[init]
    avatar: Avatar,
    name: Label,
    detail: Label,
    action: Button,
    line: Container,
}

impl Setup for FoundCell {
    fn setup(self: Weak<Self>) {
        self.avatar
            .place()
            .l(6)
            .center_y()
            .size(avatar::SIZE, avatar::SIZE);

        style::body(self.name);
        self.name.place().t(10).l(54).r(130).h(20);

        style::dim(self.detail);
        self.detail.place().t(32).l(54).r(130).h(16);

        // 16 points from the edge, the scroll bar draws over the last few.
        self.action.place().r(16).t(13).size(90, style::BUTTON_H);
        self.action.on_tap(move || {
            if self.search.is_ok() {
                self.search.act(self.index);
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl FoundCell {
    fn set_row(mut self: Weak<Self>, index: usize, search: Weak<FriendSearch>, found: &FoundUser) {
        self.index = index;
        self.search = search;

        self.avatar.show(&found.username, found.picture.as_deref());
        self.name.set_text(&found.username);
        self.action.set_hidden(found.relation == Relation::Friend);

        match found.relation {
            Relation::Stranger => {
                self.detail.set_text("");
                self.primary_action("Add");
            }
            Relation::AskedMe => {
                self.detail.set_text("Wants to be your friend");
                self.primary_action("Accept");
            }
            Relation::Asked => {
                self.detail.set_text("Request sent, waiting for an answer");
                style::ghost(self.action, "Cancel");
            }
            Relation::Friend => {
                self.detail.set_text("Your friend");
            }
        }
    }

    /// A reused row may come from "cancel", which has a border.
    fn primary_action(self: Weak<Self>, text: &str) {
        style::primary(self.action, text);
        self.action.set_border_width(0);
    }
}
