use nih_plug_vizia::vizia::prelude::*;
use std::sync::{Arc, Mutex};
use crate::AnimationParams;

/// Custom view that displays animated checkerboard grid
pub struct AsciiGridDisplay {
    anim_params: Arc<Mutex<AnimationParams>>,
}

impl AsciiGridDisplay {
    pub fn new(anim_params: Arc<Mutex<AnimationParams>>) -> Self {
        Self { anim_params }
    }

    pub fn build(self, cx: &mut Context) -> Handle<Self> {
        Self {
            anim_params: self.anim_params,
        }
        .build(cx)
        .size(Stretch(1.0))
        .background_color(Color::rgb(30, 30, 47))
    }
}

impl View for AsciiGridDisplay {}
