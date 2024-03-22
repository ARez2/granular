#![allow(unused)]
#![allow(clippy::identity_op)]

use std::num::{NonZeroU32, NonZeroU64};
use std::ops::Range;

use bytemuck_derive::{Zeroable, Pod};
use geese::{GeeseSystem, dependencies, GeeseContextHandle, Mut, EventHandlers, event_handlers};
use glam::{IVec2, Vec2};
use log::*;
use wgpu::{BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Color, ColorTargetState, Device, Extent3d, IndexFormat, RenderPipeline, Sampler, ShaderModule, Texture, TextureView};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use glam::f32::Mat4;
use palette::Srgba;
use rustc_hash::FxHashMap as HashMap;

use crate::assets::{AssetHandle, AssetSystem, ShaderAsset, TextureAsset};

use super::graphics_system::{GraphicsSystem, Vertex, VERTEX_SIZE};
use super::{Camera, DynamicBuffer};



struct Batch {
    render_pipeline_idx: usize,
    bind_group: BindGroup,
    bind_group_layout_idx: usize,
    num_textures_used: usize,
    vertices_range: Range<u64>,
    indices_end: u32
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


// We need this for Rust to store our data correctly for the shaders
#[repr(C)]
// This is so we can store this in a buffer
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct ShaderGlobals {
    transform: Mat4
}




pub struct Renderer {
    ctx: GeeseContextHandle<Self>,
    
    vertex_buffer: DynamicBuffer<Vertex>,
    index_buffer: Buffer,
    index_format: IndexFormat,
    // Links the asset id (1st u64) of a texture to its position in the internal
    // texture array (2nd u64) (and its handle, for easier access)
    texture_slots: HashMap<u64, (u64, AssetHandle<TextureAsset>)>,

    quads_to_draw: Vec<Quad>,
    batches: Vec<Batch>,
    
    bind_group: (BindGroup, BindGroupLayout),
    clear_color: Color,
    extents: Extent3d,
    shader_handle: AssetHandle<ShaderAsset>,
    render_pipeline: RenderPipeline,

    white_pixel: (Texture, TextureView, Sampler)
}
impl Renderer {
    const MAX_QUAD_COUNT: usize = 1000;
    const MAX_VERTEX_COUNT: usize = Renderer::MAX_QUAD_COUNT * 4;
    const MAX_INDEX_COUNT: usize = Renderer::MAX_QUAD_COUNT * 6;
    const MAX_TEXTURE_COUNT: usize = 16;


    pub fn start_frame(&mut self) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.begin_frame();
    }

    pub fn end_frame(&mut self) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.present_frame();
        self.batches.clear();
        self.quads_to_draw.clear();
    }


    /// Handles batching and issuing draw calls accordingly
    pub fn flush(&mut self) {
        /// Creates a new Batch object from the given parameters, uses the 1x1 white pixel when a texture is None
        /// automatically creates a new bind group for each batch and only a new bindgroup layout/ render pipeline,
        /// when the amount of textures inside the bind group has changed (reuses existing ones if not)
        let mut create_new_batch = 
        | textures: &Vec<Option<AssetHandle<TextureAsset>>>,
          render_pipelines: &mut Vec<RenderPipeline>,
          bind_group_layouts: &mut Vec<BindGroupLayout>,
          vertices_range: Range<u64>,
          indices_end: u32 | {
            
            let asset_sys = self.ctx.get::<AssetSystem>();
            let mut views = vec![];
            let mut samplers = vec![];
            
            // Populate views and samplers with the actual data, using the asset system
            textures.iter().for_each(|tex| {
                match tex {
                    // Use the 1x1 white pixel texture instead
                    None => {
                        views.push(&self.white_pixel.1);
                        samplers.push(&self.white_pixel.2);
                    },
                    Some(tex_handle) => {
                        let texture = asset_sys.get(tex_handle);
                        views.push(texture.view());
                        samplers.push(texture.sampler());
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
                    let bg = Self::create_bind_group(device, &layout, &camera, screen_size, &views, &samplers);
                    let shader = asset_sys.get(&self.shader_handle);
                    let color_state = Some(graphics_sys.surface_config().format.into());
                    let rp = Self::create_render_pipeline(device, &layout, shader.module(), color_state);
                    bind_group_layouts.push(layout);
                    bind_group_layout_idx = bind_group_layouts.len() as i32 - 1;
                    render_pipelines.push(rp);
                    (bg, render_pipelines.len() - 1)
                // We reuse another batches layout/ pipeline
                } else {
                    // Use the layout of the other batch
                    let layout = &bind_group_layouts[bind_group_layout_idx as usize];
                    (Self::create_bind_group(device, layout, &camera, screen_size, &views, &samplers), bind_group_layout_idx as usize)
                }
            };

            trace!("Creating batch with");
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
                indices_end
            })
        };


        // Sort all quads based on texture index (so that quads with the same index will be in one batch
        // and it is safe to assume that we won't have to rebind the same texture in multiple batches)
        self.quads_to_draw.sort_by_key(|quad| quad.get_texture_index());


        let mut last_batch_end_quad_idx: u64 = 0;
        let mut textures_in_batch: Vec<Option<AssetHandle<TextureAsset>>> = vec![];
        let mut vertices: Vec<Vertex> = vec![];
        // Will get filled by create_new_batch
        let mut render_pipelines = vec![];
        let mut bind_group_layouts = vec![];
        self.quads_to_draw.iter().enumerate().for_each(|(quad_idx, quad)| {
            let quad_pos = quad.center;
            //info!("Old quad pos: {}   New pos: {}", quad.center, quad_pos);
            let x = quad_pos.x; let y = quad_pos.y;
            let w = quad.size.x; let h = quad.size.y;
            let color = [quad.color.red, quad.color.green, quad.color.blue, quad.color.alpha];
            
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
                let num_quads_in_batch = quad_idx as u64;
                let vertices_range = (last_batch_end_quad_idx * 4)..(num_quads_in_batch * 4);
                let indices_end = num_quads_in_batch as u32 * 6;
                trace!("Max texture bindings reached, creating new batch");
                create_new_batch(&textures_in_batch, &mut render_pipelines, &mut bind_group_layouts, vertices_range, indices_end);
                textures_in_batch.clear();
                last_batch_end_quad_idx = num_quads_in_batch;
            };


            
            if !texture_in_batch {
                textures_in_batch.push(quad.texture.clone());
            };
            let tex_index = textures_in_batch.len() as u64 - 1;

            // Add the vertices of the quad to vertices, respecting size and attributes
            vertices.reserve(4);
            vertices.push(Vertex::new(IVec2::new(x - w, y - h), color, Vec2::new(0.0, 1.0), tex_index));
            vertices.push(Vertex::new(IVec2::new(x - w, y + h), color, Vec2::new(0.0, 0.0), tex_index));
            vertices.push(Vertex::new(IVec2::new(x + w, y + h), color, Vec2::new(1.0, 0.0), tex_index));
            vertices.push(Vertex::new(IVec2::new(x + w, y - h), color, Vec2::new(1.0, 1.0), tex_index));
        });
        // Create the last batch of this frame (with the remaining quads)
        let vertices_range = ((last_batch_end_quad_idx) * 4)..(vertices.len() as u64);
        let indices_end = (self.quads_to_draw.len() as u32 - last_batch_end_quad_idx as u32) * 6;
        create_new_batch(&textures_in_batch, &mut render_pipelines, &mut bind_group_layouts, vertices_range, indices_end);

        // Write the data from vertices to the vertex buffer
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        self.vertex_buffer.write(&graphics_sys, 0, bytemuck::cast_slice(&vertices));


        let framedata = graphics_sys.frame_data_mut();
        if framedata.is_none() {
            warn!("No frame data present, call begin_frame first!");
            return;
        };
        let framedata = framedata.unwrap();

        // Create the new render pass (clearing the screen)
        let mut rpass = framedata.2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &framedata.1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        
        for batch in self.batches.iter() {
            // We only need to reload the pipeline if the bindgroup layout changed
            // (which would happen when the number of textures that are bound changes)
            // Meaning if we draw the first 2 batches both with 16 bound textures, the layout
            // stays the same and we do not need to reload the pipeline.
            rpass.set_pipeline(&render_pipelines[batch.render_pipeline_idx]);
            // The index buffer stays the same over all batches
            rpass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
            // Only use a slice of the vertex buffer, which belongs to the current batch
            rpass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice((batch.vertices_range.start * VERTEX_SIZE as u64)..(batch.vertices_range.end * VERTEX_SIZE as u64)));
            // Use the bind group specified by the batch
            rpass.set_bind_group(0, &batch.bind_group, &[]);
            rpass.draw_indexed(0..batch.indices_end, 0, 0..1);
        }
    }


    /// Records a new quad that needs to be drawn this frame (low performance cost)
    pub fn draw_quad(&mut self, quad: &Quad) {
        self.quads_to_draw.push(quad.clone());
    }


    /// Requests a redraw from the underlying GraphicsSystem
    pub fn request_redraw(&self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.request_redraw();
    }


    /// Resizes the surface with the new_size
    pub(crate) fn resize(&mut self, new_size: &PhysicalSize<u32>) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.resize_surface(new_size);
        drop(graphics_sys);
        let mut camera = self.ctx.get_mut::<Camera>();
        camera.set_screen_size((new_size.width, new_size.height));
    }


    /// Reloads parts of the renderer depending on what asset changed
    fn on_assetchange(&mut self, event: &crate::assets::events::AssetReload) {
        if event.asset_id == **self.shader_handle.id() {
            self.reload_render_pipeline();
        // TODO: Change this to call a function that sets up a new bind group
        } else {
            // let graphics_sys = self.ctx.get::<GraphicsSystem>();
            // let device = graphics_sys.device();
            // let asset_sys = self.ctx.get::<AssetSystem>();
            // let background_tex = asset_sys.get(&self.background_image);



            // // TODO: Insert the currently tracked array of textures here, where only the element gets updated that changed
            // self.bind_group_bundle.descriptor.entries[BindGroupBundle::BG_DESC_TEX_ARRAY_IDX] = wgpu::BindGroupEntry {
            //     binding: BindGroupBundle::BG_DESC_TEX_ARRAY_IDX,
            //     resource: wgpu::BindingResource::TextureViewArray(&[
            //         &self.white_pixel.1
            //     ]),
            // };
            // self.bind_group_bundle.descriptor.entries[BindGroupBundle::BG_DESC_SAMPLER_ARRAY_IDX] = wgpu::BindGroupEntry {
            //     binding: BindGroupBundle::BG_DESC_SAMPLER_ARRAY_IDX,
            //     resource: wgpu::BindingResource::SamplerArray(&[
            //         &self.white_pixel.2
            //     ]),
            // };
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
    fn create_bind_group(device: &wgpu::Device, layout: &BindGroupLayout, camera: &Camera, screen_size: Vec2, views: &Vec<&TextureView>, samplers: &Vec<&Sampler>) -> BindGroup {
        let tex_views = views.as_slice();
        let tex_samplers = samplers.as_slice();

        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shader globals buffer"),
            contents: bytemuck::cast_slice(&[camera.canvas_transform()]),
            usage: BufferUsages::UNIFORM
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
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
    fn create_indices() -> [u16; Renderer::MAX_INDEX_COUNT] {
        let mut indices: [u16; Renderer::MAX_INDEX_COUNT] = [0; Renderer::MAX_INDEX_COUNT];
        let mut offset = 0;
        (0..Renderer::MAX_INDEX_COUNT).step_by(6).for_each(|i| {
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

impl GeeseSystem for Renderer {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<Camera>>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_assetchange);


    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let base_shader_handle = asset_sys.load::<ShaderAsset>("shaders/base.wgsl", true);
        // Drop the mutable reference, from now on we only need it immutably
        drop(asset_sys);

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let vertex_buffer = DynamicBuffer::with_capacity(
            &graphics_sys,
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            Renderer::MAX_VERTEX_COUNT);
        let indices = Renderer::create_indices();
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let conf = graphics_sys.surface_config();
        let extents = wgpu::Extent3d { width: conf.width, height: conf.height, depth_or_array_layers: 1 };

        let device = graphics_sys.device();
        


        // Set up a white 1x1 texture
        let white_texture_descriptor = wgpu::TextureDescriptor {
            size: wgpu::Extent3d::default(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("White pixel texture descriptor"),
            view_formats: &[],
        };
        let white_pixel = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("White pixel texture"),
            view_formats: &[],
            ..white_texture_descriptor
        });
        let white_pixel_view = white_pixel.create_view(&wgpu::TextureViewDescriptor::default());
        let white_pixel_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("white pixel sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            //mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        
        let q = graphics_sys.queue();
        q.write_texture(
            white_pixel.as_image_copy(),
            &[255, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d::default()
        );

        let camera = ctx.get::<Camera>();

        let asset_sys = ctx.get::<AssetSystem>();
        let bind_group_layout = Self::create_bind_group_layout(device, 1, 1);
        let bind_group = Renderer::create_bind_group(
            device,
            &bind_group_layout,
            &camera,
            Vec2::new(conf.width as f32, conf.height as f32),
            &vec![&white_pixel_view],
            &vec![&white_pixel_sampler]
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

            quads_to_draw: vec![],
            batches: vec![],
            
            bind_group: (bind_group, bind_group_layout),
            shader_handle: base_shader_handle,
            render_pipeline,
            clear_color: Color::BLACK,
            extents,

            white_pixel: (white_pixel, white_pixel_view, white_pixel_sampler),
        }
    }
}

