//! Offscreen wgpu rendering that writes to an RGBA texture
//! Texture data is readback to CPU for display in Vizia via Image elements

use wgpu::*;
use std::sync::{Arc, Mutex};

/// Holds frame data (RGBA8 pixels) ready for display
#[derive(Clone)]
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA format, width × height × 4 bytes
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
        }
    }
}

/// Manages offscreen wgpu rendering with CPU readback
pub struct OffscreenRenderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
    render_texture: Texture,
    render_target_view: TextureView,
    readback_buffer: Buffer,
    width: u32,
    height: u32,
    frame_buffer: Arc<Mutex<FrameBuffer>>,
}

impl OffscreenRenderer {
    /// Create a new offscreen renderer
    ///
    /// Does NOT bind to a window; instead renders to an offscreen texture
    /// that can be read back to CPU for display in UI.
    pub async fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        // Create offscreen render texture (RGBA8Unorm)
        let render_texture = device.create_texture(&TextureDescriptor {
            label: Some("offscreen_render_target"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let render_target_view = render_texture.create_view(&TextureViewDescriptor::default());

        // Create readback buffer (padded for alignment)
        let bytes_per_row = (width * 4 + 255) & !255; // Align to 256 bytes
        let readback_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("offscreen_readback_buffer"),
            size: (bytes_per_row * height) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            render_texture,
            render_target_view,
            readback_buffer,
            width,
            height,
            frame_buffer: Arc::new(Mutex::new(FrameBuffer::new(width, height))),
        })
    }

    /// Get current frame buffer for display
    pub fn frame_buffer(&self) -> Arc<Mutex<FrameBuffer>> {
        self.frame_buffer.clone()
    }

    /// Get render target view for wgpu rendering
    pub fn target_view(&self) -> &TextureView {
        &self.render_target_view
    }

    /// Submit a command buffer and schedule readback
    pub fn submit(&self, command_buffer: CommandBuffer) {
        self.queue.submit(std::iter::once(command_buffer));

        // Schedule readback of rendered frame
        self.schedule_readback();
    }

    /// Schedule CPU readback of current frame
    /// Copies GPU texture to readback buffer
    fn schedule_readback(&self) {
        let width = self.width;
        let height = self.height;
        let render_texture = &self.render_texture;
        let readback_buffer = &self.readback_buffer;
        let bytes_per_row = (width * 4 + 255) & !255;

        // Create copy command
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("readback_encoder"),
        });

        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                texture: render_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyBuffer {
                buffer: readback_buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Note: Actual buffer mapping would require async/await
        // For now, this schedules the copy and subsequent poll() will process it
    }

    /// Poll for GPU readback completion (call each frame from UI thread)
    pub fn poll_gpu(&self) {
        self.device.poll(Maintain::Poll);
    }
}
