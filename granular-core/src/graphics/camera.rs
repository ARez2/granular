#![allow(unused)]
use glam::{Affine2, IVec2, Mat2, Mat4, Quat, Vec2, Vec3};
use wgpu::{Buffer, BufferUsages, util::DeviceExt};
use winit::dpi::{LogicalSize, PhysicalSize};

use super::GraphicsSystem;
use crate::{Rect, utils::*};

#[derive(Debug, Clone, Copy)]
pub enum ScalingMode {
    /// Preserve aspect ratio, adding letterboxes verticall or horizontally as needed.
    KeepAspect,
    /// Fill the entire window, ignoring aspect ratio.
    Stretch,
    /// Preserve aspect ratio and use an integer scale.
    Integer,
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
    viewport: Rect,

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
        self.recalc_viewport_rect();
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

    pub fn set_scaling_mode(&mut self, scaling_mode: ScalingMode) {
        self.scaling_mode = scaling_mode;
        self.recalc_viewport_rect();
    }

    fn recalc_ortho(&mut self) {
        self.ortho_proj = Self::_recalc_ortho(self.screen_size, self.zoom, self.near, self.far);
        self.canvas_transform = self.ortho_proj * self.view;
    }

    #[allow(clippy::too_many_arguments)]
    fn _recalc_ortho(screen_size: IVec2, zoom: f32, near: f32, far: f32) -> Mat4 {
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

    pub fn get_viewport_rect(&self) -> Rect {
        self.viewport
    }

    fn recalc_viewport_rect(&mut self) {
        let game_size = self.ctx.get::<GraphicsSystem>().get_game_resolution();
        self.viewport = Self::_calc_viewport_rect(
            self.scaling_mode,
            PhysicalSize::from(self.screen_size.to_array()),
            game_size,
        )
    }

    fn _calc_viewport_rect(
        scaling_mode: ScalingMode,
        screen_size: PhysicalSize<u32>,
        game_size: LogicalSize<u32>,
    ) -> Rect {
        let logical_w = game_size.width as f32;
        let logical_h = game_size.height as f32;

        let screen_w = screen_size.width as f32;
        let screen_h = screen_size.height as f32;

        let scale_x = screen_w / logical_w;
        let scale_y = screen_h / logical_h;

        let scale = match scaling_mode {
            // Special case: handled below.
            ScalingMode::Stretch => 1.0,
            ScalingMode::KeepAspect => scale_x.min(scale_y),
            ScalingMode::Integer => scale_x.min(scale_y).floor().max(1.0),
        };

        if matches!(scaling_mode, ScalingMode::Stretch) {
            return Rect {
                position: IVec2::ZERO,
                size: IVec2::new(screen_size.width as i32, screen_size.height as i32),
            };
        }

        let width = (logical_w * scale).round() as u32;
        let height = (logical_h * scale).round() as u32;

        Rect {
            position: IVec2::new(
                (screen_size.width.saturating_sub(width)) as i32 / 2,
                (screen_size.height.saturating_sub(height)) as i32 / 2,
            ),
            size: IVec2::new(width as i32, height as i32),
        }
    }
}
impl GeeseSystem for Camera {
    const DEPENDENCIES: geese::Dependencies = dependencies().with::<GraphicsSystem>();

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let scaling_mode = ScalingMode::KeepAspect;
        let position = IVec2::ZERO;
        let angle = 0.0;
        let zoom = 1.0;

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let screen_size = IVec2::new(
            graphics_sys.surface_config().width as i32,
            graphics_sys.surface_config().height as i32,
        );
        let viewport = Self::_calc_viewport_rect(
            scaling_mode,
            PhysicalSize::from(screen_size.to_array()),
            graphics_sys.get_game_resolution(),
        );
        let left = (position.x - screen_size.x) as f32 / (2.0 * zoom);
        let right = (position.x + screen_size.x) as f32 / (2.0 * zoom);
        let bottom = (position.y - screen_size.y) as f32 / (2.0 * zoom);
        let top = (position.y + screen_size.y) as f32 / (2.0 * zoom);
        let near = -1.0;
        let far = 1.0;
        let ortho_proj = Self::_recalc_ortho(screen_size, zoom, near, far);
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
            viewport,

            canvas_transform,
            view,
            ortho_proj,
            near,
            far,

            shader_buffer,
        }
    }
}
