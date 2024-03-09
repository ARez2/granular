#![allow(unused)]
#![allow(clippy::identity_op)]

use std::num::{NonZeroU32, NonZeroU64};

use bytemuck_derive::{Zeroable, Pod};
use geese::{GeeseSystem, dependencies, GeeseContextHandle, Mut, EventHandlers, event_handlers};
use log::*;
use wgpu::{BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Color, ColorTargetState, Device, Extent3d, IndexFormat, RenderPipeline, Sampler, ShaderModule, Texture, TextureView};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use glam::f32::Mat4;
use rustc_hash::FxHashMap as HashMap;

use crate::assets::{AssetHandle, AssetSystem, ShaderAsset, TextureAsset};

use super::graphics_system::{GraphicsSystem, Vertex, VERTEX_SIZE};


// TODO: Use this
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ShaderGlobals {
    _view_proj: Mat4,
    _transform: Mat4
}


pub enum QuadColoring {
    Color(wgpu::Color),
    Texture(AssetHandle<TextureAsset>)
}


pub struct Renderer {
    ctx: GeeseContextHandle<Self>,
    
    current_batch: Vec<Vertex>,
    num_quads_drawn: u32,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_format: IndexFormat,
    // Links the asset id of a texture to its position in the internal
    // texture array (and its handle, for easier access)
    texture_slots: HashMap<u64, (u64, AssetHandle<TextureAsset>)>,
    
    
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
        self.num_quads_drawn = 0;
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.begin_frame();
    }

    pub fn end_frame(&mut self) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.present_frame();
    }


    pub fn flush(&mut self) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        let framedata = graphics_sys.frame_data_mut();
        if framedata.is_none() {
            warn!("No frame data present, call begin_frame first!");
            return;
        };
        let framedata = framedata.unwrap();

        let mut rpass = framedata.2.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &framedata.1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
        rpass.set_bind_group(0, &self.bind_group.0, &[]);
        rpass.draw_indexed(0..(self.num_quads_drawn * 6), 0, 0..1);
    }


    /// Writes the current batch into the vertex buffer and clears the current batch
    pub fn end_batch(&mut self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.queue().write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(self.current_batch.as_slice()));
        self.current_batch.clear();
    }


    /// Queues a write into the VertexBuffer. This has low overhead because the write only happens in GraphicsSystem::present_frame
    /// when wgpu::Queue::submit() gets called
    pub fn draw_quad(&mut self, pos: glam::Vec2, size: glam::Vec2, color: QuadColoring) {
        // If current batch is full:
        //    - Write vertex buffer (+ Submit it)
        //    - Clear self.current_batch
        // TODO: Test if this works
        if self.num_quads_drawn + 1 >= Renderer::MAX_QUAD_COUNT as u32
        || self.texture_slots.keys().len() + 1 >= Renderer::MAX_TEXTURE_COUNT {
            self.end_batch();
            self.flush();
            debug!("Vertex buffer or texture count would overrun, starting new batch...");
        };
        
        let (tex_index, color) = match color {
            QuadColoring::Color(col) => (0, [col.r as f32, col.g as f32, col.b as f32, col.a as f32]),
            QuadColoring::Texture(tex_handle) => {
                // If we want to draw a texture, use white color (later maybe different color to tint)
                let color = [1.0, 1.0, 1.0, 1.0];
                
                let id = **tex_handle.id();
                let slot_tex = self.texture_slots.get(&id);
                if let Some((tex_index, _)) = slot_tex {
                    (*tex_index, color)
                } else {
                    // Add 1 because of the white pixel which is always tex_index=0
                    let idx = self.texture_slots.len() as u64 + 1;
                    self.texture_slots.insert(id, (idx, tex_handle.clone()));

                    // Rebuild the bind group to include the new texture
                    let mut views = vec![
                        &self.white_pixel.1
                    ];
                    let mut samplers = vec![
                        &self.white_pixel.2
                    ];
                    let asset_sys = self.ctx.get::<AssetSystem>();
                    self.texture_slots.iter().for_each(|(_, (_, asset_handle))| {
                        let tex = asset_sys.get(asset_handle);
                        views.push(tex.view());
                        samplers.push(tex.sampler());
                    });
                    let graphics_sys = self.ctx.get::<GraphicsSystem>();
                    self.bind_group = Renderer::create_bind_group(graphics_sys.device(), &views, &samplers);
                    drop(asset_sys);
                    drop(graphics_sys);
                    self.reload_render_pipeline();
                    (idx, color)
                }
            }
        };
        let w = size.x;
        let h = size.y;
        self.current_batch.reserve(4);
        self.current_batch.push(Vertex::new([pos.x - w, pos.y - h], color, [0.0, 1.0], tex_index));
        self.current_batch.push(Vertex::new([pos.x - w, pos.y + h], color, [0.0, 0.0], tex_index));
        self.current_batch.push(Vertex::new([pos.x + w, pos.y + h], color, [1.0, 0.0], tex_index));
        self.current_batch.push(Vertex::new([pos.x + w, pos.y - h], color, [1.0, 1.0], tex_index));

        self.num_quads_drawn += 1;
    }



    pub fn request_redraw(&self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.request_redraw();
    }


    pub(crate) fn resize(&mut self, new_size: &PhysicalSize<u32>) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.resize_surface(new_size);
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


    /// Helper function for initialization
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32x2, 3 => Sint32],
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


    /// Creates the bind group and bind group layout based on a list of textures
    /// Always add the white pixel first
    fn create_bind_group(device: &wgpu::Device, views: &Vec<&TextureView>, samplers: &Vec<&Sampler>) -> (BindGroup, BindGroupLayout) {
        let tex_views = views.as_slice();
        let tex_samplers = samplers.as_slice();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    count: NonZeroU32::new(views.len() as u32),
                },
                // Sampler array
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: NonZeroU32::new(views.len() as u32),
                }
            ],
        });


        // TODO: Use a cameras matrices
        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shader globals buffer"),
            contents: bytemuck::cast_slice(&[
                glam::Mat4::default()
            ]),
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
            layout: &bind_group_layout,
            label: Some("bind group"),
        });

        (bind_group, bind_group_layout)
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
        .with::<Mut<AssetSystem>>();

    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_assetchange);


    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut asset_sys = ctx.get_mut::<AssetSystem>();
        let base_shader_handle = asset_sys.load::<ShaderAsset>("shaders/base.wgsl", true);
        // Drop the mutable reference, from now on we only need it immutably
        drop(asset_sys);

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: Renderer::MAX_VERTEX_COUNT as u64 * VERTEX_SIZE as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false
        });
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


        let asset_sys = ctx.get::<AssetSystem>();
        let (bind_group, bind_group_layout) = Renderer::create_bind_group(device, &vec![&white_pixel_view], &vec![&white_pixel_sampler]);

        let base_shader_module = asset_sys.get(&base_shader_handle);
        let render_pipeline = Self::create_render_pipeline(
            device,
            &bind_group_layout,
            base_shader_module.module(),
            Some(graphics_sys.surface_config().format.into())
        );

        drop(graphics_sys);
        drop(asset_sys);

        Self {
            ctx,

            current_batch: vec![],
            num_quads_drawn: 0,
            vertex_buffer,
            index_buffer,
            index_format: wgpu::IndexFormat::Uint16,
            texture_slots: HashMap::default(),
            
            bind_group: (bind_group, bind_group_layout),
            shader_handle: base_shader_handle,
            render_pipeline,
            clear_color: Color::BLACK,
            extents,

            white_pixel: (white_pixel, white_pixel_view, white_pixel_sampler)
        }
    }
}

