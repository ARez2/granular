use glam::I16Vec2;
use log::debug;

use super::{
    cell::{Cell, CellColor},
    chunk::{CHUNK_SIZE, NUM_CELLS_IN_CHUNK},
    material::Material,
};

pub type GridPos = I16Vec2;

// https://github.com/ARez2/FallingRust/blob/main/src/matrix.rs

#[derive(Debug)]
pub struct CellGrid {
    cells: [Cell; NUM_CELLS_IN_CHUNK],

    texture_data: Vec<u8>,
}
impl CellGrid {
    // Creates a new empty CellGrid. You can also use `CellGrid::empty()`
    pub fn new() -> Self {
        Self::empty()
    }

    // Creates a new empty CellGrid.
    pub fn empty() -> Self {
        let mut texture_data = Vec::with_capacity(4 * NUM_CELLS_IN_CHUNK);
        texture_data.resize_with(4 * NUM_CELLS_IN_CHUNK, || 0);
        Self {
            cells: [Cell::new(Material::Empty); NUM_CELLS_IN_CHUNK],
            texture_data,
        }
    }

    pub fn new_material(material: Material) -> Self {
        let mut texture_data = Vec::with_capacity(4 * NUM_CELLS_IN_CHUNK);
        for _ in 0..NUM_CELLS_IN_CHUNK {
            texture_data.extend_from_slice(&[
                material.color().red,
                material.color().green,
                material.color().blue,
                material.color().alpha,
            ]);
        }
        Self {
            cells: [Cell::new(material); NUM_CELLS_IN_CHUNK],
            texture_data,
        }
    }

    pub(super) fn get_texture_data(&self) -> &[u8] {
        &self.texture_data
    }

    /// Converts the position into an index to be used in self.data
    #[inline(always)]
    pub fn grid_idx(&self, pos: GridPos) -> usize {
        pos.x as usize + pos.y as usize * CHUNK_SIZE as usize
    }

    #[inline]
    pub fn get_cell(&self, pos: GridPos) -> &Cell {
        self.get_cell_at(self.grid_idx(pos))
    }

    #[inline]
    pub fn get_cell_at(&self, grididx: usize) -> &Cell {
        &self.cells[grididx]
    }

    #[inline]
    pub fn get_cell_mut(&mut self, pos: GridPos) -> &mut Cell {
        self.get_cell_at_mut(self.grid_idx(pos))
    }

    #[inline]
    pub fn get_cell_at_mut(&mut self, grididx: usize) -> &mut Cell {
        &mut self.cells[grididx]
    }

    pub fn set_cell(&mut self, pos: GridPos, cell: Cell) {
        let idx = self.grid_idx(pos);
        self.cells[idx] = cell;
        self.set_color_at_grididx(idx, &cell.color);
    }

    pub fn replace_cell<'a>(&'a mut self, pos: GridPos, cell: &'a mut Cell) -> &'a mut Cell {
        let grididx = self.grid_idx(pos);
        let new_col = cell.color;
        self.set_color_at_grididx(grididx, &new_col);
        let prev = std::mem::replace(&mut self.get_cell_at_mut(grididx), cell);
        prev
    }

    pub fn set_color(&mut self, pos: GridPos, color: &CellColor) {
        self.set_color_at_grididx(self.grid_idx(pos), color);
    }

    pub fn set_color_at_grididx(&mut self, grid_idx: usize, color: &CellColor) {
        self.texture_data[grid_idx * 4 + 0] = color.red;
        self.texture_data[grid_idx * 4 + 1] = color.green;
        self.texture_data[grid_idx * 4 + 2] = color.blue;
        self.texture_data[grid_idx * 4 + 3] = color.alpha;
    }

    pub fn get_color(&self, pos: GridPos) -> CellColor {
        let grid_idx = self.grid_idx(pos);
        CellColor::new(
            self.texture_data[grid_idx * 4 + 0],
            self.texture_data[grid_idx * 4 + 1],
            self.texture_data[grid_idx * 4 + 2],
            self.texture_data[grid_idx * 4 + 3],
        )
    }

    // // Sets a new cell on the grid. Replaces any other cell that might be there
    // pub fn place_cell(&mut self, cell: Cell) {
    //     let grid_idx = self.grid_idx(cell.pos());
    //     // IDEA: Maybe have one texture_data per chunk and draw each chunk seperately
    //     self.set_color_at_grididx(grid_idx, cell.color());
    //     // If there was another non-empty cell at this position, swap remove it
    //     let prev_cell_idx = self.grid[grid_idx];
    //     if prev_cell_idx != EMPTY_CELL_IDX {
    //         // Replaces the old Cell inside of self.cells with the new Cell. Since we dont change the
    //         // index inside of self.cells, we dont need to modify self.grid
    //         let _prev_cell = std::mem::replace(&mut self.cells[prev_cell_idx], cell);
    //     } else {
    //         self.grid[grid_idx] = self.cells.len(); // After pushing, it would be len()-1
    //         self.cells.push(cell);
    //     }
    // }

    // /// Replaces the cell at cellpos with the last cell in self.cells (faster than shifting) and updates self.data
    // pub fn remove_cell_at_pos(&mut self, cellpos: GridPos) {
    //     if self.cells.is_empty() {
    //         debug!("self.cells is empty, cannot remove a cell.");
    //         return;
    //     };
    //     let grid_idx = self.grid_idx(cellpos);
    //     let cell_index = self.grid[grid_idx];
    //     if cell_index == EMPTY_CELL_IDX || self.cells.len() == 1 {
    //         debug!("Skipping remove_cell_from_cells because the targetted cell is the empty cell");
    //         return;
    //     }
    //     self.grid[grid_idx] = 0;
    //     self.set_color_at_grididx_empty(grid_idx);
    //     //self.set_color_at_grididx(grid_idx, self.cells[0].color());
    //     // If our cell is at the back of the cells, then we can remove it normally
    //     if cell_index == self.cells.len() {
    //         self.cells.remove(cell_index);
    //         return;
    //     };
    //     // If not, we need to do a swap remove
    //     if self.get_cell_from_cellidx(cell_index) != &self.cells[EMPTY_CELL_IDX] {
    //         let last_cell = &self.cells[self.cells.len() - 1];
    //         let last_cell_grid_idx = self.grid_idx(last_cell.pos());
    //         self.grid[last_cell_grid_idx] = cell_index;
    //         self.cells.swap_remove(cell_index);
    //     };
    // }
}
