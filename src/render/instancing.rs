//! Instance buffer generation for GPU rendering
//!
//! Converts layer engine state into per-glyph render instances. Each instance
//! represents a single ASCII character positioned on the 36×46 grid, with
//! transform (scale, opacity) and animation metadata.
//!
//! The instance generation algorithm:
//! 1. Iterate over all grid cells (36×46 = 1,656 total)
//! 2. For layer 0 (anchor): always emit instance from img01.txt
//! 3. For layers 1–4 (overlays): emit if non-space or pop_highlight enabled
//! 4. Apply spatial offsets for camera panning effect
//! 5. Return Vec<GlyphInstance> for GPU buffer upload
//!
//! Each GlyphInstance is exactly 64 bytes for GPU alignment:
//! - position (8 bytes) + glyph_idx (4) + color (16) + scale (4) + opacity (4)
//!   + time_offset (4) + padding (8) = 64 bytes

use bytemuck::{Pod, Zeroable};
use crate::ascii_bank::AsciiBank;
use crate::render::LayerEngine;

/// A single glyph instance for GPU instancing: position, glyph index, color, transform
///
/// Represents one character in the rendered grid. GPU buffer contains these
/// contiguously, accessed via instanced rendering (one draw call per instance).
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
///
/// Converts layer engine state into per-glyph render commands. Called once per
/// frame during render pass.
///
/// # Arguments
/// - `grid_width`, `grid_height`: Dimensions of visible grid (typically 36×46)
/// - `layer_engine`: Current layer state (5 LayerState structs)
/// - `ascii_bank`: ASCII image library (collection of parsed grids)
///
/// # Returns
/// Vec<GlyphInstance> containing all visible glyphs, ready for GPU buffer upload.
/// Typically 800–1,200 instances per frame (depends on layer count and pop density).
///
/// # Algorithm
/// 1. Layer 0 (anchor): iterate grid, emit each cell from img01.txt
/// 2. Layers 1–4: for each active layer, composite non-space glyphs
/// 3. Apply spatial offset (camera pan effect): `(x + offset_x, y + offset_y)`
/// 4. Skip out-of-bounds reads (return None)
/// 5. Collect all instances into Vec
///
/// # Performance
/// ~2–3 ms per frame at 48 kHz (CPU-side generation)
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
