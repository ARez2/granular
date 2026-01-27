//! This is stuff I want to have available in basically every file

#[allow(unused_imports)]
pub use tracing::{debug, error, info, trace, warn, Level};
#[cfg(feature = "trace")]
pub use tracing::{info_span, span};

pub use geese::{
    dependencies, event_handlers, Dependencies, EventHandlers, EventQueue, GeeseContext,
    GeeseContextHandle, GeeseSystem, Mut,
};
