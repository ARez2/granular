struct Material {
    tex_coords_start: vec2f,
    tex_coords_end: vec2f,
    color: vec4f,
    density: f32,
}

const MAT_EMPTY: u32 = 0;
const MAT_SAND: u32 = 1;
const MAT_WATER: u32 = 2;
const MAT_STONE: u32 = 3;
const MAT_RED: u32 = 99;


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

