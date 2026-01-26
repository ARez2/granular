//! This is stuff I want to have available in basically every file

#[cfg(feature = "trace")]
pub use tracing::info_span;
#[allow(unused_imports)]
pub use tracing::{debug, error, info, trace, warn};

pub use geese::{
    dependencies, event_handlers, Dependencies, EventHandlers, EventQueue, GeeseContext,
    GeeseContextHandle, GeeseSystem, Mut,
};
