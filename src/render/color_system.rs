use bytemuck::{Pod, Zeroable};

/// Linear RGBA color
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { r, g, b, a }
    }

    /// From sRGB hex: 0xRRGGBB, convert to linear
    pub fn from_srgb_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let b = (hex & 0xFF) as f32 / 255.0;
        Color {
            r: r.powf(2.2),
            g: g.powf(2.2),
            b: b.powf(2.2),
            a: 1.0,
        }
    }
}

pub struct ColorPalette {
    /// Primary glyph color (base image — bright and prominent)
    pub primary: Color,
    /// Secondary colors for overlay layers (kept dim behind primary)
    pub secondary: [Color; 4],
    /// Canvas background
    pub background: Color,
    /// Kept for ABI compat
    pub emphasis: Color,
}

impl ColorPalette {
    /// Noni Dark — hue 118 (yellow-green), exact coco-skream dark values
    pub fn noni_dark() -> Self {
        ColorPalette {
            primary:    Color::from_srgb_hex(0x9BB940), // bright lime (accent)
            secondary: [
                Color::from_srgb_hex(0x7E9200),         // olive (active)
                Color::from_srgb_hex(0xDCE1CE),         // light foreground
                Color::from_srgb_hex(0x838B68),         // muted-fg
                Color::from_srgb_hex(0x3F4720),         // border
            ],
            background: Color::from_srgb_hex(0x151805), // deep dark green
            emphasis:   Color::from_srgb_hex(0xDCE1CE),
        }
    }

    /// Noni Light — hue 118, exact coco-skream light values
    pub fn noni_light() -> Self {
        ColorPalette {
            primary:    Color::from_srgb_hex(0x6D8000), // medium olive green
            secondary: [
                Color::from_srgb_hex(0x9BB940),         // bright lime accent
                Color::from_srgb_hex(0x535939),         // muted text
                Color::from_srgb_hex(0xB3BD92),         // border
                Color::from_srgb_hex(0x303617),         // dark foreground
            ],
            background: Color::from_srgb_hex(0xF1F3EA), // soft sage white
            emphasis:   Color::from_srgb_hex(0x303617),
        }
    }

    /// Paris Dark — hue 328/330 (hot pink + gold), exact coco-skream values
    pub fn paris() -> Self {
        ColorPalette {
            primary:    Color::from_srgb_hex(0xFF5FFF), // hot magenta
            secondary: [
                Color::from_srgb_hex(0xFFC474),         // warm gold
                Color::from_srgb_hex(0xF3ECF2),         // near-white fg
                Color::from_srgb_hex(0x91808F),         // muted pink-grey
                Color::from_srgb_hex(0x443042),         // deep border
            ],
            background: Color::from_srgb_hex(0x140813), // deep plum
            emphasis:   Color::from_srgb_hex(0xF3ECF2),
        }
    }

    /// Rooney Dark — hue 22 (Man Utd red + gold), exact coco-skream values
    pub fn rooney() -> Self {
        ColorPalette {
            primary:    Color::from_srgb_hex(0xFC000B), // Man Utd red
            secondary: [
                Color::from_srgb_hex(0xFFAF00),         // gold
                Color::from_srgb_hex(0xFCF3F2),         // near-white fg
                Color::from_srgb_hex(0x9B6C6A),         // muted warm
                Color::from_srgb_hex(0x4C1013),         // dark border
            ],
            background: Color::from_srgb_hex(0x140001), // near-black red
            emphasis:   Color::from_srgb_hex(0xFCF3F2),
        }
    }

    /// Brazil Light — hue 145 (forest teal) + gold, coco-skream brazil light
    pub fn brazil_light() -> Self {
        ColorPalette {
            primary:    Color::from_srgb_hex(0x007500), // forest teal-green
            secondary: [
                Color::from_srgb_hex(0xFFDB1F),         // bright yellow/gold
                Color::from_srgb_hex(0x1C882D),         // deeper green
                Color::from_srgb_hex(0xD8E5FF),         // soft blue
                Color::from_srgb_hex(0xC8DFC8),         // muted green border
            ],
            background: Color::from_srgb_hex(0xF4FAF4), // very light teal-white
            emphasis:   Color::from_srgb_hex(0x141A29),
        }
    }

    pub fn from_theme(theme_name: &str) -> Self {
        match theme_name {
            "theme-noni-light"   => Self::noni_light(),
            "theme-paris"        => Self::paris(),
            "theme-rooney"       => Self::rooney(),
            "theme-brazil-light" => Self::brazil_light(),
            _                    => Self::noni_dark(),
        }
    }

    pub fn layer_color(&self, layer_idx: usize) -> Color {
        if layer_idx == 0 { self.primary } else { self.secondary[(layer_idx - 1) % 4] }
    }
}
