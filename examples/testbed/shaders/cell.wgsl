
struct Cell {
    // needs to exist with that name!
    material: u32,
    velocity: vec2f,
    _pad: f32,
    color: vec4f
}

fn new_empty() -> Cell {
    return Cell(MAT_EMPTY, vec2f(0.0), 0.1234, vec4f(0.0, 0.0, 0.0, 1.0));
}

/// Checks for equality between two cells. Remember to update this!
fn eq(a: Cell, b: Cell) -> bool {
    return a.material == b.material && all(a.velocity == b.velocity) && all(a.color == b.color);
}

