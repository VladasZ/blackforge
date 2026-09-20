use hilen::{
    dispatch::spawn,
    refs::Weak,
    system::open_url,
    ui::{Container, Label, Setup, TextAlignment, ViewData, ViewFrame, ViewTouch, view},
};

use crate::{
    api, fonts,
    model::{Manifest, version_of},
    net::download_url,
    ui::{
        colors,
        download_button::{self, DownloadButton},
        drop_menu::DropMenu,
        text_margin,
    },
};

#[view]
pub struct Content {
    manifest: Manifest,

    #[init]
    top_rule: Container,
    bottom_rule: Container,
    eyebrow: Label,
    title: Label,
    description: Label,
    rule: Container,
    rule_inner: Container,
    download_title: Label,
    status: Label,
    mac: DownloadButton,
    win: DownloadButton,
    linux: DownloadButton,
    details_title: Label,
    details: Label,
    footer: Label,
    win_menu: DropMenu,
    linux_menu: DropMenu,
}

impl Setup for Content {
    fn setup(self: Weak<Self>) {
        self.top_rule.set_color(colors::BORDER);
        self.bottom_rule.set_color(colors::BORDER);
        self.rule_inner.set_color(colors::BORDER_SOFT);
        for label in [
            self.eyebrow,
            self.title,
            self.description,
            self.download_title,
            self.status,
            self.details_title,
            self.details,
            self.footer,
        ] {
            label.set_color(colors::CLEAR);
            label.set_alignment(TextAlignment::Left);
            label.set_font(fonts::inter(400.0));
            label.set_text_color(colors::FG);
        }
        self.eyebrow
            .set_font(fonts::heading(500.0))
            .set_text_size(12)
            .set_letter_spacing(2.0)
            .set_text_color(colors::FG_DIM)
            .set_text("VALHEIM / MOD MANAGER");
        self.title
            .set_font(fonts::heading(700.0))
            .set_text_color(colors::ACCENT)
            .set_text("Blackforge");
        self.description.set_text_size(19).set_line_height(29).set_multiline(true)
            .set_text("Install Valheim mods from Thunderstore.\nKeep separate profiles, edit mod settings and launch the game.");
        self.rule.set_color(colors::BORDER);
        self.download_title
            .set_font(fonts::heading(600.0))
            .set_text_size(24)
            .set_text("Downloads");
        self.status
            .set_text_size(13)
            .set_text_color(colors::FG_DIM)
            .set_multiline(true)
            .set_text("Checking for releases…");
        self.mac
            .set_data("macOS", "Apple Silicon + Intel · DMG", "");
        self.win.set_data("Windows", "x64 / ARM64 · Installer", "");
        self.linux
            .set_data("Linux", "x64 / ARM64 · DEB / AppImage", "");
        self.details_title
            .set_font(fonts::heading(500.0))
            .set_text_size(12)
            .set_text_color(colors::FG_DIM)
            .set_text("The mod manager");
        self.details.set_text_size(16).set_line_height(32).set_multiline(true).set_text(
            "I.     Browse and install mods with dependencies\nII.    Create profiles for different mod sets\nIII.   Edit configuration files and launch Valheim"
        );
        self.footer
            .set_text_size(12)
            .set_text_color(colors::FG_DIM)
            .set_text("Blackforge · Vladas Zakrevskis");
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
    }
}

impl Content {
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
                "Choose your operating system. Windows and Linux offer a choice of builds."
            } else {
                "No public release yet."
            },
        );
    }

    pub fn relayout(self: Weak<Self>, width: f32, viewport: f32) -> f32 {
        let pad = if width < 600.0 { 24.0 } else { 48.0 };
        let inner = (width - pad * 2.0).clamp(1.0, 1040.0);
        let x = (width - inner) / 2.0;
        let text_x = x - text_margin();
        let text_width = inner + text_margin();
        self.top_rule.set_frame((x, 22.0, inner, 1.0));
        self.eyebrow.set_frame((text_x, 38.0, text_width, 24.0));
        self.title.set_text_size((inner / 7.5).min(76.0));
        self.title.set_frame((text_x, 76.0, text_width, 96.0));
        let desc_height = self.description.size_for_width(inner).height;
        self.description
            .set_frame((text_x, 188.0, text_width, desc_height));
        let mut y = 188.0 + desc_height + 36.0;
        self.rule.set_frame((x, y, inner, 1.0));
        self.rule_inner.set_frame((x, y + 5.0, inner, 1.0));
        y += 32.0;
        self.download_title
            .set_text_size(if width < 400.0 { 21.0 } else { 24.0 });
        self.download_title.set_frame((text_x, y, text_width, 36.0));
        y += 44.0;
        let status_height = self.status.size_for_width(inner).height.max(24.0);
        self.status
            .set_frame((text_x, y, text_width, status_height));
        y += status_height + 20.0;
        let columns = if inner >= 780.0 { 3 } else { 1 };
        let card_width = (inner - 16.0 * (columns as f32 - 1.0)) / columns as f32;
        let mut anchors = Vec::new();
        for (i, button) in [self.mac, self.win, self.linux].into_iter().enumerate() {
            let bx = x + (i % columns) as f32 * (card_width + 16.0);
            let by = y + (i / columns) as f32 * (download_button::HEIGHT + 12.0);
            button.set_frame((bx, by, card_width, download_button::HEIGHT));
            anchors.push((bx, by + download_button::HEIGHT + 6.0));
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
        y += (3 / columns) as f32 * (download_button::HEIGHT + 12.0) + 40.0;
        self.details_title.set_frame((text_x, y, text_width, 24.0));
        y += 36.0;
        let details_height = self.details.size_for_width(inner).height;
        self.details
            .set_frame((text_x, y, text_width, details_height));
        y = (y + details_height + 44.0).max(viewport - 56.0);
        self.footer.set_frame((text_x, y, text_width, 24.0));
        self.bottom_rule.set_frame((x, y - 16.0, inner, 1.0));
        y + 48.0
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
