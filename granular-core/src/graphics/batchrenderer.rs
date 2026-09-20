#![allow(unused)]
#![allow(clippy::identity_op)]

use super::{
    DrawSpace,
    quad_geometry::{quad_corners, top_left_offset},
};
use bytemuck_derive::{Pod, Zeroable};
use glam::f32::Mat4;
use glam::{UVec2, Vec2};
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

use super::{
    Camera, RenderContext, Texture2D, TextureBundle, TextureHandle,
    graphics_system::GraphicsSystem, texture_atlas::DynamicTextureAtlas, vertex::*,
};
use crate::graphics::{self, IntoGpuColor};
use crate::{
    assets::{AssetHandle, AssetSystem},
    utils::*,
};

#[derive(Debug, Clone, PartialEq)]
struct Quad {
    pub topleft: Vec2,
    pub size: Vec2,
    pub angle: f32,
    /// If there is a texture set, this tints the texture, otherwise the quad will have this color
    pub color: [f32; 4],
    pub texture: Option<TextureHandle>,
}

/// A simple wrapper that stores a quad and a corresponding layer
/// and texture atlas index for use in the binary heap
#[derive(Debug)]
struct BatchQuadEntry {
    layer: i32,
    draw_space: DrawSpace,
    used_texture_atlas_idx: usize,
    quad: Quad,
}
impl PartialEq for BatchQuadEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for BatchQuadEntry {}
// Equality follows the ordering keys; geometry containing floats is not Eq.
// sorts first by layer then by draw space and then by used_texture_atlas_idx
impl Ord for BatchQuadEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.layer
            .cmp(&other.layer)
            .then(self.draw_space.cmp(&other.draw_space))
            .then(
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

#[derive(Debug)]
struct Batch {
    atlas_idx: usize,
    vertices_range: Range<u64>,
    indices_end: u32,
    layer: i32,
    draw_space: DrawSpace,
}

/// This is only used internally to tell the render method where it is drawing
#[derive(Debug, Clone, Copy)]
enum BatchRenderTarget {
    Game,
    Surface,
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

    globals_bind_group_layout: BindGroupLayout,
    world_game_bind_group: BindGroup,
    world_surface_bind_group: BindGroup,
    surface_pixels_bind_group: BindGroup,
    ui_points_bind_group: BindGroup,

    shader_handle: AssetHandle<ShaderModule>,
    render_pipeline: RenderPipeline,
    clear_color: Color,

    white_pixel_handle: TextureHandle,

    atlas_bind_group_layout: BindGroupLayout,
    atlasses_dirty: bool,
    texture_atlasses: Vec<(DynamicTextureAtlas, BindGroup)>,
}
#[profiling::all_functions]
impl BatchRenderer {
    const MAX_QUAD_COUNT: usize = 10000;
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
        let mut previous_draw_space = DrawSpace::World;
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
            let current_draw_space = entry.draw_space;
            // Since the quads are ordered by layer, this means that we have now iterated through
            // all quads in this layer and we need to create a batch with the last ones
            if !first_iteration
                && (current_layer != previous_layer
                    || current_atlas_index != previous_atlas_index
                    || current_draw_space != previous_draw_space)
            {
                let vertices_range = (last_batch_end_quad_idx * 4)..(total_quads_processed * 4);
                let indices_end = num_quads_in_batch as u32 * 6;
                self.batches.push(Batch {
                    atlas_idx: previous_atlas_index,
                    vertices_range,
                    indices_end,
                    layer: previous_layer,
                    draw_space: previous_draw_space,
                });
                last_batch_end_quad_idx = total_quads_processed;
                num_quads_in_batch = 0;
            }

            let center = quad.topleft - top_left_offset(quad.size, current_draw_space);
            let quad_pts = quad_corners(center, quad.size, quad.angle, current_draw_space);

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
            // Top left
            self.vertices_to_draw.push(Vertex::new(
                quad_pts[0],
                quad.color,
                atlas_tex_coords_start,
            ));
            // Bottom left
            self.vertices_to_draw.push(Vertex::new(
                quad_pts[1],
                quad.color,
                Vec2::new(atlas_tex_coords_start.x, atlas_tex_coords_end.y),
            ));
            // Bottom right
            self.vertices_to_draw
                .push(Vertex::new(quad_pts[2], quad.color, atlas_tex_coords_end));
            // Top right
            self.vertices_to_draw.push(Vertex::new(
                quad_pts[3],
                quad.color,
                Vec2::new(atlas_tex_coords_end.x, atlas_tex_coords_start.y),
            ));

            first_iteration = false;
            previous_layer = current_layer;
            previous_atlas_index = current_atlas_index;
            previous_draw_space = current_draw_space;
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
            draw_space: previous_draw_space,
        });
    }

    fn prepare_to_render(&mut self) {
        if self.quads_to_draw.is_empty() {
            return;
        }
        self.create_batches();

        let device = {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            let mut context = graphics_sys.render_context();

            // Write the data from vertices to the vertex buffer
            // if this panics, increase MAX_QUAD_COUNT
            context.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&self.vertices_to_draw),
            );
            context.device.clone()
        };

        // meaning if we will want to render quads with textures, which havent been rendered to any atlas yet or have changed
        if self.atlasses_dirty {
            let mut atlas_encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
            {
                let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
                let mut context = graphics_sys.render_context();
                context.queue.submit(Some(atlas_encoder.finish()));
            }
            self.atlasses_dirty = false;
        }
    }

    fn render_batch_layers(&mut self, clear: bool, render_target: BatchRenderTarget) {
        if self.quads_to_draw.is_empty() && !clear {
            return;
        }
        self.prepare_to_render();

        let view = self.ctx.get::<Camera>().render_view();
        let viewport = view.game_viewport_surface_px;
        let surface_size = view.presentation.surface_size;
        let target_size = match render_target {
            BatchRenderTarget::Game => {
                super::view_mapping::game_target_size(view.presentation.game_size)
            }
            BatchRenderTarget::Surface => surface_size,
        };

        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        let mut context = graphics_sys.render_context();

        let layer_range = i32::MIN..i32::MAX;

        #[cfg(feature = "trace")]
        let prof = context.profiler.lock().unwrap();
        #[cfg(feature = "trace")]
        let mut profiler_scope = prof.scope("BatchRenderer render", &mut context.encoder);

        let rpass_desc = wgpu::RenderPassDescriptor {
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
        };

        #[cfg(feature = "trace")]
        let mut rpass = profiler_scope.scoped_render_pass("main render pass", rpass_desc);
        #[cfg(not(feature = "trace"))]
        let mut rpass = context.encoder.begin_render_pass(&rpass_desc);

        // Matrices include presentation placement. Never apply the viewport twice.
        rpass.set_viewport(
            0.0,
            0.0,
            target_size.x as f32,
            target_size.y as f32,
            0.0,
            1.0,
        );
        for batch in &self.batches {
            rpass.set_pipeline(&self.render_pipeline);

            let globals = match (batch.draw_space, render_target) {
                (DrawSpace::World, BatchRenderTarget::Game) => {
                    rpass.set_scissor_rect(0, 0, target_size.x, target_size.y);
                    &self.world_game_bind_group
                }
                (DrawSpace::World, BatchRenderTarget::Surface) => {
                    rpass.set_scissor_rect(
                        viewport.origin.x,
                        viewport.origin.y,
                        viewport.size.x,
                        viewport.size.y,
                    );
                    &self.world_surface_bind_group
                }
                (DrawSpace::SurfacePixels, BatchRenderTarget::Surface) => {
                    rpass.set_scissor_rect(0, 0, surface_size.x, surface_size.y);
                    &self.surface_pixels_bind_group
                }
                (DrawSpace::UiPoints, BatchRenderTarget::Surface) => {
                    rpass.set_scissor_rect(0, 0, surface_size.x, surface_size.y);
                    &self.ui_points_bind_group
                }
                _ => panic!("SurfacePixels and UiPoints require the surface render phase"),
            };
            rpass.set_bind_group(0, globals, &[]);

            rpass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
            rpass.set_vertex_buffer(
                0,
                self.vertex_buffer.slice(
                    (batch.vertices_range.start * VERTEX_SIZE as u64)
                        ..(batch.vertices_range.end * VERTEX_SIZE as u64),
                ),
            );

            rpass.set_bind_group(1, &self.texture_atlasses[batch.atlas_idx].1, &[]);
            rpass.draw_indexed(0..batch.indices_end, 0, 0..1);
        }
    }

    fn on_game_render(&mut self, _: &graphics::events::RenderGame) {
        self.render_batch_layers(true, BatchRenderTarget::Game);
    }
    fn on_game_render_done(&mut self, _: &graphics::events::GameRenderingDone) {
        self.end_frame();
    }

    fn on_ui_render(&mut self, _: &graphics::events::RenderUi) {
        self.render_batch_layers(false, BatchRenderTarget::Surface);
    }
    fn on_ui_render_done(&mut self, _: &graphics::events::UiRenderingDone) {
        self.end_frame();
    }

    /// Performs clean-up at the end of the frame
    fn end_frame(&mut self) {
        self.batches.clear();
        self.quads_to_draw.clear();
        self.changed_asset_ids.clear();
        self.vertices_to_draw.clear();
    }

    /// Records a new quad that needs to be drawn this frame with positive angles rotating CCW in World and CW in SurfacePixels/UiPoints.
    pub fn draw_quad<C: IntoGpuColor>(
        &mut self,
        topleft: Vec2,
        size: Vec2,
        angle: f32,
        color: C,
        texture: Option<AssetHandle<TextureBundle>>,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        assert!(topleft.is_finite() && size.is_finite() && angle.is_finite());
        assert!(size.x >= 0.0 && size.y >= 0.0);
        let mut used_texture_atlas_idx = 0;
        if let Some(handle) = &texture {
            let mut has_texture = false;
            for (idx, (atlas, _)) in self.texture_atlasses.iter().enumerate() {
                if atlas.contains_texture(handle) {
                    has_texture = true;
                    used_texture_atlas_idx = idx;
                    break;
                }
            }
            if !has_texture {
                self.atlasses_dirty = true;
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

        let rgba: [f32; 4] = color.into_gpu_color();
        self.quads_to_draw.push(std::cmp::Reverse(BatchQuadEntry {
            layer,
            draw_space,
            used_texture_atlas_idx,
            quad: Quad {
                topleft,
                size,
                angle,
                color: rgba,
                texture,
            },
        }));
    }

    /// Records a new quad that needs to be drawn this frame. Draws the quad with its center at the `center` position and extending `size/2` to either side with positive angles rotating CCW in World and CW in SurfacePixels/UiPoints.
    pub fn draw_quad_with_center<C: IntoGpuColor>(
        &mut self,
        center: Vec2,
        size: Vec2,
        angle: f32,
        color: C,
        texture: Option<AssetHandle<TextureBundle>>,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        let topleft = center + top_left_offset(size, draw_space);
        self.draw_quad(topleft, size, angle, color, texture, layer, draw_space);
    }

    /// Records a new quad that needs to be drawn this frame. With positive angles rotating CCW in World and CW in SurfacePixels/UiPoints.
    pub fn draw_quad_with_bottomleft<C: IntoGpuColor>(
        &mut self,
        bottomleft: Vec2,
        size: Vec2,
        angle: f32,
        color: C,
        texture: Option<AssetHandle<TextureBundle>>,
        layer: i32,
        draw_space: DrawSpace,
    ) {
        let topleft = bottomleft + Vec2::new(0.0, draw_space.top_sign() * size.y);
        self.draw_quad(topleft, size, angle, color, texture, layer, draw_space);
    }

    /// Notifies the BatchRenderer that this texture has changed it's content and needs to be updated
    pub fn mark_quad_texture_dirty(&mut self, texture: AssetHandle<TextureBundle>) {
        for (idx, (atlas, _)) in self.texture_atlasses.iter_mut().enumerate() {
            if atlas.contains_texture(&texture) {
                atlas.mark_texture_dirty(texture);
                self.atlasses_dirty = true;
                break;
            }
        }
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
                    Some(&self.globals_bind_group_layout),
                    Some(&self.atlas_bind_group_layout),
                ],
                asset_sys.get(&self.shader_handle).unwrap(),
                graphics_sys.get_game_view_format(),
            );
        } else {
            self.changed_asset_ids.insert(event.asset_id);
        }
    }

    pub fn get_surface_size(&self) -> UVec2 {
        let size = self.ctx.get::<GraphicsSystem>().get_surface_resolution();
        UVec2::new(size.width, size.height)
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
                    attributes: &VERTEX_ATTR,
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fragment_main"),
                targets: &[color_state],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
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
    fn create_indices() -> [u32; BatchRenderer::MAX_INDEX_COUNT] {
        let mut indices: [u32; BatchRenderer::MAX_INDEX_COUNT] =
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

    #[cfg(target_arch = "wasm32")]
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_game_render)
        .with(Self::on_game_render_done)
        .with(Self::on_ui_render)
        .with(Self::on_ui_render_done);
    #[cfg(not(target_arch = "wasm32"))]
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_assetchange)
        .with(Self::on_game_render)
        .with(Self::on_game_render_done)
        .with(Self::on_ui_render)
        .with(Self::on_ui_render_done);

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
            Some((
                &[255, 255, 255, 255],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: None,
                },
            )),
        );

        let camera = ctx.get::<Camera>();
        let globals_bind_group_layout = Self::create_globals_bind_group_layout(device);

        let world_game_bind_group = Self::create_globals_bind_group(
            device,
            &globals_bind_group_layout,
            camera.world_to_game_clip_buffer(),
        );
        let world_surface_bind_group = Self::create_globals_bind_group(
            device,
            &globals_bind_group_layout,
            camera.world_to_surface_clip_buffer(),
        );
        let surface_pixels_bind_group = Self::create_globals_bind_group(
            device,
            &globals_bind_group_layout,
            camera.surface_pixels_to_clip_buffer(),
        );
        let ui_points_bind_group = Self::create_globals_bind_group(
            device,
            &globals_bind_group_layout,
            camera.ui_points_to_clip_buffer(),
        );

        let atlas_bgl = Self::create_atlas_bind_group_layout(device);
        let first_atlas = DynamicTextureAtlas::new(
            "BatchRenderer atlas",
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
            .load(asset_source!("../shaders/batch_renderer.wgsl"), ())
            .unwrap();
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let render_pipeline = Self::create_render_pipeline(
            graphics_sys.device(),
            &[Some(&globals_bind_group_layout), Some(&atlas_bgl)],
            ctx.get::<AssetSystem>().get(&shader_handle).unwrap(),
            graphics_sys.get_game_view_format(),
        );
        drop(graphics_sys);

        Self {
            ctx,

            vertex_buffer,
            index_buffer,
            index_format: wgpu::IndexFormat::Uint32,

            quads_to_draw: BinaryHeap::new(),
            changed_asset_ids: HashSet::default(),
            batches: vec![],
            vertices_to_draw: Vec::with_capacity(1000),

            globals_bind_group_layout,
            world_game_bind_group,
            world_surface_bind_group,
            surface_pixels_bind_group,
            ui_points_bind_group,

            shader_handle,
            render_pipeline,
            clear_color: Color::BLACK,

            white_pixel_handle,

            atlas_bind_group_layout: atlas_bgl,
            atlasses_dirty: false,
            texture_atlasses,
        }
    }
}
