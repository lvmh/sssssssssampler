//! Offscreen wgpu rendering with CPU readback for Vizia display
//! Renders to GPU texture, reads back to CPU for image display

use wgpu::*;
use std::sync::{Arc, Mutex};

/// Frame data ready for display (RGBA8 format)
#[derive(Clone)]
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA format, width × height × 4 bytes
    /// Theme background color in sRGB [R, G, B] for canvas fill
    pub bg_rgb: [u8; 3],
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
            bg_rgb: [0, 0, 0],
        }
    }
}

/// Manages offscreen wgpu rendering with CPU readback
pub struct OffscreenRenderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
    render_texture: Texture,
    render_target_view: TextureView,
    staging_buffer: Buffer,
    width: u32,
    height: u32,
    bytes_per_row: u32,
    current_frame: Arc<Mutex<Option<FrameBuffer>>>,
}

impl OffscreenRenderer {
    /// Create a new offscreen renderer that renders to a texture and reads back to CPU
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

        // Staging buffer (padded to wgpu alignment requirement)
        // wgpu requires: bytes_per_row % 256 == 0
        let unpadded = width * 4;
        let bytes_per_row = ((unpadded + 255) / 256) * 256;
        let total_bytes = (bytes_per_row * height) as u64;

        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("offscreen_staging_buffer"),
            size: total_bytes,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            render_texture,
            render_target_view,
            staging_buffer,
            width,
            height,
            bytes_per_row,
            current_frame: Arc::new(Mutex::new(Some(FrameBuffer::new(width, height)))),
        })
    }

    /// Get render target view for wgpu rendering
    pub fn target_view(&self) -> &TextureView {
        &self.render_target_view
    }

    /// Get current frame buffer (last readback result)
    pub fn current_frame(&self) -> Arc<Mutex<Option<FrameBuffer>>> {
        self.current_frame.clone()
    }

    /// Submit render command buffer and schedule readback
    pub fn submit_and_readback(&self, command_buffer: CommandBuffer) {
        // Submit render commands
        self.queue.submit(std::iter::once(command_buffer));

        // Schedule texture copy to staging buffer
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("readback_encoder"),
        });

        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                texture: &self.render_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyBuffer {
                buffer: &self.staging_buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Wait for GPU readback to complete and return frame buffer
    /// This is a blocking operation that polls the device
    pub fn read_frame_blocking(&self) -> Option<FrameBuffer> {
        // Map the staging buffer
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        let tx_clone = tx.clone();
        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = tx_clone.send(result);
        });

        // Poll device until the mapping completes (max 5 seconds timeout)
        let start = std::time::Instant::now();
        loop {
            self.device.poll(Maintain::Poll);

            match rx.try_recv() {
                Ok(Ok(())) => break, // Mapping successful
                Ok(Err(e)) => {
                    eprintln!("GPU mapping error: {:?}", e);
                    return None;
                }
                Err(_) => {
                    // Still waiting
                    if start.elapsed().as_secs() > 5 {
                        eprintln!("GPU readback timeout");
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_micros(100));
                }
            }
        }

        // Read the mapped data
        let mapped_range = buffer_slice.get_mapped_range();
        let mut frame_buffer = FrameBuffer::new(self.width, self.height);

        // Copy pixels, removing padding
        for row in 0..self.height {
            let src_offset = (row * self.bytes_per_row) as usize;
            let dst_offset = (row * self.width * 4) as usize;
            let row_bytes = (self.width * 4) as usize;
            frame_buffer.pixels[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&mapped_range[src_offset..src_offset + row_bytes]);
        }

        drop(mapped_range);
        self.staging_buffer.unmap();

        // Cache the frame
        if let Ok(mut frame) = self.current_frame.lock() {
            *frame = Some(frame_buffer.clone());
        }

        Some(frame_buffer)
    }
}
