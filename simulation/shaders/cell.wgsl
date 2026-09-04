const MAT_EMPTY: u32 = 0;
const MAT_SAND: u32 = 1;
const MAT_WATER: u32 = 2;
const MAT_STONE: u32 = 3;
const MAT_RED: u32 = 99;

@export struct Cell {
    material: u32,
    velocity: vec2f,
    _pad: f32,
    color: vec4f
}

fn new_cell(material: u32, velocity: vec2f) -> Cell {
    return Cell(material, velocity, 0.1234, get_material_color(material));
}

fn new_empty() -> Cell {
    return new_cell(MAT_EMPTY, vec2f(0.0));
}

/// Checks for equality between two cells. Remember to update this!
fn eq(a: Cell, b: Cell) -> bool {
    return a.material == b.material && all(a.velocity == b.velocity) && all(a.color == b.color);
}

fn get_density(material: u32) -> u32 {
    switch material {
        case MAT_EMPTY: {
            return 0u;
        }
        case MAT_WATER: {
            return 1u;
        }
        case MAT_SAND: {
            return 2u;
        }
        case MAT_STONE: {
            return 10u;
        }
        default {
            return 2u;
        }
    }
}

fn get_material_color(material: u32) -> vec4f {
    switch material {
        case MAT_EMPTY: {
            return vec4f(0.0, 0.0, 0.0, 1.0);
        }
        case MAT_WATER: {
            return vec4f(0.0, 0.0, 1.0, 1.0);
        }
        case MAT_SAND: {
            return vec4f(1.0, 1.0, 0.0, 1.0);
        }
        case MAT_STONE: {
            return vec4f(0.2, 0.2, 0.2, 1.0);
        }
        case MAT_RED: {
            return vec4f(1.0, 0.0, 0.0, 1.0);
        }
        default {
            return vec4f(1.0, 0.0, 1.0, 1.0);
        }
    }
    return vec4f(1.0, 0.0, 1.0, 1.0);
}


/// For some reason, the include macro in Rust complains if there is no entry point
/// but we need Intent from this inside Rust, so we need to add it in Rust
@compute @workgroup_size(1, 1, 1)
fn stub() {
}
