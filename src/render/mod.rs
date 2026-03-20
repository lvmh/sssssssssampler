pub mod glyph_atlas;
pub mod color_system;
pub mod audio_analysis;
pub mod layer_engine;
pub mod ascii_render;
pub mod motion;
pub mod instancing;
pub mod offscreen;

pub use ascii_render::AsciiRenderer;
pub use color_system::{Color, ColorPalette};
pub use glyph_atlas::{GlyphAtlas, GlyphInfo};
pub use audio_analysis::{AudioAnalyzer, compute_rms};
pub use layer_engine::{LayerEngine, LayerState};
pub use motion::MotionSystem;
pub use instancing::{GlyphInstance, generate_instances};
pub use offscreen::{OffscreenRenderer, FrameBuffer};
