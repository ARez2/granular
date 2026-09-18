fn process_cell(cell: Cell, cell_pos: vec2i, cell_idx: u32) {
    var local_cell = cell;

    user_process_cell(&local_cell, cell_pos, cell_idx);

    // If this cell has proposed no other intent and it modified the local_cell,
    // make sure that modification gets registered
    if !eq(local_cell, cell) && intents[cell_idx].intend_kind == INTENT_NONE {
        modify_own(cell_idx);
    }
    desired_cells[cell_idx] = local_cell;
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
    process_cell(cell, vec2i(gid.xy), source_idx);
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
            next_cells[source_idx] = user_init_cell(new_empty(), vec2i(gid.xy), source_idx);
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

        default: {
        }
    }
}


// User display shader function signature:
// fn user_display(cell: Cell, material: Material, cell_pos: vec2i, cell_index: u32) -> vec4f {}
{{USER_DISPLAY_SHADER}}
