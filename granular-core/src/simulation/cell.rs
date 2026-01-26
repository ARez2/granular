use palette::Srgba;

use crate::material::Material;

pub type CellColor = Srgba<u8>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub color: CellColor, // Sand: CellColor::new(221, 193, 48, 255)
    pub material: Material,
    pub last_processed_in_tick: usize,
}
impl Cell {
    pub const fn new(material: Material) -> Self {
        Self {
            color: material.color(),
            material,
            last_processed_in_tick: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.material == Material::Empty
    }
}
impl Eq for Cell {}
