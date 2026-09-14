
// must have this name & signature
fn user_display(cell: Cell, material: Material, cell_pos: vec2i, cell_index: u32) -> vec4f {
    var color = cell.color;
    if any(material.tex_coords_start != material.tex_coords_end) {
        let atlas_size = vec2i(textureDimensions(material_texture_atlas));
        let atlas_pos_start = vec2i(material.tex_coords_start * vec2f(atlas_size));
        let atlas_pos_end = vec2i(material.tex_coords_end * vec2f(atlas_size));
        let mat_tex_size = atlas_pos_end - atlas_pos_start;
        let texture_sample_pos =
            atlas_pos_start + vec2i(
                cell_pos.x % mat_tex_size.x,
                cell_pos.y % mat_tex_size.y
            );
        // textureSample is forbidden
        color = textureLoad(
            material_texture_atlas,
            texture_sample_pos,
            0
        );
    } else {
        color = material.color;
    }

    return color;
}

