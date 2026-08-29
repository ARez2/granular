use rustc_hash::FxHashMap as HashMap;
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use std::path::PathBuf;
use std::{
    any::Any,
    hash::{Hash, Hasher},
    sync::Arc,
};

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use crate::filewatcher::FileWatcher;
use crate::utils::*;

mod asset_handle;
pub use asset_handle::AssetHandle;

mod asset_source;
pub use asset_source::AssetSource;

pub mod events {
    #[derive(Debug)]
    pub struct AssetChanged {
        pub asset_id: u64,
    }
}

fn pathbuf_to_string(pathbuf: PathBuf) -> String {
    pathbuf.as_os_str().to_str().unwrap().to_string()
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    bytes.hash(&mut hasher);
    hasher.finish()
}

type ErasedAsset = dyn Any + Send + Sync;
type AssetLoader = dyn Fn(Vec<u8>) -> anyhow::Result<Box<ErasedAsset>> + Send + Sync;

struct AssetEntry {
    asset: Box<ErasedAsset>,
    source: Option<AssetSource>,
    loader: Option<Box<AssetLoader>>,
    hash: u64,
}

pub struct AssetSystem {
    ctx: GeeseContextHandle<Self>,
    next_id: u64,
    assets: HashMap<Arc<u64>, AssetEntry>,
    source_to_id: HashMap<String, u64>,
}
impl GeeseSystem for AssetSystem {
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    const DEPENDENCIES: geese::Dependencies = dependencies().with::<Mut<FileWatcher>>();

    #[cfg(any(target_arch = "wasm32", not(debug_assertions)))]
    const EVENT_HANDLERS: EventHandlers<Self> = Self::DEFAULT_EVENT_HANDLERS;
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    const EVENT_HANDLERS: EventHandlers<Self> =
        Self::DEFAULT_EVENT_HANDLERS.with(Self::on_assetchange);

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,
            next_id: 0,
            assets: HashMap::default(),
            source_to_id: HashMap::default(),
        }
    }
}
impl AssetSystem {
    const DEFAULT_EVENT_HANDLERS: geese::EventHandlers<AssetSystem> =
        event_handlers().with(Self::drop_unused_assets);

    /// Fetches an asset by its AssetHandle. Returns None if it wasnt found
    pub fn get<T>(&self, handle: &AssetHandle<T>) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        let entry = self.assets.get(handle.id().as_ref())?;
        entry.asset.downcast_ref::<T>()
    }

    /// Fetches an asset by its AssetHandle. Returns None if it wasnt found
    pub fn get_mut<T>(&mut self, handle: &AssetHandle<T>) -> Option<&mut T>
    where
        T: Send + Sync + 'static,
    {
        let entry = self.assets.get_mut(handle.id().as_ref())?;
        entry.asset.downcast_mut::<T>()
    }

    /// Loads a new asset from the given source. You can use the `asset_source!("path/to/asset")` macro here.
    /// The loader closure should take the bytes and produce a anyhow::Result with the asset inside.
    pub fn load<T, F>(&mut self, source: AssetSource, loader: F) -> anyhow::Result<AssetHandle<T>>
    where
        T: Send + Sync + 'static,
        F: Fn(Vec<u8>) -> anyhow::Result<T> + Send + Sync + 'static,
    {
        let source_string = source.to_string();
        if let Some(id) = self.source_to_id.get(&source_string).cloned() {
            warn!("Asset already exists. Returning existing handle...");
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            self.reload_asset(id);

            let key_value = self.assets.get_key_value(&id).unwrap();
            return Ok(AssetHandle::new(key_value.0.clone()));
        }

        let bytes = source.read();
        if let Err(e) = bytes {
            error!(
                "Error while reading asset source '{}': {}",
                source_string, e
            );
            return Err(e);
        }
        let bytes = bytes.unwrap();
        let hash = hash_bytes(&bytes);
        // This moves/ captures the user-provided loader and uses it to create a new
        let final_loader: Box<AssetLoader> = Box::new(move |bytes| Ok(Box::new(loader(bytes)?)));

        let asset = (final_loader)(bytes);
        if let Err(e) = asset {
            error!("Error while loading asset: {}", e);
            return Err(e);
        }
        let asset = asset.unwrap();

        let entry = AssetEntry {
            asset,
            source: Some(source),
            loader: Some(final_loader),
            hash,
        };

        let id = self.next_id;
        self.next_id += 1;
        let asset_id = Arc::new(id);
        self.assets.insert(asset_id.clone(), entry);
        self.source_to_id.insert(source_string, id);

        #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
        {
            if let Some(entry) = self.assets.get_mut(&id)
                && let Some(AssetSource::File { path }) = &entry.source
            {
                self.ctx
                    .get_mut::<FileWatcher>()
                    .watch(path.parent().unwrap(), true);
            }
        }

        Ok(AssetHandle::new(asset_id))
    }

    /// Registers an asset where the data of the asset is already present outside and does not need to be loaded first.
    pub fn register<T>(&mut self, asset: T) -> AssetHandle<T>
    where
        T: Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;

        let asset_id = Arc::new(id);

        self.assets.insert(
            asset_id.clone(),
            AssetEntry {
                source: None,
                asset: Box::new(asset),
                loader: None,
                hash: 0,
            },
        );

        AssetHandle::new(asset_id)
    }

    /// Removes assets which are no longer used. Gets called every 2.5 seconds.
    fn drop_unused_assets(&mut self, _: &crate::events::timing::FixedTick<2500>) {
        let mut removed_usizes = vec![];
        self.assets.retain(|arc, _| {
            if Arc::strong_count(arc) <= 1 {
                removed_usizes.push(**arc);
                false
            } else {
                true
            }
        });
        self.source_to_id.retain(|path, id| {
            let should_drop = removed_usizes.contains(id);
            if should_drop {
                debug!("Removing asset at '{}'", path.as_str());
            }
            !should_drop
        });
    }

    /// Event handler for when a file changes. Reloads the asset if neccessary using `reload_asset`
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    fn on_assetchange(&mut self, event: &crate::filewatcher::events::FilesChanged) {
        for path in event.paths.clone() {
            let string = pathbuf_to_string(path);
            let Some(id) = self.source_to_id.get(&string) else {
                continue;
            };
            self.reload_asset(*id);
        }
    }

    /// Performs the actual reload of the asset and emits `events::AssetChanged` but only if the bytes actually changed.
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    fn reload_asset(&mut self, asset_id: u64) {
        let Some(entry) = self.assets.get_mut(&asset_id) else {
            return;
        };
        let Some(source) = &entry.source else {
            return;
        };
        let Some(loader) = &entry.loader else {
            return;
        };
        let bytes_res = source.read();
        if let Err(e) = bytes_res {
            warn!(
                "Error while reading asset source '{}': {}, {asset_id}",
                source, e
            );
            return;
        }
        let bytes = bytes_res.unwrap();
        let hash = hash_bytes(&bytes);

        if hash != entry.hash {
            let new_asset = (loader)(bytes);
            if let Err(e) = new_asset {
                error!("Error while reloading asset: {:?}", e);
                return;
            }
            entry.asset = new_asset.unwrap();
            entry.hash = hash;
            self.ctx.raise_event(events::AssetChanged { asset_id });
        }
    }
}
