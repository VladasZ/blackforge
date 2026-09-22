//! Search Thunderstore and add a mod. The results come in pages, the next
//! page is built when the list reaches its bottom.

use std::{cmp::Reverse, collections::HashSet};

use anyhow::Result;
use blackforge_core::{
    broken::Broken, forge::Forge, ident::PackageId, progress::Progress, thunderstore::Package,
};
use hilen::{
    refs::{Weak, weak_from_ref},
    system::open_url,
    ui::{
        Button, CellRegistry, Container, DropDown, Label, Setup, TableData, TableView,
        TextAlignment, TextField, ToLabel, VerticalAlignment, View, ViewData, ViewFrame,
        ViewTooltip, view,
    },
};

use crate::{
    backend,
    ui::{
        busy, colors, hover,
        icon_button::{self, IconButton},
        mod_icon::ModIcon,
        mod_info::ModInfo,
        mods_page::lock_change_summary,
        names,
        pill::{self, Pill, compact},
        style, toast,
    },
};

const SEARCH_WIDTH: f32 = 340.0;
const SORT_WIDTH: f32 = 150.0;
const FIELD_GAP: f32 = 10.0;
/// The title and subtitle keep this much room left of the fields before the
/// fields move to their own row.
const TITLE_ROOM: f32 = 24.0;
/// The fields in their own row sit this far under the header.
const FIELDS_ROW_T: f32 = style::HEADER - 8.0;
const ADD_WIDTH: f32 = 104.0;
/// The page button sits left of the add button.
const PAGE_RIGHT: f32 = 16.0 + ADD_WIDTH + 8.0;
/// The pills end where the page button starts, with a gap.
const PILLS_RIGHT: f32 = PAGE_RIGHT + icon_button::SIZE + 16.0;
const PILL_GAP: f32 = 8.0;
/// Where the first line of a row starts: the name, the pills and the button.
const TOP_LINE: f32 = 12.0;
const ICON: f32 = 40.0;
/// The name and the description start right of the icon.
const TEXT_LEFT: f32 = 4.0 + ICON + 12.0;
const DESCRIPTION_T: f32 = TOP_LINE + 46.0;
const DESCRIPTION_R: f32 = 16.0;
const ROW_PAD_B: f32 = 14.0;
/// A row with no description still fits the icon.
const MIN_ROW: f32 = TOP_LINE + ICON + ROW_PAD_B;
/// Rows built per page. A search that finds every package still opens at
/// once, the rest is built as the list scrolls.
const PAGE: usize = 100;

/// The order of the found packages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Sort {
    /// A hit in the name first, then live before deprecated, then more
    /// downloads. The order of the core.
    #[default]
    BestMatch,
    /// Most downloads first, wherever the words were found.
    Downloads,
}

impl ToLabel for Sort {
    fn to_label(&self) -> String {
        match self {
            Self::BestMatch => "Best match",
            Self::Downloads => "Downloads",
        }
        .to_owned()
    }
}

#[derive(Clone, Debug)]
struct Hit {
    id: String,
    version: String,
    downloads: String,
    description: String,
    page_url: String,
    installed: bool,
    deprecated: bool,
    /// The server lists it as broken on this game version.
    broken: bool,
}

struct Found {
    query: u64,
    ids: Vec<PackageId>,
    hits: Vec<Hit>,
}

#[view]
pub struct BrowsePage {
    /// Every package the search found, in order. `hits` holds the rows built
    /// so far, the first pages of this list.
    found: Vec<PackageId>,
    hits: Vec<Hit>,
    /// Counts the searches, so a slow old one cannot replace a newer one.
    query: u64,
    sort: Sort,
    /// A next page is being built, the bottom of the list does not ask again.
    paging: bool,

    #[init]
    title: Label,
    subtitle: Label,
    sort_by: DropDown<Sort>,
    search: TextField,
    table: TableView,
    /// Never shown. It has the look of a description, so it can say how tall
    /// one is at the width the table has now.
    probe: Label,
}

impl Setup for BrowsePage {
    fn setup(mut self: Weak<Self>) {
        style::title(self.title, "Browse");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);

        style::field(self.search, "Search Thunderstore");
        self.search.changed.val(move |text| self.find(text));

        self.sort_by
            .set_values(vec![Sort::BestMatch, Sort::Downloads]);
        self.sort_by.set_text_size(13);
        self.sort_by.set_text_color(colors::FG);
        self.sort_by.set_accent_color(colors::ACCENT);
        self.sort_by.set_color(colors::FIELD_BG);
        self.sort_by.set_border_color(colors::BORDER);
        self.sort_by.set_border_width(1);
        self.sort_by.set_corner_radius(7);
        self.sort_by.on_changed(move |sort| {
            self.sort = sort;
            self.refresh();
        });

        self.table.set_data_source(self).register_cell::<HitCell>();
        style::table(self.table);
        self.table.set_variable_heights(true);
        self.table
            .bottom_reached()
            .sub(self, move || self.next_page());

        style::dim(self.probe);
        self.probe.set_multiline(true);
        self.probe.set_hidden(true);

        // A new width moves the fields and wraps the descriptions again.
        self.size_changed().sub(move || {
            self.place_header();
            self.table.reload_data();
        });
        self.place_header();

        self.find(String::new());
    }
}

impl BrowsePage {
    /// The search field and the sort sit right of the title while the row has
    /// room for both, and move to their own row under it when it does not.
    fn place_header(self: Weak<Self>) {
        let fields = SORT_WIDTH + FIELD_GAP + SEARCH_WIDTH;
        let title = self.subtitle.content_size().width.max(120.0);
        let one_row = self.width() >= style::PAGE_PAD * 2.0 + title + TITLE_ROOM + fields;

        if one_row {
            self.search
                .place()
                .clear()
                .t(28)
                .r(style::PAGE_PAD)
                .size(SEARCH_WIDTH, style::FIELD_H);
            self.sort_by
                .place()
                .clear()
                .t(28)
                .r(style::PAGE_PAD + SEARCH_WIDTH + FIELD_GAP)
                .size(SORT_WIDTH, style::FIELD_H);
            self.subtitle
                .place()
                .clear()
                .t(56)
                .l(style::PAGE_PAD)
                .r(style::PAGE_PAD + fields + TITLE_ROOM)
                .h(16);
        } else {
            self.search
                .place()
                .clear()
                .t(FIELDS_ROW_T)
                .l(style::PAGE_PAD)
                .r(style::PAGE_PAD + SORT_WIDTH + FIELD_GAP)
                .h(style::FIELD_H);
            self.sort_by
                .place()
                .clear()
                .t(FIELDS_ROW_T)
                .r(style::PAGE_PAD)
                .size(SORT_WIDTH, style::FIELD_H);
            self.subtitle
                .place()
                .clear()
                .t(56)
                .l(style::PAGE_PAD)
                .r(style::PAGE_PAD)
                .h(16);
        }

        let table_t = if one_row {
            style::HEADER
        } else {
            FIELDS_ROW_T + style::FIELD_H + 12.0
        };
        self.table
            .place()
            .clear()
            .t(table_t)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);
    }

    fn find(mut self: Weak<Self>, text: String) {
        self.query += 1;
        let query = self.query;
        let sort = self.sort;

        backend::load(
            "searching Thunderstore",
            move |forge, progress| async move {
                let index = backend::index(forge, &progress).await?;
                let mut found = index.search(&text);
                if sort == Sort::Downloads {
                    found.sort_by_key(|package| Reverse(package.downloads));
                }
                let ids: Vec<PackageId> = found.iter().map(|package| package.id.clone()).collect();
                let hits = build_hits(forge, &progress, &ids[..ids.len().min(PAGE)]).await?;
                Ok(Found { query, ids, hits })
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(found) if found.query == self.query => {
                        self.subtitle.set_text(match found.ids.len() {
                            1 => "1 package".to_owned(),
                            count => format!("{count} packages"),
                        });
                        self.found = found.ids;
                        self.hits = found.hits;
                        self.paging = false;
                        self.table.set_content_offset(0);
                        self.table.reload_data();
                        self.place_header();
                    }
                    Ok(_) => {}
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    /// Builds the rows of the next page of the search that shows now.
    fn next_page(mut self: Weak<Self>) {
        if self.paging || self.hits.len() >= self.found.len() {
            return;
        }
        self.paging = true;
        let query = self.query;
        let start = self.hits.len();
        let ids = self.found[start..(start + PAGE).min(self.found.len())].to_vec();

        backend::load(
            "reading more packages",
            move |forge, progress| async move { build_hits(forge, &progress, &ids).await },
            move |result| {
                if !self.is_ok() || query != self.query {
                    return;
                }
                self.paging = false;
                match result {
                    Ok(hits) => {
                        self.hits.extend(hits);
                        self.table.reload_data();
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn refresh(self: Weak<Self>) {
        self.find(self.search.text().to_owned());
    }

    /// A change of the profile only moves the installed marks, the rows and
    /// the scroll position stay.
    fn refresh_installed(mut self: Weak<Self>) {
        backend::load(
            "reading the mods",
            |forge, progress| async move { installed_ids(forge, &progress).await },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(installed) => {
                        for hit in &mut self.hits {
                            hit.installed = installed.contains(&hit.id);
                        }
                        self.table.reload_data();
                    }
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn open_page(self: Weak<Self>, index: usize) {
        let Some(hit) = self.hits.get(index) else {
            return;
        };
        if let Err(error) = open_url(&hit.page_url) {
            toast::error(format!("Cannot open the page: {error}"));
        }
    }

    fn add(self: Weak<Self>, index: usize, button: Weak<Button>) {
        let Some(hit) = self.hits.get(index) else {
            return;
        };
        let id = hit.id.clone();
        busy::press(button, "Adding...");
        backend::change(
            "adding the mod",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let (id, change) = forge.add(&profile, &id, &progress).await?;
                forge.sync(&profile, &progress).await?;
                Ok(format!("Added {id}, {}", lock_change_summary(&change)))
            },
            move |result| {
                match result {
                    Ok(text) => toast::success(text),
                    Err(error) => toast::failure(&error),
                }
                if self.is_ok() {
                    self.refresh_installed();
                }
            },
        );
    }

    fn description_width(&self) -> f32 {
        self.table.width() - TEXT_LEFT - DESCRIPTION_R
    }
}

/// The row of one package, flagged against the lock and the broken list.
fn hit(package: &Package, installed: &HashSet<String>, broken: &Broken) -> Hit {
    let id = package.id.to_string();
    let version = package
        .latest()
        .map(|latest| latest.version.to_string())
        .unwrap_or_default();
    Hit {
        installed: installed.contains(&id),
        id,
        version,
        downloads: compact(package.downloads),
        description: backend::summary(package),
        page_url: package.package_url.clone(),
        deprecated: package.deprecated,
        broken: broken.contains(&package.id),
    }
}

/// The ids of every package in the lock, dependencies included.
async fn installed_ids(forge: &Forge, progress: &Progress) -> Result<HashSet<String>> {
    let profile = backend::profile(forge, progress).await?;
    Ok(profile
        .lock()
        .await?
        .packages
        .iter()
        .map(|package| package.id.to_string())
        .collect())
}

/// The rows of `ids`, in their order.
async fn build_hits(forge: &Forge, progress: &Progress, ids: &[PackageId]) -> Result<Vec<Hit>> {
    let index = backend::index(forge, progress).await?;
    let broken = backend::broken_known(forge, progress).await;
    let installed = installed_ids(forge, progress).await?;
    Ok(ids
        .iter()
        .filter_map(|id| index.get(id))
        .map(|package| hit(package, &installed, &broken))
        .collect())
}

impl TableData for BrowsePage {
    fn cell_height(&self, index: usize) -> f32 {
        let description = &self.hits[index].description;
        if description.is_empty() {
            return MIN_ROW;
        }
        self.probe.set_text(description);
        let height = self.probe.size_for_width(self.description_width()).height;
        (DESCRIPTION_T + height + ROW_PAD_B).max(MIN_ROW)
    }

    fn number_of_cells(&self) -> usize {
        self.hits.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<HitCell>();
        let width = self.description_width();
        cell.set_hit(index, weak_from_ref(self), &self.hits[index], width);
        cell
    }

    fn cell_selected(&mut self, index: usize) {
        let page = weak_from_ref(self);
        if let Some(hit) = self.hits.get(index) {
            ModInfo::open(&hit.id, move |changed| {
                if changed && page.is_ok() {
                    page.refresh_installed();
                }
            });
        }
    }
}

#[view]
struct HitCell {
    index: usize,
    page: Weak<BrowsePage>,

    #[init]
    icon: ModIcon,
    name: Label,
    author: Label,
    description: Label,
    version: Pill,
    downloads: Pill,
    deprecated: Pill,
    broken: Pill,
    installed: Label,
    open_page: IconButton,
    add: Button,
    line: Container,
}

impl Setup for HitCell {
    fn setup(self: Weak<Self>) {
        self.icon.place().l(4).t(TOP_LINE).size(ICON, ICON);

        style::body(self.name);
        self.name.set_ellipsize(true);

        style::dim(self.author);
        self.author.set_ellipsize(true);
        self.author
            .place()
            .t(TOP_LINE + 24.0)
            .l(TEXT_LEFT)
            .r(16)
            .h(16);

        self.deprecated.warn();
        self.broken.warn();

        // The whole text, wrapped. The row is as tall as the text needs.
        style::dim(self.description);
        self.description.set_multiline(true);
        self.description
            .set_vertical_alignment(VerticalAlignment::Top);

        // Same rectangle as the add button, so the column reads as one shape.
        self.installed.set_text("Installed");
        self.installed.set_text_size(13).set_text_color(colors::OK);
        self.installed.set_alignment(TextAlignment::Center);
        self.installed.set_color(colors::OK_BG);
        self.installed.set_corner_radius(7);
        self.installed
            .place()
            .r(16)
            .t(TOP_LINE - 2.0)
            .size(ADD_WIDTH, 28);

        self.open_page.set_icon("open_page.svg");
        self.open_page.set_tooltip("Open on Thunderstore");
        self.open_page
            .place()
            .r(PAGE_RIGHT)
            .t(TOP_LINE - 2.0)
            .size(icon_button::SIZE, icon_button::SIZE);
        self.open_page.tapped.sub(move || {
            if self.page.is_ok() {
                self.page.open_page(self.index);
            }
        });

        style::outline(self.add, "Add");
        self.add.place().r(16).t(TOP_LINE - 2.0).size(ADD_WIDTH, 28);
        self.add.on_tap(move || {
            if self.page.is_ok() {
                self.page.add(self.index, self.add);
            }
        });
        busy::track(self.add);

        // The wash says the row can be clicked, the click opens the details.
        hover::row(self);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl HitCell {
    fn set_hit(
        mut self: Weak<Self>,
        index: usize,
        page: Weak<BrowsePage>,
        hit: &Hit,
        description_width: f32,
    ) {
        self.index = index;
        self.page = page;
        self.icon.show(&hit.id, &hit.version);
        self.name.set_text(names::title(&hit.id));
        self.author.set_text(names::author(&hit.id));

        self.description.set_text(&hit.description);
        self.description.set_hidden(hit.description.is_empty());
        let height = self.description.size_for_width(description_width).height;
        self.description
            .place()
            .clear()
            .t(DESCRIPTION_T)
            .l(TEXT_LEFT)
            .r(DESCRIPTION_R)
            .h(height);

        // The pills sit right to left, each as wide as its text.
        let mut right = PILLS_RIGHT;
        let downloads = self.downloads.set("pill_downloads.svg", &hit.downloads);
        self.downloads
            .place()
            .clear()
            .r(right)
            .t(TOP_LINE)
            .size(downloads, pill::HEIGHT);
        right += downloads + PILL_GAP;
        let version = self.version.set("pill_version.svg", &hit.version);
        self.version
            .place()
            .clear()
            .r(right)
            .t(TOP_LINE)
            .size(version, pill::HEIGHT);
        right += version + PILL_GAP;
        self.deprecated.set_hidden(!hit.deprecated);
        if hit.deprecated {
            let deprecated = self.deprecated.set("pill_deprecated.svg", "Deprecated");
            self.deprecated
                .place()
                .clear()
                .r(right)
                .t(TOP_LINE)
                .size(deprecated, pill::HEIGHT);
            right += deprecated + PILL_GAP;
        }
        self.broken.set_hidden(!hit.broken);
        if hit.broken {
            let broken = self.broken.set("pill_broken.svg", "Broken");
            self.broken
                .place()
                .clear()
                .r(right)
                .t(TOP_LINE)
                .size(broken, pill::HEIGHT);
            right += broken + PILL_GAP;
        }
        // The name ends where the pills start, a row has 2 to 4 of them.
        self.name
            .place()
            .clear()
            .t(TOP_LINE + 2.0)
            .l(TEXT_LEFT)
            .r(right + PILL_GAP)
            .h(20);
        self.installed.set_hidden(!hit.installed);
        self.add.set_hidden(hit.installed);
    }
}
