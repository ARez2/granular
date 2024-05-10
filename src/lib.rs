
pub mod prelude {
    pub use granular_core::{
        GranularEngine,
        events,
        input_system::*,
        AssetSystem, assets::{AssetHandle, TextureAsset},
        Camera, BatchRenderer, graphics
    };
}