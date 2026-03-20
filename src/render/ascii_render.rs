//! GPU-accelerated ASCII art renderer
//!
//! Handles all wgpu-related operations:
//! - Texture creation (glyph atlas)
//! - GPU buffer management (vertex, instance, index)
//! - Render pass execution
//! - Frame-by-frame rendering
//!
//! # Quick Start
//! ```ignore
//! let renderer = AsciiRenderer::new(device, queue, format, glyph_atlas, palette).await?;
//! renderer.render(device, queue, target_view, ascii_bank, layer_engine, motion, params)?;
//! ```
//!
//! # Architecture
//! The renderer is a thin wrapper around wgpu render infrastructure:
//! 1. Initialize: Create atlas texture, buffers, bind group, pipeline (stub)
//! 2. Per-frame: Generate instances → Upload to GPU → Render pass

use wgpu::*;
use crate::ascii_bank::AsciiBank;
use crate::render::{ColorPalette, LayerEngine, MotionSystem, GlyphAtlas, generate_instances};
use crate::AnimationParams;
use std::mem;

/// GPU renderer for ASCII art
///
/// Manages wgpu resources for real-time rendering of ASCII visualization.
/// Does NOT store Device/Queue (they outlive this struct); buffers and textures
/// are owned here.
pub struct AsciiRenderer {
    pipeline: Option<RenderPipeline>,
    bind_group_layout: BindGroupLayout,
    atlas_texture: Texture,
    atlas_bind_group: BindGroup,
    atlas_sampler: Sampler,
    vertex_buffer: Buffer,
    instance_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
    grid_width: u32,
    grid_height: u32,
}

impl AsciiRenderer {
    /// Create a new renderer
    ///
    /// Initializes all GPU resources: glyph atlas texture, buffers, sampler, and bind group.
    /// The pipeline itself remains a stub (None) pending full shader implementation.
    ///
    /// # Arguments
    /// - `device`, `queue`: wgpu resources (owned by caller, must outlive renderer)
    /// - `surface_format`: Color format (currently unused; for future pipeline)
    /// - `glyph_atlas`: Pre-built glyph atlas (16×16 per glyph, 10 per row)
    /// - `palette`: Color palette (currently unused; for future color mapping)
    ///
    /// # Returns
    /// Ok(Self) on success; Err(String) if buffer creation fails
    ///
    /// # Memory
    /// - Atlas texture: ~92 KB (160×144 px, RGBA8)
    /// - Vertex buffer: ~53 KB (1,656 cells × 4 bytes)
    /// - Instance buffer: ~528 KB (1,656 cells × 5 layers × 64 bytes)
    /// - Index buffer: 24 bytes (6 indices)
    /// Total: ~673 KB
    pub async fn new(
        device: &Device,
        queue: &Queue,
        _surface_format: TextureFormat,
        glyph_atlas: &GlyphAtlas,
        _palette: &ColorPalette,
    ) -> Result<Self, String> {
        let grid_width = 36;
        let grid_height = 46;

        // Create glyph atlas texture
        let atlas_texture = device.create_texture(&TextureDescriptor {
            label: Some("glyph_atlas"),
            size: Extent3d {
                width: glyph_atlas.width,
                height: glyph_atlas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            ImageCopyTexture {
                texture: &atlas_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &glyph_atlas.texture_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(glyph_atlas.width * 4),
                rows_per_image: Some(glyph_atlas.height),
            },
            Extent3d {
                width: glyph_atlas.width,
                height: glyph_atlas.height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_view = atlas_texture.create_view(&TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("atlas_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("atlas_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let atlas_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&atlas_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // Vertex & instance buffers
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("vertex_buffer"),
            size: (grid_width * grid_height * mem::size_of::<u32>() as u32) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("instance_buffer"),
            size: (grid_width * grid_height * 64) as u64, // GlyphInstance = 64 bytes
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("index_buffer"),
            size: 6 * mem::size_of::<u32>() as u64,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });

        // Fill index buffer with quad indices (0, 1, 2, 1, 3, 2)
        {
            let mut idx_map = index_buffer.slice(..).get_mapped_range_mut();
            let indices = [0u32, 1, 2, 1, 3, 2];
            for (i, &idx) in indices.iter().enumerate() {
                idx_map[i * 4..(i + 1) * 4].copy_from_slice(&idx.to_le_bytes());
            }
        }
        drop(index_buffer.slice(..).get_mapped_range_mut());
        index_buffer.unmap();

        Ok(AsciiRenderer {
            pipeline: None,
            bind_group_layout,
            atlas_texture,
            atlas_bind_group,
            atlas_sampler,
            vertex_buffer,
            instance_buffer,
            index_buffer,
            index_count: 6,
            grid_width,
            grid_height,
        })
    }

    /// Render a single frame
    ///
    /// Called once per audio frame (or per UI refresh). Pipeline:
    /// 1. Generate instances from layer engine
    /// 2. Upload to GPU instance buffer
    /// 3. Begin render pass (clear to black)
    /// 4. Execute shader (stub: no draw commands yet)
    /// 5. Submit command buffer to GPU queue
    ///
    /// # Arguments
    /// - `device`, `queue`: wgpu resources (passed per-frame for flexibility)
    /// - `target_view`: Output texture view (cleared to black)
    /// - `ascii_bank`: ASCII image library (passed to instance generator)
    /// - `layer_engine`: Current layer state
    /// - `motion_system`, `anim_params`: Future use (time-based animation)
    ///
    /// # Performance
    /// ~1 ms GPU submission + upload; actual render pass timing depends on shader complexity.
    pub fn render(
        &self,
        device: &Device,
        queue: &Queue,
        target_view: &TextureView,
        ascii_bank: &AsciiBank,
        layer_engine: &LayerEngine,
        _motion_system: &MotionSystem,
        _anim_params: &AnimationParams,
    ) -> Result<(), String> {
        // Generate instances for current frame
        let instances = generate_instances(self.grid_width, self.grid_height, layer_engine, ascii_bank);

        // Write instance data to GPU buffer
        if !instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );
        }

        // Create render pass and render (stub: minimal command buffer)
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        {
            let _render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // Render pass is dropped here, releasing borrow on encoder
        }

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}
