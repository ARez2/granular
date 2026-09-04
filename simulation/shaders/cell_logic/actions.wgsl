#import ../shared.wgsl as Shared

const PRIORITY_REACTION: u32 = 0;
const PRIORITY_DOWN: u32 = 1;
const PRIORITY_DIAGONAL: u32 = 2;
const PRIORITY_SWAP: u32 = 3;
const PRIORITY_MODIFY: u32 = 0;

/// Simple movement from source_idx to destination_idx
/// next_cells[destination_idx] = desired_cells[source_idx]
fn move_to(source_idx: u32, destination_idx: u32) {
    Shared::propose_interaction(
        source_idx,
        destination_idx,
        Shared::INTENT_MOVE,
        PRIORITY_DOWN,
    );
}


/// Swap source_idx with destination_idx
/// next_cells[destination_idx] = desired_cells[source_idx] and next_cells[source_idx] = current_cells[destination_idx]
fn swap(source_idx: u32, destination_idx: u32) {
    Shared::propose_interaction(
        source_idx,
        destination_idx,
        Shared::INTENT_SWAP,
        PRIORITY_SWAP,
    );
}

/// Mark that we want to modify only this cell
/// next_cells[source_idx] = desired_cells[source_idx]
fn modify_own(source_idx: u32) {
    Shared::propose_interaction(source_idx, source_idx, Shared::INTENT_MODIFY_OWN, PRIORITY_MODIFY);
}

/// Mark that we want to modify only another cell
/// next_cells[destination_idx] = desired_cells[source_idx]
fn modify_other(source_idx: u32, destination_idx: u32) {
    Shared::propose_interaction(source_idx, source_idx, Shared::INTENT_MODIFY_OWN, PRIORITY_MODIFY);
}