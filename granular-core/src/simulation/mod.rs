use geese::{event_handlers, EventHandlers, GeeseContextHandle, GeeseSystem};
use glam::IVec2;
use grid::CellGrid;
use log::{debug, error, info};
use palette::Srgba;
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use std::{
    cell::UnsafeCell,
    time::{Duration, Instant},
};

pub mod cell;
pub mod chunk;
mod grid;
use chunk::Chunk;
pub mod material;
use crate::{
    cell::Cell,
    chunk::{CHUNK_SIZE, HALO_DIRECTIONS},
    simulation::grid::GridPos,
};
use material::Material;

pub const NUM_CHUNKS_X: usize = 8;
pub const NUM_CHUNKS_Y: usize = 8;
pub const NUM_CHUNKS_TOTAL: usize = NUM_CHUNKS_X * NUM_CHUNKS_Y;

pub struct Simulation {
    ctx: GeeseContextHandle<Self>,
    chunks: [Chunk; NUM_CHUNKS_TOTAL],
    center_position: IVec2,
    center_chunk_pos: IVec2,
    pub tick: usize,
}
impl Simulation {
    pub fn setup(&mut self, _: &crate::events::Initialized) {
        for chunk_idx in 0..self.chunks.len() {
            let chunk_pos = self.chunks[chunk_idx].position;
            for (neigh_idx, neigh_dir) in HALO_DIRECTIONS.iter().enumerate() {
                // Calculate the position of the neighbor based on the halo direction
                let neigh_pos = chunk_pos + IVec2::from(*neigh_dir);
                // Calculate the index inside of chunks for the neighbor based on its position
                let neigh_idx_in_chunks = Self::get_idx_in_chunks(neigh_pos);
                // Compare if the chunk in chunks at that index actually has the prev. calculated pos
                if self.chunks[neigh_idx_in_chunks].position == neigh_pos {
                    self.chunks[chunk_idx].halo[neigh_idx] =
                        Some(self.chunks[neigh_idx_in_chunks].grid.get());
                }
            }
        }
    }

    pub fn get_chunks(&self) -> &[Chunk; NUM_CHUNKS_TOTAL] {
        &self.chunks
    }

    fn get_idx_in_chunks(chunk_pos: IVec2) -> usize {
        // pos.x = 0 -> arr_x = (0 + 4) % 4 = 0
        // pos.x = 4 -> arr_x = (4 + 4) % 4 = 0
        // pos.x = 5 -> arr_x = (5 + 4) % 4 = 1
        let arr_x = (chunk_pos.x + (NUM_CHUNKS_X as i32 / 2)).rem_euclid(NUM_CHUNKS_X as i32);
        let arr_y = (chunk_pos.y + (NUM_CHUNKS_Y as i32 / 2)).rem_euclid(NUM_CHUNKS_Y as i32);
        // info!(
        //     "  Chunk pos: {:?} at 2D index {},{}",
        //     chunk_pos, arr_x, arr_y
        // );
        arr_y as usize * NUM_CHUNKS_X + arr_x as usize
    }

    fn add_chunk(&mut self, chunk: Chunk) {
        return;
        let arr_idx = Self::get_idx_in_chunks(chunk.position);
        info!("    Currently there: {:?}", self.chunks[arr_idx].position);
        let prev_chunk = &self.chunks[arr_idx];
        if chunk.position != prev_chunk.position {
            self.chunks[arr_idx] = chunk;
            // TODO: Storing/ Loading of new/old chunk
        }
    }

    /// Taking the center of the screen in global pixel coordinates, updates the simulation to
    /// load/ unload chunks on the edges depending on where the center of the simulation moved
    pub fn set_center_position(&mut self, pos: IVec2) {
        // if pos != self.center_position {
        //     let new_chunk_pos = pos / IVec2::new(CHUNK_SIZE as i32, CHUNK_SIZE as i32);
        //     let new_max_chunk_pos =
        //         new_chunk_pos + IVec2::new(NUM_CHUNKS_X as i32 - 1, NUM_CHUNKS_Y as i32 - 1);
        //     let chunk_pos_diff = new_max_chunk_pos
        //         - (self.center_chunk_pos
        //             + IVec2::new(NUM_CHUNKS_X as i32 - 1, NUM_CHUNKS_Y as i32 - 1));

        //     if chunk_pos_diff == IVec2::ZERO {
        //         return;
        //     }
        //     info!("Pos: {}     Diff: {}", new_chunk_pos, chunk_pos_diff);
        //     let old = self.center_chunk_pos;
        //     let hchunks = NUM_CHUNKS_X as i32 / 2;
        //     // [1, 0] -> Right Edge   -> IVec2(old.x + NUM_CHUNKS-1, old.y + NUM_CHUNKS-1) to IVec2(old.x + NUM_CHUNKS-1, old.y - NUM_CHUNKS)
        //     // [0, -1] -> Bottom Edge -> IVec2(old.x - NUM_CHUNKS, old.y - NUM_CHUNKS)     to IVec2(old.x + NUM_CHUNKS-1, old.y - NUM_CHUNKS)
        //     for new_y in (old.y - hchunks)..(old.y + hchunks) {
        //         if chunk_pos_diff.x == 1 {
        //             self.add_chunk(Chunk {
        //                 position: IVec2::new(old.x + hchunks, new_y),
        //             });
        //         } else if chunk_pos_diff.x == -1 {
        //             self.add_chunk(Chunk {
        //                 position: IVec2::new(old.x - hchunks - 1, new_y),
        //             });
        //         }
        //     }
        //     for new_x in (old.x - hchunks)..(old.x + hchunks) {
        //         if chunk_pos_diff.y == 1 {
        //             self.add_chunk(Chunk {
        //                 position: IVec2::new(new_x, old.y + hchunks),
        //             });
        //         } else if chunk_pos_diff.y == -1 {
        //             self.add_chunk(Chunk {
        //                 position: IVec2::new(new_x, old.y - hchunks - 1),
        //             });
        //         }
        //     }
        //     self.center_chunk_pos = new_chunk_pos;
        //     self.center_position = pos;
        // }
    }

    pub const DEBUG_UPDATE: bool = false;
    pub fn update(&mut self, _: &crate::events::timing::FixedTick<16>) {
        self.tick += 1;
        if Self::DEBUG_UPDATE {
            let phase = (self.tick / 16) % 4;
            self.chunks.par_iter_mut().for_each(|chunk| {
                let should_update = chunk.should_update(phase as u8);
                if should_update {
                    chunk.update(self.tick);
                }
            });
        } else {
            #[allow(clippy::collapsible_else_if)]
            if self.tick > 60 {
                for phase in 0..4 {
                    //let start = Instant::now();
                    self.chunks.par_iter_mut().for_each(|chunk| {
                        let should_update = chunk.should_update(phase);
                        if should_update {
                            chunk.update(self.tick);
                        }
                    });
                    //aggregated_time += start.elapsed();
                }
            }
        }
    }
}
impl GeeseSystem for Simulation {
    const EVENT_HANDLERS: EventHandlers<Self> =
        event_handlers().with(Self::update).with(Self::setup);

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut chunks: [Chunk; NUM_CHUNKS_TOTAL] = core::array::from_fn(|i| Chunk {
            grid: UnsafeCell::new(CellGrid::empty()),
            position: IVec2::ZERO,
            halo: [None; 8],
        });

        let mut idx = 0;
        for y in -(NUM_CHUNKS_Y as i32) / 2..NUM_CHUNKS_Y as i32 / 2 {
            for x in -(NUM_CHUNKS_X as i32) / 2..NUM_CHUNKS_X as i32 / 2 {
                let position = IVec2::new(x, y);
                let mat = if y == 0 {
                    Material::Sand
                } else {
                    Material::Empty
                };
                let grid = CellGrid::new_material(mat);
                chunks[idx] = Chunk {
                    grid: UnsafeCell::new(grid),
                    position,
                    halo: [None; 8],
                };
                idx += 1;
            }
        }

        Self {
            ctx,
            chunks,
            center_position: IVec2::new(0, 0),
            center_chunk_pos: IVec2::new(0, 0),
            tick: 0,
        }
    }
}
