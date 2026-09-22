//! Friends. Signed out it is a Google button, on the first sign in it asks
//! for a username, after that it is two tabs: the list of friends and
//! requests, and the search for new people.

use std::collections::BTreeMap;

use anyhow::{Error as AnyError, Result};
use blackforge_api::{Friends, username};
use blackforge_core::{Error, social::client::SocialClient};
use hilen::{
    Event,
    dispatch::after,
    login::{GoogleLogin, GoogleLoginButton},
    refs::{Weak, weak_from_ref},
    store::SessionStore,
    ui::{
        Button, CellRegistry, Container, Label, Question, Setup, TableData, TableView, TextField,
        View, ViewData, view,
    },
};

use crate::{
    backend, social,
    ui::{
        avatar::{self, Avatar},
        colors,
        friend_search::FriendSearch,
        nav_item::NavItem,
        style, toast,
    },
};

const ROW_HEIGHT: f32 = 58.0;
const TAB_WIDTH: f32 = 140.0;
const TAB_HEIGHT: f32 = 36.0;
/// From the top of the tabs to the top of what the open tab shows.
const TAB_CONTENT: f32 = 50.0;
/// The list is asked for again this often while the page is open.
const POLL_SECONDS: f32 = 30.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Loading,
    SignedOut,
    PickName,
    Friends,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Friends,
    Search,
}

#[derive(Clone, Debug)]
enum Row {
    Incoming(String),
    Friend { username: String, in_game: bool },
    Outgoing(String),
}

impl Row {
    fn username(&self) -> &str {
        match self {
            Self::Incoming(username) | Self::Friend { username, .. } | Self::Outgoing(username) => {
                username
            }
        }
    }
}

#[view]
pub struct FriendsPage {
    /// A tap on the mods button of a friend, with the username.
    pub open_friend: Event<String>,

    state: State,
    tab: Tab,
    rows: Vec<Row>,
    pictures: BTreeMap<String, String>,
    polling: bool,

    #[init]
    title: Label,
    subtitle: Label,
    sign_out: Button,
    login: GoogleLoginButton,
    name_field: TextField,
    name_save: Button,
    name_note: Label,
    friends_tab: NavItem,
    search_tab: NavItem,
    table: TableView,
    search: FriendSearch,
}

impl Setup for FriendsPage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Friends");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(600, 16);

        style::ghost(self.sign_out, "Sign out");
        self.sign_out
            .place()
            .t(28)
            .r(style::PAGE_PAD)
            .size(90, style::BUTTON_H);
        self.sign_out.on_tap(move || self.sign_out());

        self.login
            .place()
            .t(style::HEADER + 8.0)
            .l(style::PAGE_PAD)
            .size(240, 40);
        self.login.logged_in.val(move |_| {
            social::share_profile();
            self.load();
        });
        self.login.failed.val(toast::error);

        style::field(self.name_field, "pick a username");
        self.name_field
            .place()
            .t(style::HEADER + 8.0)
            .l(style::PAGE_PAD)
            .size(260, style::FIELD_H);
        style::primary(self.name_save, "Save");
        self.name_save
            .place()
            .t(style::HEADER + 9.0)
            .l(style::PAGE_PAD + 270.0)
            .size(80, style::BUTTON_H);
        self.name_save.on_tap(move || self.ask_name());
        style::dim(self.name_note);
        self.name_note.set_text(
            "Friends find you by this name. Latin letters, digits and the underscore, 3 to 20 characters. It cannot be \
             changed later.",
        );
        self.name_note
            .place()
            .t(style::HEADER + 52.0)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(16);

        self.friends_tab
            .set_content("tab_friends.svg", "My friends");
        self.friends_tab
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .size(TAB_WIDTH, TAB_HEIGHT);
        self.friends_tab
            .tapped
            .sub(move || self.select(Tab::Friends));

        self.search_tab.set_content("tab_search.svg", "Find people");
        self.search_tab
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD + TAB_WIDTH + 8.0)
            .size(TAB_WIDTH, TAB_HEIGHT);
        self.search_tab.tapped.sub(move || self.select(Tab::Search));

        self.table
            .set_data_source(self)
            .register_cell::<FriendCell>();
        style::table(self.table);
        self.table
            .place()
            .t(style::HEADER + TAB_CONTENT)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);

        self.search.set_page(self);
        self.search
            .place()
            .t(style::HEADER + TAB_CONTENT)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);

        self.load();
    }
}

impl FriendsPage {
    fn show(mut self: Weak<Self>, state: State) {
        self.state = state;

        self.login.set_hidden(state != State::SignedOut);
        self.sign_out
            .set_hidden(matches!(state, State::SignedOut | State::Loading));

        let picking = state == State::PickName;
        self.name_field.set_hidden(!picking);
        self.name_save.set_hidden(!picking);
        self.name_note.set_hidden(!picking);

        let friends = state == State::Friends;
        self.friends_tab.set_hidden(!friends);
        self.search_tab.set_hidden(!friends);
        self.friends_tab.set_selected(self.tab == Tab::Friends);
        self.search_tab.set_selected(self.tab == Tab::Search);
        self.table
            .set_hidden(!(friends && self.tab == Tab::Friends));
        self.search
            .set_hidden(!(friends && self.tab == Tab::Search));

        // The friends state writes its own line, it knows the username.
        let text = match state {
            State::Loading => "loading",
            State::SignedOut => "sign in to see what your friends play with",
            State::PickName => "one more step, your username",
            State::Friends => return,
        };
        self.subtitle.set_text(text);
    }

    fn select(mut self: Weak<Self>, tab: Tab) {
        self.tab = tab;
        self.show(self.state);

        // A request sent from the search is a new row here.
        if tab == Tab::Friends {
            self.refresh();
        }
    }

    /// Finds out which of the states the page is in.
    fn load(self: Weak<Self>) {
        if !social::signed_in() {
            return self.show(State::SignedOut);
        }
        self.show(State::Loading);

        self.call(
            "loading your account",
            |client| async move { Ok(client.me().await?) },
            move |me| match me.username {
                Some(name) => self.show_friends(&name),
                None => self.show(State::PickName),
            },
        );
    }

    fn show_friends(mut self: Weak<Self>, name: &str) {
        self.subtitle.set_text(format!("signed in as {name}"));
        self.show(State::Friends);
        self.refresh();

        if !self.polling {
            self.polling = true;
            self.poll();
        }
    }

    /// Runs as long as the page lives, the shell drops the page on a switch.
    fn poll(self: Weak<Self>) {
        after(POLL_SECONDS, move || {
            if !self.is_ok() {
                return;
            }
            if self.state == State::Friends {
                self.refresh();
            }
            self.poll();
        });
    }

    fn refresh(self: Weak<Self>) {
        self.call(
            "loading friends",
            |client| async move { Ok(client.friends().await?) },
            move |friends| self.set_friends(friends),
        );
    }

    fn set_friends(mut self: Weak<Self>, friends: Friends) {
        let incoming = friends.incoming.into_iter().map(Row::Incoming);
        let accepted = friends.friends.into_iter().map(|friend| Row::Friend {
            username: friend.username,
            in_game: friend.in_game,
        });
        let outgoing = friends.outgoing.into_iter().map(Row::Outgoing);

        self.rows = incoming.chain(accepted).chain(outgoing).collect();
        self.pictures = friends.pictures;
        self.table.reload_data();
    }

    fn ask_name(self: Weak<Self>) {
        let name = match username::normalize(self.name_field.text()) {
            Ok(name) => name,
            Err(error) => return toast::error(error.to_string()),
        };

        Question::ask(format!(
            "Your username will be {name}. It cannot be changed later. Save it?"
        ))
        .on_yes(move || {
            let saved = name.clone();
            self.call(
                "saving the username",
                move |client| async move { Ok(client.set_username(&saved).await?) },
                move |me| self.show_friends(&me.username.unwrap_or_default()),
            );
        });
    }

    /// The first button of a row.
    fn primary(self: Weak<Self>, index: usize) {
        match self.rows.get(index).cloned() {
            Some(Row::Incoming(name)) => {
                self.friend_action("accepting", name, |client, name| async move {
                    Ok(client.accept_friend(&name).await?)
                });
            }
            Some(Row::Friend { username, .. }) => self.open_friend.trigger(username),
            Some(Row::Outgoing(_)) | None => {}
        }
    }

    /// The second button of a row. Unfriend asks first, it ends the
    /// friendship for both sides.
    fn secondary(self: Weak<Self>, index: usize) {
        match self.rows.get(index).cloned() {
            Some(Row::Incoming(name)) => {
                self.friend_action("declining", name, |client, name| async move {
                    Ok(client.decline_friend(&name).await?)
                });
            }
            Some(Row::Friend { username, .. }) => {
                Question::ask(format!("Remove {username} from your friends?")).on_yes(move || {
                    self.friend_action(
                        "removing the friend",
                        username.clone(),
                        |client, name| async move { Ok(client.remove_friend(&name).await?) },
                    );
                });
            }
            Some(Row::Outgoing(name)) => {
                self.friend_action("taking the request back", name, |client, name| async move {
                    Ok(client.remove_friend(&name).await?)
                });
            }
            None => {}
        }
    }

    fn friend_action<Fut>(
        self: Weak<Self>,
        title: &str,
        name: String,
        work: impl FnOnce(SocialClient, String) -> Fut + Send + 'static,
    ) where
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.call(
            title,
            move |client| work(client, name),
            move |()| self.refresh(),
        );
    }

    fn sign_out(self: Weak<Self>) {
        GoogleLogin::logout(move |result| {
            if let Err(error) = result {
                toast::failure(&error);
            }
            social::forget_upload();
            if self.is_ok() {
                self.load();
            }
        });
    }

    /// One call to the server. A failure shows as a toast. A session the
    /// server no longer knows is forgotten here too, and the page goes back
    /// to the Google button. The search tab calls through here as well.
    pub(super) fn call<T, Fut>(
        self: Weak<Self>,
        title: &str,
        work: impl FnOnce(SocialClient) -> Fut + Send + 'static,
        done: impl FnOnce(T) + Send + 'static,
    ) where
        T: Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        backend::load(
            title,
            |_, _| async move { work(social::client()?).await },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(value) => done(value),
                    Err(error) if session_ended(&error) => self.forget_session(),
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn forget_session(self: Weak<Self>) {
        if let Err(error) = SessionStore::clear() {
            toast::failure(&error);
        }
        social::forget_upload();
        toast::info("the session ended, sign in again");
        self.show(State::SignedOut);
    }
}

fn session_ended(error: &AnyError) -> bool {
    matches!(error.downcast_ref::<Error>(), Some(Error::NotSignedIn))
}

impl TableData for FriendsPage {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let row = &self.rows[index];
        let picture = self.pictures.get(row.username()).map(String::as_str);

        let cell = registry.cell::<FriendCell>();
        cell.set_row(index, weak_from_ref(self), row, picture);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct FriendCell {
    index: usize,
    page: Weak<FriendsPage>,

    #[init]
    avatar: Avatar,
    name: Label,
    detail: Label,
    primary: Button,
    secondary: Button,
    line: Container,
}

impl Setup for FriendCell {
    fn setup(self: Weak<Self>) {
        self.avatar
            .place()
            .l(6)
            .center_y()
            .size(avatar::SIZE, avatar::SIZE);

        style::body(self.name);
        self.name.place().t(10).l(54).r(220).h(20);

        style::dim(self.detail);
        self.detail.place().t(32).l(54).r(220).h(16);

        // 16 points from the edge, the scroll bar draws over the last few.
        self.secondary.place().r(16).t(13).size(90, style::BUTTON_H);
        self.secondary.on_tap(move || {
            if self.page.is_ok() {
                self.page.secondary(self.index);
            }
        });

        self.primary.place().r(114).t(13).size(90, style::BUTTON_H);
        self.primary.on_tap(move || {
            if self.page.is_ok() {
                self.page.primary(self.index);
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl FriendCell {
    fn set_row(
        mut self: Weak<Self>,
        index: usize,
        page: Weak<FriendsPage>,
        row: &Row,
        picture: Option<&str>,
    ) {
        self.index = index;
        self.page = page;

        self.avatar.show(row.username(), picture);
        self.name.set_text(row.username());

        // The dot on the picture: a request that waits for me, or in game.
        match row {
            Row::Incoming(_) => {
                self.detail.set_text("wants to be your friend");
                self.avatar.set_status(Some(colors::WARN));
                style::primary(self.primary, "Accept");
                style::ghost(self.secondary, "Decline");
                self.primary.set_hidden(false);
            }
            Row::Friend { in_game, .. } => {
                self.detail
                    .set_text(if *in_game { "in game" } else { "not in game" });
                self.avatar.set_status(in_game.then_some(colors::OK));
                style::ghost(self.primary, "Mods");
                style::ghost(self.secondary, "Unfriend");
                self.primary.set_hidden(false);
            }
            Row::Outgoing(_) => {
                self.detail.set_text("request sent, waiting for an answer");
                self.avatar.set_status(None);
                style::ghost(self.secondary, "Cancel");
                self.primary.set_hidden(true);
            }
        }
    }
}
