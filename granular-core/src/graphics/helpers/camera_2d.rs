use glam::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMotion {
    /// 1:1 mapping of the cameras transform, regardless of the game's resolution.
    /// Could cause subpixel/ edge flickering, so its not recommended.
    Free,
    /// Snaps the camera position to the game grid using a "world -> game" transform
    PixelSnapped,
    /// First snaps into the game grid like PixelSnapped mode, but then when
    /// rendering the game, compensates in surface pixels.
    /// Feels smoother than PixelSnapped.
    /// Best with integer presentation_settings scaling and unrotated pixel art.
    SmoothPixel,
}
impl CameraMotion {
    /// Returns true if this motion mode snaps the translation of the camera to the game grid when displaying
    pub fn translation_snaps_to_gamepx(&self) -> bool {
        matches!(self, Self::PixelSnapped | Self::SmoothPixel)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Camera2D {
    /// Desired center in world space. Never overwritten by snapping.
    pub(crate) position: Vec2,
    /// Angle in radians (+Angle = CCW rotation)
    pub(crate) rotation: f32,
    /// The zoom factor of the camera. Higher = closer
    pub(crate) zoom: f32,
    /// The way the camera position is handled
    pub(crate) motion: CameraMotion,
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
    pub(crate) fn follow(&mut self, target: Vec2, speed: f32, dt: f32) {
        assert!(target.is_finite() && speed.is_finite() && dt.is_finite());
        assert!(speed >= 0.0 && dt >= 0.0);
        self.position = self.position.lerp(target, 1.0 - (-speed * dt).exp());
    }
}
