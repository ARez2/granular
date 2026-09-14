use encase::ShaderType;
use granular::prelude::*;
use granular::simulation::prelude::*;

#[MatName]
pub enum MaterialName {
    Empty, // Needs to exist
    Sand,
    Water,
    Rock,
    Red = 99,
}

#[derive(ShaderType, Clone)]
pub struct Material {
    pub tex_coords_start: Vec2,
    pub tex_coords_end: Vec2,
    pub color: Vec4,
    pub density: f32,
}
impl Material {
    /// Constructs a new material but without the color, to make it quicker (as when using a texture, color is always white)
    pub fn new(density: f32) -> Self {
        Self::new_with_color(Vec4::ONE, density)
    }

    pub fn new_with_color(color: Vec4, density: f32) -> Self {
        Self {
            color,
            density,
            ..Default::default()
        }
    }
}
impl Default for Material {
    fn default() -> Self {
        Self {
            tex_coords_start: Vec2::ZERO,
            tex_coords_end: Vec2::ZERO,
            color: vec4(1.0, 0.0, 1.0, 1.0),
            density: 0.0,
        }
    }
}

#[derive(ShaderType, Clone, Copy)]
pub struct Cell {
    material_name: u32,
    velocity: Vec2,
    _pad: f32,
    color: Vec4,
}
impl Cell {
    pub fn new(material: MaterialName, velocity: Vec2, color: Vec4) -> Self {
        Self {
            material_name: material as u32,
            velocity,
            _pad: 0.0,
            color,
        }
    }
}
impl Default for Cell {
    fn default() -> Self {
        Self {
            material_name: MaterialName::Empty as u32,
            velocity: Vec2::ZERO,
            _pad: 0.0,
            color: vec4(1.0, 0.0, 1.0, 1.0),
        }
    }
}
