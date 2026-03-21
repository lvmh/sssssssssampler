//! Synchronous rendering interface for Vizia UI thread
//! Handles wgpu rendering on demand without async/await complexity

use crate::render::{OffscreenRenderer, FrameBuffer};
use crate::AnimationParams;
use std::sync::{Arc, Mutex};

/// Manages on-demand wgpu rendering for UI display
pub struct UiRenderer {
    offscreen: Arc<Mutex<Option<OffscreenRenderer>>>,
}

impl UiRenderer {
    pub fn new() -> Self {
        Self {
            offscreen: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize renderer with wgpu device/queue (call once on UI thread)
    pub async fn initialize(
        &self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let renderer = OffscreenRenderer::new(device, queue, width, height).await?;

        if let Ok(mut offscreen) = self.offscreen.lock() {
            *offscreen = Some(renderer);
        }

        Ok(())
    }

    /// Render a frame and return frame buffer
    /// Call this from the UI thread to generate pixels for display
    pub fn render_frame(
        &self,
        anim_params: &AnimationParams,
    ) -> Option<FrameBuffer> {
        // For now: return gradient test pattern to verify display works
        let width = 36;
        let height = 46;
        let mut pixels = vec![0u8; (width * height * 4) as usize];

        // Create checkerboard pattern driven by RMS
        let brightness = 0.3 + (anim_params.rms * 0.7);

        for row in 0..height {
            for col in 0..width {
                let idx = ((row * width + col) * 4) as usize;
                let checkerboard = (col + row) % 2 == 0;

                let (r, g, b) = if checkerboard {
                    // Soft Violet
                    let v = (brightness * 122.0) as u8;
                    (v, (brightness * 108.0) as u8, 255)
                } else {
                    // Muted Green
                    ((brightness * 76.0) as u8, (brightness * 175.0) as u8, (brightness * 130.0) as u8)
                };

                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = 255; // Alpha
            }
        }

        Some(FrameBuffer {
            width,
            height,
            pixels,
            bg_rgb: [0, 0, 0],
            primary_rgb: [200, 200, 200],
            emphasis_rgb: [180, 180, 180],
            preset_idx: 4,
            theme_idx: 1,
        })
    }
}
