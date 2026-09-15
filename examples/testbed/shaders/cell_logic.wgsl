/// Available cell actions (from actions.wgsl):
/// - move_to(source_idx: u32, destination_idx: u32)
/// - swap(source_idx: u32, destination_idx: u32)
/// - modify_own(source_idx: u32)
/// - modify_other(source_idx: u32, destination_idx: u32)


// Required function. Gets called at the beginning of every step to init
// cells coming from the CPU
fn user_init_cell(cpu_cell: Cell, cell_pos: vec2i, cell_idx: u32) -> Cell {
    let material = get_material(cpu_cell.material);
    var color: vec4f;
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

    if cell_pos.x >= 65 && cell_pos.x <= 70 && cell_pos.y == 50 && cpu_cell.material == MAT_SAND {
        color = vec4f(1.0, 0.0, 0.0, 1.0);
    }

    return Cell(cpu_cell.material, cpu_cell.velocity, 0.1234, color);
}


// source_idx is the idx of the cell that wants to create the new cell
fn create_cell(source_idx: u32, cell_idx: u32, cell: Cell) -> Cell {
    if source_idx == cell_idx {
        modify_own(source_idx);
    } else {
        modify_other(source_idx, cell_idx);
    }
    return cell;
}

fn is_empty(idx: u32) -> bool {
    return current_cells[idx].material == MAT_EMPTY;
}


fn random_bool(cell_idx: u32) -> bool {
    return (hash_u32(cell_idx ^ params.tick) & 1u) == 0u;
}

fn pos_inside_grid(pos: vec2i) -> bool {
    return all(pos >= vec2i(0, 0)) && all(pos < vec2i(i32(GRID_WIDTH), i32(GRID_HEIGHT)));
}




fn move_or_swap(source_idx: u32, destination_idx: u32) {
    let destination_cell = current_cells[destination_idx];
    if destination_cell.material == MAT_EMPTY {
        move_to(source_idx, destination_idx);
    } else {
        swap(source_idx, destination_idx);
    }
}

fn try_density_move_or_swap(source_idx: u32, destination_idx: u32) -> bool {
    let source = current_cells[source_idx];
    let own_material = materials[source.material];
    let destination = current_cells[destination_idx];
    let destination_material = materials[destination.material];
    if destination_material.density < own_material.density {
        move_or_swap(source_idx, destination_idx);
        return true;
    }
    return false;
}


fn sweep_density(source_idx: u32, start_pos: vec2i, end_pos: vec2i) -> vec2i {
    let source = current_cells[source_idx];
    let own_material = materials[source.material];
    let line = bresenham(start_pos, end_pos);

    var last_valid: vec2i = start_pos;
    for (var i: u32 = 0u; i < line.count; i++) {
        let p = line.points[i];
        if all(p == start_pos) {
            continue;
        }
        if !pos_inside_grid(p) {
            return last_valid;
        }
        let idx_res = pos_to_idx(p);
        if !idx_res.valid {
            return last_valid;
        }
        let dest_idx = idx_res.index;
        let destination = current_cells[dest_idx];
        let destination_material = materials[destination.material];
        if destination_material.density < own_material.density {
            last_valid = p;
        } else {
            break;
        }
    }
    return last_valid;
}


fn process_movable_solid(cell: ptr<function, Cell>, cell_idx: u32) -> bool {
    (*cell).velocity += vec2f(0.0, 2.0);

    let current_pos = idx_to_pos(cell_idx);
    let maybe_idx = pos_to_idx(current_pos).index;


    let velocity_sweeped_pos = sweep_density(cell_idx, current_pos, current_pos + vec2i((*cell).velocity));
    var idx_res = pos_to_idx(velocity_sweeped_pos);
    if !idx_res.valid {
        return false;
    }
    let below_idx = idx_res.index;

    if try_density_move_or_swap(cell_idx, below_idx) {
        return true;
    } else {
        (*cell).velocity.y = 0.0;
    }

    let prefer_downleft = random_bool(cell_idx);
    var directions: array<vec2i, 2>;
    if prefer_downleft {
        directions = array(vec2i(-1, 1), vec2i(1, 1));
    } else {
        directions = array(vec2i(1, 1), vec2i(-1, 1));
    }

    // important: fixed sized array dont support arrayLength for some reason. So this need to match the size!
    for(var i = 0u; i < 2; i++) {
        let dir = directions[i];
        idx_res = idx_from_offset(cell_idx, dir);
        let dir_idx = idx_res.index;
        if !idx_res.valid {
            return false;
        }

        if try_density_move_or_swap(cell_idx, dir_idx) {
            return true;
        }
    }
    return false;
}


fn process_liquid(cell: ptr<function, Cell>, cell_idx: u32) -> bool {
    let prefer_left = random_bool(cell_idx);

    let left_res = idx_from_offset(cell_idx, vec2i(-1, 0));
    let left_idx = left_res.index;
    let right_res = idx_from_offset(cell_idx, vec2i(1, 0));
    let right_idx = right_res.index;
    
    if prefer_left {
        if left_res.valid && try_density_move_or_swap(cell_idx, left_idx) {
            return true;
        }
        if right_res.valid && try_density_move_or_swap(cell_idx, right_idx) {
            return true;
        }
    } else {
        if right_res.valid && try_density_move_or_swap(cell_idx, right_idx) {
            return true;
        }
        if left_res.valid && try_density_move_or_swap(cell_idx, left_idx) {
            return true;
        }
    }
    return false;
}



fn user_process_cell(cell: ptr<function, Cell>, cell_pos: vec2i, cell_idx: u32) {
    var debug_color = vec4f(0.0, 0.0, 0.0, 0.0);
    debug_color = print_value(debug_color, cell_pos, vec2i(0, 5), 12.4, 2, vec4f(1.0, 0.0, 0.0, 1.0));
    textureStore(debug_tex0, cell_pos, debug_color); 

    switch cell.material {
        case MAT_SAND {
            let r = process_movable_solid(cell, cell_idx);
        }
        case MAT_WATER {
            if !process_movable_solid(cell, cell_idx) {
                let r = process_liquid(cell, cell_idx);
            }
        }
        case MAT_EMPTY {
        }
        default {

        }
    }
}
