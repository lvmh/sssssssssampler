use nih_plug_vizia::vizia::prelude::*;
use crate::anim_state::SharedAnimParams;
use crate::ascii_bank::AsciiBank;

/// Vizia view that embeds a wgpu render surface
pub struct AsciiRenderView {
    anim_params: SharedAnimParams,
}

impl AsciiRenderView {
    pub fn new(cx: &mut Context, anim_params: SharedAnimParams) -> Handle<Self> {
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
