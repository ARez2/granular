use std::{marker::PhantomData, path::{Path, PathBuf}, sync::Arc};
use log::{debug, info, warn};
use rustc_hash::FxHashMap as HashMap;
use geese::*;

mod holder;
use holder::{TypedAssetHolder, AssetHolder};

use crate::{filewatcher::FileWatcher, graphics::GraphicsSystem};


mod texture_asset;
pub use texture_asset::TextureAsset;
mod shader_asset;
pub use shader_asset::ShaderAsset;


pub mod events {
    pub struct AssetReload {
        pub asset_id: u64
    }
}


pub trait Asset: 'static {
    fn from_path(ctx: &GeeseContextHandle<AssetSystem>, path: &Path) -> Self;
}

#[derive(Debug, Eq, PartialEq)]
pub struct AssetHandle<T: Asset> {
    id: Arc<u64>,
    marker: std::marker::PhantomData<T>
}
impl<T: Asset> AssetHandle<T> {
    pub fn new(id: Arc<u64>) -> Self {
        Self {
            id,
            marker: PhantomData
        }
    }
}
impl<T: Asset> std::hash::Hash for AssetHandle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.id)
    }
}
impl<T: Asset> AssetHandle<T> {
    pub fn id(&self) -> &Arc<u64> {
        &self.id
    }
}
impl<T: Asset> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            marker: self.marker
        }
    }
}



pub struct AssetSystem {
    ctx: GeeseContextHandle<Self>,
    assets: HashMap<Arc<u64>, Box<dyn AssetHolder>>,
    path_to_id: HashMap<PathBuf, u64>,
    base_path: PathBuf,
}
impl AssetSystem {
    pub fn get<T: Asset>(&self, handle: &AssetHandle<T>) -> &T {
        self.assets.get(handle.id()).unwrap().as_any().downcast_ref().expect("Invalid type given as generic")
    }


    pub fn get_handle<T: Asset>(&self, path: impl TryInto<PathBuf>) -> AssetHandle<T> {
        let path = self.add_basepath(path);

        let id = self.path_to_id.get(&path).unwrap();
        let key_value = self.assets.get_key_value(id).unwrap();
        AssetHandle::new(key_value.0.clone())
    }


    pub fn load<T: Asset>(&mut self, path: impl TryInto<PathBuf>, hot_reload: bool) -> AssetHandle<T> {
        let path = self.add_basepath(path);

        let id = self.assets.len() as u64;
        // If this is a new asset, create it and return a new handle,
        if !self.assets.contains_key(&id) {
            self.assets.insert(Arc::new(id), Box::new(TypedAssetHolder::new(T::from_path(&self.ctx, &path))));
            let arc = self.assets.get_key_value(&(self.assets.len() as u64 - 1)).unwrap().0;
            self.path_to_id.insert(path.clone(), id);
            
            if hot_reload {
                let mut filewatcher = self.ctx.get_mut::<FileWatcher>();
                filewatcher.watch(path, true);
            };

            AssetHandle::new(arc.clone())
        } else { // else, clone the existing handle
            self.get_handle(path)
        }
    }


    fn reload(&mut self, event: &crate::filewatcher::events::FilesChanged) {
        for path in event.paths.iter() {
            let id = self.path_to_id.get(path);
            if let Some(id) = id {
                let asset = self.assets.get_mut(id);
                if let Some(asset) = asset {
                    if !Path::exists(path) {
                        warn!("Tried reloading file from: '{}' but it doesn't exist!", path.display());
                        continue;
                    }
                    asset.update_from_path(&self.ctx, path);
                    info!("Reloading asset at {}", path.display());
                    self.ctx.raise_event(events::AssetReload{asset_id: *id})
                }
            };
            
        }
    }


    pub fn add_basepath(&self, to_path: impl TryInto<PathBuf>) -> PathBuf {
        let path: PathBuf = to_path.try_into().ok().expect("Could not add base path");
        self.base_path.join(path)
    }

    pub fn drop_unused_assets(&mut self, _: &crate::events::timing::FixedTick::<2500>) {
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
                debug!("Removing asset at '{}'", path.display());
            }
            !should_drop
        });
    }
}
impl GeeseSystem for AssetSystem {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<FileWatcher>>()
        .with::<GraphicsSystem>();
    const EVENT_HANDLERS: geese::EventHandlers<Self> = event_handlers()
        .with(Self::reload)
        .with(Self::drop_unused_assets);


    fn new(ctx: geese::GeeseContextHandle<Self>) -> Self {
        let cur = std::env::current_exe().unwrap();
        let base_path = cur.parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
        
        Self {
            ctx,
            base_path,
            assets: HashMap::default(),
            path_to_id: HashMap::default()
        }
    }
}