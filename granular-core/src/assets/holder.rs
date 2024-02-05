use std::{any::Any, path::PathBuf};
use geese::GeeseContextHandle;

use super::{Asset, AssetServer};




pub(super) trait AssetHolder {
    fn as_any(&self) -> &dyn Any;
    fn update_from_path(&mut self, ctx: &GeeseContextHandle<AssetServer>, path: &PathBuf);
}

pub(super) struct TypedAssetHolder<T: Asset> {
    value: T
}
impl<T: Asset> TypedAssetHolder<T> {
    pub fn new(value: T) -> Self {
        Self {
            value
        }
    }
}
impl<T: Asset> AssetHolder for TypedAssetHolder<T> {
    fn as_any(&self) -> &dyn Any {
        &self.value
    }
    
    fn update_from_path(&mut self, ctx: &GeeseContextHandle<AssetServer>, path: &PathBuf) {
        self.value = T::from_path(ctx, path);
    }
}