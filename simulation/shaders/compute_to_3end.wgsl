@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn display(@builtin(global_invocation_id) gid: vec3u) {
    var idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    let cell = next_cells[source_idx];

    let color = user_display(cell, vec2i(gid.xy), source_idx);
    let srgb_color = linear_to_srgb4(color);
    textureStore(display_texture, simcoord_to_texel(vec2i(gid.xy)), srgb_color);
}


@compute @workgroup_size(WORKGROUP_SIZE_X, WORKGROUP_SIZE_Y, 1)
fn extract_bodies(@builtin(global_invocation_id) gid: vec3u) {
    var idx_res = pos_to_idx(vec2i(gid.xy));
    let source_idx = idx_res.index;
    if !idx_res.valid {
        return;
    }

    let rbcell_idx = atomicLoad(&rb_metadata[source_idx].owner);
    if rbcell_idx != NO_BODY_CELL {
        // This automatically handles the following cases:
        // - cell moves inside of Body A
        //    => rb_cells[rbcell_idx] receives the moving cell (and the empty cell at the original position will
        //       be rb_cells[rbcell_idx] for another invocation, so it will also get stored inside of A)
        // - cell moves from Body A to the free world
        //    => rb_cells[rbcell_idx] will be the empty cell left behind, and next_cells contains the moved cell
        // - cell moves from Body A to Body B
        //    => if rb_cells[rbcell_idx] lies in the stamp of Body, then that RBCell will be part of Body B and adopt the cell
        rb_cells[rbcell_idx].inner_cell = next_cells[source_idx];

        next_cells[source_idx] = user_init_cell(
            new_empty(),
            false,
            vec2i(gid.xy),
            source_idx,
        );
    }
}


@compute @workgroup_size(64, 1, 1)
fn create_collision(@builtin(global_invocation_id) gid: vec3u) {
    let word_idx = gid.x;

    // Both arrays currently have NUM_COLLISION_INTEGERS entries.
    if word_idx >= NUM_COLLISION_INTEGERS {
        return;
    }

    var world_bits = 0u;
    var rb_bits = 0u;
    for (var bit = 0u; bit < 32u; bit++) {
        let cell_idx = word_idx * 32u + bit;

        // World occupancy, after body extraction.
        if cell_idx < GRID_WIDTH * GRID_HEIGHT {
            let has_moved = !eq(input_cells[cell_idx], next_cells[cell_idx]);
            if matname_has_collision(next_cells[cell_idx].material) && !has_moved {
                world_bits |= 1u << bit;
            }
        }

        // Independent index into the contiguous RB-cell pool.
        if cell_idx < arrayLength(&rb_cells) {
            let cell = rb_cells[cell_idx];
            let valid = (cell.flags & RBCELL_FLAG_VALID) != 0u;

            if valid && matname_has_collision(cell.inner_cell.material) {
                rb_bits |= 1u << bit;
            }
        }
    }

    collision_data.world_collision[word_idx] = world_bits;
    collision_data.rb_collision[word_idx] = rb_bits;
}
