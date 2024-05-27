use geese::{GeeseContextHandle, GeeseSystem};


mod grid;
use glam::IVec2;
use grid::CellGrid;
use log::info;
use palette::Srgba;

pub(self) mod cell;
pub(self) mod chunk;
use chunk::Chunk;

pub const GRID_WIDTH: usize = 600;
pub const GRID_HEIGHT: usize = 400;
pub const CHUNK_WIDTH: usize = 50;
pub const CHUNK_HEIGHT: usize = 50;
pub const NUM_CHUNKS: i32 = 8;

pub struct Simulation {
    ctx: GeeseContextHandle<Self>,
    grid: CellGrid,
    chunks: Vec<Chunk>,
    center_position: IVec2,
    center_chunk_pos: IVec2
}
impl Simulation {
    pub(crate) fn get_grid_texture_data(&self) -> &[u8] {
        self.grid.get_texture_data()
    }

    fn add_chunk(&mut self, chunk: Chunk) {
    //fn add_chunk(&mut self, chunk: Chunk) {
        let halfsize = NUM_CHUNKS/2;
        let arr_x = (chunk.position.x + halfsize).rem_euclid(NUM_CHUNKS);
        let arr_y = (chunk.position.y + halfsize).rem_euclid(NUM_CHUNKS);
        info!("  Chunk pos: {:?} at 2D index {},{}", chunk.position, arr_x, arr_y);
        let arr_idx = arr_y as usize * NUM_CHUNKS as usize + arr_x as usize;
        info!("    Currently there: {:?}", self.chunks[arr_idx].position);
        let prev_chunk = &self.chunks[arr_idx];
        if chunk.position != prev_chunk.position {
            self.chunks[arr_idx] = chunk;
            // TODO: Storing/ Loading of new/old chunk
        }
    }

    pub fn set_center_position(&mut self, pos: IVec2) {
        if pos != self.center_position {
            let new_chunk_pos = pos / IVec2::new(CHUNK_WIDTH as i32, CHUNK_HEIGHT as i32);
            let new_max_chunk_pos = new_chunk_pos + IVec2::new(NUM_CHUNKS - 1, NUM_CHUNKS - 1);
            let chunk_pos_diff = new_max_chunk_pos - (self.center_chunk_pos + IVec2::new(NUM_CHUNKS-1, NUM_CHUNKS-1));
            
            if chunk_pos_diff == IVec2::ZERO {
                return;
            }
            info!("Pos: {}     Diff: {}", new_chunk_pos, chunk_pos_diff);
            let old = self.center_chunk_pos;
            let hchunks = NUM_CHUNKS / 2;
            // [1, 0] -> Right Edge   -> IVec2(old.x + NUM_CHUNKS-1, old.y + NUM_CHUNKS-1) to IVec2(old.x + NUM_CHUNKS-1, old.y - NUM_CHUNKS)
            // [0, -1] -> Bottom Edge -> IVec2(old.x - NUM_CHUNKS, old.y - NUM_CHUNKS)     to IVec2(old.x + NUM_CHUNKS-1, old.y - NUM_CHUNKS)
            for new_y in (old.y - hchunks)..(old.y + hchunks) {
                if chunk_pos_diff.x == 1 {
                    self.add_chunk(Chunk { position: IVec2::new(old.x + hchunks, new_y) });
                } else if chunk_pos_diff.x == -1 {
                    self.add_chunk(Chunk { position: IVec2::new(old.x - hchunks - 1, new_y) });
                }
            }
            for new_x in (old.x - hchunks)..(old.x + hchunks) {
                if chunk_pos_diff.y == 1 {
                    self.add_chunk(Chunk { position: IVec2::new(new_x, old.y + hchunks) });
                } else if chunk_pos_diff.y == -1 {
                    self.add_chunk(Chunk { position: IVec2::new(new_x, old.y - hchunks - 1) });
                }
            }
            self.center_chunk_pos = new_chunk_pos;
            self.center_position = pos;
        }
    }
}
impl GeeseSystem for Simulation {
    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let mut grid = CellGrid::empty();
        let mut chunks = vec![];
        for y in -NUM_CHUNKS as i32/2..NUM_CHUNKS/2 {
            for x in -NUM_CHUNKS as i32/2..NUM_CHUNKS/2 {
                chunks.push(Chunk {position: IVec2::new(x, y)});
            }
        }
        Self {
            ctx,
            grid,
            chunks,
            center_position: IVec2::new(0, 0),
            center_chunk_pos: IVec2::new(0, 0)
        }
    }
}