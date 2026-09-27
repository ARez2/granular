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

    atomicStore(&rb_metadata[source_idx].owner, NO_BODY_CELL);
    // clear debug texture
    write_debug_tex(vec2i(gid.xy), vec4f(0.0));
}


fn rb_cell_world_pos(
    local_pos: vec2i,
    angle_degrees: f32,
    rb_position: vec2f,
) -> vec2i {
    let angle =
        angle_degrees - 360.0 * floor(angle_degrees / 360.0);

    let turns = u32(floor((angle + 45.0) / 90.0));
    let theta = radians(angle - 90.0 * f32(turns));

    var p = local_pos;

    // Zellmittelpunkte um den Körperursprung drehen.
    // Die Ergebnisse sind wieder Zelladressen.
    switch (turns % 4u) {
        case 1u: {
            p = vec2i(-p.y - 1, p.x);
        }
        case 2u: {
            p = -p - vec2i(1);
        }
        case 3u: {
            p = vec2i(p.y, -p.x - 1);
        }
        default: {}
    }

    let a = -tan(theta * 0.5);
    let b = sin(theta);

    // Restrotation: Jede Scherung benutzt den aktuellen
    // Zellmittelpunkt, also die Rasteradresse + 0.5.
    p.x += i32(floor(a * (f32(p.y) + 0.5) + 0.5));
    p.y += i32(floor(b * (f32(p.x) + 0.5) + 0.5));
    p.x += i32(floor(a * (f32(p.y) + 0.5) + 0.5));

    let translation =
        vec2i(floor(rb_position + vec2f(0.5)));

    return p + translation;
}

// "Stamps" the Rigidbodies into the grid (but uses the rb_metadata grid to do atomic claims)
@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn insert_bodies(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }
    let rbcell_idx = source_idx;
    if rbcell_idx >= arrayLength(&rb_cells) {
        return;
    }
    let rbcell = rb_cells[rbcell_idx];
    if (rbcell.flags & RBCELL_FLAG_VALID) == 0u {
        return;
    }

    let rb = rbs[rbcell.rb_index];
    let world_pos = rb_cell_world_pos(rbcell.rb_local_pos, rb.angle_degrees, rb.position);

    let idx = pos_to_idx(world_pos);
    if idx.valid {
        atomicMin(&rb_metadata[idx.index].owner, rbcell_idx);
    }
}


/// Merges together RB cells and other cells, inits them if needed
@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn compose_grid(@builtin(global_invocation_id) gid: vec3u) {
    let idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    var current_cell: Cell;
    let maybecell = cpu_to_gpu_buffer[source_idx];
    let rbcell_idx = atomicLoad(&rb_metadata[source_idx].owner);
    if rbcell_idx != NO_BODY_CELL {
        let rbcell = &rb_cells[rbcell_idx];

        // if the RB material isnt empty, overwrite the world material with the RB material
        if (*rbcell).inner_cell.material != MAT_EMPTY {
            // RBCell needs init
            if ((*rbcell).flags & RBCELL_FLAG_INITIALIZED) == 0u {
                let is_pixelscene_color = ((*rbcell).flags & RBCELL_FLAG_PIXELSCENE_COLOR) != 0u;
                current_cell = user_init_cell((*rbcell).inner_cell, is_pixelscene_color, vec2i(gid.xy), source_idx);
                // set initialized flag to "true"
                (*rbcell).flags |= RBCELL_FLAG_INITIALIZED;
            } else {
                current_cell = (*rbcell).inner_cell;
            }
        } else { // otherwise, adopt the world material into the RB
            current_cell = input_cells[source_idx];
        }
    } else if (maybecell.flags & MAYBECELL_FLAG_IS_SOME) != 0u { // this is the case if the user made some edits in his CPU buffer
        let is_pixelscene_color = (maybecell.flags & MAYBECELL_FLAG_PIXELSCENE_COLOR) != 0u;
        // always init the user edited cells (but respect if the edited cell contains a pixelscene color)
        current_cell = user_init_cell(maybecell.inner_cell, is_pixelscene_color, vec2i(gid.xy), source_idx);
        // Mark this user edit as "handled" (set is_some to false)
        cpu_to_gpu_buffer[source_idx].flags &= ~MAYBECELL_FLAG_IS_SOME;
    } else {
        if params.tick == 0 {
            current_cell = user_init_cell(input_cells[source_idx], false, vec2i(gid.xy), source_idx);
        } else {
            current_cell = input_cells[source_idx];
        }
    }
    current_cells[source_idx] = current_cell;

    desired_cells[source_idx] = current_cell;
    next_cells[source_idx] = current_cell;
}


// Function signature: fn user_process_cell(cell: ptr<function, Cell>, material: Material, cell_pos: vec2i, cell_idx: u32) {}
{{USER_CELL_PROCESS_SHADER}}