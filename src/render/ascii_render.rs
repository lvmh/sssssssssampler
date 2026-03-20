use wgpu::*;
use crate::ascii_bank::AsciiBank;
use crate::render::{ColorPalette, LayerEngine, MotionSystem, GlyphAtlas, generate_instances};
use crate::AnimationParams;
use std::mem;

/// GPU renderer for ASCII art
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
    /// Create a new renderer (requires a wgpu surface/device)
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

    /// Update GPU buffers and redraw
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
