//! The details of one Thunderstore package, with the way to add it.

use blackforge_core::{
    broken::Broken,
    ident::{PackageId, VersionedId},
    lock::Lockfile,
    manifest::Manifest,
    thunderstore::Package,
};
use hilen::{
    OnceEvent,
    refs::Weak,
    system::open_url,
    ui::{
        Button, Container, Label, ModalView, Setup, Size, UIColor, VerticalAlignment, ViewData,
        ViewSubviews, ViewTooltip, view,
    },
};

use crate::{
    backend,
    ui::{
        busy, colors,
        mod_icon::ModIcon,
        mod_pills::Note,
        mods_page::lock_change_summary,
        names,
        pill::{self, Pill, compact},
        style, time, toast,
    },
};

const WIDTH: f32 = 640.0;
const PAD: f32 = 24.0;
const ICON: f32 = 64.0;
/// The title and the fact lines start right of the icon.
const TEXT_LEFT: f32 = PAD + ICON + 16.0;
const SHOWN_NEEDS: usize = 6;
const SHOWN_VERSIONS: usize = 10;
const PILL_GAP: f32 = 6.0;
/// The version pills flow into 2 rows at most. What does not fit is left out,
/// the page has the full list.
const VERSION_ROWS: f32 = 2.0 * pill::HEIGHT + PILL_GAP;
/// Space between two sections, and between a section title and its content.
const GAP: f32 = 16.0;
const SMALL_GAP: f32 = 6.0;
const LINE: f32 = 16.0;
/// A long description is cut here, the page has all of it.
const MAX_DESCRIPTION: f32 = 160.0;
const HEADER_BOTTOM: f32 = 20.0 + ICON + GAP;
/// The status line and the button row under the last section.
const FOOTER: f32 = GAP + LINE + GAP + style::BUTTON_H + 16.0;

/// The mod as the profile has it.
#[derive(Clone, Debug)]
struct Installed {
    version: String,
    note: Note,
}

impl Installed {
    fn of(id: &PackageId, manifest: &Manifest, lock: &Lockfile) -> Option<Self> {
        let package = lock.packages.iter().find(|package| &package.id == id)?;
        Some(Self {
            version: package.version.to_string(),
            note: Note::of(manifest.mods.get(id)),
        })
    }

    fn line(&self) -> String {
        let how = match &self.note {
            Note::None => String::new(),
            Note::Dependency => ", as a dependency".to_owned(),
            Note::Pinned(server) => format!(", pinned for {server}"),
            Note::Disabled => ", disabled".to_owned(),
        };
        format!("Installed {}{how}", self.version)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ModDetails {
    id: String,
    newest: String,
    facts: String,
    /// The full date of the newest release, for the tooltip of the facts.
    updated: String,
    categories: String,
    description: String,
    needs: String,
    versions: Vec<String>,
    page_url: String,
    deprecated: bool,
    /// Empty when the server does not list it as broken on this game version.
    broken: String,
    installed: Option<Installed>,
}

impl ModDetails {
    fn of(package: &Package, broken: &Broken, installed: Option<Installed>) -> Self {
        let latest = package.latest();
        let newest = latest
            .map(|latest| latest.version.to_string())
            .unwrap_or_default();
        let needs: Vec<String> = latest
            .map(|latest| {
                latest
                    .dependencies
                    .iter()
                    .map(|dependency| readable_dependency(dependency))
                    .collect()
            })
            .unwrap_or_default();
        let mut shown: Vec<String> = needs.iter().take(SHOWN_NEEDS).cloned().collect();
        if needs.len() > SHOWN_NEEDS {
            shown.push(format!("and {} more", needs.len() - SHOWN_NEEDS));
        }
        if shown.is_empty() {
            shown.push("Nothing".to_owned());
        }
        let versions: Vec<String> = package
            .versions
            .iter()
            .take(SHOWN_VERSIONS)
            .map(|release| release.version.to_string())
            .collect();
        let broken = match (broken.since(&package.id), broken.version()) {
            (Some(since), Some(game)) => {
                format!("Broken on game version {since} and later, this game is on {game}")
            }
            _ => String::new(),
        };

        let updated = time::parse(&package.updated);
        Self {
            id: package.id.to_string(),
            facts: format!(
                "Newest {newest}, updated {}, {} downloads, rating {}",
                updated.map_or_else(|| package.updated.clone(), time::ago),
                compact(package.downloads),
                package.rating
            ),
            updated: updated.map_or_else(|| package.updated.clone(), time::full),
            categories: package.categories.join(", "),
            description: package.description.clone(),
            needs: shown.join("\n"),
            versions,
            page_url: package.package_url.clone(),
            deprecated: package.deprecated,
            broken,
            newest,
            installed,
        }
    }
}

/// The dependency as a reader knows it, like `BepInExPack Valheim 5.4.2350, by denikson`.
fn readable_dependency(dependency: &VersionedId) -> String {
    let id = dependency.id.to_string();
    format!(
        "{} {}, {}",
        names::title(&id),
        dependency.version,
        names::author(&id)
    )
}

#[view]
pub struct ModInfo {
    event: OnceEvent<bool>,
    details: ModDetails,
    /// One pill row or two, known once the pills are laid out.
    versions_height: f32,

    #[init]
    icon: ModIcon,
    title: Label,
    author: Label,
    facts: Label,
    categories: Label,
    description: Label,
    needs_title: Label,
    needs: Label,
    versions_title: Label,
    versions: Container,
    deprecated: Label,
    broken: Label,
    status: Label,
    open_page: Button,
    close: Button,
    add: Button,
}

impl ModalView<ModDetails, bool> for ModInfo {
    fn modal_event(&self) -> &OnceEvent<bool> {
        &self.event
    }

    /// The height it opens with. `setup_input` shrinks it to the content.
    fn modal_size() -> Size {
        (WIDTH, 580.0).into()
    }

    fn modal_scrim_color() -> UIColor {
        colors::SCRIM.into()
    }

    fn setup_input(mut self: Weak<Self>, details: ModDetails) {
        self.icon.show(&details.id, &details.newest);
        self.title.set_text(names::title(&details.id));
        self.author.set_text(names::author(&details.id));
        self.facts.set_text(&details.facts);
        self.facts
            .set_tooltip(format!("Updated {}", details.updated));
        self.categories.set_text(&details.categories);
        self.description.set_text(&details.description);
        self.needs.set_text(&details.needs);
        self.show_versions(&details.versions);
        self.deprecated.set_hidden(!details.deprecated);
        self.broken.set_hidden(details.broken.is_empty());
        self.broken.set_text(&details.broken);
        if let Some(installed) = &details.installed {
            self.status.set_text(installed.line());
        }
        self.details = details;
        self.stack();
        self.refresh_actions();
    }
}

impl Setup for ModInfo {
    fn setup(self: Weak<Self>) {
        style::card(self);
        self.set_corner_radius(14);

        style::title(self.title, "");
        self.title.set_text_size(20);
        self.title.set_ellipsize(true);
        self.icon.place().t(20).l(PAD).size(ICON, ICON);
        self.title.place().t(18).l(TEXT_LEFT).r(PAD).h(26);

        style::dim(self.author);
        self.author.place().t(44).l(TEXT_LEFT).r(PAD).h(LINE);

        style::dim(self.facts);
        self.facts.set_ellipsize(true);
        self.facts.place().t(62).l(TEXT_LEFT).r(PAD).h(LINE);

        style::dim(self.categories);
        self.categories.set_ellipsize(true);
        self.categories.place().t(80).l(TEXT_LEFT).r(PAD).h(LINE);

        style::body(self.description);
        self.description.set_multiline(true);
        self.description
            .set_vertical_alignment(VerticalAlignment::Top);

        style::dim(self.needs_title);
        self.needs_title.set_text("Needs");

        style::body(self.needs);
        self.needs.set_text_size(13);
        self.needs.set_multiline(true);
        self.needs.set_vertical_alignment(VerticalAlignment::Top);

        style::dim(self.versions_title);
        self.versions_title.set_text("Versions");

        self.versions.set_color(colors::CLEAR);

        style::dim(self.deprecated);
        self.deprecated.set_text("This package is deprecated");
        self.deprecated.set_text_color(colors::BAD);

        style::dim(self.broken);
        self.broken.set_text_color(colors::BAD);

        style::body(self.status);
        self.status.set_text_color(colors::OK);

        style::ghost(self.open_page, "Open the page");
        self.open_page
            .place()
            .b(16)
            .l(PAD)
            .size(130, style::BUTTON_H);
        self.open_page.on_tap(move || {
            if let Err(error) = open_url(&self.details.page_url) {
                toast::error(format!("cannot open the page: {error}"));
            }
        });

        style::primary(self.add, "Add");
        self.add.place().b(16).r(PAD).size(96, style::BUTTON_H);
        self.add.on_tap(move || self.add_to_profile());
        busy::track(self.add);

        style::ghost(self.close, "Close");
        self.close.on_tap(move || self.hide_modal(false));
    }
}

impl ModInfo {
    /// Looks the package up and shows it. `done` gets true when the profile
    /// was changed from the modal.
    pub fn open(id: &str, done: impl FnOnce(bool) + Send + 'static) {
        let id = id.to_owned();
        backend::load(
            "reading the package list",
            |forge, progress| async move {
                let index = backend::index(forge, &progress).await?;
                let broken = backend::broken_known(forge, &progress).await;
                let package = index.find(&id)?;
                let profile = backend::profile(forge, &progress).await?;
                let installed = Installed::of(
                    &package.id,
                    &profile.manifest().await?,
                    &profile.lock().await?,
                );
                Ok(ModDetails::of(package, &broken, installed))
            },
            move |result| match result {
                Ok(details) => Self::show_modally_with_input(details, done),
                Err(error) => toast::failure(&error),
            },
        );
    }

    /// Puts the sections under each other at the height their text needs,
    /// then fits the dialog around them.
    fn stack(self: Weak<Self>) {
        let width = WIDTH - 2.0 * PAD;
        let mut y = HEADER_BOTTOM;

        let description = self
            .description
            .size_for_width(width)
            .height
            .min(MAX_DESCRIPTION);
        self.description.place().t(y).l(PAD).r(PAD).h(description);
        y += description + GAP;

        self.needs_title.place().t(y).l(PAD).r(PAD).h(LINE);
        y += LINE + SMALL_GAP;
        let needs = self.needs.size_for_width(width).height;
        self.needs.place().t(y).l(PAD).r(PAD).h(needs);
        y += needs + GAP;

        self.versions_title.place().t(y).l(PAD).r(PAD).h(LINE);
        y += LINE + SMALL_GAP;
        self.versions
            .place()
            .t(y)
            .l(PAD)
            .r(PAD)
            .h(self.versions_height);
        y += self.versions_height;

        for warning in [self.deprecated, self.broken] {
            if !warning.is_hidden() {
                y += SMALL_GAP;
                warning.place().t(y).l(PAD).r(PAD).h(LINE);
                y += LINE;
            }
        }

        self.status.place().t(y + GAP).l(PAD).r(PAD).h(LINE);

        self.place().clear().size(WIDTH, y + FOOTER).center();
    }

    /// One pill per version, newest first, flowing left to right and wrapping
    /// into a second row. A pill that would start a third row is dropped.
    fn show_versions(mut self: Weak<Self>, versions: &[String]) {
        let room = WIDTH - 2.0 * PAD;
        let (mut x, mut y) = (0.0, 0.0);
        for version in versions {
            let mut chip = self.versions.add_view::<Pill>();
            let width = chip.set("pill_version.svg", version);
            if x > 0.0 && x + width > room {
                x = 0.0;
                y += pill::HEIGHT + PILL_GAP;
            }
            if y + pill::HEIGHT > VERSION_ROWS {
                chip.remove_from_superview();
                break;
            }
            chip.place().l(x).t(y).size(width, pill::HEIGHT);
            self.versions_height = y + pill::HEIGHT;
            x += width + PILL_GAP;
        }
    }

    /// An installed mod has no action, close then sits at the right edge.
    fn refresh_actions(self: Weak<Self>) {
        let installed = self.details.installed.is_some();
        self.add.set_hidden(installed);
        let action = if installed { 0.0 } else { 96.0 + 8.0 };
        self.close
            .place()
            .clear()
            .b(16)
            .r(PAD + action)
            .size(84, style::BUTTON_H);
    }

    fn add_to_profile(self: Weak<Self>) {
        let id = self.details.id.clone();
        busy::press(self.add, "Adding...");
        backend::change(
            "adding the mod",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let (id, change) = forge.add(&profile, &id, &progress).await?;
                forge.sync(&profile, &progress).await?;
                Ok(format!("added {id}, {}", lock_change_summary(&change)))
            },
            move |result| match result {
                Ok(text) => {
                    toast::success(text);
                    if self.is_ok() {
                        self.hide_modal(true);
                    }
                }
                Err(error) => toast::failure(&error),
            },
        );
    }
}
