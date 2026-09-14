
struct Cell {
    // needs to exist with that name!
    material: u32,
    velocity: vec2f,
    _pad: f32,
    color: vec4f
}

fn new_cell(material: u32, velocity: vec2f) -> Cell {
    return Cell(material, velocity, 0.1234, get_material_color(material));
}

fn new_empty() -> Cell {
    return new_cell(0, vec2f(0.0));
}

/// Checks for equality between two cells. Remember to update this!
fn eq(a: Cell, b: Cell) -> bool {
    return a.material == b.material && all(a.velocity == b.velocity) && all(a.color == b.color);
}

