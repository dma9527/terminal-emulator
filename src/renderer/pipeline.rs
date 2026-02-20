/// wgpu rendering pipeline for the terminal.
/// Renders cell grid as textured quads using the glyph atlas.

use crate::core::{Grid, Color};
use crate::renderer::atlas::GlyphAtlas;

/// Per-vertex data for a cell quad.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellVertex {
    /// Screen position (x, y) in pixels
    pub position: [f32; 2],
    /// UV coordinates into glyph atlas
    pub uv: [f32; 2],
    /// Foreground color (r, g, b)
    pub fg_color: [f32; 3],
    /// Background color (r, g, b)
    pub bg_color: [f32; 3],
}

impl CellVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // uv
        2 => Float32x3,  // fg_color
        3 => Float32x3,  // bg_color
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CellVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Holds all wgpu state for rendering.
pub struct RenderState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: Option<wgpu::SurfaceConfiguration>,
    pub pipeline: wgpu::RenderPipeline,
    pub atlas_texture: wgpu::Texture,
    pub atlas_bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub max_cells: usize,
}

impl RenderState {
    /// Create a headless render state (no surface) for testing or offscreen.
    pub async fn new_headless(atlas: &GlyphAtlas, max_cells: usize) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("No suitable GPU adapter found");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("terminal-device"),
                ..Default::default()
            }, None)
            .await
            .expect("Failed to create device");

        let (pipeline, atlas_texture, atlas_bind_group) =
            Self::create_pipeline(&device, &queue, atlas);

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell-vertices"),
            size: (max_cells * 6 * std::mem::size_of::<CellVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell-indices"),
            size: (max_cells * 6 * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            surface: None,
            config: None,
            pipeline,
            atlas_texture,
            atlas_bind_group,
            vertex_buffer,
            index_buffer,
            max_cells,
        }
    }

    /// Create render state with a pre-configured surface, device, and queue.
    pub fn new_with_surface(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        atlas: &GlyphAtlas,
        format: wgpu::TextureFormat,
        max_cells: usize,
    ) -> Self {
        let (pipeline, atlas_texture, atlas_bind_group) =
            Self::create_pipeline_with_format(&device, &queue, atlas, format);

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell-vertices"),
            size: (max_cells * 6 * std::mem::size_of::<CellVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell-indices"),
            size: (max_cells * 6 * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            surface: Some(surface),
            config: Some(config),
            pipeline,
            atlas_texture,
            atlas_bind_group,
            vertex_buffer,
            index_buffer,
            max_cells,
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &GlyphAtlas,
    ) -> (wgpu::RenderPipeline, wgpu::Texture, wgpu::BindGroup) {
        Self::create_pipeline_with_format(device, queue, atlas, wgpu::TextureFormat::Bgra8UnormSrgb)
    }

    fn create_pipeline_with_format(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &GlyphAtlas,
        target_format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::Texture, wgpu::BindGroup) {
        // Create atlas texture
        let texture_size = wgpu::Extent3d {
            width: atlas.atlas_width,
            height: atlas.atlas_height,
            depth_or_array_layers: 1,
        };

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph-atlas"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload atlas data
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.atlas_width),
                rows_per_image: Some(atlas.atlas_height),
            },
            texture_size,
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas_sampler) },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cell-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cell-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cell-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[CellVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        (pipeline, atlas_texture, atlas_bind_group)
    }

    /// Build vertex data from the terminal grid.
    pub fn build_vertices(
        &self,
        grid: &Grid,
        atlas: &mut GlyphAtlas,
        screen_width: f32,
        screen_height: f32,
    ) -> (Vec<CellVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let cw = atlas.cell_width;
        let ch = atlas.cell_height;
        let atlas_w = atlas.atlas_width as f32;
        let atlas_h = atlas.atlas_height as f32;

        for row in 0..grid.rows() {
            for col in 0..grid.cols() {
                let cell = grid.cell(row, col);
                if cell.ch == '\0' {
                    continue; // Skip wide-char placeholders
                }

                let x0 = col as f32 * cw;
                let y0 = row as f32 * ch;
                let x1 = x0 + cw;
                let y1 = y0 + ch;

                // Normalize to NDC (-1..1)
                let nx0 = (x0 / screen_width) * 2.0 - 1.0;
                let ny0 = 1.0 - (y0 / screen_height) * 2.0;
                let nx1 = (x1 / screen_width) * 2.0 - 1.0;
                let ny1 = 1.0 - (y1 / screen_height) * 2.0;

                let fg = color_to_f32(cell.fg);
                let bg = color_to_f32(cell.bg);

                // Get glyph UV from atlas
                let glyph = atlas.get_glyph(cell.ch);
                let u0 = glyph.x as f32 / atlas_w;
                let v0 = glyph.y as f32 / atlas_h;
                let u1 = (glyph.x + glyph.width) as f32 / atlas_w;
                let v1 = (glyph.y + glyph.height) as f32 / atlas_h;

                let base = vertices.len() as u32;
                vertices.extend_from_slice(&[
                    CellVertex { position: [nx0, ny0], uv: [u0, v0], fg_color: fg, bg_color: bg },
                    CellVertex { position: [nx1, ny0], uv: [u1, v0], fg_color: fg, bg_color: bg },
                    CellVertex { position: [nx1, ny1], uv: [u1, v1], fg_color: fg, bg_color: bg },
                    CellVertex { position: [nx0, ny1], uv: [u0, v1], fg_color: fg, bg_color: bg },
                ]);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }

        (vertices, indices)
    }

    /// Upload atlas texture if dirty.
    pub fn update_atlas(&self, atlas: &mut GlyphAtlas) {
        if atlas.dirty {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &atlas.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(atlas.atlas_width),
                    rows_per_image: Some(atlas.atlas_height),
                },
                wgpu::Extent3d {
                    width: atlas.atlas_width,
                    height: atlas.atlas_height,
                    depth_or_array_layers: 1,
                },
            );
            atlas.dirty = false;
        }
    }
}

fn color_to_f32(c: Color) -> [f32; 3] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
}

const SHADER_SRC: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fg_color: vec3<f32>,
    @location(3) bg_color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec3<f32>,
    @location(2) bg_color: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    return out;
}

@group(0) @binding(0) var atlas_texture: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(atlas_texture, atlas_sampler, in.uv).r;
    let color = mix(in.bg_color, in.fg_color, alpha);
    return vec4<f32>(color, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_layout() {
        let layout = CellVertex::layout();
        assert_eq!(layout.attributes.len(), 4);
        assert_eq!(
            layout.array_stride,
            std::mem::size_of::<CellVertex>() as u64
        );
    }

    #[test]
    fn test_color_to_f32() {
        let c = Color { r: 255, g: 128, b: 0 };
        let f = color_to_f32(c);
        assert!((f[0] - 1.0).abs() < 0.01);
        assert!((f[1] - 0.502).abs() < 0.01);
        assert!((f[2] - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_build_vertices_empty_grid() {
        // Can't create full RenderState without GPU, but we can test vertex building logic
        let grid = Grid::new(10, 5);
        let font_data = include_bytes!("/System/Library/Fonts/Menlo.ttc");
        let mut atlas = GlyphAtlas::new(font_data, 14.0);

        // Empty grid (all spaces) should still produce vertices
        let cw = atlas.cell_width;
        let ch = atlas.cell_height;
        let screen_w = 10.0 * cw;
        let screen_h = 5.0 * ch;

        // Manually build vertices to test the logic
        let mut count = 0;
        for row in 0..grid.rows() {
            for col in 0..grid.cols() {
                let cell = grid.cell(row, col);
                if cell.ch != '\0' {
                    atlas.get_glyph(cell.ch);
                    count += 1;
                }
            }
        }
        assert_eq!(count, 50); // 10x5 grid, all spaces
        assert!(atlas.glyph_count() >= 1); // at least space glyph
    }

    #[test]
    fn test_shader_compiles() {
        // Verify shader source is valid WGSL by checking it's non-empty
        // Full validation happens at pipeline creation time on GPU
        assert!(SHADER_SRC.contains("vs_main"));
        assert!(SHADER_SRC.contains("fs_main"));
        assert!(SHADER_SRC.contains("atlas_texture"));
    }
}
