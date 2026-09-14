{{USER_DEFINITIONS_SHADER}}

#include "shared.wgsl"
#include "cell_logic/actions.wgsl"
#include "cell_logic/cell_logic.wgsl"


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




/// Second pass: Propose
/// Each cell uses propose_interaction to signify its intent inside of `intents` (and `winners`)
@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn propose(@builtin(global_invocation_id) gid: vec3u) {
    var idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    let cell = current_cells[source_idx];
    process_cell(cell, source_idx);
}


/// Third pass: Resolve
/// Each cell reads out who won the claim on that cell (checks `winners`)
/// and if it won both source and destination, writes a 1 inside of `accepted`
@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn resolve(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    // grab our own Intent
    let intent = intents[source_idx];
    if intent.intend_kind == INTENT_NONE {
        return;
    }

    let destination_idx = intent.destination_idx;
    let encoded_key = intent.encoded_key;

    let source_winner = atomicLoad(&winners[source_idx]);
    let destination_winner = atomicLoad(
        &winners[destination_idx]
    );

    // we won both claims, mark 
    if source_winner == encoded_key &&
       destination_winner == encoded_key {
        accepted[source_idx] = 1u;
    }
}

/// Fourth & final pass: Commit
/// Each winning cell executes its Intent
@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn commit(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    if accepted[source_idx] == 0u {
        return;
    }

    let intent = intents[source_idx];
    let destination_idx = intent.destination_idx;

    let prev_source_cell = current_cells[source_idx];
    let source_cell = desired_cells[source_idx];
    let destination_cell = current_cells[destination_idx];

    switch intent.intend_kind {
        case INTENT_MOVE: {
            next_cells[source_idx] = new_empty();
            next_cells[destination_idx] = source_cell;
        }

        case INTENT_SWAP: {
            next_cells[source_idx] = destination_cell;
            next_cells[destination_idx] = source_cell;
        }

        case INTENT_MODIFY_OWN {
            next_cells[source_idx] = source_cell;
        }

        case INTENT_MODIFY_OTHER {
            next_cells[source_idx] = prev_source_cell;
            next_cells[destination_idx] = source_cell;
        }

        // Maybe INTENT_MODIFY, where the destination gets written by desired_cells[source_idx]
        // and the source gets written by current_cells[source_idx]?

        default: {
        }
    }
}


{{USER_DISPLAY_SHADER}}

// User display shader function signature:
// fn user_display(cell: Cell, material: Material, cell_pos: vec2i, cell_index: u32) -> vec4f {}


@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn display(@builtin(global_invocation_id) gid: vec3u) {
    var idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    let cell = current_cells[source_idx];
    let material = materials[cell.material];

    let color = user_display(cell, material, vec2i(gid.xy), source_idx);
    let srgb_color = linear_to_srgb4(color);
    textureStore(display_texture, vec2i(gid.xy), srgb_color);
}

