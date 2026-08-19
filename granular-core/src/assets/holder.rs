use std::{any::Any, sync::Arc};

use super::{Asset, AssetSystem};
use crate::{assets::AssetPath, utils::*};

#[derive(Debug, Clone)]
pub enum AssetStatus {
    Loading,
    Ready,
    Failed(Arc<anyhow::Error>),
    NotFound,
}
impl PartialEq for AssetStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Loading, Self::Loading)
            | (Self::Ready, Self::Ready)
            | (Self::NotFound, Self::NotFound) => true,

            (Self::Failed(a), Self::Failed(b)) => Arc::ptr_eq(a, b),

            _ => false,
        }
    }
}
impl Eq for AssetStatus {}

pub(super) trait AssetHolder {
    fn as_any(&self) -> Option<&dyn Any>;
    fn status(&self) -> AssetStatus;
    fn path(&self) -> &Option<AssetPath>;

    fn begin_reload(&mut self);

    fn update_from_bytes(&mut self, ctx: &GeeseContextHandle<AssetSystem>, bytes: &[u8]);

    fn fail(&mut self, error: Arc<anyhow::Error>);
}

pub(super) struct TypedAssetHolder<T: Asset> {
    value: Option<T>,
    path: Option<AssetPath>,
    loading: bool,
    error: Option<Arc<anyhow::Error>>,
}
// Main impl
impl<T: Asset> TypedAssetHolder<T> {
    pub fn loading(path: AssetPath) -> Self {
        Self {
            value: None,
            path: Some(path),
            loading: true,
            error: None,
        }
    }

    pub fn ready(value: T, path: Option<AssetPath>) -> Self {
        Self {
            value: Some(value),
            path,
            loading: false,
            error: None,
        }
    }

    #[allow(dead_code)]
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    #[allow(dead_code)]
    pub fn error(&self) -> Option<&anyhow::Error> {
        self.error.as_deref()
    }

    #[allow(dead_code)]
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    pub fn status(&self) -> AssetStatus {
        if self.loading {
            AssetStatus::Loading
        } else if self.value.is_some() {
            AssetStatus::Ready
        } else {
            AssetStatus::Failed(self.error.clone().unwrap())
        }
    }

    pub fn path(&self) -> &Option<AssetPath> {
        &self.path
    }
}
// Impl AssetHolder
impl<T: Asset> AssetHolder for TypedAssetHolder<T> {
    fn as_any(&self) -> Option<&dyn Any> {
        self.value.as_ref().map(|value| value as &dyn Any)
    }

    fn status(&self) -> AssetStatus {
        TypedAssetHolder::status(self)
    }

    fn path(&self) -> &Option<AssetPath> {
        TypedAssetHolder::path(self)
    }

    fn begin_reload(&mut self) {
        self.loading = true;
        self.error = None;
    }

    fn update_from_bytes(&mut self, ctx: &GeeseContextHandle<AssetSystem>, bytes: &[u8]) {
        match T::from_bytes(ctx, bytes) {
            Ok(value) => {
                self.value = Some(value);
                self.loading = false;
                self.error = None;
            }

            Err(error) => {
                // Keep the old asset alive if this was a reload.
                self.loading = false;
                self.error = Some(Arc::new(error));
            }
        }
    }

    fn fail(&mut self, error: Arc<anyhow::Error>) {
        self.loading = false;
        self.error = Some(error);
    }
}
