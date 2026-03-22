/// Linear RGBA color
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub(crate) _a: f32,
}

impl Color {
    /// From sRGB hex: 0xRRGGBB, convert to linear
    pub fn from_srgb_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let b = (hex & 0xFF) as f32 / 255.0;
        Color {
            r: r.powf(2.2),
            g: g.powf(2.2),
            b: b.powf(2.2),
            _a: 1.0,
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
    /// Emphasis/flash color — themed accent for glitch corruption + UI highlights
    pub emphasis: Color,
    /// True when background is perceptually light (luminance > 0.18 linear)
    pub is_light: bool,
}

/// Number of available themes
pub const THEME_COUNT: usize = 14;

/// Theme names for UI display
pub const THEME_NAMES: [&str; THEME_COUNT] = [
    "pink", "kerama", "brazil", "noni", "paris", "rooney", "k+k",
    "catppuccin", "kanagawa", "rose pine", "dracula", "papaya", "dominican", "calsonic",
];

impl ColorPalette {
    /// Compute whether a background is perceptually light (linear luminance)
    fn bg_is_light(bg: &Color) -> bool {
        bg.r * 0.2126 + bg.g * 0.7152 + bg.b * 0.0722 > 0.18
    }

    /// Build palette from theme index (0-13) and dark mode flag
    pub fn from_id_and_mode(id: usize, dark: bool) -> Self {
        match id {
            0  => if dark { Self::pink_dark() }       else { Self::pink_light() },
            1  => if dark { Self::kerama_dark() }     else { Self::kerama_light() },
            2  => if dark { Self::brazil_dark() }     else { Self::brazil_light() },
            3  => if dark { Self::noni_dark() }       else { Self::noni_light() },
            4  => if dark { Self::paris_dark() }      else { Self::paris_light() },
            5  => if dark { Self::rooney_dark() }     else { Self::rooney_light() },
            6  => if dark { Self::kk_dark() }         else { Self::kk_light() },
            7  => if dark { Self::catppuccin_dark() } else { Self::catppuccin_light() },
            8  => if dark { Self::kanagawa_dark() }   else { Self::kanagawa_light() },
            9  => if dark { Self::rosepine_dark() }   else { Self::rosepine_light() },
            10 => if dark { Self::dracula_dark() }    else { Self::dracula_light() },
            11 => if dark { Self::papaya_dark() }     else { Self::papaya_light() },
            12 => if dark { Self::dominican_dark() }  else { Self::dominican_light() },
            13 => if dark { Self::calsonic_dark() }   else { Self::calsonic_light() },
            _  => Self::noni_dark(),
        }
    }

    // Helper to build a palette with auto is_light
    fn build(bg: u32, primary: u32, accent: u32, fg: u32, muted: u32, border: u32) -> Self {
        let background = Color::from_srgb_hex(bg);
        ColorPalette {
            primary: Color::from_srgb_hex(primary),
            secondary: [
                Color::from_srgb_hex(accent),
                Color::from_srgb_hex(fg),
                Color::from_srgb_hex(muted),
                Color::from_srgb_hex(border),
            ],
            background,
            emphasis: Color::from_srgb_hex(accent), // flash = accent color (themed)
            is_light: Self::bg_is_light(&background),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 0. Pink — hue 340 (pink-magenta) + hue 355 (rose)
    // ═══════════════════════════════════════════════════════════════════════════

    fn pink_dark() -> Self {
        //           bg        primary       accent(sec0)  fg(sec1)    muted(sec2)  ghost(sec3)
        Self::build(0x2D1028, 0xFF88EB,     0xFF64A9,     0xF3EBF0,   0x968991,    0x4D2842)
    }
    fn pink_light() -> Self {
        Self::build(0xFDF5FA, 0xE36ABF,     0xFF9FDC,     0x2C2329,   0x796C74,    0xD098B8)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 1. Kerama Blue — hue 250 (cobalt) + hue 195 (teal)
    // ═══════════════════════════════════════════════════════════════════════════

    fn kerama_dark() -> Self {
        Self::build(0x0E1830, 0x31A4FA,     0x0067AB,     0xD9E5F0,   0x71889B,    0x283858)
    }
    fn kerama_light() -> Self {
        Self::build(0xE9F6FF, 0x0071D3,     0x00C7C7,     0x16212C,   0x526578,    0x90B0D8)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 2. Brazil — hue 145 (green) + hue 95 (yellow)
    // ═══════════════════════════════════════════════════════════════════════════

    fn brazil_dark() -> Self {
        //           bg        primary(green) accent(yellow) fg          muted        ghost
        Self::build(0x0C1030, 0x10922C,      0xFFDA24,      0xDBE7DB,   0x839283,    0x283858)
    }
    fn brazil_light() -> Self {
        let background = Color::from_srgb_hex(0xECF8EC);
        ColorPalette {
            primary:    Color::from_srgb_hex(0x007400),   // green
            secondary: [
                Color::from_srgb_hex(0xFFDA24),  // accent: yellow
                Color::from_srgb_hex(0x1A202C),  // fg: dark
                Color::from_srgb_hex(0x586379),  // muted
                Color::from_srgb_hex(0x88C888),  // ghost: soft green
            ],
            emphasis: Color::from_srgb_hex(0xFFDA24),
            is_light: Self::bg_is_light(&background),
            background,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 3. Noni — hue 118 (olive) + hue 122 (lime) — PRESERVED EXACTLY
    // ═══════════════════════════════════════════════════════════════════════════

    fn noni_dark() -> Self {
        let background = Color::from_srgb_hex(0x151805);
        ColorPalette {
            primary:    Color::from_srgb_hex(0x7D9100),   // --primary
            secondary: [
                Color::from_srgb_hex(0x99B741),  // --accent
                Color::from_srgb_hex(0xDBDFCD),  // --fg
                Color::from_srgb_hex(0x828968),  // --muted
                Color::from_srgb_hex(0x3F4720),  // ghost
            ],
            background,
            emphasis:   Color::from_srgb_hex(0x99B741),
            is_light: Self::bg_is_light(&background),
        }
    }
    fn noni_light() -> Self {
        let background = Color::from_srgb_hex(0xDAE0C9);
        ColorPalette {
            primary:    Color::from_srgb_hex(0x6C7E00),   // --primary
            secondary: [
                Color::from_srgb_hex(0xBEE05F),  // --accent
                Color::from_srgb_hex(0x33391D),  // --fg
                Color::from_srgb_hex(0x53593B),  // --muted
                Color::from_srgb_hex(0x283010),  // ghost
            ],
            emphasis: Color::from_srgb_hex(0xBEE05F),
            is_light: Self::bg_is_light(&background),
            background,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 4. Paris — hue 328 (electric fuchsia) + hue 72 (champagne gold) — IMPROVED
    // ═══════════════════════════════════════════════════════════════════════════

    fn paris_dark() -> Self {
        //           bg        primary(fuchsia) accent(gold)  fg          muted        ghost
        Self::build(0x1A0818, 0xFF5FFF,         0xFFC273,     0xF2EBF1,   0x8F7E8D,    0x482840)
    }
    fn paris_light() -> Self {
        Self::build(0xFFF3FF, 0xD517D6,         0xFFD998,     0x251D24,   0x726270,    0xD098C0)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 5. Rooney — hue 22 (Man Utd red) + hue 78 (badge gold)
    // ═══════════════════════════════════════════════════════════════════════════

    fn rooney_dark() -> Self {
        let background = Color::from_srgb_hex(0x140001);
        ColorPalette {
            primary:    Color::from_srgb_hex(0xFFB000),  // --primary: GOLD (not red!)
            secondary: [
                Color::from_srgb_hex(0xFFAD00),  // --accent: gold
                Color::from_srgb_hex(0xFBF2F1),  // --fg: near-white
                Color::from_srgb_hex(0x996C69),  // --muted
                Color::from_srgb_hex(0x4C1013),  // ghost: deep red
            ],
            emphasis: Color::from_srgb_hex(0xFFAD00),
            is_light: Self::bg_is_light(&background),
            background,
        }
    }
    fn rooney_light() -> Self {
        //           bg        primary(red)  accent(gold)  fg          muted        ghost
        Self::build(0xF8EDE8, 0xC90000,      0xFFB000,     0x1E0C0C,   0x825D5C,    0xD08878)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 6. k+k — near-zero chroma, grayscale
    // ═══════════════════════════════════════════════════════════════════════════

    fn kk_dark() -> Self {
        Self::build(0x1A1A20, 0xB0B6BF,     0x33363A,     0xE6EAF1,   0x7B7F85,    0x383840)
    }
    fn kk_light() -> Self {
        Self::build(0xF2F4F8, 0x383D44,     0xD2D6DD,     0x16181C,   0x6D7176,    0xC0C0C8)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 7. Catppuccin — hue 300 (mauve) + hue 265 (lavender)
    // ═══════════════════════════════════════════════════════════════════════════

    fn catppuccin_dark() -> Self {
        Self::build(0x1E1028, 0xC497F7,     0x84AAFF,     0xC2D3F5,   0x90A0C0,    0x382850)
    }
    fn catppuccin_light() -> Self {
        Self::build(0xD9E1EA, 0x8000E4,     0x4A7AFC,     0x3C3F5A,   0x6A738B,    0xA088C0)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 8. Kanagawa — hue 222 (wave blue) + hue 80 (amber)
    // ═══════════════════════════════════════════════════════════════════════════

    fn kanagawa_dark() -> Self {
        Self::build(0x101828, 0x6BB1CA,     0xDD9700,     0xE4D5B1,   0x6C6151,    0x282038)
    }
    fn kanagawa_light() -> Self {
        Self::build(0xE3D1A6, 0x287C95,     0xDD9700,     0x454758,   0x6B5E48,    0xA89868)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 9. Rosé Pine — hue 0 (rose) + hue 300 (iris)
    // ═══════════════════════════════════════════════════════════════════════════

    fn rosepine_dark() -> Self {
        Self::build(0x10101C, 0xDF6B93,     0xC1A2E7,     0xDCDDFB,   0x635883,    0x302838)
    }
    fn rosepine_light() -> Self {
        Self::build(0xF9EFE2, 0xA34E6B,     0x7F68A8,     0x473B67,   0x89839D,    0xC8B898)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 10. Dracula — hue 295 (purple) + hue 340 (pink)
    // ═══════════════════════════════════════════════════════════════════════════

    fn dracula_dark() -> Self {
        Self::build(0x1C1828, 0xB38EFF,     0xF860CD,     0xF8F6FE,   0x48619A,    0x382848)
    }
    fn dracula_light() -> Self {
        Self::build(0xF2EDFF, 0x8C4FE7,     0xF860CD,     0x1A1B28,   0x48619A,    0xB098C8)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 11. Papaya — hue 50 (McLaren orange) on achromatic neutrals
    // ═══════════════════════════════════════════════════════════════════════════

    fn papaya_dark() -> Self {
        Self::build(0x0C0C0C, 0xFF6E00,     0xFF9736,     0xEFEDEA,   0x7F7F7F,    0x2C2C2C)
    }
    fn papaya_light() -> Self {
        Self::build(0xF8F4ED, 0xFF6C00,     0xE05000,     0x131313,   0x5D5D5D,    0xD0C8B8)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 12. Dominican — hue 264 (royal blue) + hue 22 (flag red)
    // ═══════════════════════════════════════════════════════════════════════════

    fn dominican_dark() -> Self {
        //           bg        primary(blue) accent(red)   fg          muted        ghost
        Self::build(0x101840, 0x4E83DE,     0xDB898B,     0xD0D8F0,   0x6878A0,    0x283060)
    }
    fn dominican_light() -> Self {
        Self::build(0xEDF5FF, 0x3668BF,     0xDB898B,     0x101030,   0x486090,    0x98A8C8)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 13. Calsonic — hue 260 (ocean blue) + hue 18 (coral salmon)
    // ═══════════════════════════════════════════════════════════════════════════

    fn calsonic_dark() -> Self {
        Self::build(0x101838, 0x4E83DE,     0xDB898B,     0xF1F1F1,   0x67788D,    0x283058)
    }
    fn calsonic_light() -> Self {
        Self::build(0xDFEBFC, 0x3668BF,     0xDB898B,     0x002856,   0x54698A,    0x98A8C8)
    }
}
