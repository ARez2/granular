/// Coordinate space only. The render phase chooses the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DrawSpace {
    /// World units are game pixels. +X right, +Y up; camera-dependent.
    World,
    /// Physical pixels, origin at the surface's top-left. +Y down.
    SurfacePixels,
    /// Logical UI points, origin at the surface's top-left. +Y down.
    UiPoints,
}
impl DrawSpace {
    /// Local geometric meaning of "top"; not a screen conversion.
    pub(crate) fn top_sign(self) -> f32 {
        match self {
            Self::World => 1.0,
            Self::SurfacePixels | Self::UiPoints => -1.0,
        }
    }
}
