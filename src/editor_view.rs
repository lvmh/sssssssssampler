use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::AnimationParams;
use crate::render::FrameBuffer;

/// Vizia view for ASCII art rendering
pub struct AsciiRenderView {
    /// Animation parameters shared from DSP loop
    pub anim_params: Arc<Mutex<AnimationParams>>,
    /// Current frame buffer to display
    pub frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
}

impl AsciiRenderView {
    /// Create a new ASCII render view
    pub fn new(
        cx: &mut Context,
        anim_params: Arc<Mutex<AnimationParams>>,
        frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
    ) -> Handle<'_, Self> {
        Self {
            anim_params,
            frame_buffer,
        }
        .build(cx, |_cx| {})
        .size(Stretch(1.0))
        .background_color(Color::rgb(30, 30, 47))
    }
}

impl View for AsciiRenderView {
    // Rendering view container
    // Frame buffer updated externally from DSP/editor loop
}
