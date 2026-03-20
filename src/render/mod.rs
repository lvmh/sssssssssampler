pub mod glyph_atlas;
pub mod color_system;
pub mod audio_analysis;
pub mod layer_engine;
pub mod ascii_render;

pub use ascii_render::AsciiRenderer;
pub use color_system::{Color, ColorPalette};
pub use glyph_atlas::{GlyphAtlas, GlyphInfo};
pub use audio_analysis::{AudioAnalyzer, compute_rms};
