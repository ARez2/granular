#![allow(unused)]
use glam::{Affine2, IVec2, Mat2, Mat4, Quat, Vec2, Vec3};
use wgpu::{util::DeviceExt, Buffer, BufferUsages};

use super::GraphicsSystem;
use crate::utils::*;

pub enum ScalingMode {
    Keep,
    Stretch,
}

pub struct Camera {
    ctx: GeeseContextHandle<Self>,

    // === General ===
    position: IVec2,
    angle: f32,
    screen_size: Vec2,
    scaling_mode: ScalingMode,
    zoom: f32,

    // ortho_proj * view
    canvas_transform: Mat4,

    // === Internal projection ===
    scale: Vec2,
    ortho_proj: Mat4,
    view: Mat4,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
    near: f32,
    far: f32,

    // === wgpu ===
    shader_buffer: Buffer,
}
impl Camera {
    pub fn set_position(&mut self, position: IVec2) {
        self.position = position;
        self.recalc_view();
    }
    pub fn position(&self) -> IVec2 {
        self.position
    }

    pub fn translate(&mut self, offset: IVec2) {
        self.set_position(self.position + offset);
    }

    /// Sets the rotation of the camera (in radians)
    pub fn set_rotation(&mut self, rotation: f32) {
        self.angle = rotation;
        self.recalc_view();
    }
    pub fn rotation(&self) -> f32 {
        self.angle
    }

    /// A zoom of 1.0 is default, a zoom of 2.0 doubles every pixel
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;
        self.recalc_view();
    }
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub(crate) fn set_screen_size(&mut self, screen_size: (u32, u32)) {
        self.screen_size = Vec2::new(screen_size.0 as f32, screen_size.1 as f32);
        info!("Camera screen size: {}", self.screen_size);

        self.scale = 1.0 / self.screen_size;

        self.recalc_ortho();
        self.recalc_view();
    }

    pub fn canvas_transform(&self) -> Mat4 {
        self.canvas_transform
    }

    pub fn write_canvas_transform_buffer(&self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.queue().write_buffer(
            &self.shader_buffer,
            0,
            bytemuck::cast_slice(&[self.canvas_transform]),
        );
    }

    pub fn canvas_transform_buffer(&self) -> &Buffer {
        &self.shader_buffer
    }

    fn recalc_ortho(&mut self) {
        let aspect_ratio = match self.scaling_mode {
            ScalingMode::Keep => 1.0,
            ScalingMode::Stretch => self.screen_size.y / self.screen_size.x,
        };
        self.ortho_proj = Mat4::orthographic_rh_gl(
            self.left * aspect_ratio,  // left
            self.right * aspect_ratio, // right
            self.bottom,               // bottom
            self.top,                  // top
            self.near,                 // near
            self.far,                  // far
        );
        self.canvas_transform = self.ortho_proj * self.view;
    }

    fn recalc_view(&mut self) {
        self.view = Mat4::from_scale_rotation_translation(
            Vec3::new(self.scale.x * self.zoom, self.scale.y * self.zoom, 1.0),
            Quat::from_rotation_z(self.angle),
            Vec3::new(
                -self.position.x as f32 * self.scale.x,
                -self.position.y as f32 * self.scale.y,
                0.0,
            ),
        );
        self.canvas_transform = self.ortho_proj * self.view;
    }
}
impl GeeseSystem for Camera {
    const DEPENDENCIES: geese::Dependencies = dependencies().with::<GraphicsSystem>();

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let scale = Vec2::ONE;
        let (left, right, top, bottom, near, far) = (-1.0, 1.0, 1.0, -1.0, -1.0, 1.0);
        let ortho_proj = Mat4::orthographic_rh_gl(left, right, bottom, top, near, far);
        let view = Mat4::IDENTITY;
        let canvas_transform = ortho_proj * view;

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let shader_buffer =
            graphics_sys
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("SimulationRenderer Shader globals buffer"),
                    contents: bytemuck::cast_slice(&[canvas_transform]),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                });
        drop(graphics_sys);

        Self {
            ctx,

            position: IVec2::ZERO,
            angle: 0.0,
            screen_size: Vec2::ONE,
            scaling_mode: ScalingMode::Keep,
            zoom: 1.0,

            canvas_transform,

            scale,
            view,
            ortho_proj,
            left,
            right,
            top,
            bottom,
            near,
            far,

            shader_buffer,
        }
    }
}
