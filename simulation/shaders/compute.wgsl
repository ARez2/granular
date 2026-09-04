#import shared.wgsl as Shared
#import cell_logic/cell_logic.wgsl as CellLogic
#import cell.wgsl as CellMod;


/// First pass: Prepare
/// Initializes/ Clears all the buffers
@compute @workgroup_size(Shared::WORKGROUP_SIZE_X, Shared::WORKGROUP_SIZE_Y, 1)
fn prepare(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = Shared::pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    Shared::intents[source_idx] = Shared::no_intent();
    atomicStore(&Shared::winners[source_idx], Shared::NO_PROPOSAL);
    Shared::accepted[source_idx] = 0u;

    Shared::desired_cells[source_idx] = Shared::current_cells[source_idx];
    Shared::next_cells[source_idx] = Shared::current_cells[source_idx];

    textureStore(Shared::debug_tex0, gid.xy, vec4f(0.0));
}




/// Second pass: Propose
/// Each cell uses propose_interaction to signify its intent inside of `Shared::intents` (and `Shared::winners`)
@compute @workgroup_size(Shared::WORKGROUP_SIZE_X, Shared::WORKGROUP_SIZE_Y, 1)
fn propose(@builtin(global_invocation_id) gid: vec3u) {
    var idx_res = Shared::pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    let cell = Shared::current_cells[source_idx];
    CellLogic::process_cell(cell, source_idx);
}


/// Third pass: Resolve
/// Each cell reads out who won the claim on that cell (checks `Shared::winners`)
/// and if it won both source and destination, writes a 1 inside of `Shared::accepted`
@compute @workgroup_size(Shared::WORKGROUP_SIZE_X, Shared::WORKGROUP_SIZE_Y, 1)
fn resolve(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = Shared::pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    // grab our own Intent
    let intent = Shared::intents[source_idx];
    if intent.intend_kind == Shared::INTENT_NONE {
        return;
    }

    let destination_idx = intent.destination_idx;
    let encoded_key = intent.encoded_key;

    let source_winner = atomicLoad(&Shared::winners[source_idx]);
    let destination_winner = atomicLoad(
        &Shared::winners[destination_idx]
    );

    // we won both claims, mark 
    if source_winner == encoded_key &&
       destination_winner == encoded_key {
        Shared::accepted[source_idx] = 1u;
    }
}

/// Fourth & final pass: Commit
/// Each winning cell executes its Intent
@compute @workgroup_size(Shared::WORKGROUP_SIZE_X, Shared::WORKGROUP_SIZE_Y, 1)
fn commit(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = Shared::pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    if Shared::accepted[source_idx] == 0u {
        return;
    }

    let intent = Shared::intents[source_idx];
    let destination_idx = intent.destination_idx;

    let prev_source_cell = Shared::current_cells[source_idx];
    let source_cell = Shared::desired_cells[source_idx];
    let destination_cell = Shared::current_cells[destination_idx];

    switch intent.intend_kind {
        case Shared::INTENT_MOVE: {
            Shared::next_cells[source_idx] = CellMod::new_empty();
            Shared::next_cells[destination_idx] = source_cell;
        }

        case Shared::INTENT_SWAP: {
            Shared::next_cells[source_idx] = destination_cell;
            Shared::next_cells[destination_idx] = source_cell;
        }

        case Shared::INTENT_MODIFY_OWN {
            Shared::next_cells[source_idx] = source_cell;
        }

        case Shared::INTENT_MODIFY_OTHER {
            Shared::next_cells[source_idx] = prev_source_cell;
            Shared::next_cells[destination_idx] = source_cell;
        }

        // Maybe INTENT_MODIFY, where the destination gets written by Shared::desired_cells[source_idx]
        // and the source gets written by Shared::current_cells[source_idx]?

        default: {
        }
    }
}


@compute @workgroup_size(Shared::WORKGROUP_SIZE_X, Shared::WORKGROUP_SIZE_Y, 1)
fn display(@builtin(global_invocation_id) gid: vec3u) {
    var idx_res = Shared::pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    let cell = Shared::current_cells[source_idx];
    textureStore(Shared::display_texture, vec2i(gid.xy), cell.color);
}