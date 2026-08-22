pub mod prelude {
    pub use granular_core::{
        AssetSystem, BatchRenderer, Camera, GranularEngine, Simulation,
        assets::{AssetHandle, AssetStatus, TextureAsset, TextureAssetImportSettings},
        events,
        graphics::{self, TextureBundle, WindowSystem},
        input_system::*,
        utils::*,
    };
    pub use wgpu::{Extent3d, FilterMode, TextureFormat};
}
