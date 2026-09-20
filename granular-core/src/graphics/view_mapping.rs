//! World coordinate system:
//! - +Y up
//! - describes where something is in the world
//! - independent of camera
//! Game coordinate system:
//! - +Y Down
//! - describes on what pixel of the game render target something appears (on the screen)
//! Game image, surface, UI and UVs: +Y down.
//! This module owns world-to-image orientation and image-to-clip projection.
use crate::{PixelRect, Rect};
use glam::{Affine2, Mat2, Mat4, UVec2, Vec2, Vec4};

/// Always render one extra texel on every side for camera compensation.
/// The visible game resolution excludes this border, in every camera mode.
pub const GAME_PADDING: u32 = 1;
pub fn game_target_size(game_size: UVec2) -> UVec2 {
    UVec2::new(
        game_size
            .x
            .checked_add(2 * GAME_PADDING)
            .expect("game width overflow"),
        game_size
            .y
            .checked_add(2 * GAME_PADDING)
            .expect("game height overflow"),
    )
}

/// World position, where +Y is Up and +X is right
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPos(pub Vec2);

/// Pixel in the games render target, where +Y is Down and +X is right
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GamePixelPos(pub Vec2);

/// Pixel in the on the final surface, where +Y is Down and +X is right
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePos(pub Vec2);

/// Pixel in the on the ui surface, where +Y is Down and +X is right
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiPos(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingMode {
    /// Scales the game as large as possible while preserving the game resolution aspect ratio
    KeepAspect,
    /// Ignores the game resolution aspect ratio and just scales the game up to the surfacce size
    Stretch,
    /// Tries to scale the game as large as possible while only using integer scaling factors.
    /// Falls back to KeepAspect when the surface is smaller than the game image.
    Integer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMotion {
    /// 1:1 mapping of the cameras transform, regardless of the game's resolution. Could cause subpixel/ edge flickering
    Free,
    /// Snaps the camera position to the game grid using a "world -> game" transform
    PixelSnapped,
    /// First snaps into the game grid like PixelSnapped mode, but then when
    /// rendering the game, compensates in surface pixels.
    /// Best with integer presentation scaling and unrotated pixel art.
    SmoothPixel,
}

#[derive(Debug, Clone, Copy)]
pub struct Camera2D {
    /// Desired center in world space. Never overwritten by snapping.
    pub position: Vec2,
    /// Angle in radians (+Angle = CCW rotation)
    pub rotation: f32,
    /// The zoom factor of the camera. Higher = closer
    pub zoom: f32,
    /// The way the camera position is handled
    pub motion: CameraMotion,
}
impl Default for Camera2D {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            rotation: 0.0,
            zoom: 1.0,
            motion: CameraMotion::PixelSnapped,
        }
    }
}
impl Camera2D {
    /// Exponential following. `speed` is in 1/s; `dt` is in seconds.
    pub fn follow(&mut self, target: Vec2, speed: f32, dt: f32) {
        assert!(target.is_finite() && speed.is_finite() && dt.is_finite());
        assert!(speed >= 0.0 && dt >= 0.0);
        self.position = self.position.lerp(target, 1.0 - (-speed * dt).exp());
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Presentation {
    pub game_size: UVec2,
    pub surface_size: UVec2,
    pub scaling_mode: ScalingMode,
    pub dpi_scale: f32,
    pub ui_scale: f32,
}
impl Presentation {
    pub fn new(game_size: UVec2, surface_size: UVec2, dpi_scale: f32) -> Self {
        Self {
            game_size: game_size.max(UVec2::ONE),
            surface_size: surface_size.max(UVec2::ONE),
            scaling_mode: ScalingMode::KeepAspect,
            dpi_scale,
            ui_scale: 1.0,
        }
    }
    pub fn game_viewport_surface_px(self) -> PixelRect {
        assert!(self.game_size.x > 0 && self.game_size.y > 0);
        assert!(self.surface_size.x > 0 && self.surface_size.y > 0);
        if self.scaling_mode == ScalingMode::Stretch {
            return PixelRect {
                origin: UVec2::ZERO,
                size: self.surface_size,
            };
        }
        let ratio = self.surface_size.as_vec2() / self.game_size.as_vec2();
        let fit = ratio.x.min(ratio.y);
        let scale = if self.scaling_mode == ScalingMode::Integer && fit >= 1.0 {
            fit.floor()
        } else {
            fit
        };
        let size = (self.game_size.as_vec2() * scale)
            .round()
            .as_uvec2()
            .max(UVec2::ONE)
            .min(self.surface_size);
        PixelRect {
            origin: (self.surface_size - size) / 2,
            size,
        }
    }
}

/// Created by the camera each frame. Represents the different coordinate systems in use
#[derive(Debug, Clone, Copy)]
pub struct RenderView {
    pub presentation: Presentation,
    pub game_viewport_surface_px: PixelRect,
    pub presentation_offset: Vec2,
    /// Transforms a world position into its (pixel) position in the game's render target
    world_to_game: Affine2,
    /// Transforms a (pixel) position in the game's render target back into a world position
    game_to_world: Affine2,
    /// Transforms a pixel in the game's render target into a pixel on the (upscaled) surface
    game_to_surface: Affine2,
    /// Transforms a pixel on the (upscaled) surface to the game's render target
    surface_to_game: Affine2,
    /// Transforms a position from world space directly into a pixel on the (upscaled) surface
    world_to_surface: Affine2,
    /// Transforms a pixel on the (upscaled) surface into a position in world space
    surface_to_world: Affine2,
    /// Factor which scales the UI on the surface
    ui_to_surface_scale: f32,
}
impl RenderView {
    pub fn new(camera: Camera2D, presentation: Presentation) -> Self {
        assert!(camera.position.is_finite() && camera.rotation.is_finite());
        assert!(camera.zoom.is_finite() && camera.zoom > 0.0);
        let ui_to_surface_scale = presentation.dpi_scale * presentation.ui_scale;
        assert!(ui_to_surface_scale.is_finite() && ui_to_surface_scale > 0.0);
        let game_viewport_surface_px = presentation.game_viewport_surface_px();
        let game_size = presentation.game_size.as_vec2();
        let scale = game_viewport_surface_px.size.as_vec2() / game_size;

        // WORLD -> IMAGE: the only semantic Y-up -> Y-down conversion.
        let desired_world_to_game = Affine2::from_translation(game_size * 0.5)
            * Affine2::from_scale(Vec2::new(camera.zoom, -camera.zoom))
            * Affine2::from_angle(-camera.rotation)
            * Affine2::from_translation(-camera.position);
        let mut world_to_game = desired_world_to_game;
        if camera.motion != CameraMotion::Free {
            // Snap projected translation, including the half-size of odd resolutions.
            world_to_game.translation = world_to_game.translation.round();
        }
        let presentation_offset = if camera.motion == CameraMotion::SmoothPixel {
            ((desired_world_to_game.translation - world_to_game.translation) * scale).round()
        } else {
            Vec2::ZERO
        };
        let game_to_surface = Affine2::from_mat2_translation(
            Mat2::from_diagonal(scale),
            game_viewport_surface_px.origin.as_vec2() + presentation_offset,
        );
        let world_to_surface = game_to_surface * world_to_game;
        Self {
            presentation,
            game_viewport_surface_px,
            presentation_offset,
            world_to_game,
            game_to_world: world_to_game.inverse(),
            game_to_surface,
            surface_to_game: game_to_surface.inverse(),
            world_to_surface,
            surface_to_world: world_to_surface.inverse(),
            ui_to_surface_scale,
        }
    }

    /// Transforms a position in world space into a pixel in the game's render target
    pub fn world_to_game_pixels(&self, p: WorldPos) -> GamePixelPos {
        GamePixelPos(self.world_to_game.transform_point2(p.0))
    }

    /// Transforms a pixel position in the game's render target into world space
    pub fn game_pixels_to_world(&self, p: GamePixelPos) -> WorldPos {
        WorldPos(self.game_to_world.transform_point2(p.0))
    }

    /// Transforms a pixel position in the game's render target into a pixel position on the surface
    pub fn game_pixels_to_surface(&self, p: GamePixelPos) -> SurfacePos {
        SurfacePos(self.game_to_surface.transform_point2(p.0))
    }

    /// Tries to transform a pixel position on the surface into a pixel position on the game's render target.
    /// Returns None if the game pixel position falls into the letterboxes
    pub fn surface_to_game_pixels(&self, p: SurfacePos) -> Option<GamePixelPos> {
        self.contains_surface(p)
            .then(|| GamePixelPos(self.surface_to_game.transform_point2(p.0)))
    }

    /// Transforms a world position into a pixel position on surface
    pub fn world_to_surface(&self, p: WorldPos) -> SurfacePos {
        SurfacePos(self.world_to_surface.transform_point2(p.0))
    }

    /// Transforms a pixel position on the surface into a position in the world.
    /// Returns None if the surface position is outside of the surface's area.
    pub fn surface_to_world(&self, p: SurfacePos) -> Option<WorldPos> {
        self.contains_surface(p)
            .then(|| WorldPos(self.surface_to_world.transform_point2(p.0)))
    }

    /// Transforms a delta (like mouse movement) from surface space into world space
    pub fn surface_delta_to_world(&self, delta: Vec2) -> Vec2 {
        self.surface_to_world.transform_vector2(delta)
    }

    /// Transforms a UI position into a pixel position on the surface
    pub fn ui_points_to_surface(&self, p: UiPos) -> SurfacePos {
        SurfacePos(p.0 * self.ui_to_surface_scale)
    }

    /// Transforms a pixel position on the surface into a position in the UI
    pub fn surface_to_ui_points(&self, p: SurfacePos) -> UiPos {
        UiPos(p.0 / self.ui_to_surface_scale)
    }

    /// Checks if the provided surface position is inside of the game's viewport
    fn contains_surface(&self, p: SurfacePos) -> bool {
        self.game_viewport_surface_px.contains(p.0)
    }

    /// Transforms from world position into clip space of the game render target
    pub fn world_to_game_clip(&self) -> Mat4 {
        pixels_to_clip(game_target_size(self.presentation.game_size).as_vec2())
            * affine_to_mat4(
                Affine2::from_translation(Vec2::splat(GAME_PADDING as f32)) * self.world_to_game,
            )
    }

    /// Transforms from world position into clip space of the surface
    pub fn world_to_surface_clip(&self) -> Mat4 {
        self.surface_pixels_to_clip() * affine_to_mat4(self.world_to_surface)
    }

    /// Transforms from a physical window pixel into clip space of the surface
    pub fn surface_pixels_to_clip(&self) -> Mat4 {
        pixels_to_clip(self.presentation.surface_size.as_vec2())
    }

    /// Transforms a UI position via DPI scaling into clip space of the surface
    pub fn ui_points_to_clip(&self) -> Mat4 {
        self.surface_pixels_to_clip()
            * affine_to_mat4(Affine2::from_scale(Vec2::splat(self.ui_to_surface_scale)))
    }

    /// UV rectangle sampled by a fixed presentation quad. Moving the sample
    /// window instead of the quad keeps the letterbox rectangle stationary.
    pub fn game_texture_uv_rect(&self) -> Rect {
        let game = self.presentation.game_size.as_vec2();
        let target = game_target_size(self.presentation.game_size).as_vec2();
        let scale = self.game_viewport_surface_px.size.as_vec2() / game;
        Rect::new(
            (Vec2::splat(GAME_PADDING as f32) - self.presentation_offset / scale) / target,
            game / target,
        )
    }
}

/// IMAGE -> GPU: the only technical Y-down -> NDC Y-up conversion.
/// Coordinates describe pixel edges; centers are n + 0.5. No H - 1 here.
fn pixels_to_clip(size: Vec2) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(2.0 / size.x, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -2.0 / size.y, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(-1.0, 1.0, 0.0, 1.0),
    )
}
fn affine_to_mat4(a: Affine2) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(a.matrix2.x_axis.x, a.matrix2.x_axis.y, 0.0, 0.0),
        Vec4::new(a.matrix2.y_axis.x, a.matrix2.y_axis.y, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(a.translation.x, a.translation.y, 0.0, 1.0),
    )
}

#[cfg(test)]
#[path = "view_mapping_tests.rs"]
mod tests;
