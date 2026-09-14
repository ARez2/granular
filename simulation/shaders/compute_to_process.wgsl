{{USER_DEFINITIONS_SHADER}}

#include "shared.wgsl"
#include "actions.wgsl"


/// First pass: Prepare
/// Initializes/ Clears all the buffers
@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn prepare(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    intents[source_idx] = no_intent();
    atomicStore(&winners[source_idx], NO_PROPOSAL);
    accepted[source_idx] = 0u;

    desired_cells[source_idx] = current_cells[source_idx];
    next_cells[source_idx] = current_cells[source_idx];

    textureStore(debug_tex0, gid.xy, vec4f(0.0));
}

// Function signature: fn user_process_cell(cell: ptr<function, Cell>, material: Material, cell_pos: vec2i, cell_idx: u32) {}
{{USER_CELL_PROCESS_SHADER}}