use anyhow::bail;
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

mod asset_impls;

pub mod events {
    #[derive(Debug)]
    pub struct AssetChanged {
        pub asset_id: u64,
    }
}

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
fn pathbuf_to_string(pathbuf: PathBuf) -> String {
    pathbuf.as_os_str().to_str().unwrap().to_string()
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    bytes.hash(&mut hasher);
    hasher.finish()
}

pub trait Asset: 'static {
    type LoadSettings: Any + Clone + Default + 'static;
}

/// Type-erased Asset so we dont need generics in AssetEntry and AssetSystem.assets
type ErasedAsset = dyn Any;
/// Type-erased LoadSettings for an Asset so we dont need generics in AssetEntry and AssetSystem.assets
type ErasedSettings = dyn Any;
/// Type-erased Asset loader function so we can easily store it in LoaderMap
type ErasedLoader =
    dyn FnMut(Vec<u8>, &ErasedSettings) -> anyhow::Result<Box<ErasedAsset>> + 'static;
type LoaderMap = HashMap<std::any::TypeId, Box<ErasedLoader>>;

struct AssetEntry {
    /// We only need to store this in case we need to reload the asset and need to find the corresponding loader function
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    type_id: std::any::TypeId,
    /// The actual type-erased asset data
    asset: Box<ErasedAsset>,
    source: Option<AssetSource>,
    /// We only need to store this in case we ever need to reload the asset where we cant pass in new settings
    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    settings: Option<Box<ErasedSettings>>,
    /// A hash of the bytes used to construct that asset, so we can say if the asset has changed (and dont need to store the full bytes)
    hash: u64,
}

pub struct AssetSystem {
    ctx: GeeseContextHandle<Self>,
    next_id: u64,
    assets: HashMap<Arc<u64>, AssetEntry>,
    loaders: LoaderMap,
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
            loaders: HashMap::default(),
            source_to_id: HashMap::default(),
        }
    }
}
impl AssetSystem {
    const DEFAULT_EVENT_HANDLERS: geese::EventHandlers<AssetSystem> =
        event_handlers().with(Self::drop_unused_assets);

    /// Fetches an asset by its AssetHandle. Returns None if it wasnt found
    pub fn get<T: Asset>(&self, handle: &AssetHandle<T>) -> Option<&T> {
        let entry = self.assets.get(handle.id().as_ref())?;
        entry.asset.downcast_ref::<T>()
    }

    /// Fetches an asset by its AssetHandle. Returns None if it wasnt found
    pub fn get_mut<T: Asset>(&mut self, handle: &AssetHandle<T>) -> Option<&mut T> {
        let entry = self.assets.get_mut(handle.id().as_ref())?;
        entry.asset.downcast_mut::<T>()
    }

    /// Registers a new loader for that asset type. The loader only has to be registered once and can then be re-used by other systems down the line.
    pub fn add_loader<T: Asset>(
        &mut self,
        // This needs to be a full type because we want to provide some convenience for the user (like not having to wrap everything inside a Box::new(...))
        mut loader_fn: impl (FnMut(Vec<u8>, T::LoadSettings) -> anyhow::Result<T>) + 'static,
    ) {
        let closure: Box<ErasedLoader> = Box::new(move |bytes, settings| {
            let settings = (*settings)
                .downcast_ref::<T::LoadSettings>()
                .unwrap_or_else(|| {
                    panic!(
                        "Invalid LoadSettings type for asset loader. Expected: '{:?}'.   Got: '{:?}'",
                        std::any::type_name::<T::LoadSettings>(),
                        std::any::type_name_of_val(settings)
                    )
                });

            let asset = loader_fn(bytes, settings.clone())?;

            Ok(Box::new(asset) as Box<ErasedAsset>)
        });
        self.loaders.insert(std::any::TypeId::of::<T>(), closure);
    }

    fn get_next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Loads a new asset from the given source. You can use the `asset_source!("path/to/asset")` macro here.
    /// The loader closure should take the bytes and produce a anyhow::Result with the asset inside.
    pub fn load<T: Asset>(
        &mut self,
        source: AssetSource,
        settings: T::LoadSettings,
    ) -> anyhow::Result<AssetHandle<T>> {
        let source_string = source.to_string();
        if let Some(id) = self.source_to_id.get(&source_string).cloned() {
            warn!("Asset already exists. Returning existing handle...");
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            self.reload_asset(id);

            let key_value = self.assets.get_key_value(&id).unwrap();
            return Ok(AssetHandle::new(key_value.0.clone()));
        }
        let loader_opt = self.loaders.get_mut(&std::any::TypeId::of::<T>());
        let Some(loader) = loader_opt else {
            error!(
                "No loader registered for Asset of type '{}'",
                std::any::type_name::<T>()
            );
            bail!(
                "No loader registered for Asset of type '{}'",
                std::any::type_name::<T>()
            );
        };

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

        let erased_settings: Box<ErasedSettings> = Box::new(settings);
        let asset = (loader)(bytes, &*erased_settings);
        if let Err(e) = asset {
            error!("Error while loading asset: {}", e);
            return Err(e);
        }
        let asset = asset.unwrap();

        let entry = AssetEntry {
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            type_id: std::any::TypeId::of::<T>(),
            asset,
            source: Some(source),
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            settings: Some(erased_settings),
            hash,
        };

        let id = self.get_next_id();
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
    pub fn register<T: Asset>(&mut self, asset: T) -> AssetHandle<T> {
        let id = self.get_next_id();
        let asset_id = Arc::new(id);

        self.assets.insert(
            asset_id.clone(),
            AssetEntry {
                #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
                type_id: std::any::TypeId::of::<T>(),
                source: None,
                asset: Box::new(asset),
                #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
                settings: None,
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
        let type_id = self.assets.get(&asset_id).map(|v| v.type_id);
        let Some(entry) = self.assets.get_mut(&asset_id) else {
            return;
        };
        let type_id = type_id.unwrap();
        let Some(source) = &entry.source else {
            return;
        };
        let Some(prev_settings) = &entry.settings else {
            return;
        };
        let loader_opt = self.loaders.get_mut(&type_id);
        let Some(loader) = loader_opt else {
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
            // Settings has type Option<Box<ErasedSettings>>, so double deref gives just the ErasedSettings,
            // then borrow that because of the loader signature
            let new_asset = (loader)(bytes, &**prev_settings);
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
