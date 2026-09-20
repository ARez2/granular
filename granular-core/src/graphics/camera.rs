use super::view_mapping::{
    Camera2D, CameraMotion, Presentation, RenderView, ScalingMode, SurfacePos, WorldPos,
};
use super::{GraphicsSystem, WindowSystem};
use crate::utils::*;
use glam::{Mat4, UVec2, Vec2};
use wgpu::{Buffer, BufferUsages, util::DeviceExt};

/// Engine integration for desired camera state and the last prepared render snapshot.
pub struct Camera {
    ctx: GeeseContextHandle<Self>,
    state: Camera2D,
    /// Helper to determine how everything is displayed
    presentation: Presentation,
    /// Allows to transform between various coordinate systems
    render_view: RenderView,
    /// Buffer which holds the matrix to transform from world position into clip space of the game render target
    world_game_buffer: Buffer,
    /// Buffer which holds the matrix to transform from world position into clip space of the surface
    world_surface_buffer: Buffer,
    /// Buffer which holds the matrix to transform from a physical window pixel into clip space of the surface
    surface_pixels_buffer: Buffer,
    /// Buffer which holds the matrix to transform a UI position via DPI scaling into clip space of the surface
    ui_points_buffer: Buffer,
}
impl Camera {
    /// Gets the world space position of the camera
    pub fn position(&self) -> Vec2 {
        self.state.position
    }
    /// Sets the world space position of the camera
    pub fn set_position(&mut self, position: Vec2) {
        assert!(position.is_finite());
        self.state.position = position;
    }

    /// Translates the camera in world space by `offset`
    pub fn translate(&mut self, offset: Vec2) {
        self.set_position(self.state.position + offset);
    }

    /// Sets the unrotated visible bottom-left; no rounding of the desired center.
    pub fn set_bottomleft_position(&mut self, bottomleft: Vec2) {
        let game = self.ctx.get::<GraphicsSystem>().get_game_resolution();
        self.set_position(bottomleft + game.as_vec2() / (2.0 * self.state.zoom));
    }

    /// Returns the rotation of the camera (in radians)
    pub fn rotation(&self) -> f32 {
        self.state.rotation
    }
    /// Sets the angle of rotation (in radians)
    pub fn set_rotation(&mut self, angle: f32) {
        assert!(angle.is_finite());
        self.state.rotation = angle;
    }

    /// Gets the zoom factor of the camera. Higher = closer
    pub fn zoom(&self) -> f32 {
        self.state.zoom
    }
    /// Sets the zoom factor of the camera. Higher = closer
    pub fn set_zoom(&mut self, zoom: f32) {
        assert!(zoom.is_finite() && zoom > 0.0);
        self.state.zoom = zoom;
    }

    /// Configures how the camera moves
    pub fn set_motion(&mut self, motion: CameraMotion) {
        self.state.motion = motion;
    }
    /// Gets the motion mode
    pub fn motion(&self) -> CameraMotion {
        self.state.motion
    }

    /// Sets the camera to follow a target position. `speed` is in 1/s; `dt` is in seconds.
    pub fn follow(&mut self, target: Vec2, speed: f32, dt: f32) {
        self.state.follow(target, speed, dt);
    }

    /// Configures the scaling mode for the application
    pub fn set_scaling_mode(&mut self, mode: ScalingMode) {
        self.presentation.scaling_mode = mode;
    }

    /// Configures UI scale factor
    pub fn set_ui_scale(&mut self, scale: f32) {
        assert!(scale.is_finite() && scale > 0.0);
        self.presentation.ui_scale = scale;
    }

    /// Last prepared view; setters affect the next PrepareRender event.
    /// Use this snapshot for picking against the currently displayed view.
    pub fn render_view(&self) -> RenderView {
        self.render_view
    }

    /// Transforms a world position into a pixel position on surface
    pub fn world_to_surface(&self, p: WorldPos) -> SurfacePos {
        self.render_view.world_to_surface(p)
    }

    /// Transforms a pixel position on the surface into a position in the world. Returns None if the surface position is outside of the surface's area.
    pub fn surface_to_world(&self, p: SurfacePos) -> Option<WorldPos> {
        self.render_view.surface_to_world(p)
    }

    /// Gets buffer which holds the matrix to transform from world position into clip space of the game render target
    pub(crate) fn world_to_game_clip_buffer(&self) -> &Buffer {
        &self.world_game_buffer
    }

    /// Gets buffer which holds the matrix to transform from world position into clip space of the surface
    pub(crate) fn world_to_surface_clip_buffer(&self) -> &Buffer {
        &self.world_surface_buffer
    }

    /// Gets buffer which holds the matrix to transform from a physical window pixel into clip space of the surface
    pub(crate) fn surface_pixels_to_clip_buffer(&self) -> &Buffer {
        &self.surface_pixels_buffer
    }

    /// Gets buffer which holds the matrix to transform a UI position via DPI scaling into clip space of the surface
    pub(crate) fn ui_points_to_clip_buffer(&self) -> &Buffer {
        &self.ui_points_buffer
    }

    /// Updates the `Presentation` and creates a new `RenderView`
    fn on_prepare_render(&mut self, _: &super::events::PrepareRender) {
        let graphics = self.ctx.get::<GraphicsSystem>();
        let surface = graphics.get_surface_resolution();
        self.presentation.game_size = graphics.get_game_resolution();
        self.presentation.surface_size = UVec2::new(surface.width, surface.height);
        // Read per frame: DPI changes do not depend on an extra event handler.
        self.presentation.dpi_scale = self.ctx.get::<WindowSystem>().scale_factor() as f32;
        self.render_view = RenderView::new(self.state, self.presentation);
        for (buffer, matrix) in [
            (
                &self.world_game_buffer,
                self.render_view.world_to_game_clip(),
            ),
            (
                &self.world_surface_buffer,
                self.render_view.world_to_surface_clip(),
            ),
            (
                &self.surface_pixels_buffer,
                self.render_view.surface_pixels_to_clip(),
            ),
            (&self.ui_points_buffer, self.render_view.ui_points_to_clip()),
        ] {
            graphics
                .queue()
                .write_buffer(buffer, 0, bytemuck::bytes_of(&matrix));
        }
    }

    /// Small helper to create a buffer
    fn uniform(device: &wgpu::Device, label: &str, matrix: Mat4) -> Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::bytes_of(&matrix),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
    }
}
impl GeeseSystem for Camera {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<GraphicsSystem>()
        .with::<WindowSystem>();
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers().with(Self::on_prepare_render);

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let graphics = ctx.get::<GraphicsSystem>();
        let surface = graphics.get_surface_resolution();
        let presentation = Presentation::new(
            graphics.get_game_resolution(),
            UVec2::new(surface.width, surface.height),
            ctx.get::<WindowSystem>().scale_factor() as f32,
        );
        let state = Camera2D::default();
        let render_view = RenderView::new(state, presentation);
        let device = graphics.device();
        let world_game_buffer = Self::uniform(
            device,
            "World -> game clip buffer",
            render_view.world_to_game_clip(),
        );
        let world_surface_buffer = Self::uniform(
            device,
            "World -> surface clip buffer",
            render_view.world_to_surface_clip(),
        );
        let surface_pixels_buffer = Self::uniform(
            device,
            "Surface pixels -> clip buffer",
            render_view.surface_pixels_to_clip(),
        );
        let ui_points_buffer = Self::uniform(
            device,
            "UI points -> clip buffer",
            render_view.ui_points_to_clip(),
        );
        drop(graphics);
        Self {
            ctx,
            state,
            presentation,
            render_view,
            world_game_buffer,
            world_surface_buffer,
            surface_pixels_buffer,
            ui_points_buffer,
        }
    }
}
