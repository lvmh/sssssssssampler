use bytemuck::{Pod, Zeroable};
use crate::ascii_bank::AsciiBank;
use crate::render::LayerEngine;

/// A single glyph instance for GPU instancing: position, glyph index, color, transform
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct GlyphInstance {
    /// Grid cell position (x, y)
    pub position: [i32; 2],
    /// Glyph index in atlas
    pub glyph_idx: u32,
    /// RGBA color (normalized 0.0–1.0, packed as u32 in GPU)
    pub color: [f32; 4],
    /// Scale factor for pop effect (1.0 = normal, 1.5 = pop)
    pub scale: f32,
    /// Opacity (fade for pop effect)
    pub opacity: f32,
    /// Time offset (for animation)
    pub time_offset: i32,
    /// Padding to 64 bytes (required for GPU buffer alignment)
    pub _pad: [f32; 2],
}

/// Generate instances for a single frame
/// Returns Vec<GlyphInstance> for all visible glyphs across all layers
pub fn generate_instances(
    grid_width: u32,
    grid_height: u32,
    layer_engine: &LayerEngine,
    ascii_bank: &AsciiBank,
) -> Vec<GlyphInstance> {
    let mut instances = Vec::new();

    // Iterate over all grid cells
    for y in 0..grid_height {
        for x in 0..grid_width {
            // Layer 0 (anchor): always img01.txt at full opacity
            let anchor_layer = &layer_engine.layers()[0];
            if let Some(glyph_char) = get_grid_char(
                anchor_layer.image_idx,
                x as i32,
                y as i32,
                anchor_layer.spatial_offset,
                ascii_bank,
                grid_width,
                grid_height,
            ) {
                let glyph_idx = char_to_glyph_idx(glyph_char);
                instances.push(GlyphInstance {
                    position: [x as i32, y as i32],
                    glyph_idx,
                    color: [1.0, 1.0, 1.0, 1.0], // White
                    scale: 1.0,
                    opacity: 1.0,
                    time_offset: anchor_layer.time_offset,
                    _pad: [0.0; 2],
                });
            }

            // Secondary layers (1–4): overlay with pop highlights
            for layer_idx in 1..layer_engine.layers().len() {
                let layer = &layer_engine.layers()[layer_idx];
                if layer.weight <= 0.0 {
                    continue; // Skip inactive layers
                }

                // Check if this layer overrides the space at (x, y)
                if let Some(glyph_char) = get_grid_char(
                    layer.image_idx,
                    x as i32,
                    y as i32,
                    layer.spatial_offset,
                    ascii_bank,
                    grid_width,
                    grid_height,
                ) {
                    // Skip spaces unless pop_highlight is true
                    if glyph_char == ' ' && !layer.pop_highlight {
                        continue;
                    }

                    let glyph_idx = char_to_glyph_idx(glyph_char);
                    let (scale, opacity) = if layer.pop_highlight {
                        (1.5, 0.7) // Pop effect: 1.5x scale, 70% opacity
                    } else {
                        (1.0, layer.weight)
                    };

                    instances.push(GlyphInstance {
                        position: [x as i32, y as i32],
                        glyph_idx,
                        color: [1.0, 1.0, 1.0, 1.0], // White (color applied in shader)
                        scale,
                        opacity,
                        time_offset: layer.time_offset,
                        _pad: [0.0; 2],
                    });
                }
            }
        }
    }

    instances
}

/// Get character at grid position from an ASCII bank image
/// Handles spatial offsets and viewport clipping
fn get_grid_char(
    image_idx: usize,
    grid_x: i32,
    grid_y: i32,
    spatial_offset: (i32, i32),
    ascii_bank: &AsciiBank,
    grid_width: u32,
    grid_height: u32,
) -> Option<char> {
    use crate::ascii_bank::CHARSET;

    // Check bounds
    if image_idx >= ascii_bank.len() {
        return None;
    }

    // Clip to grid bounds
    if grid_x < 0 || grid_y < 0 || grid_x >= grid_width as i32 || grid_y >= grid_height as i32 {
        return None;
    }

    // Apply spatial offset
    let src_x = grid_x + spatial_offset.0;
    let src_y = grid_y + spatial_offset.1;

    // Clip to source image bounds
    if src_x < 0 || src_y < 0 || src_x >= ascii_bank.width as i32 || src_y >= ascii_bank.height as i32 {
        return None;
    }

    // Get density index from ASCII bank
    let density_idx = ascii_bank.get_cell(image_idx, src_x as usize, src_y as usize) as usize;

    // Map to character
    if density_idx < CHARSET.len() {
        Some(CHARSET[density_idx])
    } else {
        Some(' ')
    }
}

/// Map ASCII character to glyph index (0–127 for printable ASCII)
fn char_to_glyph_idx(c: char) -> u32 {
    let code = c as u32;
    if code < 128 {
        code
    } else {
        32 // Default to space for non-ASCII
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_to_glyph_idx() {
        assert_eq!(char_to_glyph_idx(' '), 32);
        assert_eq!(char_to_glyph_idx('A'), 65);
        assert_eq!(char_to_glyph_idx('0'), 48);
    }

    #[test]
    fn test_glyph_instance_size() {
        // Verify alignment is correct for GPU buffer
        let size = std::mem::size_of::<GlyphInstance>();
        assert_eq!(size, 64, "GlyphInstance should be 64 bytes for GPU alignment");
    }
}
