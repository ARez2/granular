use crate::CellStruct;
use encase::ShaderType;
use glam::prelude::*;

#[derive(ShaderType)]
pub(super) struct Params {
    pub(super) tick: u32,
}

#[derive(ShaderType)]
pub(super) struct Intent {
    pub(super) destination_index: u32,
    pub(super) encoded_key: u32,
    pub(super) intent_kind: u32,
    pub(super) _padding: u32,
}

pub(super) const MAYBECELL_FLAG_IS_SOME: u32 = 1u32 << 0u32;
#[allow(unused)]
pub(super) const MAYBECELL_FLAG_PIXELSCENE_COLOR: u32 = 1u32 << 1u32;
#[derive(Debug, Clone, Copy, encase::ShaderType)]
pub(super) struct MaybeCell<C: CellStruct> {
    pub(super) inner_cell: C,
    pub(super) flags: u32,
}

#[allow(unused)]
pub(super) const RBCELL_FLAG_INITIALIZED: u32 = 1u32 << 0u32;
pub(super) const RBCELL_FLAG_VALID: u32 = 1u32 << 1u32;
pub(super) const RBCELL_FLAG_PIXELSCENE_COLOR: u32 = 1u32 << 2u32;
#[derive(Debug, Clone, Copy, encase::ShaderType)]
pub(super) struct RBCell<C: CellStruct> {
    pub(super) inner_cell: C,
    pub(super) rb_local_pos: IVec2,
    pub(super) rb_index: u32,
    pub(super) flags: u32,
}
impl<C: CellStruct> Default for RBCell<C> {
    fn default() -> Self {
        Self {
            inner_cell: C::default(),
            rb_local_pos: IVec2::ZERO,
            rb_index: 0,
            flags: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, encase::ShaderType)]
pub(super) struct RB {
    pub(super) position: Vec2,
    pub(super) angle_degrees: f32,
    pub(super) rbcells_start: u32,
    pub(super) rbcells_end: u32,
}

#[derive(Debug, Clone, Copy, encase::ShaderType, Default)]
pub(super) struct RBWorldMetadata {
    pub(super) owner: u32,
}
