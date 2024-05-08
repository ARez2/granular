use super::grid::GridPos;




#[derive(Debug, PartialEq, Eq)]
pub struct Cell {
    pos: GridPos
}
impl Cell {
    pub fn new(pos: GridPos) -> Self {
        Self {
            pos
        }
    }

    pub fn pos(&self) -> GridPos {
        self.pos
    }
}