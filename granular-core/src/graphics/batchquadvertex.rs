use bytemuck::{Pod, Zeroable};
use glam::Vec2;

#[repr(u32)]
pub(crate) enum VertexShape {
    Textured,
    Circle { thickness: f32 },
}
impl VertexShape {
    fn shape_and_params(self) -> (u32, f32) {
        match self {
            Self::Textured => (0, 0.0),
            Self::Circle { thickness } => (1, thickness),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct QuadVertex {
    pos: Vec2,
    color: [f32; 4],
    tex_coords: Vec2,
    shape: u32,
    shape_params: f32,
}
pub const VERTEX_ATTR: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4, 2 => Float32x2, 3 => Uint32, 4 => Float32];
impl QuadVertex {
    pub fn new(pos: Vec2, color: [f32; 4], tex_coords: Vec2, shape: VertexShape) -> Self {
        let (shape, shape_params) = shape.shape_and_params();
        Self {
            pos,
            color,
            tex_coords,
            shape,
            shape_params,
        }
    }
}
pub const VERTEX_SIZE: usize = std::mem::size_of::<QuadVertex>();
