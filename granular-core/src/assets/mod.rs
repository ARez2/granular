use std::{marker::PhantomData, path::{Path, PathBuf}};
use log::warn;
use rustc_hash::FxHashMap as HashMap;
use geese::*;

mod holder;
use holder::{TypedAssetHolder, AssetHolder};

use crate::{filewatcher::FileWatcher, graphics::GraphicsSystem};


mod texture_asset;
pub use texture_asset::TextureAsset;


pub trait Asset: 'static {
    fn from_path(ctx: &GeeseContextHandle<AssetServer>, path: &PathBuf) -> Self;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AssetHandle<T: Asset> {
    id: usize,
    marker: std::marker::PhantomData<T>
}
impl<T: Asset> std::hash::Hash for AssetHandle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.id)
    }
}
impl<T: Asset> AssetHandle<T> {
    pub fn id(&self) -> usize {
        self.id
    }
}


pub struct AssetServer {
    ctx: GeeseContextHandle<Self>,
    assets: HashMap<usize, Box<dyn AssetHolder>>,
    path_to_id: HashMap<PathBuf, usize>,
    assets_path: PathBuf,
}

impl AssetServer {
    pub fn get<T: Asset>(&self, handle: &AssetHandle<T>) -> &T {
        &self.assets.get(&handle.id()).unwrap().as_any().downcast_ref().expect("Invalid type given as generic")
    }

    pub fn load<T: Asset>(&mut self, path: impl TryInto<PathBuf>) -> AssetHandle<T> {
        let path: PathBuf = path.try_into().ok().unwrap();
        let path = self.assets_path.join(path);

        let handle = AssetHandle {
            id: self.assets.len(),
            marker: PhantomData
        };

        if !self.assets.contains_key(&handle.id()) {
            self.assets.insert(handle.id(), Box::new(TypedAssetHolder::new(T::from_path(&self.ctx, &path))));
            self.path_to_id.insert(path, handle.id());
        };
        
        handle
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
                }
            };
            
        }
    }
}
impl GeeseSystem for AssetServer {
    const DEPENDENCIES: geese::Dependencies = dependencies()
        .with::<Mut<FileWatcher>>()
        .with::<GraphicsSystem>();
    const EVENT_HANDLERS: geese::EventHandlers<Self> = event_handlers()
        .with(Self::reload);

    fn new(mut ctx: geese::GeeseContextHandle<Self>) -> Self {
        let cur = std::env::current_exe().unwrap();
        let base_directory = cur.parent().unwrap().parent().unwrap().parent().unwrap();
        let assets_path = base_directory.join("assets");
        let mut filewatcher = ctx.get_mut::<FileWatcher>();
        filewatcher.watch(assets_path.clone(), true);
        drop(filewatcher);
        
        Self {
            ctx,
            assets_path,
            assets: HashMap::default(),
            path_to_id: HashMap::default()
        }
    }
}