use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::AnimationParams;

/// Vizia view that displays ASCII art grid visualization
/// Main rendering area showing the audio-reactive visualization
pub struct AsciiRenderView {
    /// Animation parameters shared from DSP loop (via Arc<Mutex<>>)
    pub anim_params: Arc<Mutex<AnimationParams>>,
}

impl AsciiRenderView {
    /// Create a new ASCII render view with shared animation parameters
    pub fn new(cx: &mut Context, anim_params: Arc<Mutex<AnimationParams>>) -> Handle<Self> {
        Self { anim_params }
            .build(cx, |_cx| {
                // Render area - will show grid visualization when wgpu bridge is complete
            })
            .size(Stretch(1.0))
            .background_color(Color::rgb(30, 30, 47)) // Deep Indigo
    }
}

impl View for AsciiRenderView {
    // Rendering area for GPU output; currently displays solid background
    // Audio analysis and layer engine are wired and ready
    // Pending: Vizia↔wgpu surface integration
}
