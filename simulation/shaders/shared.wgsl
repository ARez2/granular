const GRID_WIDTH = {{GRID_WIDTH}};
const GRID_HEIGHT = {{GRID_HEIGHT}};

const WORKGROUP_SIZE_X: u32 = 8;
const WORKGROUP_SIZE_Y: u32 = 8;

#include "debug_print.wgsl"
#include "blend_modes.wgsl"

struct Params {
    tick: u32,
}

@group(0) @binding(0)
var<storage, read> input_cells: array<Cell>;

const MAYBECELL_FLAG_IS_SOME = 1u << 0u;
const MAYBECELL_FLAG_PIXELSCENE_COLOR = 1u << 1u;
// Basically Option<Cell>
struct MaybeCell {
    inner_cell: Cell,
    flags: u32
}
@group(0) @binding(1)
var<storage, read_write> cpu_to_gpu_buffer: array<MaybeCell>;

// Cells after RB's and user edits are inserted
@group(0) @binding(2)
var<storage, read_write> current_cells: array<Cell>;

@group(0) @binding(3)
var<storage, read_write> intents: array<Intent>;

// Best proposal for each destination.
@group(0) @binding(4)
var<storage, read_write> winners: array<atomic<u32>>;

// One entry per source: 1 if its move is accepted.
@group(0) @binding(5)
var<storage, read_write> accepted: array<u32>;

@group(0) @binding(6)
var<storage, read_write> next_cells: array<Cell>;

@group(0) @binding(7)
var<uniform> params: Params;

// Each cell can use this buffer to write its next desired state, which will then get copied to next_cells, if that cell won
@group(0) @binding(8)
var<storage, read_write> desired_cells: array<Cell>;

/// In WGSL, -1 % 8 = -1  (and not 7), so this function does what you'd expect
fn rem_euclid(value: i32, period: i32) -> i32 {
    return ((value % period) + period) % period;
}

fn y_up_to_texel(pos: vec2i, height: i32) -> vec2i {
    return vec2i(pos.x, height - 1 - pos.y);
}

// Converts from the +Y Up the engine uses back to +Y Down of textures
fn simcoord_to_texel(pos: vec2i) -> vec2i {
    return y_up_to_texel(pos, i32(GRID_HEIGHT));
}


const RBCELL_FLAG_INITIALIZED = 1u << 0u;
const RBCELL_FLAG_VALID = 1u << 1u;
const RBCELL_FLAG_PIXELSCENE_COLOR = 1u << 2u;
struct RBCell {
    inner_cell: Cell,
    rb_local_pos: vec2i,
    rb_index: u32,
    flags: u32,
}

@group(0) @binding(9)
var<storage, read_write> rb_cells: array<RBCell>;

struct RB {
    // Position (in grid units)
    position: vec2f,
    angle_degrees: f32,
    rbcells_start: u32,
    rbcells_end: u32,
}
@group(0) @binding(10)
var<storage, read_write> rbs: array<RB>;

const NO_BODY_CELL: u32 = 0xffffffffu;
struct RBWorldMetadata {
    // this is the RBCell which owns this world pos
    owner: atomic<u32>,
}
@group(0) @binding(11)
var<storage, read_write> rb_metadata: array<RBWorldMetadata>;



@group(1) @binding(0)
var display_texture : texture_storage_2d<rgba8unorm, write>;

@group(1) @binding(1)
var<storage, read_write> materials: array<Material>;


@group(4) @binding(0)
var debug_tex0: texture_storage_2d<rgba8unorm, write>;

fn print_value_with_font_size(
    fragCoord: vec2i,
    vPixelCoords: vec2i,
    vFontSize: vec2<f32>,
    fValue: f32,
    // fMaxDigits: f32,
    fDecimalPlaces: u32,
    font_color: vec4f,
) {
    var default_color = vec4f(0.0, 0.0, 0.0, 0.0);

    let fMaxDigits = f32(max(0, digits_before_decimal(fValue) - 1));
    let is_digit = PrintValue(
        vec2f(fragCoord - vPixelCoords) / vFontSize,
        fValue,
        fMaxDigits,
        f32(fDecimalPlaces),
    );
    if is_digit > 0.5 {
        let output_col = mix(default_color, font_color, is_digit);
        textureStore(debug_tex0, simcoord_to_texel(fragCoord), output_col);
    }
}

// Default font size is optimized at 128x128 so scale it up
const DEFAULT_FONT_SIZE: vec2f = vec2f(5.0 * (f32(GRID_WIDTH) / f32(128.0)), 6.0 * (f32(GRID_HEIGHT) / f32(128.0)));

fn print_value(
    fragCoord: vec2i,
    vPixelCoords: vec2i,
    fValue: f32,
    // fMaxDigits: f32,
    fDecimalPlaces: u32,
    font_color: vec4f,
) {
    print_value_with_font_size(fragCoord, vPixelCoords, DEFAULT_FONT_SIZE, fValue, fDecimalPlaces, font_color);
}



const MAX_LINE_POINTS: u32 = u32(ceil(sqrt(f32(GRID_WIDTH)*f32(GRID_WIDTH) + f32(GRID_HEIGHT) * f32(GRID_HEIGHT))));
struct LineResult {
    count: u32,
    points: array<vec2i, MAX_LINE_POINTS>,
};
fn bresenham(start: vec2i, end: vec2i) -> LineResult {
    var result: LineResult;
    result.count = 0u;
    
    var x = start.x;
    var y = start.y;
    
    let dx = abs(end.x - start.x);
    let dy = abs(end.y - start.y);
    
    let sx = select(-1, 1, start.x < end.x);
    let sy = select(-1, 1, start.y < end.y);
    
    var err = dx - dy;
    
    loop {
        // Add current point if we haven't exceeded max capacity
        if (result.count < MAX_LINE_POINTS) {
            result.points[result.count] = vec2i(x, y);
            result.count += 1u;
        } else {
            break;
        }
        
        // Check if we've reached the endpoint
        if (x == end.x && y == end.y) {
            break;
        }
        
        // Calculate error and update coordinates
        let e2 = 2 * err;
        
        if (e2 > -dy) {
            err = err - dy;
            x = x + sx;
        }
        
        if (e2 < dx) {
            err = err + dx;
            y = y + sy;
        }
    }
    
    return result;
}


fn hash_u32(value: u32) -> u32 {
    var x = value;
    x ^= x >> 16u;
    x *= 0x7feb352du;
    x ^= x >> 15u;
    x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}


const NO_PROPOSAL: u32 = 0xffffffff;

const INTENT_NONE: u32 = 0;
const INTENT_MOVE: u32 = 1;
const INTENT_SWAP: u32 = 2;
const INTENT_MODIFY_OWN: u32 = 3;
const INTENT_MODIFY_OTHER: u32 = 4;

struct Intent {
    // Index, which this Intent targets
    destination_idx: u32,
    encoded_key: u32,
    // One of the INTENT_*
    intend_kind: u32,
    _padding: u32,
}

fn no_intent() -> Intent {
    return Intent(
        NO_PROPOSAL,
        NO_PROPOSAL,
        INTENT_NONE,
        0u,
    );
}


struct IndexResult {
    index: u32,
    valid: bool
}

fn pos_to_idx(pos: vec2i) -> IndexResult {
    if pos.x < 0 || pos.y < 0 ||
       pos.x >= i32(GRID_WIDTH) ||
       pos.y >= i32(GRID_HEIGHT) {
        return IndexResult(0u, false);
    }

    let idx = u32(pos.y) * GRID_WIDTH + u32(pos.x);
    if idx >= arrayLength(&current_cells) {
        return IndexResult(0u, false);
    }

    return IndexResult(idx, true);
}

fn idx_from_offset(idx: u32, offset: vec2i) -> IndexResult {
    return pos_to_idx(idx_to_pos(idx) + offset);
}

fn idx_to_pos(idx: u32) -> vec2i {
    return vec2i(i32(idx % GRID_WIDTH), i32(idx / GRID_WIDTH));
}


// Priority occupies the upper 4 bits.
// The encoded source occupies the lower 28 bits.
const SOURCE_BITS: u32 = 28;
const SOURCE_MASK: u32 = 0x0fffffff;

fn tie_seed() -> u32 {
    return hash_u32(params.tick) & SOURCE_MASK;
}

/// Uses source_idx and priority to create a u32 which gets stored in `Intent.encoded_key`
fn encode_proposal(priority: u32, source_idx: u32) -> u32 {
    // XOR is reversible, allowing the source_idx to be recovered later.
    let encoded_source = (source_idx ^ tie_seed()) & SOURCE_MASK;
    return (priority << SOURCE_BITS) | encoded_source;
}

fn decode_source(proposal_key: u32) -> u32 {
    let encoded_source = proposal_key & SOURCE_MASK;
    return encoded_source ^ tie_seed();
}


fn is_body_cell(idx: u32) -> bool {
    return atomicLoad(&rb_metadata[idx].owner) != NO_BODY_CELL;
}

/// Saves a Intent struct in this cell's slot in `intents`.
/// Also writes the key into `winners` via atomicMin
fn propose_interaction(
    source_idx: u32,
    destination_idx: u32,
    intend_kind: u32,
    priority: u32,
) {
    let encoded_key = encode_proposal(priority, source_idx);

    // Only the source_idx invocation writes intents[source_idx].
    intents[source_idx] = Intent(
        destination_idx,
        encoded_key,
        intend_kind,
        0u,
    );

    // The interaction must win ownership of both cells (happens in resolve)
    atomicMin(&winners[source_idx], encoded_key);
    atomicMin(&winners[destination_idx], encoded_key);
}


fn linear_to_srgb(x: f32) -> f32 {
    if x <= 0.0031308 {
        return 12.92 * x;
    }

    return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}

fn linear_to_srgb4(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        linear_to_srgb(c.r),
        linear_to_srgb(c.g),
        linear_to_srgb(c.b),
        c.a
    );
}


fn map_rangef(val: f32, input_start: f32, input_end: f32, output_start: f32, output_end: f32) -> f32 {
    let slope = (output_end - output_start) / (input_end - input_start);
    return output_start + slope * (val - input_start);
}

fn map_rangei(val: i32, input_start: i32, input_end: i32, output_start: i32, output_end: i32) -> i32 {
    return i32(map_rangef(f32(val), f32(input_start), f32(input_end), f32(output_start), f32(output_end)));
}

fn map_rangeu(val: u32, input_start: u32, input_end: u32, output_start: u32, output_end: u32) -> u32 {
    return u32(map_rangef(f32(val), f32(input_start), f32(input_end), f32(output_start), f32(output_end)));
}

/// Use this to access materials
fn get_material(material_idx: u32) -> Material {
    return materials[material_idx];
}