use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::AnimationParams;
use crate::ascii_bank::AsciiBank;

/// Vizia view that embeds a wgpu render surface
pub struct AsciiRenderView {
    /// Animation parameters shared from DSP loop (via Arc<Mutex<>>)
    anim_params: Arc<Mutex<AnimationParams>>,
}

impl AsciiRenderView {
    /// Create a new ASCII render view with shared animation parameters
    pub fn new(cx: &mut Context, anim_params: Arc<Mutex<AnimationParams>>) -> Handle<Self> {
        Self { anim_params }
            .build(cx, |_cx| {})
            .size(Stretch(1.0))
            .background_color(Color::rgb(30, 30, 47)) // Deep Indigo
    }
}

impl View for AsciiRenderView {
    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // TODO: Integrate wgpu rendering into Vizia canvas
    }
}
