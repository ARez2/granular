use std::borrow::Cow;

use encase::{ShaderType, UniformBuffer};
use glam::{IVec4, Vec2, Vec4};
use wgpu::{BindGroup, BindGroupLayout, Device, RenderPipeline, ShaderModule};

use crate::{
    AssetSystem, Camera, TimeSystem,
    assets::AssetHandle,
    graphics::{BindGroupBuilder, GraphicsSystem},
    utils::*,
};

#[derive(Debug, Clone, Copy, ShaderType)]
struct DisplayParams {
    viewport_rect: Vec4,
    surface_size: Vec2,
}

#[derive(Debug, Clone, Copy, ShaderType)]
struct BackgroundParams {
    surface_size: Vec2,
    time: f32,
    _pad: f32,
}

pub(crate) struct GameRenderer {
    ctx: GeeseContextHandle<Self>,

    display_shader_handle: AssetHandle<ShaderModule>,
    display_pipeline: RenderPipeline,
    params_bind_group: (BindGroup, BindGroupLayout),
    game_tex_bind_group: (BindGroup, BindGroupLayout),
    display_params: DisplayParams,
    display_params_bytes: [u8; size_of::<DisplayParams>()],
    display_params_buffer: wgpu::Buffer,

    bg_shader_handle: AssetHandle<ShaderModule>,
    bg_pipeline: RenderPipeline,
    bg_params_bind_group: (BindGroup, BindGroupLayout),
    bg_params: BackgroundParams,
    bg_params_bytes: [u8; size_of::<BackgroundParams>()],
    bg_params_buffer: wgpu::Buffer,
}
impl GameRenderer {
    fn on_game_render_fully_done(&mut self, _: &crate::graphics::events::DisplayGameRender) {
        let time = {
            self.ctx
                .get::<TimeSystem>()
                .time_since_start()
                .as_secs_f32()
        };
        let viewport_rect = {
            let camera = self.ctx.get::<Camera>();
            camera.get_viewport_rect()
        };
        let viewport_rect = IVec4::new(
            viewport_rect.position.x,
            viewport_rect.position.y,
            viewport_rect.size.x,
            viewport_rect.size.y,
        )
        .as_vec4();
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        let surface_size = graphics_sys.get_surface_resolution();
        let context = graphics_sys.render_context();

        {
            self.bg_params = BackgroundParams {
                surface_size: Vec2::new(surface_size.width as f32, surface_size.height as f32),
                time,
                _pad: 0.0,
            };
            // Write back any changes to params into the buffer
            let mut writer = UniformBuffer::new(&mut self.bg_params_bytes);
            let _ = writer.write(&self.bg_params);
            context
                .queue
                .write_buffer(&self.bg_params_buffer, 0, &self.bg_params_bytes);

            #[cfg(feature = "trace")]
            let prof = context.profiler.lock().unwrap();
            #[cfg(feature = "trace")]
            let mut profiler_scope =
                prof.scope("GameRenderer background render", &mut context.encoder);

            let rpass_desc = wgpu::RenderPassDescriptor {
                label: Some("GameRenderer background pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &context.view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                    resolve_target: None,
                })],
                ..Default::default()
            };

            #[cfg(feature = "trace")]
            let mut rpass = profiler_scope
                .scoped_render_pass("GameRenderer background render pass", rpass_desc);
            #[cfg(not(feature = "trace"))]
            let mut rpass = context.encoder.begin_render_pass(&rpass_desc);
            rpass.set_pipeline(&self.bg_pipeline);
            rpass.set_bind_group(0, &self.bg_params_bind_group.0, &[]);
            rpass.draw(0..6, 0..1);
        }
        {
            self.display_params = DisplayParams {
                viewport_rect,
                surface_size: Vec2::new(surface_size.width as f32, surface_size.height as f32),
            };
            // Write back any changes to params into the buffer
            let mut writer = UniformBuffer::new(&mut self.display_params_bytes);
            let _ = writer.write(&self.display_params);
            context
                .queue
                .write_buffer(&self.display_params_buffer, 0, &self.display_params_bytes);

            #[cfg(feature = "trace")]
            let prof = context.profiler.lock().unwrap();
            #[cfg(feature = "trace")]
            let mut profiler_scope = prof.scope("GameRenderer render", &mut context.encoder);

            let rpass_desc = wgpu::RenderPassDescriptor {
                label: Some("GameRenderer display pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &context.view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                    resolve_target: None,
                })],
                ..Default::default()
            };

            #[cfg(feature = "trace")]
            let mut rpass =
                profiler_scope.scoped_render_pass("GameRenderer display render pass", rpass_desc);
            #[cfg(not(feature = "trace"))]
            let mut rpass = context.encoder.begin_render_pass(&rpass_desc);
            rpass.set_pipeline(&self.display_pipeline);
            rpass.set_bind_group(0, &self.params_bind_group.0, &[]);
            rpass.set_bind_group(1, &self.game_tex_bind_group.0, &[]);
            rpass.draw(0..6, 0..1);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_assetchange(&mut self, event: &crate::assets::events::AssetChanged) {
        let asset_sys = self.ctx.get::<AssetSystem>();
        if event.asset_id == **self.bg_shader_handle.id() {
            let graphics_sys = self.ctx.get::<GraphicsSystem>();
            self.bg_pipeline = Self::create_render_pipeline(
                graphics_sys.device(),
                "GameRenderer background render pipeline",
                &[Some(&self.bg_params_bind_group.1)],
                asset_sys.get(&self.bg_shader_handle).unwrap(),
                graphics_sys.get_surface_view_format(),
            );
            debug!("reload background shader");
        } else if event.asset_id == **self.display_shader_handle.id() {
            let graphics_sys = self.ctx.get::<GraphicsSystem>();
            self.display_pipeline = Self::create_render_pipeline(
                graphics_sys.device(),
                "GameRenderer display render pipeline",
                &[
                    Some(&self.params_bind_group.1),
                    Some(&self.game_tex_bind_group.1),
                ],
                asset_sys.get(&self.display_shader_handle).unwrap(),
                graphics_sys.get_surface_view_format(),
            );
        }
    }

    /// Helper function for creating a new render pipeline
    fn create_render_pipeline(
        device: &Device,
        name: &str,
        bind_group_layouts: &[Option<&BindGroupLayout>],
        shader: &ShaderModule,
        surface_format: wgpu::TextureFormat,
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{} layout", name)),
            bind_group_layouts,
            immediate_size: 0,
        });
        let color_state = Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(name),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
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
}
impl GeeseSystem for GameRenderer {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<Camera>()
        .with::<TimeSystem>()
        .with::<Mut<AssetSystem>>()
        .with::<Mut<GraphicsSystem>>();

    #[cfg(target_arch = "wasm32")]
    const EVENT_HANDLERS: EventHandlers<Self> =
        event_handlers().with(Self::on_game_render_fully_done);
    #[cfg(not(target_arch = "wasm32"))]
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::on_game_render_fully_done)
        .with(Self::on_assetchange);

    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        let display_shader_handle = ctx
            .get_mut::<AssetSystem>()
            .load(asset_source!("../shaders/game_display.wgsl"), ())
            .unwrap();
        let bg_shader_handle = ctx
            .get_mut::<AssetSystem>()
            .load(asset_source!("../shaders/fullscreen_quad.wgsl"), ())
            .unwrap();

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();
        let queue = graphics_sys.queue();

        let viewport_rect = { ctx.get::<Camera>().get_viewport_rect() };
        let viewport_rect = IVec4::new(
            viewport_rect.position.x,
            viewport_rect.position.y,
            viewport_rect.size.x,
            viewport_rect.size.y,
        )
        .as_vec4();
        let surface_size = graphics_sys.get_surface_resolution();

        let display_params = DisplayParams {
            viewport_rect,
            surface_size: Vec2::new(surface_size.width as f32, surface_size.height as f32),
        };
        let mut display_params_bytes = [0u8; size_of::<DisplayParams>()];
        let display_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GameRenderer display Params buffer"),
            size: display_params_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut writer = UniformBuffer::new(&mut display_params_bytes);
        let _ = writer.write(&display_params);
        queue.write_buffer(&display_params_buffer, 0, &display_params_bytes);

        let (params_bgl, params_bg) = BindGroupBuilder::new()
            .add_binding_with_resource(
                0,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                display_params_buffer.as_entire_binding(),
            )
            .build("GameRenderer params buffer", device);
        let params_bind_group = (params_bg, params_bgl);

        let (tex_bgl, tex_bg) = {
            let asset_sys = ctx.get::<AssetSystem>();
            let game_tex = asset_sys
                .get(&graphics_sys.get_game_render_target())
                .unwrap();
            BindGroupBuilder::new()
                .add_binding_with_resource(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    wgpu::BindingResource::TextureView(game_tex.view()),
                )
                .add_binding_with_resource(
                    1,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    wgpu::BindingResource::Sampler(game_tex.sampler()),
                )
                .build("GameRenderer game texture bind group", device)
        };
        let game_tex_bind_group = (tex_bg, tex_bgl);

        let display_pipeline = Self::create_render_pipeline(
            device,
            "GameRenderer display render pipeline",
            &[Some(&params_bind_group.1), Some(&game_tex_bind_group.1)],
            ctx.get::<AssetSystem>()
                .get(&display_shader_handle)
                .unwrap(),
            graphics_sys.get_surface_view_format(),
        );

        let bg_params = BackgroundParams {
            surface_size: Vec2::new(surface_size.width as f32, surface_size.height as f32),
            time: 0.0,
            _pad: 0.0,
        };
        let mut bg_params_bytes = [0u8; size_of::<BackgroundParams>()];
        let bg_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GameRenderer background Params buffer"),
            size: bg_params_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut writer = UniformBuffer::new(&mut bg_params_bytes);
        let _ = writer.write(&bg_params);
        queue.write_buffer(&bg_params_buffer, 0, &bg_params_bytes);
        let (bg_params_bgl, bg_params_bg) = BindGroupBuilder::new()
            .add_binding_with_resource(
                0,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                bg_params_buffer.as_entire_binding(),
            )
            .build("GameRenderer background params buffer", device);
        let bg_params_bind_group = (bg_params_bg, bg_params_bgl);
        let bg_pipeline = Self::create_render_pipeline(
            device,
            "GameRenderer background render pipeline",
            &[Some(&bg_params_bind_group.1)],
            ctx.get::<AssetSystem>().get(&bg_shader_handle).unwrap(),
            graphics_sys.get_surface_view_format(),
        );

        drop(graphics_sys);

        Self {
            ctx,

            display_shader_handle,
            display_pipeline,
            params_bind_group,
            game_tex_bind_group,
            display_params,
            display_params_bytes,
            display_params_buffer,

            bg_shader_handle,
            bg_pipeline,
            bg_params_bind_group,
            bg_params,
            bg_params_bytes,
            bg_params_buffer,
        }
    }
}
