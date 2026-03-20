use bytemuck::{Pod, Zeroable};

/// Metadata for a single glyph in the atlas.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct GlyphInfo {
    /// UV coords: (u_min, v_min, u_max, v_max) as normalized floats [0..1]
    pub uv: [f32; 4],
}

/// Glyph atlas texture: 8x8 grid of ASCII chars (space to @), monospace font
pub struct GlyphAtlas {
    /// Raw RGBA8 texture data (atlas_w × atlas_h × 4)
    pub texture_data: Vec<u8>,
    /// Texture width in pixels
    pub width: u32,
    /// Texture height in pixels
    pub height: u32,
    /// Info per glyph (84 glyphs, indexed by char_to_idx)
    pub glyphs: Vec<GlyphInfo>,
}

impl GlyphAtlas {
    /// Build a simple monospace glyph atlas: 10 glyphs per row, 9 rows
    /// Each glyph cell = 16×16 pixels, filled with ASCII rasterization
    pub fn new(charset_len: usize) -> Self {
        let glyphs_per_row = 10;
        let glyph_size = 16;
        let rows = (charset_len + glyphs_per_row - 1) / glyphs_per_row;

        let atlas_w = (glyphs_per_row * glyph_size) as u32;
        let atlas_h = (rows * glyph_size) as u32;

        let mut texture_data = vec![0u8; (atlas_w * atlas_h * 4) as usize];
        let mut glyphs = Vec::new();

        for i in 0..charset_len {
            let row = i / glyphs_per_row;
            let col = i % glyphs_per_row;

            let x = (col * glyph_size) as u32;
            let y = (row * glyph_size) as u32;

            // Rasterize character at (x, y)
            rasterize_glyph(&mut texture_data, atlas_w, x, y, glyph_size as u32);

            let u_min = x as f32 / atlas_w as f32;
            let v_min = y as f32 / atlas_h as f32;
            let u_max = (x + glyph_size as u32) as f32 / atlas_w as f32;
            let v_max = (y + glyph_size as u32) as f32 / atlas_h as f32;

            glyphs.push(GlyphInfo { uv: [u_min, v_min, u_max, v_max] });
        }

        GlyphAtlas { texture_data, width: atlas_w, height: atlas_h, glyphs }
    }
}

/// Simple rasterization: fill a glyph cell with a test pattern (bright center)
fn rasterize_glyph(data: &mut [u8], atlas_w: u32, x: u32, y: u32, size: u32) {
    for py in 0..size {
        for px in 0..size {
            let pos = ((y + py) * atlas_w + (x + px)) as usize * 4;
            if pos + 3 < data.len() {
                // Simple: center pixel bright, fade outward
                let dx = (px as i32 - size as i32 / 2).abs() as u32;
                let dy = (py as i32 - size as i32 / 2).abs() as u32;
                let dist = (dx * dx + dy * dy) as f32;
                let max_dist = (size * size) as f32;
                let brightness = ((1.0 - (dist / max_dist)).max(0.0) * 255.0) as u8;

                data[pos] = brightness;     // R
                data[pos + 1] = brightness; // G
                data[pos + 2] = brightness; // B
                data[pos + 3] = 255;        // A
            }
        }
    }
}
