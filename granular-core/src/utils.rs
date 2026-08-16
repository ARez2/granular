//! This is stuff I want to have available in basically every file

#[allow(unused_imports)]
pub use log::{debug, error, info, trace, warn};

pub use geese::{
    dependencies, event_handlers, Dependencies, EventHandlers, EventQueue, GeeseContext,
    GeeseContextHandle, GeeseSystem, Mut,
};
