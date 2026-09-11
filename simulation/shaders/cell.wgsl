#import material.wgsl as Mats;

@export struct Cell {
    material: u32,
    velocity: vec2f,
    _pad: f32,
    color: vec4f
}

fn new_cell(material: u32, velocity: vec2f) -> Cell {
    return Cell(material, velocity, 0.1234, Mats::get_material_color(material));
}

fn new_empty() -> Cell {
    return new_cell(Mats::MAT_EMPTY, vec2f(0.0));
}

/// Checks for equality between two cells. Remember to update this!
fn eq(a: Cell, b: Cell) -> bool {
    return a.material == b.material && all(a.velocity == b.velocity) && all(a.color == b.color);
}




/// For some reason, the include macro in Rust complains if there is no entry point
/// but we need Intent from this inside Rust, so we need to add it in Rust
@compute @workgroup_size(1, 1, 1)
fn stub() {
}
