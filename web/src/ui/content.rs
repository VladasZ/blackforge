use hilen::{
    dispatch::spawn,
    refs::Weak,
    system::open_url,
    ui::{Container, Gradient, Label, Setup, UIEvents, ViewData, ViewFrame, ViewTouch, view},
};

use crate::{
    api, art, fonts,
    model::{Manifest, version_of},
    net::download_url,
    ui::{
        colors,
        download_button::{self, DownloadButton},
        drop_menu::DropMenu,
        embossed::Embossed,
        heading::{self, Heading},
        sign::Sign,
        textured::Textured,
    },
};

#[view]
pub struct Content {
    manifest: Manifest,

    #[init]
    ground: Textured,
    specks: Textured,
    vignette: Container,
    sign: Sign,
    description: Embossed,
    downloads: Heading,
    status: Label,
    mac: DownloadButton,
    win: DownloadButton,
    linux: DownloadButton,
    footer_rule: Container,
    footer_lift: Container,
    footer: Label,
    win_menu: DropMenu,
    linux_menu: DropMenu,
}

impl Setup for Content {
    fn setup(self: Weak<Self>) {
        self.ground.place().back();
        self.specks.place().back();
        self.vignette.place().back();

        self.description
            .set_shade(colors::INK_LIFT)
            .set_text(
                "Install Valheim mods from Thunderstore.\nEdit mod settings and launch the game.",
            )
            .each(|label| {
                label
                    .set_font(fonts::inter(500.0))
                    .set_line_height(30)
                    .set_multiline(true);
            });
        self.description.face().set_text_color(colors::INK);

        self.downloads.set_text("Downloads", 26.0);

        // Small text stays one plain copy. The second copy that presses the
        // big lines into the sheet only smears letters of this size.
        self.status
            .set_color(colors::CLEAR)
            .set_font(fonts::inter(500.0))
            .set_text_size(14)
            .set_multiline(true)
            .set_text_color(colors::INK_DIM)
            .set_text("Checking for releases…");

        self.mac
            .set_data("macOS", "Apple Silicon + Intel · DMG", "");
        self.win.set_data("Windows", "x64 / ARM64 · Installer", "");
        self.linux
            .set_data("Linux", "x64 / ARM64 · DEB / AppImage", "");

        self.footer_rule.set_color(colors::RULE);
        self.footer_lift.set_color(colors::RULE_LIFT);
        self.footer
            .set_color(colors::CLEAR)
            .set_font(fonts::heading(600.0))
            .set_text_size(12.5)
            .set_letter_spacing(2.5)
            .set_text_color(colors::INK_DIM)
            .set_text("Blackforge · VladasZ");

        self.win_menu.set_hidden(true);
        self.linux_menu.set_hidden(true);
        self.enable_touch_low_priority();
        self.touch().up_inside.sub(self, move || self.close_menus());
        self.mac.on_tap.sub(self, move || {
            self.close_menus();
            if let Some(file) = self.manifest.mac.clone() {
                self.pick("mac", file, None);
            }
        });
        self.win.on_tap.sub(self, move || {
            self.win_menu.set_hidden(!self.win_menu.is_hidden());
            self.linux_menu.set_hidden(true);
        });
        self.linux.on_tap.sub(self, move || {
            self.linux_menu.set_hidden(!self.linux_menu.is_hidden());
            self.win_menu.set_hidden(true);
        });
        self.win_menu.on_pick.val(self, move |(file, arch)| {
            self.close_menus();
            self.pick("windows", file, Some(arch));
        });
        self.linux_menu.on_pick.val(self, move |(file, arch)| {
            self.close_menus();
            self.pick("linux", file, Some(arch));
        });

        self.restyle();
        UIEvents::theme_changed().sub(self, move || self.restyle());
    }
}

impl Content {
    // A texture and a gradient hold plain colors, the engine re-resolves
    // only view and text colors on a theme switch.
    fn restyle(self: Weak<Self>) {
        self.ground
            .set_texture(art::ground(), art::GROUND_TILE, (true, true));
        self.specks
            .set_texture(art::specks(), art::SPECKS_TILE, (true, true));
        self.vignette.apply_gradient(Gradient::radial_at(
            (0.5, 0.3),
            colors::VIGNETTE_START.resolve(),
            colors::VIGNETTE_END.resolve(),
        ));
    }

    fn close_menus(self: Weak<Self>) {
        self.win_menu.set_hidden(true);
        self.linux_menu.set_hidden(true);
    }

    pub fn manifest_failed(self: Weak<Self>) {
        self.status
            .set_text("Downloads are unavailable. The release list could not be loaded.");
    }

    pub fn set_manifest(mut self: Weak<Self>, manifest: Manifest) {
        self.manifest = manifest;
        let win = self.manifest.win_choices();
        let linux = self.manifest.linux_groups();
        self.mac.set_data(
            "macOS",
            "Apple Silicon + Intel · DMG",
            &self
                .manifest
                .mac
                .as_deref()
                .map(version_of)
                .unwrap_or_default(),
        );
        self.win.set_data(
            "Windows",
            "x64 / ARM64 · Installer",
            &win.first().map(|c| version_of(&c.file)).unwrap_or_default(),
        );
        self.linux.set_data(
            "Linux",
            "x64 / ARM64 · DEB / AppImage",
            &linux
                .first()
                .and_then(|(_, c)| c.first())
                .map(|c| version_of(&c.file))
                .unwrap_or_default(),
        );
        self.status.set_text(
            if self.manifest.mac.is_some() || !win.is_empty() || !linux.is_empty() {
                ""
            } else {
                "No public release yet."
            },
        );
    }

    pub fn relayout(self: Weak<Self>, width: f32, viewport: f32) -> f32 {
        let pad = if width < 600.0 { 20.0 } else { 48.0 };
        let inner = (width - pad * 2.0).clamp(1.0, 1040.0);
        let x = (width - inner) / 2.0;

        let mut y = if width < 600.0 { 28.0 } else { 44.0 };
        let sign_height = self.sign.height_for_width(inner);
        self.sign.set_frame((x, y, inner, sign_height));
        y += sign_height + 40.0;

        self.description.each(|label| {
            label.set_text_size(if width < 600.0 { 17 } else { 19 });
        });
        let description_height = self.description.height_for_width(inner);
        self.description
            .set_frame((x, y, inner, description_height));
        y += description_height + 48.0;

        self.downloads.set_frame((x, y, inner, heading::HEIGHT));
        y += heading::HEIGHT + 14.0;

        // The line speaks only while the list loads, or when there is
        // nothing to download. With releases on the boards it takes no room.
        if self.status.text().is_empty() {
            self.status.set_frame((x, y, inner, 0.0));
            y += 16.0;
        } else {
            let status_height = self.status.size_for_width(inner).height.max(22.0);
            self.status.set_frame((x, y, inner, status_height));
            y += status_height + 26.0;
        }

        let columns = if inner >= 780.0 { 3 } else { 1 };
        let card_width = (inner - 20.0 * (columns as f32 - 1.0)) / columns as f32;
        let mut anchors = Vec::new();
        for (i, button) in [self.mac, self.win, self.linux].into_iter().enumerate() {
            let bx = x + (i % columns) as f32 * (card_width + 20.0);
            let by = y + (i / columns) as f32 * (download_button::HEIGHT + 20.0);
            button.set_frame((bx, by, card_width, download_button::HEIGHT));
            anchors.push((bx, by + download_button::HEIGHT + 8.0));
        }
        let win_height = self.win_menu.set_groups(
            &[("Windows".to_string(), self.manifest.win_choices())],
            card_width,
            false,
            None,
        );
        self.win_menu
            .set_frame((anchors[1].0, anchors[1].1, card_width, win_height));
        let linux_height =
            self.linux_menu
                .set_groups(&self.manifest.linux_groups(), card_width, true, None);
        self.linux_menu
            .set_frame((anchors[2].0, anchors[2].1, card_width, linux_height));
        y += (3 / columns) as f32 * (download_button::HEIGHT + 20.0);

        y = (y + 64.0).max(viewport - 58.0);
        self.footer_rule.set_frame((x, y, inner, 1.0));
        self.footer_lift.set_frame((x, y + 1.0, inner, 1.0));
        self.footer.set_frame((x, y + 18.0, inner, 22.0));
        y + 58.0
    }

    fn pick(self: Weak<Self>, os: &'static str, file: String, arch: Option<String>) {
        let url = download_url(&file);
        if let Err(error) = open_url(&url) {
            log::error!("Failed to open {url}: {error}");
        }
        let version = version_of(&file);
        spawn(async move {
            api::track(os, &version, arch.as_deref()).await;
        });
    }
}
