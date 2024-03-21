#![allow(unused)]
use glam::{Affine2, Mat4, Quat, Vec2, Vec3};


pub struct Camera {
    transform: Affine2,
    angle: f32
}
impl Camera {
    /// Sets the translation of the cameras transform
    pub fn set_position(&mut self, position: Vec2) {
        self.transform.translation = position;
    }

    pub fn position(&self) -> Vec2 {
        self.transform.translation
    }


    /// Set rotation of the camera (in radians)
    pub fn set_rotation(&mut self, rotation: f32) {
        self.angle = rotation;
        self.transform = Affine2::from_angle_translation(self.angle, self.transform.translation);
    }

    pub fn rotation(&self) -> f32 {
        self.angle
    }


    pub fn set_scale(&mut self, scale: Vec2) {
        self.transform = Affine2::from_scale_angle_translation(scale, self.angle, self.transform.translation);
    }


    pub fn get_view_proj(&self, screen_size: Vec2) -> Mat4 {
        let aspect_ratio = screen_size.x / screen_size.y;

        // Compute the orthographic projection matrix
        let ortho_proj = Mat4::orthographic_rh_gl(
            -1.0 * aspect_ratio, // left
            1.0 * aspect_ratio,  // right
            -1.0,                // bottom
            1.0,                 // top
            -1.0,                // near
            1.0,                 // far
        );

        // Construct the view matrix (inverse of camera transform)
        let view_mat = Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            Quat::from_rotation_z(self.angle),
            Vec3::new(self.transform.translation.x, -self.transform.translation.y, 0.0));

        ortho_proj * view_mat
    }
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            transform: Affine2::from_scale_angle_translation(Vec2::ONE, 0.0, Vec2::ONE),
            angle: 0.0
        }
    }
}