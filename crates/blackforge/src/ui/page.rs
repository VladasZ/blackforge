#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Mods,
    Browse,
    Configs,
    Share,
    Game,
    Doctor,
}

impl Page {
    pub const ALL: [Self; 6] = [
        Self::Mods,
        Self::Browse,
        Self::Configs,
        Self::Share,
        Self::Game,
        Self::Doctor,
    ];

    /// Lucide line icons, <https://lucide.dev>, ISC licensed. The accent color
    /// is baked into the files, an `ImageView` cannot tint.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Mods => "nav_mods.svg",
            Self::Browse => "nav_browse.svg",
            Self::Configs => "nav_configs.svg",
            Self::Share => "nav_share.svg",
            Self::Game => "nav_game.svg",
            Self::Doctor => "nav_doctor.svg",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Mods => "Mods",
            Self::Browse => "Browse",
            Self::Configs => "Configs",
            Self::Share => "Share",
            Self::Game => "Game",
            Self::Doctor => "Doctor",
        }
    }
}
