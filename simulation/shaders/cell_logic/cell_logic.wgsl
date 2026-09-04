#import ../shared.wgsl as Shared
#import ../cell.wgsl as CellMod;
#import actions.wgsl::{move_to, swap, modify_own, modify_other}




// source_idx is the idx of the cell that wants to create the new cell
fn create_cell(source_idx: u32, cell_idx: u32, cell: CellMod::Cell) -> CellMod::Cell {
    if source_idx == cell_idx {
        modify_own(source_idx);
    } else {
        modify_other(source_idx, cell_idx);
    }
    return cell;
}

fn is_empty(idx: u32) -> bool {
    return Shared::current_cells[idx].material == CellMod::MAT_EMPTY;
}


fn random_bool(cell_idx: u32) -> bool {
    return (Shared::hash_u32(cell_idx ^ Shared::params.tick) & 1u) == 0u;
}

fn pos_inside_grid(pos: vec2i) -> bool {
    return all(pos >= vec2i(0, 0)) && all(pos < vec2i(i32(Shared::GRID_WIDTH), i32(Shared::GRID_HEIGHT)));
}




fn move_or_swap(source_idx: u32, destination_idx: u32) {
    let destination_cell = Shared::current_cells[destination_idx];
    if destination_cell.material == CellMod::MAT_EMPTY {
        move_to(source_idx, destination_idx);
    } else {
        swap(source_idx, destination_idx);
    }
}

fn try_density_move_or_swap(source_idx: u32, destination_idx: u32) -> bool {
    let own_density = CellMod::get_density(Shared::current_cells[source_idx].material);
    let destination = Shared::current_cells[destination_idx];
    let destination_density = CellMod::get_density(destination.material);
    if destination_density < own_density {
        move_or_swap(source_idx, destination_idx);
        return true;
    }
    return false;
}


fn sweep_density(source_idx: u32, start_pos: vec2i, end_pos: vec2i) -> vec2i {
    let own_density = CellMod::get_density(Shared::current_cells[source_idx].material);
    let line = Shared::bresenham(start_pos, end_pos);

    var last_valid: vec2i = start_pos;
    for (var i: u32 = 0u; i < line.count; i++) {
        let p = line.points[i];
        if all(p == start_pos) {
            continue;
        }
        if !pos_inside_grid(p) {
            return last_valid;
        }
        let idx_res = Shared::pos_to_idx(p);
        if !idx_res.valid {
            return last_valid;
        }
        let dest_idx = idx_res.index;
        let destination = Shared::current_cells[dest_idx];
        let destination_density = CellMod::get_density(destination.material);
        if destination_density < own_density {
            last_valid = p;
        } else {
            break;
        }
    }
    return last_valid;
}


fn process_movable_solid(cell: ptr<function, CellMod::Cell>, cell_idx: u32) -> bool {
    (*cell).velocity += vec2f(0.0, 2.0);

    let current_pos = Shared::idx_to_pos(cell_idx);
    let maybe_idx = Shared::pos_to_idx(current_pos).index;


    let velocity_sweeped_pos = sweep_density(cell_idx, current_pos, current_pos + vec2i((*cell).velocity));
    var idx_res = Shared::pos_to_idx(velocity_sweeped_pos);
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
        idx_res = Shared::idx_from_offset(cell_idx, dir);
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


fn process_liquid(cell: ptr<function, CellMod::Cell>, cell_idx: u32) -> bool {
    let prefer_left = random_bool(cell_idx);

    let left_res = Shared::idx_from_offset(cell_idx, vec2i(-1, 0));
    let left_idx = left_res.index;
    let right_res = Shared::idx_from_offset(cell_idx, vec2i(1, 0));
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



fn process_cell(cell: CellMod::Cell, cell_idx: u32) {
    let pos = Shared::idx_to_pos(cell_idx);
    
    var debug_color = vec4f(0.0, 0.0, 0.0, 0.0);
    debug_color = Shared::print_value(debug_color, pos, vec2i(0, 5), 12.4, 2, vec4f(1.0, 0.0, 0.0, 1.0));
    textureStore(Shared::debug_tex0, pos, debug_color); 


    var local_cell = cell;
    switch cell.material {
        case CellMod::MAT_SAND {
            let r = process_movable_solid(&local_cell, cell_idx);
        }
        case CellMod::MAT_WATER {
            if !process_movable_solid(&local_cell, cell_idx) {
                let r = process_liquid(&local_cell, cell_idx);
            }
        }
        case CellMod::MAT_EMPTY {
        }
        default {

        }
    }

    // If this cell has proposed no other intent and it modified the local_cell,
    // make sure that modification gets registered
    if !CellMod::eq(local_cell, cell) && Shared::intents[cell_idx].intend_kind == Shared::INTENT_NONE {
        modify_own(cell_idx);
    }
    Shared::desired_cells[cell_idx] = local_cell;
}
