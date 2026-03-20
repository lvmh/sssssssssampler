use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::render::FrameBuffer;

/// Custom view that displays frame buffer as ASCII grid
pub struct AsciiGridDisplay {
    frame_buffer: Arc<Mutex<Option<FrameBuffer>>>,
}

impl AsciiGridDisplay {
    pub fn new(frame_buffer: Arc<Mutex<Option<FrameBuffer>>>) -> Self {
        Self { frame_buffer }
    }

    pub fn build(self, cx: &mut Context) -> Handle<Self> {
        Self {
            frame_buffer: self.frame_buffer,
        }
        .build(cx)
        .size(Stretch(1.0))
        .background_color(Color::rgb(30, 30, 47))
    }
}

impl View for AsciiGridDisplay {
    // Frame buffer display container
    // Uses Element styling and layout system
}
