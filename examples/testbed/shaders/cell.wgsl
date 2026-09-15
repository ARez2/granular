// needs to be group(2) for now
@group(2) @binding(1)
var material_texture_atlas: texture_2d<f32>;
@group(2) @binding(2)
var material_texture_atlas_sampler: sampler;

struct Cell {
    // needs to exist with that name!
    // this is just the "name" of the material.
    // To get the "Material" struct, call "get_material" with this number
    // To check if some cell "is" sand for example, use cell.material == MAT_SAND
    material: u32,
    velocity: vec2f,
    _pad: f32,
    color: vec4f
}

/// Still required for now. However when calling this, the caller will also call user_init_cell
/// with that new empty cell so that it shouldnt be an issue
fn new_empty() -> Cell {
    return Cell(MAT_EMPTY, vec2f(0.0), 0.1234, vec4f(0.0, 0.0, 0.0, 1.0));
}

/// Checks for equality between two cells. Remember to update this!
fn eq(a: Cell, b: Cell) -> bool {
    return a.material == b.material && all(a.velocity == b.velocity) && all(a.color == b.color);
}

