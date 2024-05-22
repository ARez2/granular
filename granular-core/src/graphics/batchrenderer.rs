#![allow(unused)]
#![allow(clippy::identity_op)]

use std::collections::BinaryHeap;
use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;

use bytemuck_derive::{Zeroable, Pod};
use geese::{GeeseSystem, dependencies, GeeseContextHandle, Mut, EventHandlers, event_handlers};
use glam::{IVec2, Vec2};
use log::*;
use palette::cast::ComponentsInto;
use wgpu::{BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Color, ColorTargetState, Device, Extent3d, IndexFormat, RenderPass, RenderPipeline, Sampler, ShaderModule, Texture, TextureView};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use glam::f32::Mat4;
use palette::Srgba;
use rustc_hash::FxHashMap as HashMap;

use crate::assets::{AssetHandle, AssetSystem, ShaderAsset, TextureAsset};

use super::graphics_system::{GraphicsSystem, Vertex, VERTEX_SIZE};
use super::{Camera, DynamicBuffer, TextureBundle};



struct Batch {
    render_pipeline_idx: usize,
    bind_group: BindGroup,
    bind_group_layout_idx: usize,
    num_textures_used: usize,
    vertices_range: Range<u64>,
    indices_end: u32,
    layer: i32
}




#[derive(Debug, Clone)]
pub struct Quad {
    pub center: IVec2,
    pub size: IVec2,
    /// If there is a texture set, this tints the texture
    pub color: Srgba,
    pub texture: Option<AssetHandle<TextureAsset>>
}
impl Quad {
    pub(crate) fn get_texture_index(&self) -> u64 {
        match &self.texture {
            None => 0,
            Some(tex_handle) => **tex_handle.id()
        }
    }
}
impl PartialEq for Quad {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
impl Eq for Quad {}



/// A simple wrapper that stores a quad and a corresponding layer
/// for use in the binary heap
#[derive(Debug, PartialEq, Eq)]
struct BatchQuadEntry {
    layer: i32,
    quad: Quad
}
impl PartialOrd for BatchQuadEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.layer.cmp(&other.layer))
    }
}
impl Ord for BatchQuadEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.layer.cmp(&other.layer)
    }
}







/// A simple batch renderer that supports layering of quads
pub struct BatchRenderer {
    ctx: GeeseContextHandle<Self>,

    vertex_buffer: DynamicBuffer<Vertex>,
    index_buffer: Buffer,
    index_format: IndexFormat,
    // Links the asset id (1st u64) of a texture to its position in the internal
    // texture array (2nd u64) (and its handle, for easier access)
    texture_slots: HashMap<u64, (u64, AssetHandle<TextureAsset>)>,

    quads_to_draw: BinaryHeap<std::cmp::Reverse<BatchQuadEntry>>,
    batches: Vec<Batch>,
    vertices_to_draw: Vec<Vertex>,
    render_pipelines: Vec<RenderPipeline>,
    
    bind_group: (BindGroup, BindGroupLayout),

    render_pipeline: RenderPipeline,
    shader_handle: AssetHandle<ShaderAsset>,
    clear_color: Color,

    white_pixel: TextureBundle
}
impl BatchRenderer {
    const MAX_QUAD_COUNT: usize = 1000;
    const MAX_VERTEX_COUNT: usize = BatchRenderer::MAX_QUAD_COUNT * 4;
    const MAX_INDEX_COUNT: usize = BatchRenderer::MAX_QUAD_COUNT * 6;
    const MAX_TEXTURE_COUNT: usize = 15;
    
    
    pub(super) fn end_frame(&mut self) {
        self.batches.clear();
        self.quads_to_draw.clear();
        self.render_pipelines.clear();
        self.vertices_to_draw.clear();
    }


    /// Handles batching and issuing draw calls accordingly
    pub(super) fn create_batches(&mut self) {
        let cam = self.ctx.get::<Camera>();
        let shaderglobals = cam.canvas_transform_buffer();

        /// Creates a new Batch object from the given parameters, uses the 1x1 white pixel when a texture is None
        /// automatically creates a new bind group for each batch and only a new bindgroup layout/ render pipeline,
        /// when the amount of textures inside the bind group has changed (reuses existing ones if not)
        let mut create_new_batch = 
        | textures: &Vec<Option<AssetHandle<TextureAsset>>>,
          bind_group_layouts: &mut Vec<BindGroupLayout>,
          vertices_range: Range<u64>,
          indices_end: u32,
          batch_layer: i32 | {
            let asset_sys = self.ctx.get::<AssetSystem>();
            let mut views = vec![];
            let mut samplers = vec![];
            
            // Populate views and samplers with the actual data, using the asset system
            textures.iter().for_each(|tex| {
                match tex {
                    // Use the 1x1 white pixel texture instead
                    None => {
                        views.push(self.white_pixel.view());
                        samplers.push(self.white_pixel.sampler());
                    },
                    Some(tex_handle) => {
                        let asset = asset_sys.get(tex_handle);
                        views.push(asset.texture().view());
                        samplers.push(asset.texture().sampler());
                    }
                };
            });

            // See if another batch has already created a bind group layout with that many textures
            // use that if possible
            let num_textures_used = textures.len();
            let mut bind_group_layout_idx = -1;
            for batch in self.batches.iter() {
                if batch.num_textures_used == num_textures_used {
                    bind_group_layout_idx = batch.bind_group_layout_idx as i32;
                    break;
                }
            };
            let graphics_sys = self.ctx.get::<GraphicsSystem>();
            let device = graphics_sys.device();
            let screen_size = {
                let sc = graphics_sys.surface_config();
                Vec2::new(sc.width as f32, sc.height as f32)
            };
            let (bind_group, render_pipeline_idx) = {
                let camera = self.ctx.get::<Camera>();
                // We want to create a completely new layout and render pipeline for this batch
                if bind_group_layout_idx == -1 {
                    let layout = Self::create_bind_group_layout(device, views.len() as u32, samplers.len() as u32);
                    let bg = Self::create_bind_group(device, &layout, shaderglobals, &views, &samplers);
                    let shader = asset_sys.get(&self.shader_handle);
                    let color_state = Some(wgpu::ColorTargetState {
                        format: graphics_sys.surface_config().format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    });
                    let rp = Self::create_render_pipeline(device, &layout, shader.module(), color_state);
                    bind_group_layouts.push(layout);
                    bind_group_layout_idx = bind_group_layouts.len() as i32 - 1;
                    self.render_pipelines.push(rp);
                    (bg, self.render_pipelines.len() - 1)
                // We reuse another batches layout/ pipeline
                } else {
                    // Use the layout of the other batch
                    let layout = &bind_group_layouts[bind_group_layout_idx as usize];
                    (Self::create_bind_group(device, layout, shaderglobals, &views, &samplers), bind_group_layout_idx as usize)
                }
            };

            trace!("Creating batch with");
            trace!("    - Layer {}", batch_layer);
            trace!("    - Vert. range: {:?}", vertices_range);
            trace!("    - Ind. end: {:?}", indices_end);
            trace!("    - Num textures: {}", num_textures_used);
            trace!("    - Bind group layout idx {}", bind_group_layout_idx);
            self.batches.push(Batch {
                render_pipeline_idx,
                bind_group,
                bind_group_layout_idx: bind_group_layout_idx as usize,
                num_textures_used,
                vertices_range,
                indices_end,
                layer: batch_layer
            })
        };

        let total_quads_to_draw = self.quads_to_draw.len();

        let mut last_batch_end_quad_idx: u64 = 0;
        let mut textures_in_batch: Vec<Option<AssetHandle<TextureAsset>>> = vec![];
        // Will get filled by create_new_batch
        let mut bind_group_layouts = vec![];
        
        let mut previous_layer = 0;
        let mut first_iteration = true;
        let mut num_quads_in_batch = 0;
        let mut total_quads_processed = 0;
        loop {
            let current_quad = self.quads_to_draw.pop();
            // We have reached the end of the heap
            if current_quad.is_none() {
                break;
            };
            let entry = current_quad.unwrap().0;
            let quad = entry.quad; let current_layer = entry.layer;
            // Since the quads are ordered by layer, this means that we have now iterated through
            // all quads in this layer and we need to create a batch with the last ones
            if !first_iteration && current_layer != previous_layer {
                let vertices_range = (last_batch_end_quad_idx * 4)..(total_quads_processed * 4);
                let indices_end = num_quads_in_batch as u32 * 6;
                create_new_batch(&textures_in_batch, &mut bind_group_layouts, vertices_range, indices_end, previous_layer);
                textures_in_batch.clear();
                last_batch_end_quad_idx = total_quads_processed;
                num_quads_in_batch = 0;
            }


            let quad_pos = quad.center;
            //info!("Old quad pos: {}   New pos: {}", quad.center, quad_pos);
            let x = quad_pos.x; let y = quad_pos.y;
            let w = quad.size.x; let h = quad.size.y;
            let color: [f32; 4] = quad.color.into();
            
            let mut texture_in_batch = false;
            // Custom comparison to see if this quads texture was already in this batches textures
            for tex in textures_in_batch.iter() {
                match &quad.texture {
                    None => {
                        if tex.is_none() {
                            texture_in_batch = true;
                        }
                    },
                    Some(quad_tex_handle) => {
                        if let Some(tex_handle) = tex {
                            if **tex_handle.id() == **quad_tex_handle.id() {
                                texture_in_batch = true;
                            }
                        };
                    }
                }
            };

            // In case we run out of bind slots, we create a new batch (and therefore new bind group)
            if textures_in_batch.len() >= Self::MAX_TEXTURE_COUNT && !texture_in_batch {
                let vertices_range = (last_batch_end_quad_idx * 4)..(total_quads_processed * 4);
                let indices_end = num_quads_in_batch as u32 * 6;
                create_new_batch(&textures_in_batch, &mut bind_group_layouts, vertices_range, indices_end, current_layer);
                textures_in_batch.clear();
                last_batch_end_quad_idx = total_quads_processed;
                num_quads_in_batch = 0;
            };

            if !texture_in_batch {
                textures_in_batch.push(quad.texture.clone());
            };
            let tex_index = textures_in_batch.len() as u64 - 1;

            // Add the vertices of the quad to vertices, respecting size and attributes
            self.vertices_to_draw.reserve(4);
            self.vertices_to_draw.push(Vertex::new(IVec2::new(x - w, y - h), color, Vec2::new(0.0, 1.0), tex_index));
            self.vertices_to_draw.push(Vertex::new(IVec2::new(x - w, y + h), color, Vec2::new(0.0, 0.0), tex_index));
            self.vertices_to_draw.push(Vertex::new(IVec2::new(x + w, y + h), color, Vec2::new(1.0, 0.0), tex_index));
            self.vertices_to_draw.push(Vertex::new(IVec2::new(x + w, y - h), color, Vec2::new(1.0, 1.0), tex_index));

            first_iteration = false;
            previous_layer = current_layer;
            num_quads_in_batch += 1;
            total_quads_processed += 1;
        };

        // Create the last batch of this frame (with the remaining quads)
        let vertices_range = ((last_batch_end_quad_idx) * 4)..(self.vertices_to_draw.len() as u64);
        let indices_end = num_quads_in_batch as u32 * 6;
        create_new_batch(&textures_in_batch, &mut bind_group_layouts, vertices_range, indices_end, previous_layer);
    }


    pub(super) fn prepare_to_render(&mut self) {
        // Write the data from vertices to the vertex buffer
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        self.vertex_buffer.write(&graphics_sys, 0, bytemuck::cast_slice(&self.vertices_to_draw));
    }


    pub fn render_batch_layers(&mut self, layer_range: Range<i32>, clear: bool) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        let framedata = graphics_sys.frame_data_mut();
        if framedata.is_none() {
            warn!("No frame data present, call begin_frame first!");
            return;
        };
        let framedata = framedata.unwrap();
        
        let mut rpass = framedata.2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("BatchRenderer render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &framedata.1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: match clear {
                        true => wgpu::LoadOp::Clear(Color::BLACK),
                        false => wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.batches.iter().filter(|b| {
            layer_range.contains(&b.layer)
        }).for_each(|batch| {
            // We only need to reload the pipeline if the bindgroup layout changed
            // (which would happen when the number of textures that are bound changes)
            // Meaning if we draw the first 2 batches both with 16 bound textures, the layout
            // stays the same and we do not need to reload the pipeline.
            rpass.set_pipeline(&self.render_pipelines[batch.render_pipeline_idx]);
            // The index buffer stays the same over all batches
            rpass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
            // Only use a slice of the vertex buffer, which belongs to the current batch
            rpass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice((batch.vertices_range.start * VERTEX_SIZE as u64)..(batch.vertices_range.end * VERTEX_SIZE as u64)));
            // Use the bind group specified by the batch
            rpass.set_bind_group(0, &batch.bind_group, &[]);
            rpass.draw_indexed(0..batch.indices_end, 0, 0..1);
        });
    }


    /// Records a new quad that needs to be drawn this frame (low performance cost, even though quad gets cloned)
    pub fn draw_quad(&mut self, quad: &Quad, layer: i32) {
        self.quads_to_draw.push(std::cmp::Reverse(BatchQuadEntry {
            layer,
            quad: quad.clone()
        }));
    }


    /// Reloads parts of the renderer depending on what asset changed
    fn on_assetchange(&mut self, event: &crate::assets::events::AssetReload) {
        if event.asset_id == **self.shader_handle.id() {
            self.reload_render_pipeline();
        }
    }


    /// Helper function to set up a new render pipeline using the same shaders
    fn reload_render_pipeline(&mut self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        let asset_sys = self.ctx.get::<AssetSystem>();
        let shader = asset_sys.get(&self.shader_handle);
        self.render_pipeline = Self::create_render_pipeline(
            graphics_sys.device(),
            &self.bind_group.1,
            shader.module(),
            Some(graphics_sys.surface_config().format.into()));
    }


    /// Helper function for creating a new render pipeline
    fn create_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        shader: &ShaderModule,
        color_state: Option<ColorTargetState>
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("main"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vert_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: VERTEX_SIZE as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex, // position        color       tex_coords     tex_index
                    attributes: &wgpu::vertex_attr_array![0 => Sint32x2, 1 => Float32x4, 2 => Float32x2, 3 => Sint32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: "uniform_main",
                targets: &[color_state],
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }


    /// Creates a new bind group layout from a number of texture views/ samplers
    fn create_bind_group_layout(device: &Device, num_views: u32, num_samplers: u32) -> BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZeroU64::new(64).unwrap()),
                    },
                    count: None,
                },
                // Texture array
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: NonZeroU32::new(num_views),
                },
                // Sampler array
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: NonZeroU32::new(num_samplers),
                }
            ],
        })
    }


    /// Creates the bind group based on a list of textures
    fn create_bind_group(device: &wgpu::Device, layout: &BindGroupLayout, shaderglobals: &Buffer, views: &Vec<&TextureView>, samplers: &Vec<&Sampler>) -> BindGroup {
        let tex_views = views.as_slice();
        let tex_samplers = samplers.as_slice();

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shaderglobals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureViewArray(tex_views),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::SamplerArray(tex_samplers),
                }
            ],
            layout,
            label: Some("bind group"),
        });

        bind_group
    }


    /// Creates an array of indices, following the typical quad indexing method (0-1-2, 2-3-0)
    fn create_indices() -> [u16; BatchRenderer::MAX_INDEX_COUNT] {
        let mut indices: [u16; BatchRenderer::MAX_INDEX_COUNT] = [0; BatchRenderer::MAX_INDEX_COUNT];
        let mut offset = 0;
        (0..BatchRenderer::MAX_INDEX_COUNT).step_by(6).for_each(|i| {
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

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_assetchange);


    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let base_shader_handle = asset_sys.load::<ShaderAsset>("shaders/batch_renderer.wgsl", true);
        // Drop the mutable reference, from now on we only need it immutably
        drop(asset_sys);

        let graphics_sys = ctx.get::<GraphicsSystem>();
        
        let vertex_buffer = DynamicBuffer::with_capacity(
            "Dynamic vertex buffer",
            &graphics_sys,
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            BatchRenderer::MAX_VERTEX_COUNT);
        let indices = BatchRenderer::create_indices();
        let device = graphics_sys.device();
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Set up a white 1x1 texture
        let queue = graphics_sys.queue();
        let white_pixel = TextureBundle::new(device, queue,
            "White pixel texture",
            wgpu::Extent3d::default(),
            wgpu::TextureDescriptor {
                size: wgpu::Extent3d::default(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                label: Some("White pixel texture descriptor"),
                view_formats: &[]
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
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            }
        );
        
        let camera = ctx.get::<Camera>();
        let asset_sys = ctx.get::<AssetSystem>();
        let conf = graphics_sys.surface_config();
        let bind_group_layout = Self::create_bind_group_layout(device, 1, 1);
        let bind_group = BatchRenderer::create_bind_group(
            device,
            &bind_group_layout,
            camera.canvas_transform_buffer(),
            &vec![white_pixel.view()],
            &vec![white_pixel.sampler()]
        );

        let base_shader_module = asset_sys.get(&base_shader_handle);
        let render_pipeline = Self::create_render_pipeline(
            device,
            &bind_group_layout,
            base_shader_module.module(),
            Some(graphics_sys.surface_config().format.into())
        );

        drop(graphics_sys);
        drop(asset_sys);
        drop(camera);

        Self {
            ctx,

            vertex_buffer,
            index_buffer,
            index_format: wgpu::IndexFormat::Uint16,
            texture_slots: HashMap::default(),

            quads_to_draw: BinaryHeap::new(),
            batches: vec![],
            render_pipelines: Vec::with_capacity(10),
            vertices_to_draw: Vec::with_capacity(1000),
            
            bind_group: (bind_group, bind_group_layout),

            render_pipeline,
            clear_color: Color::RED,
            shader_handle: base_shader_handle,

            white_pixel,
        }
    }
}

