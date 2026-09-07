use glam::IVec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub position: IVec2,
    pub size: IVec2,
}
impl Rect {
    pub fn new(position: IVec2, size: IVec2) -> Self {
        Self { position, size }
    }
}
