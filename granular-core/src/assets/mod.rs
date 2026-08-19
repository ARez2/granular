use rustc_hash::FxHashMap as HashMap;
use std::{path::PathBuf, sync::Arc};

use crate::{filewatcher::FileWatcher, graphics::GraphicsSystem, utils::*};

mod holder;
pub use holder::AssetStatus;
use holder::{AssetHolder, TypedAssetHolder};

mod asset_path;
pub use asset_path::AssetPath;

mod asset_handle;
pub use asset_handle::AssetHandle;

mod asset_source;
use asset_source::AssetSource;

mod texture_asset;
pub use texture_asset::TextureAsset;
mod shader_asset;
pub use shader_asset::ShaderAsset;

pub mod events {
    pub struct AssetLoaded {
        pub asset_id: u64,
    }
}

pub trait Asset: 'static {
    fn from_bytes(ctx: &GeeseContextHandle<AssetSystem>, bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: std::marker::Sized;
}

pub struct AssetSystem {
    ctx: GeeseContextHandle<Self>,
    asset_source: Arc<dyn AssetSource>,
    assets: HashMap<Arc<u64>, Box<dyn AssetHolder>>,
    path_to_id: HashMap<AssetPath, u64>,
    next_id: u64,
}
#[profiling::all_functions]
impl AssetSystem {
    pub fn get<T: Asset>(&self, handle: &AssetHandle<T>) -> Option<&T> {
        self.assets
            .get(handle.id())
            .and_then(|holder| holder.as_any())
            .and_then(|value| value.downcast_ref::<T>())
    }

    pub fn status<T: Asset>(&self, handle: &AssetHandle<T>) -> AssetStatus {
        let asset = self.assets.get(handle.id());
        if let Some(asset) = asset {
            return asset.status();
        } else {
            return AssetStatus::NotFound;
        }
    }

    pub fn path<T: Asset>(&self, handle: &AssetHandle<T>) -> &Option<AssetPath> {
        let asset = self.assets.get(handle.id());
        if let Some(asset) = asset {
            return asset.path();
        } else {
            return &None;
        }
    }

    /// Loads a new asset from the given path. Returns a handle to that asset. Note that `hot_reload` does nothing on WASM.
    pub fn load<T: Asset>(
        &mut self,
        path: impl Into<AssetPath>,
        hot_reload: bool,
    ) -> AssetHandle<T> {
        // let path = self.add_basepath(path);
        let path = path.into();

        if let Some(id) = self.path_to_id.get(&path) {
            let key_value = self.assets.get_key_value(id).unwrap();
            return AssetHandle::new(key_value.0.clone());
        }

        let id = self.get_next_id();

        let handle = {
            let key = Arc::new(id);

            self.assets.insert(
                key.clone(),
                Box::new(TypedAssetHolder::<T>::loading(path.clone())),
            );

            AssetHandle::new(key)
        };
        let abspath = self.asset_source.make_assetpath_absolute(&path);
        self.path_to_id.insert(abspath, id);

        #[cfg(not(target_arch = "wasm32"))]
        if hot_reload {
            let mut filewatcher = self.ctx.get_mut::<FileWatcher>();
            filewatcher.watch(path.clone().as_str(), true);
            // self.watch(path.clone());
        }

        self.queue_load(id, path.clone());

        handle
    }

    /// Registers an asset where the data of the asset is already present outside and does not need to be loaded first. The `assetname` helps identify the asset in error logs etc.
    pub fn register<T: Asset>(&mut self, asset: T, assetname: Option<&str>) -> AssetHandle<T> {
        let id = self.get_next_id();
        let key = Arc::new(id);
        self.assets.insert(
            key.clone(),
            Box::new(TypedAssetHolder::ready(
                asset,
                assetname.map(AssetPath::new),
            )),
        );

        AssetHandle::new(key)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn reload(&mut self, event: &crate::filewatcher::events::FilesChanged) {
        for path in &event.paths {
            debug!("Reload event for {}", path.display());
            let asset_path = AssetPath::new(path.clone().into_string().unwrap());
            let Some(id) = self.path_to_id.get(&asset_path).copied() else {
                continue;
            };

            let Some(asset) = self.assets.get_mut(&id) else {
                continue;
            };

            asset.begin_reload();
            info!("Reloading asset at {}", asset_path.as_str());
            self.queue_load(id, asset_path);
        }
    }

    // #[profiling::skip]
    // pub fn add_basepath(&self, to_path: impl TryInto<PathBuf>) -> PathBuf {
    //     let path: PathBuf = to_path.try_into().ok().expect("Could not add base path");
    //     self.base_path.join(path)
    // }

    pub fn drop_unused_assets(&mut self, _: &crate::events::timing::FixedTick<2500>) {
        let mut removed_usizes = vec![];
        self.assets.retain(|arc, _| {
            if Arc::strong_count(arc) <= 1 {
                removed_usizes.push(**arc);
                false
            } else {
                true
            }
        });
        self.path_to_id.retain(|path, id| {
            let should_drop = removed_usizes.contains(id);
            if should_drop {
                debug!("Removing asset at '{}'", path.as_str());
            }
            !should_drop
        });
    }

    #[profiling::skip]
    fn get_next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn queue_load(&mut self, asset_id: u64, path: AssetPath) {
        let source = self.asset_source.clone();
        let mut executor = self.ctx.get_mut::<FutureExecutor>();
        executor.spawn(async move { source.load(asset_id, &path).await });
    }

    /// Finishes loading an asset
    fn asset_loaded(
        &mut self,
        event: &future_executor::events::FutureReady<asset_source::AssetLoadResult>,
    ) {
        let (asset_id, bytes) = &event.0;
        let Some(holder) = self.assets.get_mut(asset_id) else {
            return;
        };

        match bytes {
            Ok(bytes) => {
                holder.update_from_bytes(&self.ctx, bytes);

                self.ctx.raise_event(events::AssetLoaded {
                    asset_id: *asset_id,
                });
            }

            Err(error) => {
                holder.fail(error.clone());
            }
        }
    }
}
#[profiling::all_functions]
impl GeeseSystem for AssetSystem {
    #[cfg(not(target_arch = "wasm32"))]
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<FileWatcher>>()
        .with::<Mut<FutureExecutor>>()
        .with::<GraphicsSystem>();

    #[cfg(target_arch = "wasm32")]
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<FutureExecutor>>()
        .with::<GraphicsSystem>();

    #[cfg(not(target_arch = "wasm32"))]
    const EVENT_HANDLERS: geese::EventHandlers<Self> = event_handlers()
        .with(Self::reload)
        .with(Self::asset_loaded)
        .with(Self::drop_unused_assets);

    #[cfg(target_arch = "wasm32")]
    const EVENT_HANDLERS: geese::EventHandlers<Self> = event_handlers()
        .with(Self::asset_loaded)
        .with(Self::drop_unused_assets);

    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let base_path;
        if let Ok(cur) = std::env::current_exe() {
            base_path = cur
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf();
        } else {
            base_path = PathBuf::default();
        }

        #[cfg(not(target_arch = "wasm32"))]
        let asset_source = asset_source::FsAssetSource { base_path };
        #[cfg(target_arch = "wasm32")]
        let asset_source = asset_source::WebAssetSource {
            base_url: String::from("replace_me"),
        };
        let asset_source = Arc::new(asset_source);

        Self {
            ctx,
            asset_source,
            assets: HashMap::default(),
            path_to_id: HashMap::default(),
            next_id: 0,
        }
    }
}
