#![allow(unused)]
#![allow(clippy::identity_op)]

use bytemuck_derive::{Pod, Zeroable};
use glam::f32::Mat4;
use glam::{IVec2, UVec2, Vec2};
use palette::Srgba;
use palette::cast::ComponentsInto;
use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;
use std::collections::BinaryHeap;
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Color, ColorTargetState,
    Device, Extent3d, IndexFormat, RenderPass, RenderPipeline, Sampler, ShaderModule, Texture,
    TextureView,
};
use winit::dpi::PhysicalSize;

use super::graphics_system::{GraphicsSystem, VERTEX_SIZE, Vertex};
use super::{Camera, TextureBundle};
use crate::graphics::{
    RenderContext, Texture2D, TextureHandle, texture_atlas::DynamicTextureAtlas,
};
use crate::{
    assets::{AssetHandle, AssetSystem},
    utils::*,
};

pub type QuadTexture = Option<TextureHandle>;

#[derive(Debug, Clone, PartialEq)]
pub struct Quad {
    pub center: IVec2,
    pub size: IVec2,
    /// If there is a texture set, this tints the texture, otherwise the quad will have this color
    pub color: Srgba,
    pub texture: QuadTexture,
}
impl Eq for Quad {}

/// A simple wrapper that stores a quad and a corresponding layer
/// and texture atlas index for use in the binary heap
#[derive(Debug, PartialEq, Eq)]
struct BatchQuadEntry {
    layer: i32,
    used_texture_atlas_idx: usize,
    quad: Quad,
}
// sorts first by layer and then by used_texture_atlas_idx
impl Ord for BatchQuadEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.layer.cmp(&other.layer).then(
            self.used_texture_atlas_idx
                .cmp(&other.used_texture_atlas_idx),
        )
    }
}

impl PartialOrd for BatchQuadEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

struct Batch {
    atlas_idx: usize,
    vertices_range: Range<u64>,
    indices_end: u32,
    layer: i32,
}

/// A simple batch renderer that supports layering of quads
pub struct BatchRenderer {
    ctx: GeeseContextHandle<Self>,

    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_format: IndexFormat,

    quads_to_draw: BinaryHeap<std::cmp::Reverse<BatchQuadEntry>>,
    /// This is filled whenever we get an AssetChanged event and then when we create the batches, we check if the quad's texture
    /// is in this set (so it has changed), and if so, we remove and re-add it to our atlasses.
    /// This is cleared inside of `end_frame`
    changed_asset_ids: HashSet<u64>,
    batches: Vec<Batch>,
    vertices_to_draw: Vec<Vertex>,

    globals_bind_group: (BindGroup, BindGroupLayout),

    shader_handle: AssetHandle<ShaderModule>,
    render_pipeline: RenderPipeline,
    clear_color: Color,

    white_pixel_handle: TextureHandle,

    atlas_bind_group_layout: BindGroupLayout,
    texture_atlasses: Vec<(DynamicTextureAtlas, BindGroup)>,
}
impl BatchRenderer {
    const MAX_QUAD_COUNT: usize = 1000;
    const MAX_VERTEX_COUNT: usize = BatchRenderer::MAX_QUAD_COUNT * 4;
    const MAX_INDEX_COUNT: usize = BatchRenderer::MAX_QUAD_COUNT * 6;
    const DEFAULT_TEXATLAS_WIDTH: u32 = 2048;
    const DEFAULT_TEXATLAS_HEIGHT: u32 = 2048;
    const DEFAULT_TEXATLAS_FILTERING: wgpu::FilterMode = wgpu::FilterMode::Nearest;

    /// Handles batching and issuing draw calls accordingly
    fn create_batches(&mut self) {
        let total_quads_to_draw = self.quads_to_draw.len();
        let max_textures = {
            let graphics_sys = self.ctx.get::<GraphicsSystem>();
            graphics_sys.device().limits().max_bindings_per_bind_group / 2
        };

        let mut previous_layer = 0;
        let mut previous_atlas_index = 0;
        let mut first_iteration = true;
        let mut num_quads_in_batch = 0;
        let mut last_batch_end_quad_idx: u64 = 0;
        let mut total_quads_processed = 0;
        loop {
            let current_quad = self.quads_to_draw.pop();
            // We have reached the end of the heap
            if current_quad.is_none() {
                break;
            };
            let entry = current_quad.unwrap().0;
            let quad = entry.quad;
            let current_layer = entry.layer;
            // this is mutable because it might get changed later when we assign a new atlas because the texture has changed
            let mut current_atlas_index = entry.used_texture_atlas_idx;
            // Since the quads are ordered by layer, this means that we have now iterated through
            // all quads in this layer and we need to create a batch with the last ones
            if !first_iteration
                && (current_layer != previous_layer || current_atlas_index != previous_atlas_index)
            {
                let vertices_range = (last_batch_end_quad_idx * 4)..(total_quads_processed * 4);
                let indices_end = num_quads_in_batch as u32 * 6;
                self.batches.push(Batch {
                    atlas_idx: previous_atlas_index,
                    vertices_range,
                    indices_end,
                    layer: previous_layer,
                });
                last_batch_end_quad_idx = total_quads_processed;
                num_quads_in_batch = 0;
            }

            let quad_pos = quad.center;
            //info!("Old quad pos: {}   New pos: {}", quad.center, quad_pos);
            let x = quad_pos.x;
            let y = quad_pos.y;
            let w = quad.size.x;
            let h = quad.size.y;
            let color: [f32; 4] = quad.color.into();
            let quad_tex = quad
                .texture
                .clone()
                .unwrap_or(self.white_pixel_handle.clone());
            if self.changed_asset_ids.contains(quad_tex.id()) {
                let mut prev_atlas = &mut self.texture_atlasses[current_atlas_index].0;
                // remove the tex from the atlas it is currently in
                prev_atlas.remove_texture(quad_tex.clone());
                // and find a new atlas which has enough space to fit the texture (since texture size could have changed, this might not be the same atlas)
                current_atlas_index = self.insert_texture_into_atlas(&quad_tex);
            }
            let (atlas_tex_coords_start, atlas_tex_coords_end) = self.texture_atlasses
                [current_atlas_index]
                .0
                .get_texture_coords(&quad_tex)
                .expect("Texture coords should exist for each quad");

            // Add the vertices of the quad to vertices, respecting size and attributes
            self.vertices_to_draw.reserve(4);
            self.vertices_to_draw.push(Vertex::new(
                IVec2::new(x - w, y - h),
                color,
                atlas_tex_coords_start,
            ));
            self.vertices_to_draw.push(Vertex::new(
                IVec2::new(x - w, y + h),
                color,
                Vec2::new(atlas_tex_coords_start.x, atlas_tex_coords_end.y),
            ));
            self.vertices_to_draw.push(Vertex::new(
                IVec2::new(x + w, y + h),
                color,
                atlas_tex_coords_end,
            ));
            self.vertices_to_draw.push(Vertex::new(
                IVec2::new(x + w, y - h),
                color,
                Vec2::new(atlas_tex_coords_end.x, atlas_tex_coords_start.y),
            ));

            first_iteration = false;
            previous_layer = current_layer;
            previous_atlas_index = current_atlas_index;
            num_quads_in_batch += 1;
            total_quads_processed += 1;
        }

        // Create the last batch of this frame (with the remaining quads)
        let vertices_range = ((last_batch_end_quad_idx) * 4)..(self.vertices_to_draw.len() as u64);
        let indices_end = num_quads_in_batch as u32 * 6;
        self.batches.push(Batch {
            atlas_idx: previous_atlas_index,
            vertices_range,
            indices_end,
            layer: previous_layer,
        });
    }

    pub(super) fn prepare_to_render(&mut self, context: &mut RenderContext) {
        self.create_batches();

        // Write the data from vertices to the vertex buffer
        context.queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.vertices_to_draw),
        );

        let mut atlas_encoder =
            context
                .device
                .create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor {
                    label: Some("Atlas command encoder"),
                });

        {
            let asset_sys = self.ctx.get::<AssetSystem>();
            for (atlas, _) in &mut self.texture_atlasses {
                atlas.rebuild_atlas(
                    |handle| asset_sys.get(handle).unwrap().texture(),
                    &mut atlas_encoder,
                );
            }
        }
        context.queue.submit(Some(atlas_encoder.finish()));
    }

    pub(super) fn render_batch_layers(
        &mut self,
        context: &mut RenderContext,
        layer_range: Range<i32>,
        clear: bool,
    ) {
        let mut rpass = context
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BatchRenderer render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &context.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match clear {
                            true => wgpu::LoadOp::Clear(self.clear_color),
                            false => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

        self.batches
            .iter()
            .filter(|b| layer_range.contains(&b.layer))
            .for_each(|batch| {
                rpass.set_pipeline(&self.render_pipeline);
                // The index buffer stays the same over all batches
                rpass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
                // Only use a slice of the vertex buffer, which belongs to the current batch
                rpass.set_vertex_buffer(
                    0,
                    self.vertex_buffer.slice(
                        (batch.vertices_range.start * VERTEX_SIZE as u64)
                            ..(batch.vertices_range.end * VERTEX_SIZE as u64),
                    ),
                );
                // Use the bind group specified by the batch
                rpass.set_bind_group(0, &self.globals_bind_group.0, &[]);
                rpass.set_bind_group(1, &self.texture_atlasses[batch.atlas_idx].1, &[]);
                rpass.draw_indexed(0..batch.indices_end, 0, 0..1);
            });
    }

    /// Performs clean-up at the end of the frame
    pub(super) fn end_frame(&mut self, _context: &mut RenderContext) {
        self.batches.clear();
        self.quads_to_draw.clear();
        self.changed_asset_ids.clear();
        self.vertices_to_draw.clear();
    }

    /// Records a new quad that needs to be drawn this frame (low performance cost, even though quad gets cloned)
    pub fn draw_quad(&mut self, quad: &Quad, layer: i32) {
        let mut used_texture_atlas_idx = 0;
        if let Some(handle) = &quad.texture {
            let mut has_texture = false;
            for (idx, (atlas, _)) in self.texture_atlasses.iter().enumerate() {
                if atlas.contains_texture(handle) {
                    has_texture = true;
                    used_texture_atlas_idx = idx;
                    break;
                }
            }
            if !has_texture {
                let texture_size = {
                    let asset_sys = self.ctx.get::<AssetSystem>();
                    let tex = asset_sys.get(handle).unwrap().texture();
                    UVec2::new(tex.size().width, tex.size().height)
                };
                self.insert_texture_into_atlas(handle);
            }
        } else {
            // the white pixel is always in the first atlas since we add in in the new() function
            used_texture_atlas_idx = 0;
        }
        self.quads_to_draw.push(std::cmp::Reverse(BatchQuadEntry {
            layer,
            used_texture_atlas_idx,
            quad: quad.clone(),
        }));
    }

    fn insert_texture_into_atlas(&mut self, handle: &TextureHandle) -> usize {
        let texture_size = {
            let asset_sys = self.ctx.get::<AssetSystem>();
            let tex = asset_sys.get(handle).unwrap().texture();
            UVec2::new(tex.size().width, tex.size().height)
        };
        let mut used_texture_atlas_idx = 0;
        for (idx, (atlas, _)) in self.texture_atlasses.iter_mut().enumerate() {
            if atlas.add_texture(handle.clone(), texture_size).is_ok() {
                used_texture_atlas_idx = idx;
                break;
            }
        }
        used_texture_atlas_idx
    }

    /// Reloads parts of the renderer depending on what asset changed. Ignored on wasm
    #[cfg(not(target_arch = "wasm32"))]
    fn on_assetchange(&mut self, event: &crate::assets::events::AssetChanged) {
        let asset_sys = self.ctx.get::<AssetSystem>();
        if event.asset_id == **self.shader_handle.id() {
            let graphics_sys = self.ctx.get::<GraphicsSystem>();
            self.render_pipeline = Self::create_render_pipeline(
                graphics_sys.device(),
                &[
                    Some(&self.globals_bind_group.1),
                    Some(&self.atlas_bind_group_layout),
                ],
                asset_sys.get(&self.shader_handle).unwrap(),
                graphics_sys.get_surface_view_format(),
            );
        } else {
            self.changed_asset_ids.insert(event.asset_id);
        }
    }

    /// Helper function for creating a new render pipeline
    fn create_render_pipeline(
        device: &Device,
        bind_group_layouts: &[Option<&BindGroupLayout>],
        shader: &ShaderModule,
        surface_format: wgpu::TextureFormat,
    ) -> RenderPipeline {
        // IDEA: Create pipelines with different bind group layouts beforehand
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main"),
            bind_group_layouts,
            immediate_size: 0,
        });
        let color_state = Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("batch renderer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vert_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_SIZE as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex, // position        color       tex_coords
                    attributes: &wgpu::vertex_attr_array![0 => Sint32x2, 1 => Float32x4, 2 => Float32x2],
                })],
                compilation_options: Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fragment_main"),
                targets: &[color_state],
                compilation_options: Default::default()
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None
        })
    }

    /// Creates the BGL for the Globals struct in the shader
    fn create_globals_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Globals bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZeroU64::new(64).unwrap()),
                },
                count: None,
            }],
        })
    }

    /// Creates the bind group for the Globals struct in the shader
    fn create_globals_bind_group(
        device: &wgpu::Device,
        globals_layout: &BindGroupLayout,
        shaderglobals: &Buffer,
    ) -> BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shaderglobals.as_entire_binding(),
            }],
            layout: globals_layout,
            label: Some("Globals bind group"),
        })
    }

    /// Creates the BGL for the texture atlas
    fn create_atlas_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Atlas bind group layout"),
            entries: &[
                // Texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    /// Creates the bind group for the texture atlas
    fn create_bind_group_for_atlas(
        device: &Device,
        atlas_bgl: &BindGroupLayout,
        atlas: &DynamicTextureAtlas,
    ) -> BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(atlas.sampler()),
                },
            ],
            layout: atlas_bgl,
            label: Some(&format!("Bind group for {:?}", atlas)),
        })
    }

    /// Creates an array of indices, following the typical quad indexing method (0-1-2, 2-3-0)
    fn create_indices() -> [u16; BatchRenderer::MAX_INDEX_COUNT] {
        let mut indices: [u16; BatchRenderer::MAX_INDEX_COUNT] =
            [0; BatchRenderer::MAX_INDEX_COUNT];
        let mut offset = 0;
        (0..BatchRenderer::MAX_INDEX_COUNT)
            .step_by(6)
            .for_each(|i| {
                indices[i + 0] = 0 + offset;
                indices[i + 1] = 1 + offset;
                indices[i + 2] = 2 + offset;

                indices[i + 3] = 2 + offset;
                indices[i + 4] = 3 + offset;
                indices[i + 5] = 0 + offset;

                offset += 4;
            });
        indices
    }
}

impl GeeseSystem for BatchRenderer {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<Camera>>();

    #[cfg(not(target_arch = "wasm32"))]
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers().with(Self::on_assetchange);

    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BatchRenderer vertex buffer"),
            size: (BatchRenderer::MAX_VERTEX_COUNT * size_of::<Vertex>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices = BatchRenderer::create_indices();
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Set up a white 1x1 texture
        let queue = graphics_sys.queue();
        let white_pixel = TextureBundle::new(
            device,
            queue,
            "White pixel texture",
            wgpu::TextureDescriptor {
                size: wgpu::Extent3d::default(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                label: Some("White pixel texture descriptor"),
                view_formats: &[],
            },
            &wgpu::TextureViewDescriptor::default(),
            &wgpu::SamplerDescriptor {
                label: Some("white pixel sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                //mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
        );

        let camera = ctx.get::<Camera>();
        let conf = graphics_sys.surface_config();
        let globals_bgl = Self::create_globals_bind_group_layout(device);
        let globals_bg =
            Self::create_globals_bind_group(device, &globals_bgl, camera.canvas_transform_buffer());

        let atlas_bgl = Self::create_atlas_bind_group_layout(device);
        let first_atlas = DynamicTextureAtlas::new(
            device,
            queue,
            Self::DEFAULT_TEXATLAS_WIDTH,
            Self::DEFAULT_TEXATLAS_HEIGHT,
            Self::DEFAULT_TEXATLAS_FILTERING,
        );
        let first_atlas_bg = Self::create_bind_group_for_atlas(device, &atlas_bgl, &first_atlas);
        let mut texture_atlasses = vec![(first_atlas, first_atlas_bg)];

        drop(graphics_sys);
        drop(camera);
        let white_pixel_handle = {
            let mut asset_sys = ctx.get_mut::<AssetSystem>();
            let white_pixel_handle = asset_sys.register(white_pixel);
            texture_atlasses[0]
                .0
                .add_texture(white_pixel_handle.clone(), UVec2::new(1, 1));
            white_pixel_handle
        };

        let shader_handle = ctx
            .get_mut::<AssetSystem>()
            .load(asset_source!("../shaders/batch_renderer.wgsl"))
            .unwrap();
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let render_pipeline = Self::create_render_pipeline(
            graphics_sys.device(),
            &[Some(&globals_bgl), Some(&atlas_bgl)],
            ctx.get::<AssetSystem>().get(&shader_handle).unwrap(),
            graphics_sys.get_surface_view_format(),
        );
        drop(graphics_sys);

        Self {
            ctx,

            vertex_buffer,
            index_buffer,
            index_format: wgpu::IndexFormat::Uint16,

            quads_to_draw: BinaryHeap::new(),
            changed_asset_ids: HashSet::default(),
            batches: vec![],
            vertices_to_draw: Vec::with_capacity(1000),

            globals_bind_group: (globals_bg, globals_bgl),

            shader_handle,
            render_pipeline,
            clear_color: Color::BLACK,

            white_pixel_handle,

            atlas_bind_group_layout: atlas_bgl,
            texture_atlasses,
        }
    }
}
