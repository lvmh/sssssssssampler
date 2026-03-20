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
    /// Layer 0 (dominant): primary color
    pub primary: Color,
    /// Layer 1–4: secondary colors, distinct from primary
    pub secondary: [Color; 4],
    /// Background color
    pub background: Color,
    /// Emphasis color for non-space characters
    pub emphasis: Color,
}

impl ColorPalette {
    /// Noni Dark: deep indigo with soft violet and warm accents
    pub fn noni_dark() -> Self {
        ColorPalette {
            primary: Color::from_srgb_hex(0x7C6CFF),    // Soft Violet
            secondary: [
                Color::from_srgb_hex(0x4CAF82),         // Muted Green
                Color::from_srgb_hex(0xF4B860),         // Warm Amber
                Color::from_srgb_hex(0x6A5AEF),         // Deeper Violet
                Color::from_srgb_hex(0x5DADE2),         // Soft Blue
            ],
            background: Color::from_srgb_hex(0x1E1E2F), // Deep Indigo
            emphasis: Color::from_srgb_hex(0xF5F5F7),   // Soft White
        }
    }

    /// Noni Light: soft white background with warm accents (inverted emphasis)
    pub fn noni_light() -> Self {
        ColorPalette {
            primary: Color::from_srgb_hex(0x7C6CFF),    // Soft Violet
            secondary: [
                Color::from_srgb_hex(0x4CAF82),         // Muted Green
                Color::from_srgb_hex(0xF4B860),         // Warm Amber
                Color::from_srgb_hex(0x6A5AEF),         // Deeper Violet
                Color::from_srgb_hex(0x5DADE2),         // Soft Blue
            ],
            background: Color::from_srgb_hex(0xF5F5F7), // Soft White
            emphasis: Color::from_srgb_hex(0x1E1E2F),   // Deep Indigo
        }
    }

    /// Paris: cool blues and silvers
    pub fn paris() -> Self {
        ColorPalette {
            primary: Color::from_srgb_hex(0x5DADE2),    // Soft Blue
            secondary: [
                Color::from_srgb_hex(0xB0C4DE),         // Light Steel Blue
                Color::from_srgb_hex(0x4A90E2),         // Dodger Blue
                Color::from_srgb_hex(0x87CEEB),         // Sky Blue
                Color::from_srgb_hex(0xE0FFFF),         // Light Cyan
            ],
            background: Color::from_srgb_hex(0x2C3E50), // Midnight Blue
            emphasis: Color::from_srgb_hex(0xECF0F1),   // Clouds
        }
    }

    /// Rooney: warm golds and terracottas
    pub fn rooney() -> Self {
        ColorPalette {
            primary: Color::from_srgb_hex(0xD4A574),    // Tan
            secondary: [
                Color::from_srgb_hex(0xCD7F32),         // Bronze
                Color::from_srgb_hex(0xDEB887),         // Burlywood
                Color::from_srgb_hex(0xB8860B),         // Goldenrod
                Color::from_srgb_hex(0xA0522D),         // Sienna
            ],
            background: Color::from_srgb_hex(0x3E2723), // Dark Brown
            emphasis: Color::from_srgb_hex(0xF5DEB3),   // Wheat
        }
    }

    /// Factory: get palette by theme name (from editor.rs Theme enum)
    pub fn from_theme(theme_name: &str) -> Self {
        match theme_name {
            "theme-noni-light" => Self::noni_light(),
            "theme-paris" => Self::paris(),
            "theme-rooney" => Self::rooney(),
            _ => Self::noni_dark(), // default
        }
    }

    /// Get layer color (index 0 = primary, 1–4 = secondary)
    pub fn layer_color(&self, layer_idx: usize) -> Color {
        if layer_idx == 0 {
            self.primary
        } else {
            self.secondary[(layer_idx - 1) % 4]
        }
    }
}
