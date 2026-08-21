use super::Asset;
use std::{marker::PhantomData, sync::Arc};

#[derive(Debug, Eq, PartialEq)]
pub struct AssetHandle<T: Asset> {
    id: Arc<u64>,
    marker: std::marker::PhantomData<T>,
}
impl<T: Asset> AssetHandle<T> {
    /// Creates a new AssetHandle for the asset stores at `id`
    pub(super) fn new(id: Arc<u64>) -> Self {
        Self {
            id,
            marker: PhantomData,
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
            marker: self.marker,
        }
    }
}
