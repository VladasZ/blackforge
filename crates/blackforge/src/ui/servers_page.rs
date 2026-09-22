//! Registered game servers, every one with the mods it requires. A player
//! installs what their profile lacks with one button, an owner registers,
//! edits or removes their own servers. The address and the password of a
//! server are no part of this, players join through the game as before.

use blackforge_api::servers::Server;
use blackforge_core::servers::{Needs, ServerInstall, fetch, needs};
use hilen::{
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, Label, ModalView, Question, Setup, TableData, TableView,
        TextAlignment, View, ViewData, view,
    },
};

use crate::{
    backend, cloud, social,
    ui::{colors, server_modal::ServerModal, style, toast},
};

const ROW_HEIGHT: f32 = 80.0;
const BUTTON_WIDTH: f32 = 90.0;
const GAP: f32 = 8.0;
/// The text of a row ends where the three buttons start.
const TEXT_RIGHT: f32 = 16.0 + 3.0 * BUTTON_WIDTH + 2.0 * GAP + 16.0;

#[derive(Clone, Debug)]
struct ServerRow {
    server: Server,
    needs: Needs,
    /// Registered under my username, so the row gets edit and remove.
    mine: bool,
}

#[view]
pub struct ServersPage {
    rows: Vec<ServerRow>,

    #[init]
    title: Label,
    subtitle: Label,
    register: Button,
    note: Label,
    table: TableView,
}

impl Setup for ServersPage {
    fn setup(self: Weak<Self>) {
        style::title(self.title, "Servers");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle
            .set_text("the mods a server needs, installed with one click");
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(600, 16);

        style::primary(self.register, "register a server");
        self.register
            .place()
            .t(28)
            .r(style::PAGE_PAD)
            .size(150, style::BUTTON_H);
        self.register.on_tap(move || self.register());

        style::dim(self.note);
        self.note.set_alignment(TextAlignment::Center);
        self.note
            .place()
            .t(style::HEADER + 40.0)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .h(20);

        self.table
            .set_data_source(self)
            .register_cell::<ServerCell>();
        style::table(self.table);
        self.table
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);

        self.refresh();
    }
}

impl ServersPage {
    fn refresh(self: Weak<Self>) {
        self.note.set_text("loading");
        self.note.set_hidden(false);

        backend::load(
            "loading the servers",
            |forge, progress| async move {
                let servers = fetch(forge.client()).await?;
                // The list is public, the username only marks my own rows.
                // Signed out, or a failed lookup, marks none.
                let me = match social::client() {
                    Ok(client) => client.me().await.ok().and_then(|me| me.username),
                    Err(_) => None,
                };
                let profile = backend::profile(forge, &progress).await?;
                let manifest = profile.manifest().await?;
                let lock = profile.lock().await?;
                let rows = servers
                    .into_iter()
                    .map(|server| ServerRow {
                        needs: needs(&manifest, &lock, &server.mods),
                        mine: me.as_deref() == Some(server.owner.as_str()),
                        server,
                    })
                    .collect();
                Ok(rows)
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(rows) => self.set_rows(rows),
                    Err(error) => {
                        self.note.set_text("the servers did not load");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn set_rows(mut self: Weak<Self>, rows: Vec<ServerRow>) {
        self.note
            .set_text("No server is registered yet. Register yours with the button above.");
        self.note.set_hidden(!rows.is_empty());
        self.rows = rows;
        self.table.reload_data();
    }

    fn register(self: Weak<Self>) {
        if !social::signed_in() {
            return toast::info("sign in on the Friends page to register a server");
        }
        ServerModal::show_modally_with_input(None, move |saved| {
            if saved && self.is_ok() {
                self.refresh();
            }
        });
    }

    fn edit(self: Weak<Self>, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        ServerModal::show_modally_with_input(Some(row.server.clone()), move |saved| {
            if saved && self.is_ok() {
                self.refresh();
            }
        });
    }

    fn install(self: Weak<Self>, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let server = row.server.clone();
        backend::change(
            &format!("installing the mods of {}", server.name),
            move |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let done = forge
                    .install_server(&profile, &server.mods, &progress)
                    .await?;
                Ok(format!("{}: {}", server.name, install_summary(&done)))
            },
            move |result| {
                match result {
                    Ok(text) => toast::success(text),
                    Err(error) => toast::failure(&error),
                }
                if self.is_ok() {
                    self.refresh();
                }
            },
        );
    }

    fn ask_delete(self: Weak<Self>, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let id = row.server.id.clone();
        let name = row.server.name.clone();
        Question::ask(format!(
            "Remove {name} from the list? The mods of nobody change."
        ))
        .on_yes(move || self.delete(id, name));
    }

    fn delete(self: Weak<Self>, id: String, name: String) {
        backend::load(
            "removing the server",
            |_, _| async move {
                social::client()?.delete_server(&id).await?;
                Ok(())
            },
            move |result| {
                match result {
                    Ok(()) => toast::success(format!("{name} is removed")),
                    Err(error) => toast::failure(&error),
                }
                if self.is_ok() {
                    self.refresh();
                }
            },
        );
    }
}

/// What the install did, for the toast.
fn install_summary(done: &ServerInstall) -> String {
    let mut parts = Vec::new();
    if !done.needs.missing.is_empty() {
        parts.push(format!("{} mods added", done.needs.missing.len()));
    }
    if !done.needs.other_version.is_empty() {
        parts.push(format!(
            "{} moved to the server version",
            done.needs.other_version.len()
        ));
    }
    if !done.needs.disabled.is_empty() {
        parts.push(format!("{} enabled", done.needs.disabled.len()));
    }
    if parts.is_empty() {
        return "you had everything already".to_owned();
    }
    parts.push(format!("{} files installed", done.sync.installed.len()));
    parts.join(", ")
}

/// `Owner-Name` without the owner, the way a player knows the mod.
fn short_name(id: &str) -> &str {
    id.split_once('-').map_or(id, |(_, name)| name)
}

fn mods_line(server: &Server) -> String {
    server
        .mods
        .iter()
        .map(|server_mod| format!("{} {}", short_name(&server_mod.id), server_mod.version))
        .collect::<Vec<_>>()
        .join(", ")
}

impl TableData for ServersPage {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<ServerCell>();
        cell.set_row(index, weak_from_ref(self), &self.rows[index]);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct ServerCell {
    index: usize,
    page: Weak<ServersPage>,

    #[init]
    name: Label,
    owner: Label,
    mods: Label,
    ready: Label,
    install: Button,
    edit: Button,
    delete: Button,
    line: Container,
}

impl Setup for ServerCell {
    fn setup(self: Weak<Self>) {
        style::body(self.name);
        self.name.set_text_size(16);
        self.name.set_ellipsize(true);
        self.name.place().t(12).l(4).r(TEXT_RIGHT).h(22);

        style::dim(self.owner);
        self.owner.set_ellipsize(true);
        self.owner.place().t(36).l(4).r(TEXT_RIGHT).h(16);

        style::dim(self.mods);
        self.mods.set_text_color(colors::FG);
        self.mods.set_ellipsize(true);
        self.mods.place().t(54).l(4).r(TEXT_RIGHT).h(16);

        // The same rectangle as the install button, so the column reads as one.
        self.ready.set_text("you have all");
        self.ready.set_text_size(13).set_text_color(colors::OK);
        self.ready.set_alignment(TextAlignment::Center);
        self.ready.set_color(colors::OK_BG);
        self.ready.set_corner_radius(7);
        self.ready
            .place()
            .r(16)
            .t(24)
            .size(BUTTON_WIDTH, style::BUTTON_H);

        style::primary(self.install, "install");
        self.install
            .place()
            .r(16)
            .t(24)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.install.on_tap(move || {
            if self.page.is_ok() {
                self.page.install(self.index);
            }
        });

        style::ghost(self.edit, "edit");
        self.edit
            .place()
            .r(16.0 + BUTTON_WIDTH + GAP)
            .t(24)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.edit.on_tap(move || {
            if self.page.is_ok() {
                self.page.edit(self.index);
            }
        });

        style::danger(self.delete, "remove");
        self.delete
            .place()
            .r(16.0 + 2.0 * (BUTTON_WIDTH + GAP))
            .t(24)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.delete.on_tap(move || {
            if self.page.is_ok() {
                self.page.ask_delete(self.index);
            }
        });

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl ServerCell {
    fn set_row(mut self: Weak<Self>, index: usize, page: Weak<ServersPage>, row: &ServerRow) {
        self.index = index;
        self.page = page;

        let server = &row.server;
        self.name.set_text(&server.name);
        self.owner.set_text(format!(
            "by {}, {} mods, updated {}",
            server.owner,
            server.mods.len(),
            cloud::when(server.updated)
        ));
        self.mods.set_text(mods_line(server));

        let ready = row.needs.is_empty();
        self.ready.set_hidden(!ready);
        self.install.set_hidden(ready);
        self.install.set_text(match row.needs.count() {
            1 => "install 1 mod".to_owned(),
            count => format!("install {count} mods"),
        });
        self.edit.set_hidden(!row.mine);
        self.delete.set_hidden(!row.mine);
    }
}
