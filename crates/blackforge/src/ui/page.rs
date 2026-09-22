#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Mods,
    Browse,
    Configs,
    Friends,
    Servers,
    Share,
    Doctor,
    Settings,
}

impl Page {
    pub const ALL: [Self; 8] = [
        Self::Mods,
        Self::Browse,
        Self::Configs,
        Self::Friends,
        Self::Servers,
        Self::Share,
        Self::Doctor,
        Self::Settings,
    ];

    /// Lucide line icons, <https://lucide.dev>, ISC licensed. The accent color
    /// is baked into the files, an `ImageView` cannot tint.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Mods => "nav_mods.svg",
            Self::Browse => "nav_browse.svg",
            Self::Configs => "nav_configs.svg",
            Self::Friends => "nav_friends.svg",
            Self::Servers => "nav_servers.svg",
            Self::Share => "nav_share.svg",
            Self::Doctor => "nav_doctor.svg",
            Self::Settings => "nav_settings.svg",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Mods => "Mods",
            Self::Browse => "Browse",
            Self::Configs => "Configs",
            Self::Friends => "Friends",
            Self::Servers => "Servers",
            Self::Share => "Share",
            Self::Doctor => "Doctor",
            Self::Settings => "Settings",
        }
    }
}
