const GRID_WIDTH = 128u;
const GRID_HEIGHT = 128u;

const WORKGROUP_SIZE_X: u32 = 8;
const WORKGROUP_SIZE_Y: u32 = 8;

#import cell.wgsl as CellMod;
#import debug_print.wgsl as DebugPrint;

@export struct Params {
    tick: u32,
}

@group(0) @binding(0)
var<storage, read> current_cells: array<CellMod::Cell>;

@group(0) @binding(1)
var<storage, read_write> intents: array<Intent>;

// Best proposal for each destination.
@group(0) @binding(2)
var<storage, read_write> winners: array<atomic<u32>>;

// One entry per source: 1 if its move is accepted.
@group(0) @binding(3)
var<storage, read_write> accepted: array<u32>;

@group(0) @binding(4)
var<storage, read_write> next_cells: array<CellMod::Cell>;

@group(0) @binding(5)
var<uniform> params: Params;

// Each cell can use this buffer to write its next desired state, which will then get copied to next_cells, if that cell won
@group(0) @binding(6)
var<storage, read_write> desired_cells: array<CellMod::Cell>;

@group(1) @binding(0)
var debug_tex0: texture_storage_2d<rgba8unorm, write>;

@group(2) @binding(0)
var display_texture : texture_storage_2d<rgba8unorm, write>;


fn print_value_with_font_size(
    prev_color: vec4f,
    fragCoord: vec2i,
    vPixelCoords: vec2i,
    vFontSize: vec2<f32>,
    fValue: f32,
    // fMaxDigits: f32,
    fDecimalPlaces: u32,
    font_color: vec4f,
) -> vec4f {
    let fMaxDigits = f32(max(0, DebugPrint::digits_before_decimal(fValue) - 1));
    let is_digit = DebugPrint::PrintValue(
        vec2f(fragCoord - vPixelCoords) / vFontSize,
        fValue,
        fMaxDigits,
        f32(fDecimalPlaces),
    );
    return mix(prev_color, font_color, is_digit);
}

// Default font size is optimized at 128x128 so scale it up
const DEFAULT_FONT_SIZE: vec2f = vec2f(5.0 * (f32(GRID_WIDTH) / f32(128.0)), 6.0 * (f32(GRID_HEIGHT) / f32(128.0)));

fn print_value(
    prev_color: vec4f,
    fragCoord: vec2i,
    vPixelCoords: vec2i,
    fValue: f32,
    // fMaxDigits: f32,
    fDecimalPlaces: u32,
    font_color: vec4f,
) -> vec4f {
    return print_value_with_font_size(prev_color, fragCoord, vPixelCoords, DEFAULT_FONT_SIZE, fValue, fDecimalPlaces, font_color);
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

@export struct Intent {
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
    let idx = pos.y * i32(GRID_WIDTH) + pos.x;
    if pos.x >= i32(GRID_WIDTH) || pos.x >= i32(GRID_WIDTH) || idx >= i32(arrayLength(&current_cells)) {
        return IndexResult(u32(idx), false);
    }
    return IndexResult(u32(idx), true);
}

fn idx_to_pos(idx: u32) -> vec2i {
    return vec2i(i32(idx % GRID_WIDTH), i32(idx / GRID_WIDTH));
}

fn idx_from_offset(idx: u32, offset: vec2i) -> IndexResult {
    let pos = idx_to_pos(idx);
    if (offset.x < 0 && pos.x <= 0) || (offset.x > 0 && pos.x >= i32(GRID_WIDTH) - 1) || (offset.y < 0 && pos.y <= 0) || (offset.y > 0 && pos.y >= i32(GRID_HEIGHT) - 1) {
        return IndexResult(idx, false);
    }
    let offset_pos = vec2i(pos) + offset;
    let offset_res = pos_to_idx(offset_pos);
    let offset_idx = offset_res.index;
    if !offset_res.valid {
        return IndexResult(idx, false);
    }
    return IndexResult(offset_idx, true);
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