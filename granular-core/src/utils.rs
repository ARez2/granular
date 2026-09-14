//! This is stuff I want to have available in basically every file

use std::path::PathBuf;

#[allow(unused_imports)]
pub use log::{debug, error, info, trace, warn};

pub use geese::{
    Dependencies, EventHandlers, EventQueue, GeeseContext, GeeseContextHandle, GeeseSystem, Mut,
    dependencies, event_handlers,
};

pub use crate::future_executor::{self, FutureExecutor};
pub use crate::{asset_source, assets::AssetSource};
pub use proc_macros::validate_filepath;

pub fn pathbuf_to_string(pathbuf: PathBuf) -> String {
    pathbuf.as_os_str().to_str().unwrap().to_string()
}
