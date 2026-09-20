use hilen::{gm::color::Color, ui::DynamicColor};

const fn pair(light: &str, dark: &str) -> DynamicColor {
    DynamicColor::new(Color::hex(light), Color::hex(dark))
}

pub const CLEAR: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);

// The page and the ink written straight on it.
pub const BG: DynamicColor = pair("#e6d8b8", "#17130f");
pub const INK: DynamicColor = pair("#2c1d0f", "#e6d6b5");
pub const INK_DIM: DynamicColor = pair("#4a3519", "#c4b492");
pub const INK_TITLE: DynamicColor = pair("#4a2c10", "#d2a75e");
/// The second copy of a line of ink, one point lower. Light under dark ink
/// reads as pressed into the sheet, black under light ink as raised.
pub const INK_LIFT: DynamicColor = DynamicColor::new(
    Color::rgba(1.0, 0.97, 0.9, 0.75),
    Color::rgba(0.0, 0.0, 0.0, 0.85),
);
pub const RULE: DynamicColor = pair("#6b4a22", "#b58d4a");
pub const RULE_LIFT: DynamicColor = DynamicColor::new(
    Color::rgba(1.0, 0.97, 0.9, 0.7),
    Color::rgba(0.0, 0.0, 0.0, 0.7),
);

/// The corners of the page darken, a candle lit room by night and an aged
/// sheet by day. The start keeps the hue with no alpha, so the ramp does not
/// pass through gray.
pub const VIGNETTE_START: DynamicColor = DynamicColor::new(
    Color::rgba(0.29, 0.19, 0.06, 0.0),
    Color::rgba(0.0, 0.0, 0.0, 0.0),
);
pub const VIGNETTE_END: DynamicColor = DynamicColor::new(
    Color::rgba(0.29, 0.19, 0.06, 0.45),
    Color::rgba(0.0, 0.0, 0.0, 0.72),
);

// Brass letters on wood, the same in both themes.
pub const WOOD_BASE: DynamicColor = pair("#5d3c1f", "#3b2717");
pub const BRASS_TOP: Color = Color::hex("#f6dc98");
pub const BRASS_BOTTOM: Color = Color::hex("#b9873a");
pub const BRASS_DIM: Color = Color::hex("#c4a86f");
pub const WOOD_TEXT_SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.8);
pub const WOOD_TEXT_LIGHT: Color = Color::rgba(1.0, 0.93, 0.75, 0.22);
pub const WOOD_EDGE: Color = Color::rgba(0.0, 0.0, 0.0, 0.55);
pub const WOOD_EDGE_LIGHT: Color = Color::rgba(1.0, 0.9, 0.7, 0.16);
pub const WOOD_HOVER: Color = Color::rgba(1.0, 0.85, 0.55, 0.10);

// The brass plate that reads as the button of a download board.
pub const PLATE_TOP: Color = Color::hex("#e9c878");
pub const PLATE_BOTTOM: Color = Color::hex("#a2752f");
pub const PLATE_HOVER_TOP: Color = Color::hex("#fbe4a4");
pub const PLATE_HOVER_BOTTOM: Color = Color::hex("#bf9040");
pub const PLATE_EDGE: Color = Color::hex("#2e1d08");
pub const PLATE_TEXT: Color = Color::hex("#2b1a06");
pub const PLATE_TEXT_LIGHT: Color = Color::rgba(1.0, 0.95, 0.78, 0.55);
pub const PLATE_OFF_TOP: Color = Color::hex("#3d3b3f");
pub const PLATE_OFF_BOTTOM: Color = Color::hex("#232224");
pub const PLATE_OFF_EDGE: Color = Color::hex("#0b0a0b");
pub const PLATE_OFF_TEXT: Color = Color::hex("#98948d");
pub const PLATE_OFF_TEXT_SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.8);

// Forged iron.
pub const IRON_TOP: Color = Color::hex("#4c4a4f");
pub const IRON_BOTTOM: Color = Color::hex("#1f1e20");
pub const IRON_EDGE: Color = Color::hex("#0b0a0b");
/// A choice in the menu, sunk into the iron like a key.
pub const IRON_KEY: Color = Color::rgba(0.0, 0.0, 0.0, 0.28);
pub const IRON_TEXT: Color = Color::hex("#e6d6b5");
pub const IRON_TEXT_DIM: Color = Color::hex("#a39c90");

pub const SHADOW: DynamicColor = DynamicColor::new(
    Color::rgba(0.2, 0.12, 0.02, 0.45),
    Color::rgba(0.0, 0.0, 0.0, 0.75),
);
