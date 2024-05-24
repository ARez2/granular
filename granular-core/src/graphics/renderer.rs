use geese::{dependencies, GeeseContextHandle, GeeseSystem, Mut};
use glam::Mat4;
use bytemuck_derive::{Zeroable, Pod};
use wgpu::{util::DeviceExt, Buffer, BufferUsages};
use winit::dpi::PhysicalSize;

use crate::{BatchRenderer, Camera};

use super::{GraphicsSystem, SimulationRenderer};

pub struct Renderer {
    ctx: GeeseContextHandle<Self>,
}
impl Renderer {
    pub fn start_frame(&mut self) {
        let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
        graphics_sys.begin_frame();
    }


    pub fn end_frame(&mut self) {
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            graphics_sys.present_frame();
        }
        {
            let mut batch_renderer = self.ctx.get_mut::<BatchRenderer>();
            batch_renderer.end_frame();
        }
    }


    /// Resizes the surface with the new_size
    pub(crate) fn resize(&mut self, new_size: PhysicalSize<u32>) {
        {
            let mut graphics_sys = self.ctx.get_mut::<GraphicsSystem>();
            graphics_sys.resize_surface(new_size);
        }
        {
            let mut camera = self.ctx.get_mut::<Camera>();
            camera.set_screen_size((new_size.width, new_size.height));
        }
        {
            let mut sim_renderer = self.ctx.get_mut::<SimulationRenderer>();
            sim_renderer.resize(new_size);
        }
    }


    /// Requests a redraw from the underlying GraphicsSystem
    pub fn request_redraw(&self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.request_redraw();
    }


    pub fn render(&mut self) {
        {
            let camera = self.ctx.get::<Camera>();
            camera.write_canvas_transform_buffer();
        }

        let mut batch_renderer = self.ctx.get_mut::<BatchRenderer>();
        batch_renderer.create_batches();
        batch_renderer.prepare_to_render();
        batch_renderer.render_batch_layers(i32::MIN..0, true);
        drop(batch_renderer);
        {
            let mut sim_renderer = self.ctx.get_mut::<SimulationRenderer>();
            sim_renderer.render();
        }
        let mut batch_renderer = self.ctx.get_mut::<BatchRenderer>();
        batch_renderer.render_batch_layers(0..i32::MAX, false);
    }
}
impl GeeseSystem for Renderer {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<GraphicsSystem>>()
        .with::<Mut<BatchRenderer>>()
        .with::<Mut<SimulationRenderer>>()
        .with::<Mut<Camera>>();

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let camera = ctx.get::<Camera>();
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let shaderglobals_buffer = graphics_sys.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shader globals buffer"),
            contents: bytemuck::cast_slice(&[camera.canvas_transform()]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST
        });

        drop(camera);
        drop(graphics_sys);
        
        Self {
            ctx
        }
    }
}