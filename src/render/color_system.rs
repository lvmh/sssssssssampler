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

/// Convert OKLCh → linear sRGB `Color` (no gamma — output is already linear).
/// h_deg is in degrees (0–360). Suitable for the Color struct directly.
pub fn oklch_to_color(l: f32, c: f32, h_deg: f32) -> Color {
    let h = h_deg.to_radians();
    let a = c * h.cos();
    let b = c * h.sin();

    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;

    Color {
        r: ( 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3).clamp(0.0, 1.0),
        g: (-1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3).clamp(0.0, 1.0),
        b: (-0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3).clamp(0.0, 1.0),
        _a: 1.0,
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
    /// Per-depth vibrant chart colors: chart[depth] for overlay slots (depth 0–3)
    pub chart: [Color; 4],
}

/// Number of available themes
pub const THEME_COUNT: usize = 14;

/// Theme names for UI display
pub const THEME_NAMES: [&str; THEME_COUNT] = [
    "pink", "kerama", "brazil", "noni", "paris", "rooney", "k+k",
    "catppuccin", "kanagawa", "rose pine", "dracula", "papaya", "dr", "calsonic",
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
    fn build(bg: u32, primary: u32, accent: u32, fg: u32, muted: u32, border: u32, chart: [Color; 4]) -> Self {
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
            chart,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 0. Pink — hue 340 (pink-magenta) + hue 355 (rose)
    // ═══════════════════════════════════════════════════════════════════════════

    fn pink_dark() -> Self {
        let chart = [
            oklch_to_color(0.75, 0.18, 340.0),  // chart-1: hot pink
            oklch_to_color(0.70, 0.15,  25.0),  // chart-2: warm rose-orange
            oklch_to_color(0.65, 0.12, 300.0),  // chart-3: violet
            oklch_to_color(0.80, 0.20, 350.0),  // chart-4: deep rose
        ];
        Self::build(0x2D1028, 0xFF88EB, 0xFF64A9, 0xF3EBF0, 0x968991, 0x4D2842, chart)
    }
    fn pink_light() -> Self {
        let chart = [
            oklch_to_color(0.38, 0.22, 340.0),  // layer 0: deep magenta — strong anchor
            oklch_to_color(0.52, 0.26, 310.0),  // layer 1: rich violet-pink — transient pop
            oklch_to_color(0.45, 0.20, 358.0),  // layer 2: dark rose — drift layer
            oklch_to_color(0.58, 0.15, 325.0),  // layer 3: soft mauve — ghost echo
        ];
        Self::build(0xFAEEF6, 0xB5007A, 0x8B0057, 0x2C2329, 0x796C74, 0xD098B8, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 1. Kerama Blue — hue 250 (cobalt) + hue 195 (teal)
    // ═══════════════════════════════════════════════════════════════════════════

    fn kerama_dark() -> Self {
        let chart = [
            oklch_to_color(0.65, 0.26, 250.0),  // cobalt blue
            oklch_to_color(0.68, 0.24, 195.0),  // teal
            oklch_to_color(0.73, 0.20, 225.0),  // sky blue
            oklch_to_color(0.60, 0.26, 270.0),  // violet-blue
        ];
        Self::build(0x0E1830, 0x31A4FA, 0x0067AB, 0xD9E5F0, 0x71889B, 0x283858, chart)
    }
    fn kerama_light() -> Self {
        let chart = [
            oklch_to_color(0.32, 0.24, 250.0),  // layer 0: deep cobalt — strong anchor
            oklch_to_color(0.48, 0.22, 195.0),  // layer 1: dark teal — transient pop
            oklch_to_color(0.40, 0.20, 270.0),  // layer 2: indigo — drift layer
            oklch_to_color(0.55, 0.18, 215.0),  // layer 3: slate blue — ghost echo
        ];
        // Warmer bg so cobalt has something to fight against rather than pure white
        Self::build(0xE0EEF8, 0x004DA8, 0x007BA0, 0x10202C, 0x526578, 0x90B0D8, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 2. Brazil — hue 145 (green) + hue 95 (yellow)
    // ═══════════════════════════════════════════════════════════════════════════

    fn brazil_dark() -> Self {
        let chart = [
            oklch_to_color(0.65, 0.28, 145.0),  // vivid green
            oklch_to_color(0.88, 0.20,  92.0),  // bright yellow
            oklch_to_color(0.60, 0.26, 158.0),  // jungle green
            oklch_to_color(0.76, 0.24, 112.0),  // lime
        ];
        let background = Color::from_srgb_hex(0x0C1030);
        ColorPalette {
            primary:    Color::from_srgb_hex(0x10922C),
            secondary: [
                Color::from_srgb_hex(0xFFDA24),
                Color::from_srgb_hex(0xDBE7DB),
                Color::from_srgb_hex(0x839283),
                Color::from_srgb_hex(0x283858),
            ],
            emphasis: Color::from_srgb_hex(0xFFDA24),
            is_light: Self::bg_is_light(&background),
            background,
            chart,
        }
    }
    fn brazil_light() -> Self {
        let chart = [
            oklch_to_color(0.35, 0.22, 145.0),  // layer 0: deep forest green — strong anchor
            oklch_to_color(0.55, 0.28,  88.0),  // layer 1: rich mustard/olive — transient pop
            oklch_to_color(0.42, 0.18, 162.0),  // layer 2: jungle green — drift layer
            oklch_to_color(0.52, 0.20, 130.0),  // layer 3: lime-green — ghost echo
        ];
        let background = Color::from_srgb_hex(0xDEF0DE);
        ColorPalette {
            primary:    Color::from_srgb_hex(0x005A00),
            secondary: [
                Color::from_srgb_hex(0xD4B000),
                Color::from_srgb_hex(0x1A202C),
                Color::from_srgb_hex(0x586379),
                Color::from_srgb_hex(0x88C888),
            ],
            emphasis: Color::from_srgb_hex(0xD4B000),
            is_light: Self::bg_is_light(&background),
            background,
            chart,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 3. Noni — hue 118 (olive) + hue 122 (lime) — PRESERVED EXACTLY
    // ═══════════════════════════════════════════════════════════════════════════

    fn noni_dark() -> Self {
        let chart = [
            oklch_to_color(0.68, 0.28, 118.0),  // vivid olive-green
            oklch_to_color(0.76, 0.26, 122.0),  // lime
            oklch_to_color(0.62, 0.24, 135.0),  // moss green
            oklch_to_color(0.82, 0.20, 100.0),  // yellow-green
        ];
        let background = Color::from_srgb_hex(0x151805);
        ColorPalette {
            primary:    Color::from_srgb_hex(0x7D9100),
            secondary: [
                Color::from_srgb_hex(0x99B741),
                Color::from_srgb_hex(0xDBDFCD),
                Color::from_srgb_hex(0x828968),
                Color::from_srgb_hex(0x3F4720),
            ],
            background,
            emphasis:   Color::from_srgb_hex(0x99B741),
            is_light: Self::bg_is_light(&background),
            chart,
        }
    }
    fn noni_light() -> Self {
        let chart = [
            oklch_to_color(0.33, 0.20, 118.0),  // layer 0: deep olive-black — strong anchor
            oklch_to_color(0.50, 0.26, 105.0),  // layer 1: vivid olive-yellow — transient pop
            oklch_to_color(0.40, 0.18, 135.0),  // layer 2: dark moss — drift layer
            oklch_to_color(0.55, 0.16, 118.0),  // layer 3: mid olive — ghost echo
        ];
        let background = Color::from_srgb_hex(0xD0D8B8);
        ColorPalette {
            primary:    Color::from_srgb_hex(0x6C7E00),
            secondary: [
                Color::from_srgb_hex(0xBEE05F),
                Color::from_srgb_hex(0x33391D),
                Color::from_srgb_hex(0x53593B),
                Color::from_srgb_hex(0x283010),
            ],
            emphasis: Color::from_srgb_hex(0xBEE05F),
            is_light: Self::bg_is_light(&background),
            background,
            chart,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 4. Paris — hue 328 (electric fuchsia) + hue 72 (champagne gold)
    // ═══════════════════════════════════════════════════════════════════════════

    fn paris_dark() -> Self {
        let chart = [
            oklch_to_color(0.78, 0.28, 328.0),  // electric fuchsia
            oklch_to_color(0.90, 0.22,  88.0),  // blazing yellow pop
            oklch_to_color(0.60, 0.18, 300.0),  // violet
            oklch_to_color(0.72, 0.22, 340.0),  // deep pink
        ];
        Self::build(0x1A0818, 0xFF5FFF, 0xFFC273, 0xF2EBF1, 0x8F7E8D, 0x482840, chart)
    }
    fn paris_light() -> Self {
        let chart = [
            oklch_to_color(0.36, 0.28, 328.0),  // layer 0: deep fuchsia-black — strong anchor
            oklch_to_color(0.50, 0.22, 295.0),  // layer 1: dark violet — transient pop
            oklch_to_color(0.42, 0.26, 350.0),  // layer 2: dark rose-red — drift layer
            oklch_to_color(0.58, 0.18, 315.0),  // layer 3: mauve — ghost echo
        ];
        Self::build(0xF8EEFF, 0xA800B0, 0x6B0097, 0x1A0D1C, 0x726270, 0xD098C0, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 5. Rooney — hue 22 (Man Utd red) + hue 78 (badge gold)
    // ═══════════════════════════════════════════════════════════════════════════

    fn rooney_dark() -> Self {
        let chart = [
            oklch_to_color(0.58, 0.30, 22.0),  // Man Utd red
            oklch_to_color(0.82, 0.20, 78.0),  // badge gold
            oklch_to_color(0.48, 0.28, 22.0),  // deep crimson
            oklch_to_color(0.72, 0.22, 35.0),  // orange-red
        ];
        let background = Color::from_srgb_hex(0x140001);
        ColorPalette {
            primary:    Color::from_srgb_hex(0xFFB000),
            secondary: [
                Color::from_srgb_hex(0xFFAD00),
                Color::from_srgb_hex(0xFBF2F1),
                Color::from_srgb_hex(0x996C69),
                Color::from_srgb_hex(0x4C1013),
            ],
            emphasis: Color::from_srgb_hex(0xFFAD00),
            is_light: Self::bg_is_light(&background),
            background,
            chart,
        }
    }
    fn rooney_light() -> Self {
        let chart = [
            oklch_to_color(0.34, 0.28,  22.0),  // layer 0: blood red-black — strong anchor
            oklch_to_color(0.50, 0.24,  45.0),  // layer 1: dark amber-gold — transient pop
            oklch_to_color(0.40, 0.26,  10.0),  // layer 2: dark crimson — drift layer
            oklch_to_color(0.55, 0.22,  32.0),  // layer 3: warm orange-red — ghost echo
        ];
        Self::build(0xF5E8E4, 0xA00000, 0xC07000, 0x1E0C0C, 0x825D5C, 0xD08878, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 6. k+k — near-zero chroma, grayscale
    // ═══════════════════════════════════════════════════════════════════════════

    fn kk_dark() -> Self {
        let chart = [
            oklch_to_color(0.78, 0.06, 250.0),  // cool blue-grey
            oklch_to_color(0.65, 0.05, 100.0),  // warm grey
            oklch_to_color(0.55, 0.06, 280.0),  // slate grey
            oklch_to_color(0.88, 0.03, 180.0),  // near-white
        ];
        Self::build(0x1A1A20, 0xB0B6BF, 0x33363A, 0xE6EAF1, 0x7B7F85, 0x383840, chart)
    }
    fn kk_light() -> Self {
        let chart = [
            oklch_to_color(0.22, 0.012, 260.0),  // layer 0: near-black cool gray — strong anchor
            oklch_to_color(0.40, 0.010, 230.0),  // layer 1: dark blue-gray — transient pop
            oklch_to_color(0.32, 0.008, 280.0),  // layer 2: dark slate — drift layer
            oklch_to_color(0.52, 0.008, 250.0),  // layer 3: medium gray — ghost echo
        ];
        Self::build(0xECEEF2, 0x282D34, 0xC0C4CC, 0x16181C, 0x6D7176, 0xC0C0C8, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 7. Catppuccin — hue 305 (mauve) + hue 265 (lavender)
    // ═══════════════════════════════════════════════════════════════════════════

    fn catppuccin_dark() -> Self {
        let chart = [
            oklch_to_color(0.76, 0.14, 305.0),  // mauve
            oklch_to_color(0.75, 0.13, 265.0),  // lavender
            oklch_to_color(0.70, 0.14, 190.0),  // teal
            oklch_to_color(0.74, 0.15, 130.0),  // green
        ];
        Self::build(0x1E1028, 0xC497F7, 0x84AAFF, 0xC2D3F5, 0x90A0C0, 0x382850, chart)
    }
    fn catppuccin_light() -> Self {
        let chart = [
            oklch_to_color(0.35, 0.26, 300.0),  // layer 0: deep mauve — strong anchor
            oklch_to_color(0.45, 0.24, 258.0),  // layer 1: dark lavender-blue — transient pop
            oklch_to_color(0.40, 0.20, 180.0),  // layer 2: dark teal — drift layer
            oklch_to_color(0.52, 0.18, 145.0),  // layer 3: forest green — ghost echo
        ];
        // Latte bg — warmer tinted parchment from Catppuccin Latte palette
        Self::build(0xE6E9F0, 0x7F00CC, 0x1E66F5, 0x3C3F5A, 0x6A738B, 0xA088C0, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 8. Kanagawa — hue 222 (wave blue) + hue 80 (amber)
    // ═══════════════════════════════════════════════════════════════════════════

    fn kanagawa_dark() -> Self {
        let chart = [
            oklch_to_color(0.65, 0.22, 222.0),  // wave blue
            oklch_to_color(0.76, 0.24,  80.0),  // amber
            oklch_to_color(0.60, 0.20, 200.0),  // slate blue
            oklch_to_color(0.72, 0.22,  55.0),  // sakura gold
        ];
        Self::build(0x101828, 0x6BB1CA, 0xDD9700, 0xE4D5B1, 0x6C6151, 0x282038, chart)
    }
    fn kanagawa_light() -> Self {
        let chart = [
            oklch_to_color(0.35, 0.18, 222.0),  // layer 0: deep wave blue — strong anchor
            oklch_to_color(0.48, 0.26,  72.0),  // layer 1: dark amber-gold — transient pop
            oklch_to_color(0.40, 0.18, 148.0),  // layer 2: dark spring green — drift layer
            oklch_to_color(0.50, 0.20,   8.0),  // layer 3: dark sakura red — ghost echo
        ];
        // Kanagawa lotus bg — warm parchment
        Self::build(0xD5C4A1, 0x1A5E72, 0xAA6C00, 0x3A3A4A, 0x6B5E48, 0xA89868, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 9. Rosé Pine — hue 0 (rose) + hue 300 (iris)
    // ═══════════════════════════════════════════════════════════════════════════

    fn rosepine_dark() -> Self {
        let chart = [
            oklch_to_color(0.62, 0.26,   0.0),  // rose love
            oklch_to_color(0.68, 0.24, 300.0),  // iris
            oklch_to_color(0.58, 0.22,  20.0),  // deep love
            oklch_to_color(0.65, 0.20, 332.0),  // moon foam
        ];
        Self::build(0x10101C, 0xDF6B93, 0xC1A2E7, 0xDCDDFB, 0x635883, 0x302838, chart)
    }
    fn rosepine_light() -> Self {
        let chart = [
            oklch_to_color(0.36, 0.18,   5.0),  // layer 0: deep wine rose — strong anchor
            oklch_to_color(0.42, 0.22, 295.0),  // layer 1: dark iris — transient pop
            oklch_to_color(0.40, 0.14, 195.0),  // layer 2: dark foam teal — drift layer
            oklch_to_color(0.52, 0.20,  80.0),  // layer 3: dark gold — ghost echo
        ];
        // Rosé Pine Dawn bg — warm cream
        Self::build(0xEEE4D4, 0x8B2252, 0x5C3F87, 0x3A3049, 0x89839D, 0xC8B898, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 10. Dracula — hue 295 (purple) + hue 340 (pink)
    // ═══════════════════════════════════════════════════════════════════════════

    fn dracula_dark() -> Self {
        let chart = [
            oklch_to_color(0.74, 0.18, 295.0),  // purple
            oklch_to_color(0.72, 0.22, 340.0),  // pink
            oklch_to_color(0.72, 0.18, 170.0),  // cyan
            oklch_to_color(0.78, 0.15,  75.0),  // yellow
        ];
        Self::build(0x1C1828, 0xB38EFF, 0xF860CD, 0xF8F6FE, 0x48619A, 0x382848, chart)
    }
    fn dracula_light() -> Self {
        let chart = [
            oklch_to_color(0.34, 0.26, 295.0),  // layer 0: deep dracula purple — strong anchor
            oklch_to_color(0.45, 0.28, 340.0),  // layer 1: dark pink — transient pop
            oklch_to_color(0.40, 0.22, 170.0),  // layer 2: dark cyan — drift layer
            oklch_to_color(0.52, 0.18, 290.0),  // layer 3: mid purple — ghost echo
        ];
        Self::build(0xE8E4F5, 0x6020C0, 0xCC0088, 0x1A1B28, 0x48619A, 0xB098C8, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 11. Papaya — hue 50 (McLaren orange) on achromatic neutrals
    // ═══════════════════════════════════════════════════════════════════════════

    fn papaya_dark() -> Self {
        let chart = [
            oklch_to_color(0.65, 0.28,  50.0),  // vivid orange
            oklch_to_color(0.75, 0.24,  35.0),  // amber
            oklch_to_color(0.70, 0.22,  68.0),  // yellow-orange
            oklch_to_color(0.58, 0.30,  42.0),  // deep orange
        ];
        Self::build(0x0C0C0C, 0xFF6E00, 0xFF9736, 0xEFEDEA, 0x7F7F7F, 0x2C2C2C, chart)
    }
    fn papaya_light() -> Self {
        let chart = [
            oklch_to_color(0.38, 0.26,  42.0),  // layer 0: burnt orange-black — strong anchor
            oklch_to_color(0.52, 0.28,  60.0),  // layer 1: dark amber — transient pop
            oklch_to_color(0.30, 0.05,  50.0),  // layer 2: near-black warm — drift layer
            oklch_to_color(0.55, 0.22,  55.0),  // layer 3: mid amber — ghost echo
        ];
        Self::build(0xF0EAE0, 0xC04800, 0xA03000, 0x0C0C0C, 0x5D5D5D, 0xD0C8B8, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 12. Dominican — hue 264 (royal blue) + hue 22 (flag red)
    // ═══════════════════════════════════════════════════════════════════════════

    fn dominican_dark() -> Self {
        let chart = [
            oklch_to_color(0.60, 0.26, 264.0),  // royal blue
            oklch_to_color(0.56, 0.24,  22.0),  // flag red
            oklch_to_color(0.65, 0.24, 240.0),  // ocean blue
            oklch_to_color(0.68, 0.22,  32.0),  // coral
        ];
        Self::build(0x101840, 0x4E83DE, 0xDB898B, 0xD0D8F0, 0x6878A0, 0x283060, chart)
    }
    fn dominican_light() -> Self {
        let chart = [
            oklch_to_color(0.30, 0.26, 264.0),  // layer 0: deep royal blue — strong anchor
            oklch_to_color(0.40, 0.28,  22.0),  // layer 1: dark flag red — transient pop
            oklch_to_color(0.36, 0.24, 242.0),  // layer 2: midnight blue — drift layer
            oklch_to_color(0.50, 0.22,  38.0),  // layer 3: dark coral — ghost echo
        ];
        Self::build(0xD8E8F8, 0x1840A0, 0xA02020, 0x080820, 0x486090, 0x98A8C8, chart)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 13. Calsonic — hue 258 (ocean blue) + hue 20 (coral salmon)
    // ═══════════════════════════════════════════════════════════════════════════

    fn calsonic_dark() -> Self {
        let chart = [
            oklch_to_color(0.60, 0.26, 258.0),  // ocean blue
            oklch_to_color(0.68, 0.24,  20.0),  // coral salmon
            oklch_to_color(0.65, 0.24, 240.0),  // deep indigo
            oklch_to_color(0.65, 0.22,  38.0),  // warm coral
        ];
        Self::build(0x101838, 0x4E83DE, 0xDB898B, 0xF1F1F1, 0x67788D, 0x283058, chart)
    }
    fn calsonic_light() -> Self {
        let chart = [
            oklch_to_color(0.30, 0.22, 258.0),  // layer 0: deep ocean blue — strong anchor
            oklch_to_color(0.46, 0.24,  18.0),  // layer 1: dark coral-red — transient pop
            oklch_to_color(0.36, 0.20, 240.0),  // layer 2: midnight indigo — drift layer
            oklch_to_color(0.52, 0.18,  32.0),  // layer 3: dark salmon — ghost echo
        ];
        Self::build(0xD0DFEE, 0x1A4A9A, 0xA04040, 0x001830, 0x54698A, 0x98A8C8, chart)
    }
}
