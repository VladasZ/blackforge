//! Search Thunderstore and add a mod.

use std::{cmp::Reverse, collections::HashSet};

use hilen::{
    refs::{Weak, weak_from_ref},
    system::open_url,
    ui::{
        Button, CellRegistry, Container, DropDown, Label, Setup, TableData, TableView,
        TextAlignment, TextField, ToLabel, VerticalAlignment, View, ViewData, ViewTooltip, view,
    },
};

use crate::{
    backend,
    ui::{
        colors, hover,
        icon_button::{self, IconButton},
        mod_icon::ModIcon,
        mod_info::ModInfo,
        mods_page::lock_change_summary,
        names,
        pill::{self, Pill, compact},
        style, toast,
    },
};

const ROW_HEIGHT: f32 = 104.0;
const SEARCH_WIDTH: f32 = 340.0;
const SORT_WIDTH: f32 = 150.0;
const ADD_WIDTH: f32 = 84.0;
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
/// Rows past this add nothing, the user narrows the search instead.
const SHOWN: usize = 200;

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
            Self::BestMatch => "best match",
            Self::Downloads => "downloads",
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
    total: usize,
    hits: Vec<Hit>,
}

#[view]
pub struct BrowsePage {
    hits: Vec<Hit>,
    /// Counts the searches, so a slow old one cannot replace a newer one.
    query: u64,
    sort: Sort,

    #[init]
    title: Label,
    subtitle: Label,
    sort_by: DropDown<Sort>,
    search: TextField,
    table: TableView,
}

impl Setup for BrowsePage {
    fn setup(mut self: Weak<Self>) {
        style::title(self.title, "Browse");
        self.title.place().t(24).l(style::PAGE_PAD).size(300, 30);

        style::dim(self.subtitle);
        self.subtitle.place().t(56).l(style::PAGE_PAD).size(500, 16);

        style::field(self.search, "search Thunderstore");
        self.search
            .place()
            .t(28)
            .r(style::PAGE_PAD)
            .size(SEARCH_WIDTH, style::FIELD_H);
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
        self.sort_by
            .place()
            .t(28)
            .r(style::PAGE_PAD + SEARCH_WIDTH + 10.0)
            .size(SORT_WIDTH, style::FIELD_H);
        self.sort_by.on_changed(move |sort| {
            self.sort = sort;
            self.refresh();
        });

        self.table.set_data_source(self).register_cell::<HitCell>();
        style::table(self.table);
        self.table
            .place()
            .t(style::HEADER)
            .l(style::PAGE_PAD)
            .r(style::PAGE_PAD)
            .b(0);

        self.find(String::new());
    }
}

impl BrowsePage {
    fn find(mut self: Weak<Self>, text: String) {
        self.query += 1;
        let query = self.query;
        let sort = self.sort;

        backend::load(
            "searching Thunderstore",
            move |forge, progress| async move {
                let index = backend::index(forge, &progress).await?;
                let broken = backend::broken_known(forge, &progress).await;
                let profile = backend::profile(forge, &progress).await?;
                let installed: HashSet<String> = profile
                    .lock()
                    .await?
                    .packages
                    .iter()
                    .map(|package| package.id.to_string())
                    .collect();

                let mut found = index.search(&text);
                if sort == Sort::Downloads {
                    found.sort_by_key(|package| Reverse(package.downloads));
                }
                let hits = found
                    .iter()
                    .take(SHOWN)
                    .map(|package| {
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
                    })
                    .collect();
                Ok(Found {
                    query,
                    total: found.len(),
                    hits,
                })
            },
            move |result| {
                if !self.is_ok() {
                    return;
                }
                match result {
                    Ok(found) if found.query == self.query => {
                        self.subtitle.set_text(if found.total > found.hits.len() {
                            format!(
                                "{} packages, the first {} are shown",
                                found.total,
                                found.hits.len()
                            )
                        } else {
                            format!("{} packages", found.total)
                        });
                        self.hits = found.hits;
                        self.table.reload_data();
                    }
                    Ok(_) => {}
                    Err(error) => toast::failure(&error),
                }
            },
        );
    }

    fn refresh(self: Weak<Self>) {
        self.find(self.search.text().to_owned());
    }

    fn open_page(self: Weak<Self>, index: usize) {
        let Some(hit) = self.hits.get(index) else {
            return;
        };
        if let Err(error) = open_url(&hit.page_url) {
            toast::error(format!("cannot open the page: {error}"));
        }
    }

    fn add(self: Weak<Self>, index: usize) {
        let Some(hit) = self.hits.get(index) else {
            return;
        };
        let id = hit.id.clone();
        backend::change(
            "adding the mod",
            |forge, progress| async move {
                let profile = backend::profile(forge, &progress).await?;
                let (id, change) = forge.add(&profile, &id, &progress).await?;
                forge.sync(&profile, &progress).await?;
                Ok(format!("added {id}, {}", lock_change_summary(&change)))
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
}

impl TableData for BrowsePage {
    fn cell_height(&self, _: usize) -> f32 {
        ROW_HEIGHT
    }

    fn number_of_cells(&self) -> usize {
        self.hits.len()
    }

    fn setup_cell(&mut self, index: usize, registry: &mut CellRegistry) -> Weak<dyn View> {
        let cell = registry.cell::<HitCell>();
        cell.set_hit(index, weak_from_ref(self), &self.hits[index]);
        cell
    }

    fn cell_selected(&mut self, index: usize) {
        let page = weak_from_ref(self);
        if let Some(hit) = self.hits.get(index) {
            ModInfo::open(&hit.id, move |changed| {
                if changed && page.is_ok() {
                    page.refresh();
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

        // The whole row width and 2 lines. Thunderstore caps a description at
        // 250 characters, so at a normal window size nothing is cut. A click
        // on the row opens the details with the full text in any case.
        style::dim(self.description);
        self.description.set_multiline(true);
        self.description
            .set_vertical_alignment(VerticalAlignment::Top);
        self.description
            .place()
            .t(TOP_LINE + 46.0)
            .l(TEXT_LEFT)
            .r(16)
            .h(36);

        // Same rectangle as the add button, so the column reads as one shape.
        self.installed.set_text("installed");
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
        self.open_page.set_tooltip("open on Thunderstore");
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
                self.page.add(self.index);
            }
        });

        // The wash says the row can be clicked, the click opens the details.
        hover::row(self);

        self.line.set_color(colors::BORDER);
        self.line.place().l(0).r(0).b(0).h(1);
    }
}

impl HitCell {
    fn set_hit(mut self: Weak<Self>, index: usize, page: Weak<BrowsePage>, hit: &Hit) {
        self.index = index;
        self.page = page;
        self.icon.show(&hit.id, &hit.version);
        self.name.set_text(names::title(&hit.id));
        self.author.set_text(names::author(&hit.id));
        self.description.set_text(&hit.description);

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
            let deprecated = self.deprecated.set("pill_deprecated.svg", "deprecated");
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
            let broken = self.broken.set("pill_broken.svg", "broken");
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
