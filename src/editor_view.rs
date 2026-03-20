use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::AnimationParams;

/// Vizia view displaying audio-reaktive ASCII art grid
pub struct AsciiRenderView {
    /// Animation parameters shared from DSP loop (via Arc<Mutex<>>)
    pub anim_params: Arc<Mutex<AnimationParams>>,
}

impl AsciiRenderView {
    /// Create a new ASCII render view
    pub fn new(
        cx: &mut Context,
        anim_params: Arc<Mutex<AnimationParams>>,
        _frame_buffer: Arc<Mutex<Option<()>>>,
    ) -> Handle<'_, Self> {
        Self { anim_params }
            .build(cx, |_cx| {})
            .size(Stretch(1.0))
            .background_color(Color::rgb(30, 30, 47)) // Deep Indigo
    }
}

impl View for AsciiRenderView {
    // Rendering container for ASCII visualization
    // Ready for GPU texture or Canvas-based rendering
}
