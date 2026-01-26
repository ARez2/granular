use glam::IVec2;
use std::{cell::UnsafeCell, ops::Range};

use crate::{
    material::Material,
    simulation::grid::{CellGrid, GridPos},
};

pub const CHUNK_SIZE: u16 = 32;
pub const NUM_CELLS_IN_CHUNK: usize = CHUNK_SIZE as usize * CHUNK_SIZE as usize;
pub const HALO_DIRECTIONS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

#[derive(Debug)]
pub struct Chunk {
    pub position: IVec2,
    pub(super) grid: UnsafeCell<CellGrid>,
    pub(super) halo: [Option<*mut CellGrid>; 8],
}
unsafe impl Send for Chunk {}
unsafe impl Sync for Chunk {}
impl Chunk {
    pub fn get_grid(&self) -> &CellGrid {
        unsafe { &(*self.grid.get()) }
    }

    #[allow(clippy::mut_from_ref)] // I think this is unavoidable if we use interior mutability (UnsafeCell)
    fn get_grid_mut(&self) -> &mut CellGrid {
        unsafe { &mut (*self.grid.get()) }
    }

    fn update_range(&self, range: Range<i16>, reverse: bool) -> Box<dyn Iterator<Item = i16>> {
        if reverse {
            Box::new(range.rev())
        } else {
            Box::new(range)
        }
    }

    /// Updates all cells inside this chunk.
    pub fn update(&mut self, tick: usize) {
        let own_grid = self.get_grid_mut();
        for y in self.update_range(0..CHUNK_SIZE as i16, tick.is_multiple_of(2)) {
            for x in self.update_range(0..CHUNK_SIZE as i16, tick.is_multiple_of(4)) {
                let pos = GridPos::new(x, y);
                let cell = own_grid.get_cell(pos);
                let own_material = cell.material;
                if cell.last_processed_in_tick == tick {
                    continue;
                }

                // TODO: Enforce only moving CHUNK_SIZE/2

                if own_material == Material::Sand {
                    let below_pos = pos + GridPos::Y;
                    // Component is 0, when inside this chunk, -1 or 1 when outside this chunk
                    let dir_hint = IVec2::new(
                        (below_pos.x >= CHUNK_SIZE as i16) as i32 - (below_pos.x < 0) as i32,
                        (below_pos.y >= CHUNK_SIZE as i16) as i32 - (below_pos.y < 0) as i32,
                    );

                    let below_grid_opt = self.get_grid_from_dir(dir_hint);
                    if let Some(below_grid) = below_grid_opt {
                        let below_pos = below_pos
                            .rem_euclid(GridPos::new(CHUNK_SIZE as i16, CHUNK_SIZE as i16));

                        let below_mat = below_grid.get_cell(below_pos).material;
                        if below_mat == Material::Empty {
                            let cell_mut = own_grid.get_cell_mut(pos);
                            cell_mut.last_processed_in_tick = tick;
                            std::mem::swap(cell_mut, below_grid.get_cell_mut(below_pos));
                            own_grid.set_color(pos, &below_mat.color());
                            below_grid.set_color(below_pos, &own_material.color());
                        }
                    }
                }
            }
        }
    }

    /// Given a directional hint (meaning which in which direction in chunk-space
    /// the sample will take place), pick the halo grid corresponding to that direction,
    /// or our own grid, if the direction is (0, 0)
    #[allow(clippy::mut_from_ref)] // I think this is unavoidable if we use interior mutability (UnsafeCell)
    fn get_grid_from_dir(&self, dir_hint: IVec2) -> Option<&mut CellGrid> {
        let hint: (i32, i32) = dir_hint.into();
        if hint == (0, 0) {
            return Some(self.get_grid_mut());
        }
        for (i, dir) in HALO_DIRECTIONS.iter().enumerate() {
            if *dir == hint {
                if let Some(grid) = self.halo[i] {
                    return Some(unsafe { &mut (*grid) });
                }
                return None;
            }
        }
        eprintln!("Got {hint:?}. This shouldn't be possible!");
        unimplemented!()
    }

    // The simulation is updated in a checkerboard pattern like in Noita, and each update is
    // split into multiple phases (each checkerboard pattern is one phase). This function
    // returns true if this chunk should update itself in the current phase.
    pub fn should_update(&self, phase: u8) -> bool {
        ((self.position.x.abs() & 1) | ((self.position.y.abs() & 1) << 1)) == phase as i32
    }

    pub fn get_texture_data(&self) -> &[u8] {
        self.get_grid().get_texture_data()
    }
}
