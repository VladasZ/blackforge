//! The details of one Thunderstore package, with the way to add it or to pin
//! it to a version.

use blackforge_core::{broken::Broken, manifest::VersionReq, thunderstore::Package};
use hilen::{
    OnceEvent,
    refs::Weak,
    system::open_url,
    ui::{
        Button, Container, Label, ModalView, Setup, Size, TextField, UIColor, VerticalAlignment,
        ViewData, ViewSubviews, view,
    },
};

use crate::{
    backend,
    ui::{
        colors,
        mod_icon::ModIcon,
        mods_page::lock_change_summary,
        pill::{self, Pill},
        style, toast,
    },
};

const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 580.0;
const PAD: f32 = 24.0;
const ICON: f32 = 64.0;
/// The title and the two fact lines start right of the icon.
const TEXT_LEFT: f32 = PAD + ICON + 16.0;
const SHOWN_NEEDS: usize = 6;
const SHOWN_VERSIONS: usize = 10;
const PILL_GAP: f32 = 6.0;
/// The version pills flow into 2 rows at most. What does not fit is left out,
/// the page has the full list.
const VERSION_ROWS: f32 = 2.0 * pill::HEIGHT + PILL_GAP;

#[derive(Clone, Debug, Default)]
pub struct ModDetails {
    id: String,
    newest: String,
    facts: String,
    categories: String,
    description: String,
    needs: String,
    versions: Vec<String>,
    page_url: String,
    deprecated: bool,
    /// Empty when the server does not list it as broken on this game version.
    broken: String,
}

impl ModDetails {
    fn of(package: &Package, broken: &Broken) -> Self {
        let latest = package.latest();
        let newest = latest
            .map(|latest| latest.version.to_string())
            .unwrap_or_default();
        let needs: Vec<String> = latest
            .map(|latest| {
                latest
                    .dependencies
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let mut shown: Vec<String> = needs.iter().take(SHOWN_NEEDS).cloned().collect();
        if needs.len() > SHOWN_NEEDS {
            shown.push(format!("and {} more", needs.len() - SHOWN_NEEDS));
        }
        if shown.is_empty() {
            shown.push("nothing".to_owned());
        }
        let needs_text = shown.join("\n");
        let versions: Vec<String> = package
            .versions
            .iter()
            .take(SHOWN_VERSIONS)
            .map(|release| release.version.to_string())
            .collect();
        let broken = match (broken.since(&package.id), broken.version()) {
            (Some(since), Some(game)) => {
                format!("broken on game version {since} and later, this game is on {game}")
            }
            _ => String::new(),
        };

        Self {
            id: package.id.to_string(),
            facts: format!(
                "newest {newest}, updated {}, {} downloads, rating {}",
                package.updated.get(..10).unwrap_or(&package.updated),
                package.downloads,
                package.rating
            ),
            categories: package.categories.join(", "),
            description: package.description.clone(),
            needs: needs_text,
            versions,
            page_url: package.package_url.clone(),
            deprecated: package.deprecated,
            broken,
            newest,
        }
    }
}

#[view]
pub struct ModInfo {
    event: OnceEvent<bool>,
    details: ModDetails,

    #[init]
    icon: ModIcon,
    title: Label,
    facts: Label,
    categories: Label,
    description: Label,
    needs_title: Label,
    needs: Label,
    versions_title: Label,
    versions: Container,
    deprecated: Label,
    broken: Label,
    version: TextField,
    open_page: Button,
    close: Button,
    add: Button,
}

impl ModalView<ModDetails, bool> for ModInfo {
    fn modal_event(&self) -> &OnceEvent<bool> {
        &self.event
    }

    fn modal_size() -> Size {
        (WIDTH, HEIGHT).into()
    }

    fn modal_scrim_color() -> UIColor {
        colors::SCRIM.into()
    }

    fn setup_input(mut self: Weak<Self>, details: ModDetails) {
        self.icon.show(&details.id, &details.newest);
        self.title.set_text(&details.id);
        self.facts.set_text(&details.facts);
        self.categories.set_text(&details.categories);
        self.description.set_text(&details.description);
        self.needs.set_text(&details.needs);
        self.show_versions(&details.versions);
        self.deprecated.set_hidden(!details.deprecated);
        self.broken.set_hidden(details.broken.is_empty());
        self.broken.set_text(&details.broken);
        self.details = details;
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

        self.title.place().t(20).l(TEXT_LEFT).r(PAD).h(26);

        style::dim(self.facts);
        self.facts.set_ellipsize(true);
        self.facts.place().t(52).l(TEXT_LEFT).r(PAD).h(16);

        style::dim(self.categories);
        self.categories.set_ellipsize(true);
        self.categories.place().t(72).l(TEXT_LEFT).r(PAD).h(16);

        style::body(self.description);
        self.description.set_multiline(true);
        self.description
            .set_vertical_alignment(VerticalAlignment::Top);
        self.description.place().t(100).l(PAD).r(PAD).h(80);

        style::dim(self.needs_title);
        self.needs_title.set_text("needs");
        self.needs_title.place().t(192).l(PAD).r(PAD).h(16);

        style::body(self.needs);
        self.needs.set_text_size(13);
        self.needs.set_multiline(true);
        self.needs.set_vertical_alignment(VerticalAlignment::Top);
        self.needs.place().t(212).l(PAD).r(PAD).h(126);

        style::dim(self.versions_title);
        self.versions_title.set_text("versions");
        self.versions_title.place().t(350).l(PAD).r(PAD).h(16);

        self.versions.set_color(colors::CLEAR);
        self.versions.place().t(372).l(PAD).r(PAD).h(VERSION_ROWS);

        style::dim(self.deprecated);
        self.deprecated.set_text("this package is deprecated");
        self.deprecated.set_text_color(colors::BAD);
        self.deprecated.place().t(434).l(PAD).r(PAD).h(16);

        style::dim(self.broken);
        self.broken.set_text_color(colors::BAD);
        self.broken.place().t(454).l(PAD).r(PAD).h(16);

        style::field(self.version, "version, empty for the newest");
        self.version.place().b(64).l(PAD).w(280).h(style::FIELD_H);

        style::ghost(self.open_page, "open the page");
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

        style::primary(self.add, "add");
        self.add.place().b(16).r(PAD).size(96, style::BUTTON_H);
        self.add.on_tap(move || self.add_to_profile());

        style::ghost(self.close, "close");
        self.close
            .place()
            .b(16)
            .r(PAD + 104.0)
            .size(84, style::BUTTON_H);
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
                Ok(ModDetails::of(index.find(&id)?, &broken))
            },
            move |result| match result {
                Ok(details) => Self::show_modally_with_input(details, done),
                Err(error) => toast::failure(&error),
            },
        );
    }

    /// One pill per version, newest first, flowing left to right and wrapping
    /// into a second row. A pill that would start a third row is dropped.
    fn show_versions(self: Weak<Self>, versions: &[String]) {
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
            x += width + PILL_GAP;
        }
    }

    fn add_to_profile(self: Weak<Self>) {
        let id = self.details.id.clone();
        let typed = self.version.text().trim().to_owned();
        let version = if typed.is_empty() {
            VersionReq::Latest
        } else {
            match typed.parse() {
                Ok(version) => version,
                Err(error) => {
                    toast::error(format!("{error}"));
                    return;
                }
            }
        };

        backend::change(
            "adding the mod",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let (id, change) = forge.add(&profile, &id, version, &progress).await?;
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
