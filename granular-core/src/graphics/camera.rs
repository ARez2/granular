#![allow(unused)]
use glam::{Affine2, IVec2, Mat2, Mat4, Quat, Vec2, Vec3, Vec4, Vec4Swizzles};
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
    zoom: f32,
    scaling_mode: ScalingMode,
    /// This describes where the game texture will be blitted in the final image (by the GameRenderer)
    viewport: Rect,

    screen_size: IVec2,
    /// Game coordinates => game NDC
    game_canvas_transform: Mat4,
    /// Screen coordinates => surface NDC
    /// Does NOT include camera position/rotation/zoom.
    screen_canvas_transform: Mat4,

    // === Internal projection ===
    game_ortho_proj: Mat4,
    screen_ortho_proj: Mat4,
    view: Mat4,
    near: f32,
    far: f32,

    // === wgpu ===
    game_shader_buffer: Buffer,
    screen_shader_buffer: Buffer,
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

    /// Positions the camera so that the bottom-left corner of the visible
    /// game area is `bottomleft`. This ignores rotation.
    pub fn set_bottomleft_position(&mut self, bottomleft: IVec2) {
        let game_resolution = self.ctx.get::<GraphicsSystem>().get_game_resolution();
        let visible_size =
            Vec2::new(game_resolution.width as f32, game_resolution.height as f32) / self.zoom;
        let center_offset = (visible_size * 0.5).round().as_ivec2();

        self.set_position(bottomleft + center_offset);
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
        self.recalc_game_ortho();
    }
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn set_scaling_mode(&mut self, scaling_mode: ScalingMode) {
        self.scaling_mode = scaling_mode;
        self.recalc_viewport_rect();
    }

    pub(crate) fn set_screen_size(&mut self, screen_size: (u32, u32)) {
        self.screen_size = IVec2::new(screen_size.0 as i32, screen_size.1 as i32);
        info!("Camera screen size: {}", self.screen_size);

        self.recalc_screen_ortho();
        self.recalc_viewport_rect();
    }

    /// Returns the Buffer which contains `game_canvas_transform`
    pub fn game_canvas_transform_buffer(&self) -> &Buffer {
        &self.game_shader_buffer
    }

    /// Returns the Buffer which contains `screen_canvas_transform`
    pub fn screen_canvas_transform_buffer(&self) -> &Buffer {
        &self.screen_shader_buffer
    }

    pub(crate) fn write_canvas_transform_buffers(&self) {
        let graphics_sys = self.ctx.get::<GraphicsSystem>();
        graphics_sys.queue().write_buffer(
            &self.game_shader_buffer,
            0,
            bytemuck::cast_slice(&[self.game_canvas_transform]),
        );
        graphics_sys.queue().write_buffer(
            &self.screen_shader_buffer,
            0,
            bytemuck::cast_slice(&[self.screen_canvas_transform]),
        );
    }

    fn write_game_canvas_transform_buffer(&self) {
        self.ctx.get::<GraphicsSystem>().queue().write_buffer(
            &self.game_shader_buffer,
            0,
            bytemuck::cast_slice(&[self.game_canvas_transform]),
        );
    }

    fn write_screen_canvas_transform_buffer(&self) {
        self.ctx.get::<GraphicsSystem>().queue().write_buffer(
            &self.screen_shader_buffer,
            0,
            bytemuck::cast_slice(&[self.screen_canvas_transform]),
        );
    }

    fn recalc_game_ortho(&mut self) {
        let game_resolution = self.ctx.get::<GraphicsSystem>().get_game_resolution();
        let game_size = IVec2::new(game_resolution.width as i32, game_resolution.height as i32);
        self.game_ortho_proj = Self::_recalc_ortho(game_size, self.zoom, self.near, self.far);
        self.game_canvas_transform = self.game_ortho_proj * self.view;
    }

    fn recalc_screen_ortho(&mut self) {
        self.screen_ortho_proj = Self::_recalc_ortho(self.screen_size, 1.0, self.near, self.far);
        self.screen_canvas_transform = self.screen_ortho_proj;
    }

    #[allow(clippy::too_many_arguments)]
    fn _recalc_ortho(screen_size: IVec2, zoom: f32, near: f32, far: f32) -> Mat4 {
        let half_width = screen_size.x as f32 / (2.0 * zoom);
        let half_height = screen_size.y as f32 / (2.0 * zoom);

        let left = -half_width;
        let right = half_width;
        let bottom = -half_height;
        let top = half_height;
        glam::camera::rh::proj::directx::orthographic(left, right, bottom, top, near, far)
    }

    fn recalc_view(&mut self) {
        self.view = Self::_recalc_view(self.position, self.angle);
        self.game_canvas_transform = self.game_ortho_proj * self.view;
    }

    fn _recalc_view(position: IVec2, angle: f32) -> Mat4 {
        Mat4::from_rotation_translation(
            Quat::from_rotation_z(angle),
            Vec3::new(position.x as f32, position.y as f32, 0.0),
        )
        .inverse()
    }

    pub(crate) fn get_viewport_rect(&self) -> Rect {
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

    /// Converts a position from surface space (the window/ final resolution) into world space (where the game entities live)
    pub fn surface_to_world(&self, surface_pos: Vec2) -> Option<Vec2> {
        let ndc = self.surface_to_game_ndc(surface_pos)?;
        let world = self.game_canvas_transform.inverse() * Vec4::new(ndc.x, ndc.y, 0.0, 1.0);
        Some(world.xy())
    }

    /// Converts a position from world space (where the game entities live) into surface space (the window/final resolution).
    pub fn world_to_surface(&self, world_pos: Vec2) -> Vec2 {
        let clip = self.game_canvas_transform * Vec4::new(world_pos.x, world_pos.y, 0.0, 1.0);

        // Technically unnecessary for an orthographic projection, but makes this robust if the projection ever changes.
        let ndc = clip.xy() / clip.w;

        self.game_ndc_to_surface(ndc)
    }

    /// Converts a position from surface space to the game resolution but stays in screen-space
    pub fn surface_to_game_screen(&self, surface_pos: Vec2) -> Option<Vec2> {
        let ndc = self.surface_to_game_ndc(surface_pos)?;
        let game_resolution = self.ctx.get::<GraphicsSystem>().get_game_resolution();
        let half_size =
            Vec2::new(game_resolution.width as f32, game_resolution.height as f32) * 0.5;
        Some(ndc * half_size)
    }

    /// Converts a position from game-screen space (centered, +Y up, game-resolution units) into surface space.
    pub fn game_screen_to_surface(&self, game_pos: Vec2) -> Vec2 {
        let game_resolution = self.ctx.get::<GraphicsSystem>().get_game_resolution();

        let half_size =
            Vec2::new(game_resolution.width as f32, game_resolution.height as f32) * 0.5;

        let ndc = game_pos / half_size;

        self.game_ndc_to_surface(ndc)
    }

    fn surface_to_game_ndc(&self, surface_pos: Vec2) -> Option<Vec2> {
        let viewport_pos = self.viewport.position.as_vec2();
        let viewport_size = self.viewport.size.as_vec2();

        let local = surface_pos - viewport_pos;
        if local.x < 0.0
            || local.y < 0.0
            || local.x >= viewport_size.x
            || local.y >= viewport_size.y
        {
            return None;
        }

        let uv = local / viewport_size;
        Some(Vec2::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0))
    }

    fn game_ndc_to_surface(&self, ndc: Vec2) -> Vec2 {
        let viewport_pos = self.viewport.position.as_vec2();
        let viewport_size = self.viewport.size.as_vec2();

        let uv = Vec2::new((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5);

        viewport_pos + uv * viewport_size
    }
}
impl GeeseSystem for Camera {
    const DEPENDENCIES: geese::Dependencies = dependencies().with::<GraphicsSystem>();

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let scaling_mode = ScalingMode::KeepAspect;
        let position = IVec2::ZERO;
        let angle = 0.0;
        let zoom = 1.0;
        let near = -1.0;
        let far = 1.0;

        let graphics_sys = ctx.get::<GraphicsSystem>();
        let game_resolution = graphics_sys.get_game_resolution();
        let game_size = IVec2::new(game_resolution.width as i32, game_resolution.height as i32);
        let screen_size = IVec2::new(
            graphics_sys.surface_config().width as i32,
            graphics_sys.surface_config().height as i32,
        );

        let game_ortho_proj = Self::_recalc_ortho(game_size, zoom, near, far);
        let screen_ortho_proj = Self::_recalc_ortho(screen_size, 1.0, near, far);

        let view = Self::_recalc_view(position, angle);

        let game_canvas_transform = game_ortho_proj * view;
        let screen_canvas_transform = screen_ortho_proj;

        let viewport = Self::_calc_viewport_rect(
            scaling_mode,
            PhysicalSize::from(screen_size.to_array()),
            graphics_sys.get_game_resolution(),
        );

        let game_shader_buffer =
            graphics_sys
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera game canvas transform"),
                    contents: bytemuck::cast_slice(&[game_canvas_transform]),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                });
        let screen_shader_buffer =
            graphics_sys
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera screen canvas transform"),
                    contents: bytemuck::cast_slice(&[screen_canvas_transform]),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                });

        drop(graphics_sys);

        Self {
            ctx,

            position,
            angle,
            zoom,
            scaling_mode,
            viewport,

            screen_size,
            game_canvas_transform,
            screen_canvas_transform,

            game_ortho_proj,
            screen_ortho_proj,
            view,
            near,
            far,

            game_shader_buffer,
            screen_shader_buffer,
        }
    }
}
