use super::DrawSpace;
use glam::{Mat2, Vec2};

/// Describes how to get from  the center of the quad to the topleft position (is up or down, depending on the DrawSpace)
pub(crate) fn top_left_offset(size: Vec2, space: DrawSpace) -> Vec2 {
    Vec2::new(-size.x, space.top_sign() * size.y) * 0.5
}

/// Returns the corners of a quad with those parameters (TL, BL, BR, TR). Rotation is around the rectangle's center.
pub(crate) fn quad_corners(center: Vec2, size: Vec2, angle: f32, space: DrawSpace) -> [Vec2; 4] {
    let half = size * 0.5;
    let top = space.top_sign() * half.y;
    let rotation = Mat2::from_angle(angle);
    [
        Vec2::new(-half.x, top),
        Vec2::new(-half.x, -top),
        Vec2::new(half.x, -top),
        Vec2::new(half.x, top),
    ]
    .map(|p| center + rotation * p)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn odd_and_single_pixel_quads_preserve_size() {
        for space in [
            DrawSpace::World,
            DrawSpace::SurfacePixels,
            DrawSpace::UiPoints,
        ] {
            for size in [Vec2::ONE, Vec2::new(17.0, 13.0)] {
                for angle in [0.0, 0.37, -1.2] {
                    let p = quad_corners(Vec2::new(2.25, -3.75), size, angle, space);
                    assert!(((p[1] - p[0]).length() - size.y).abs() < 1e-5);
                    assert!(((p[2] - p[1]).length() - size.x).abs() < 1e-5);
                    assert!(((p[1] - p[0]).dot(p[2] - p[1])).abs() < 1e-4);
                }
            }
        }
    }
    #[test]
    fn top_left_anchor_and_uv_order_follow_space() {
        for space in [
            DrawSpace::World,
            DrawSpace::SurfacePixels,
            DrawSpace::UiPoints,
        ] {
            let anchor = Vec2::new(20.25, 30.5);
            let size = Vec2::new(17.0, 13.0);
            let center = anchor - top_left_offset(size, space);
            let p = quad_corners(center, size, 0.0, space);
            assert_eq!(p[0], anchor);
            assert_eq!(p[3] - p[0], Vec2::new(17.0, 0.0));
            assert_eq!(p[1].y - p[0].y, -space.top_sign() * 13.0);
        }
    }
}
