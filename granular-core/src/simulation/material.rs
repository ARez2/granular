use crate::cell::CellColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    Empty,
    Sand,
    Red,
    Green,
    Blue,
}
impl Material {
    pub const fn color(&self) -> CellColor {
        match self {
            Self::Empty => CellColor::new(10, 10, 10, 255),
            Self::Sand => CellColor::new(221, 193, 48, 255),
            Self::Red => CellColor::new(255, 0, 0, 255),
            Self::Green => CellColor::new(0, 255, 0, 255),
            Self::Blue => CellColor::new(0, 0, 255, 255),
        }
    }
}
