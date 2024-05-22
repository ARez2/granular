use palette::Srgba;

use super::grid::GridPos;


pub type CellColor = Srgba<u8>;


#[derive(Debug, PartialEq)]
pub struct Cell {
    pos: GridPos,
    color: CellColor // Sand: CellColor::new(221, 193, 48, 255)
}
impl Cell {
    pub fn new(pos: GridPos, color: CellColor) -> Self {
        Self {
            pos,
            color
        }
    }

    pub fn pos(&self) -> GridPos {
        self.pos
    }

    pub fn color(&self) -> &CellColor {
        &self.color
    }
}
impl Eq for Cell {}