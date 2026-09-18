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
    textureStore(display_texture, vec2i(gid.xy), srgb_color);
}

