use log::{debug, warn};

use crate::{GRID_HEIGHT, GRID_WIDTH};

use super::cell::{Cell, CellColor};

pub type GridPos = (usize, usize);
pub const EMPTY_CELL_IDX: usize = 0;

const TOTAL_NUM_CELLS: usize = GRID_WIDTH * GRID_HEIGHT;

// https://github.com/ARez2/FallingRust/blob/main/src/matrix.rs

pub struct CellGrid {
    cells: Vec<Cell>,
    grid: Vec<usize>,
    
    texture_data: Vec<u8>
}
impl CellGrid {
    // Creates a new empty CellGrid. You can also use `CellGrid::empty()`
    pub fn new() -> Self {
        Self::empty()
    }

    // Creates a new empty CellGrid.
    pub fn empty() -> Self {
        let mut texture_data = Vec::with_capacity(4 * TOTAL_NUM_CELLS);
        texture_data.resize_with(4 * TOTAL_NUM_CELLS, || 0);
        let mut grid = Vec::with_capacity(TOTAL_NUM_CELLS);
        grid.resize_with(TOTAL_NUM_CELLS, || 0);
        Self {
            cells: vec![Cell::new((0, 0), CellColor::new(0, 0, 0, 0))],
            grid,
            texture_data
        }
    }

    pub(super) fn get_texture_data(&self) -> &[u8] {
        &self.texture_data
    }


    /// Converts the position into an index to be used in self.data
    #[inline]
    fn grid_idx(&self, pos: GridPos) -> usize {
        pos.0 + pos.1 * GRID_WIDTH
    }


    /// Returns the Cell at that index inside of self.cells.
    /// If the cell_index is invalid, returns the empty cell
    fn get_cell_from_cellidx(&self, cell_index: usize) -> &Cell {
        if cell_index >= self.cells.len() {
            &self.cells[EMPTY_CELL_IDX]
        } else {
            &self.cells[cell_index]
        }
    }

    /// Returns the Cell at that index inside of self.cells.
    /// If the cell_index is invalid, returns the empty cell
    fn get_cell_from_cellidx_mut(&mut self, cell_index: usize) -> &mut Cell {
        if cell_index >= self.cells.len() {
            &mut self.cells[EMPTY_CELL_IDX]
        } else {
            &mut self.cells[cell_index]
        }
    }


    fn set_color_at_grididx(&mut self, grid_idx: usize, color: &CellColor) {
        self.texture_data[grid_idx * 4 + 0] = color.red;
        self.texture_data[grid_idx * 4 + 1] = color.green;
        self.texture_data[grid_idx * 4 + 2] = color.blue;
        self.texture_data[grid_idx * 4 + 3] = color.alpha;
    }

    fn set_color_at_grididx_empty(&mut self, grid_idx: usize) {
        let empty_col = self.cells[0].color();
        self.texture_data[grid_idx * 4 + 0] = empty_col.red;
        self.texture_data[grid_idx * 4 + 1] = empty_col.green;
        self.texture_data[grid_idx * 4 + 2] = empty_col.blue;
        self.texture_data[grid_idx * 4 + 3] = empty_col.alpha;
    }


    // Sets a new cell on the grid. Replaces any other cell that might be there
    pub fn place_cell(&mut self, cell: Cell) {
        let grid_idx = self.grid_idx(cell.pos());
        // IDEA: Maybe have one texture_data per chunk and draw each chunk seperately
        self.set_color_at_grididx(grid_idx, cell.color());
        // If there was another non-empty cell at this position, swap remove it
        let prev_cell_idx = self.grid[grid_idx];
        if prev_cell_idx != EMPTY_CELL_IDX {
            // Replaces the old Cell inside of self.cells with the new Cell. Since we dont change the
            // index inside of self.cells, we dont need to modify self.grid
            let _prev_cell = std::mem::replace(&mut self.cells[prev_cell_idx], cell);
        } else {
            self.grid[grid_idx] = self.cells.len(); // After pushing, it would be len()-1
            self.cells.push(cell);
        }
    }


    /// Replaces the cell at cellpos with the last cell in self.cells (faster than shifting) and updates self.data
    pub fn remove_cell_at_pos(&mut self, cellpos: GridPos) {
        if self.cells.is_empty() {
            debug!("self.cells is empty, cannot remove a cell.");
            return;
        };
        let grid_idx = self.grid_idx(cellpos);
        let cell_index = self.grid[grid_idx];
        if cell_index == EMPTY_CELL_IDX || self.cells.len() == 1 {
            debug!("Skipping remove_cell_from_cells because the targetted cell is the empty cell");
            return;
        }
        self.grid[grid_idx] = 0;
        self.set_color_at_grididx_empty(grid_idx);
        //self.set_color_at_grididx(grid_idx, self.cells[0].color());
        // If our cell is at the back of the cells, then we can remove it normally
        if cell_index == self.cells.len() {
            self.cells.remove(cell_index);
            return;
        };
        // If not, we need to do a swap remove
        if self.get_cell_from_cellidx(cell_index) != &self.cells[EMPTY_CELL_IDX] {
            let last_cell = &self.cells[self.cells.len() - 1];
            let last_cell_grid_idx = self.grid_idx(last_cell.pos());
            self.grid[last_cell_grid_idx] = cell_index;
            self.cells.swap_remove(cell_index);
        };
    }
}