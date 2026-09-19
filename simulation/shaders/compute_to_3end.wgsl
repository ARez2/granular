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