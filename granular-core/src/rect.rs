use glam::{UVec2, Vec2};

/// Continuous rectangle. The caller specifies the coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub position: Vec2,
    pub size: Vec2,
}
impl Rect {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self { position, size }
    }
}

/// Nonnegative pixel rectangle, top-left origin, +Y down; suitable for a scissor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub origin: UVec2,
    pub size: UVec2,
}
impl PixelRect {
    /// Half-open bounds: left/top included, right/bottom excluded.
    pub fn contains(self, position: Vec2) -> bool {
        let p = position - self.origin.as_vec2();
        p.x >= 0.0 && p.y >= 0.0 && p.x < self.size.x as f32 && p.y < self.size.y as f32
    }
}
