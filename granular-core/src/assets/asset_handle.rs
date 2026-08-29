use std::{marker::PhantomData, sync::Arc};

#[derive(Debug)]
pub struct AssetHandle<T> {
    id: Arc<u64>,
    _marker: std::marker::PhantomData<fn() -> T>,
}
impl<T> AssetHandle<T> {
    /// Creates a new AssetHandle for the asset stores at `id`
    pub(super) fn new(id: Arc<u64>) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }
}
impl<T> PartialEq for AssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for AssetHandle<T> {}
impl<T> std::hash::Hash for AssetHandle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(*self.id)
    }
}
impl<T> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            _marker: self._marker,
        }
    }
}
impl<T> AssetHandle<T> {
    pub fn id(&self) -> &Arc<u64> {
        &self.id
    }
}
