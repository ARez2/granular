#![allow(unused)]
use glam::{Affine2, IVec2, Mat2, Mat4, Quat, Vec2, Vec3};
use wgpu::{Buffer, BufferUsages, util::DeviceExt};

use super::GraphicsSystem;
use crate::utils::*;

#[derive(Debug, Clone, Copy)]
pub enum ScalingMode {
    Keep,
    Stretch,
}

pub struct Camera {
    ctx: GeeseContextHandle<Self>,

    // === General ===
    /// This is the center of the camera
    position: IVec2,
    angle: f32,
    screen_size: IVec2,
    scaling_mode: ScalingMode,
    zoom: f32,

    // ortho_proj * view
    canvas_transform: Mat4,

    // === Internal projection ===
    ortho_proj: Mat4,
    view: Mat4,
    near: f32,
    far: f32,

    // === wgpu ===
    shader_buffer: Buffer,
}
impl Camera {
    /// Sets the position (center of the camera)
    pub fn set_position(&mut self, position: IVec2) {
        self.position = position;
        self.recalc_view();
    }

    /// Gets the position (center of the camera)
    pub fn position(&self) -> IVec2 {
        self.position
    }

    /// Translates the cameras position by offset.
    pub fn translate(&mut self, offset: IVec2) {
        self.set_position(self.position + offset);
    }

    /// Positions the camera so that the bottom left corner of the image is `bottomleft`. This ignores rotation
    pub fn set_bottomleft_position(&mut self, bottomleft: IVec2) {
        self.set_position(bottomleft + self.screen_size / 2);
    }

    /// Sets the rotation of the camera (in radians)
    pub fn set_rotation(&mut self, rotation: f32) {
        self.angle = rotation;
        self.recalc_view();
    }

    /// Gets the rotation
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
        self.screen_size = IVec2::new(screen_size.0 as i32, screen_size.1 as i32);
        info!("Camera screen size: {}", self.screen_size);

        self.recalc_ortho();
        self.recalc_view();
    }

    /// Gets the canvas transform
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
        self.ortho_proj = Self::_recalc_ortho(
            self.scaling_mode,
            self.screen_size,
            self.zoom,
            self.near,
            self.far,
        );
        self.canvas_transform = self.ortho_proj * self.view;
    }

    #[allow(clippy::too_many_arguments)]
    fn _recalc_ortho(
        scaling_mode: ScalingMode,
        screen_size: IVec2,
        zoom: f32,
        near: f32,
        far: f32,
    ) -> Mat4 {
        let aspect_ratio = match scaling_mode {
            ScalingMode::Keep => 1.0,
            ScalingMode::Stretch => screen_size.y as f32 / screen_size.x as f32,
        };
        let half_width = screen_size.x as f32 / (2.0 * zoom);
        let half_height = screen_size.y as f32 / (2.0 * zoom);

        let left = -half_width;
        let right = half_width;
        let bottom = -half_height;
        let top = half_height;
        glam::camera::rh::proj::opengl::orthographic(left, right, bottom, top, near, far)
    }

    fn recalc_view(&mut self) {
        self.view = Self::_recalc_view(self.position, self.angle);
        self.canvas_transform = self.ortho_proj * self.view;
    }

    fn _recalc_view(position: IVec2, angle: f32) -> Mat4 {
        Mat4::from_rotation_translation(
            Quat::from_rotation_z(angle),
            Vec3::new(-position.x as f32, -position.y as f32, 0.0),
        )
    }
}
impl GeeseSystem for Camera {
    const DEPENDENCIES: geese::Dependencies = dependencies().with::<GraphicsSystem>();

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let scaling_mode = ScalingMode::Keep;
        let position = IVec2::ZERO;
        let angle = 0.0;
        let zoom = 1.0;

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let screen_size = IVec2::new(
            graphics_sys.surface_config().width as i32,
            graphics_sys.surface_config().height as i32,
        );
        let left = (position.x - screen_size.x) as f32 / (2.0 * zoom);
        let right = (position.x + screen_size.x) as f32 / (2.0 * zoom);
        let bottom = (position.y - screen_size.y) as f32 / (2.0 * zoom);
        let top = (position.y + screen_size.y) as f32 / (2.0 * zoom);
        let near = -1.0;
        let far = 1.0;
        let ortho_proj = Self::_recalc_ortho(scaling_mode, screen_size, zoom, near, far);
        let view = Self::_recalc_view(position, angle);
        let canvas_transform = ortho_proj * view;

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

            position,
            angle,
            screen_size,
            scaling_mode,
            zoom,

            canvas_transform,

            view,
            ortho_proj,
            near,
            far,

            shader_buffer,
        }
    }
}
