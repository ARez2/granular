pub mod prelude {
    pub use granular_core::{
        assets::{AssetHandle, TextureAsset},
        events,
        graphics::{self, WindowSystem},
        input_system::*,
        utils::*,
        AssetSystem, BatchRenderer, Camera, GranularEngine, Simulation,
    };
}
