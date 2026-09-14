use bytemuck::{Pod, Zeroable};
use glam::{IVec2, Vec2};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    _pos: IVec2,
    _col: [f32; 4],
    _tex_coord: Vec2,
}
pub const VERTEX_ATTR: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Sint32x2, 1 => Float32x4, 2 => Float32x2];
impl Vertex {
    pub fn new(pos: IVec2, color: [f32; 4], tex_coord: Vec2) -> Self {
        Self {
            _pos: pos,
            _col: color,
            _tex_coord: tex_coord,
        }
    }
}
pub const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();
