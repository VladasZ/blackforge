//! Registered game servers, every one with the mods it requires. A player
//! installs what their profile lacks with one button, an owner registers,
//! edits or removes their own servers. The address and the password of a
//! server are no part of this, players join through the game as before.

use blackforge_api::servers::{JOIN_ADMIN, Server, ServerMod};
use blackforge_core::servers::{Needs, ServerInstall, fetch, needs};
use hilen::{
    refs::{Weak, weak_from_ref},
    ui::{
        Button, CellRegistry, Container, ImageView, Label, ModalView, Question, Setup, TableData,
        TableView, TextAlignment, View, ViewData, ViewFrame, ViewSubviews, ViewTooltip, view,
    },
};

use crate::{
    backend, social,
    ui::{
        busy, colors, names,
        pill::{self, Pill},
        server_modal::ServerModal,
        style, time, toast,
    },
};

/// Room for 2 rows of mod pills under the name and the owner line.
const ROW_HEIGHT: f32 = 58.0 + 2.0 * pill::HEIGHT + PILL_GAP + 14.0;
const BUTTON_WIDTH: f32 = 90.0;
/// Fits "Install 12 mods".
const INSTALL_WIDTH: f32 = 124.0;
const PILL_GAP: f32 = 6.0;
const MODS_T: f32 = 58.0;
/// Room kept for the "+12 more" pill.
const MORE_WIDTH: f32 = 80.0;
const GAP: f32 = 8.0;
/// The text of a row ends where the three buttons start.
const TEXT_RIGHT: f32 = 16.0 + INSTALL_WIDTH + 2.0 * BUTTON_WIDTH + 2.0 * GAP + 16.0;

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
    /// Signed in as `JOIN_ADMIN`, so the form offers the join address.
    admin: bool,

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
            .set_text("The mods a server needs, installed with one click");
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(600, 16);

        style::primary(self.register, "Register a server");
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
        self.note.set_text("Loading");
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
                Ok((rows, me.as_deref() == Some(JOIN_ADMIN)))
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok((rows, admin)) => self.set_rows(rows, admin),
                    Err(error) => {
                        self.note.set_text("The servers did not load");
                        toast::failure(&error);
                    }
                }
            },
        );
    }

    fn set_rows(mut self: Weak<Self>, rows: Vec<ServerRow>, admin: bool) {
        self.admin = admin;
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
        ServerModal::show_modally_with_input((None, self.admin), move |saved| {
            if saved && self.is_ok() {
                self.refresh();
            }
        });
    }

    fn edit(self: Weak<Self>, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        ServerModal::show_modally_with_input(
            (Some(row.server.clone()), self.admin),
            move |saved| {
                if saved && self.is_ok() {
                    self.refresh();
                }
            },
        );
    }

    fn install(self: Weak<Self>, index: usize, button: Weak<Button>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let server = row.server.clone();
        busy::press(button, "Installing...");
        backend::change(
            &format!("installing the mods of {}", server.name),
            move |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let done = forge
                    .install_server(&profile, &server.name, &server.mods, &progress)
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

impl TableData for ServersPage {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.rows.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<ServerCell>();
        let width = self.table.width() - TEXT_RIGHT - 4.0;
        cell.set_row(index, weak_from_ref(self), &self.rows[index], width);
        cell
    }

    fn cell_selected(&mut self, _: usize) {}
}

#[view]
struct ServerCell {
    index: usize,
    page: Weak<ServersPage>,
    chips: Vec<Weak<Pill>>,

    #[init]
    name: Label,
    owner: Label,
    mods: Container,
    ready_icon: ImageView,
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

        self.mods.set_color(colors::CLEAR);
        self.mods
            .place()
            .t(MODS_T)
            .l(4)
            .r(TEXT_RIGHT)
            .h(2.0 * pill::HEIGHT + PILL_GAP);

        // Plain text with a check, no box, so it does not read as a button.
        self.ready_icon.set_image("check.svg");
        self.ready_icon.place().r(16.0 + 84.0).t(28).size(16, 16);
        self.ready.set_text("All installed");
        self.ready.set_text_size(13).set_text_color(colors::OK);
        self.ready.set_alignment(TextAlignment::Left);
        self.ready.set_color(colors::CLEAR);
        self.ready.place().r(16).t(26).size(80, 20);

        style::primary(self.install, "Install");
        self.install
            .place()
            .r(16)
            .t(24)
            .size(INSTALL_WIDTH, style::BUTTON_H);
        self.install.on_tap(move || {
            if self.page.is_ok() {
                self.page.install(self.index, self.install);
            }
        });
        busy::track(self.install);

        style::ghost(self.edit, "Edit");
        self.edit
            .place()
            .r(16.0 + INSTALL_WIDTH + GAP)
            .t(24)
            .size(BUTTON_WIDTH, style::BUTTON_H);
        self.edit.on_tap(move || {
            if self.page.is_ok() {
                self.page.edit(self.index);
            }
        });

        style::danger(self.delete, "Remove");
        self.delete
            .place()
            .r(16.0 + INSTALL_WIDTH + BUTTON_WIDTH + 2.0 * GAP)
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
    fn set_row(
        mut self: Weak<Self>,
        index: usize,
        page: Weak<ServersPage>,
        row: &ServerRow,
        width: f32,
    ) {
        self.index = index;
        self.page = page;

        let server = &row.server;
        self.name.set_text(&server.name);
        self.owner.set_text(format!(
            "By {}, {} mods, updated {}",
            server.owner,
            server.mods.len(),
            time::ago(server.updated)
        ));
        self.owner.set_tooltip(time::full(server.updated));
        self.show_mods(server, &row.needs, width);

        let ready = row.needs.is_empty();
        self.ready.set_hidden(!ready);
        self.ready_icon.set_hidden(!ready);
        self.install.set_hidden(ready);
        self.install.set_text(match row.needs.count() {
            1 => "Install 1 mod".to_owned(),
            count => format!("Install {count} mods"),
        });
        self.edit.set_hidden(!row.mine);
        self.delete.set_hidden(!row.mine);
    }

    /// One pill per mod, readable name and version, in 2 rows at most. When
    /// not all of them fit, the last place goes to "+5 more". A mod the
    /// profile lacks is marked, the ones it has stay plain.
    fn show_mods(mut self: Weak<Self>, server: &Server, needs: &Needs, width: f32) {
        for mut chip in self.chips.drain(..) {
            chip.remove_from_superview();
        }
        let texts: Vec<(String, Option<&str>)> = server
            .mods
            .iter()
            .map(|server_mod| {
                let text = format!("{} {}", names::title(&server_mod.id), server_mod.version);
                (text, lacks(needs, server_mod))
            })
            .collect();
        let rows_end = 2.0 * pill::HEIGHT + PILL_GAP;
        let (mut x, mut y) = (0.0, 0.0);
        for (shown, (text, note)) in texts.iter().enumerate() {
            let left = texts.len() - shown;
            let chip = self.mods.add_view::<Pill>();
            let chip_width = match note {
                Some(note) => chip.set_marked("pill_version.svg", text, note),
                None => chip.set("pill_version.svg", text),
            };
            let (at_x, at_y) = flow(x, y, chip_width, width);
            // A pill that is not the last one leaves room for "+n more".
            let (_, more_y) = flow(at_x + chip_width + PILL_GAP, at_y, MORE_WIDTH, width);
            let fits =
                at_y + pill::HEIGHT <= rows_end && (left == 1 || more_y + pill::HEIGHT <= rows_end);
            if !fits {
                let more_width = chip.set("pill_dependency.svg", &format!("+{left} more"));
                let (at_x, at_y) = flow(x, y, more_width, width);
                chip.place().l(at_x).t(at_y).size(more_width, pill::HEIGHT);
                self.chips.push(chip);
                break;
            }
            chip.place().l(at_x).t(at_y).size(chip_width, pill::HEIGHT);
            self.chips.push(chip);
            x = at_x + chip_width + PILL_GAP;
            y = at_y;
        }
    }
}

/// The note of a mod the profile lacks for the server, `None` when it has it.
fn lacks(needs: &Needs, server_mod: &ServerMod) -> Option<&'static str> {
    if needs.missing.contains(server_mod) {
        Some("missing")
    } else if needs
        .other_version
        .iter()
        .any(|(needed, _)| needed == server_mod)
    {
        Some("wrong version")
    } else if needs.disabled.contains(server_mod) {
        Some("disabled")
    } else {
        None
    }
}

/// Where a pill of `chip` width goes when the row has used `x` of `width`.
fn flow(x: f32, y: f32, chip: f32, width: f32) -> (f32, f32) {
    if x > 0.0 && x + chip > width {
        (0.0, y + pill::HEIGHT + PILL_GAP)
    } else {
        (x, y)
    }
}
