use geese::{GeeseContextHandle, GeeseSystem};


mod grid;
use grid::CellGrid;
use palette::Srgba;

pub(self) mod cell;

pub const GRID_WIDTH: usize = 600;
pub const GRID_HEIGHT: usize = 400;


pub struct Simulation {
    ctx: GeeseContextHandle<Self>,
    grid: CellGrid
}
impl Simulation {
    pub(crate) fn get_grid_texture_data(&self) -> &[u8] {
        self.grid.get_texture_data()
    }
}
impl GeeseSystem for Simulation {
    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut grid = CellGrid::empty();
        for y in 10..50 {
            for x in 10..50 {
                grid.place_cell(self::cell::Cell::new((x, y), Srgba::new(255, 255, 0, 255)));
            }
        }
        Self {
            ctx,
            grid
        }
    }
}