use std::{any::Any, sync::Arc};

use super::{Asset, AssetSystem};
use crate::{
    assets::{AssetPath, InternalAsset},
    utils::*,
};

/// Represents the status of a single asset.
#[derive(Debug, Clone)]
pub enum AssetStatus {
    Loading,
    Ready,
    Failed(Option<Arc<anyhow::Error>>),
    NotFound,
}
impl PartialEq for AssetStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Loading, Self::Loading)
            | (Self::Ready, Self::Ready)
            | (Self::NotFound, Self::NotFound) => true,

            (Self::Failed(a), Self::Failed(b)) => {
                (a.is_none() && b.is_none())
                    || std::sync::Arc::<anyhow::Error>::ptr_eq(
                        &a.clone().unwrap(),
                        &b.clone().unwrap(),
                    )
            }

            _ => false,
        }
    }
}
impl Eq for AssetStatus {}

/// Helper trait to be able to store TypedAssetHolder's of different generic types (for the different types of assets) inside one list.
pub(super) trait AssetHolder {
    #[allow(unused)]
    fn as_any(&self) -> &dyn Any;
    /// Converts itself into &dyn Any
    #[allow(unused)]
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    /// Converts the encapsulated Asset into &dyn Any
    fn inner_any(&self) -> Option<&dyn Any>;
    fn inner_any_mut(&mut self) -> Option<&mut dyn Any>;
    fn status(&self) -> AssetStatus;
    fn path(&self) -> &Option<AssetPath>;

    /// Prepares for reloading (disabled in wasm since there are no FileWatcher events which could trigger a reload)
    #[cfg(not(target_arch = "wasm32"))]
    fn begin_reload(&mut self);

    /// Updates its own contents based on the bytes received (and if those can be used to successfully create an Asset)
    fn update_from_bytes(&mut self, ctx: &mut GeeseContextHandle<AssetSystem>, bytes: &[u8]);

    /// Fails the loading process with the provided error code.
    fn fail(&mut self, error: Arc<anyhow::Error>);
}

/// This encapsulates an asset and holds an error if the asset failed to load.
pub(super) struct TypedAssetHolder<T: Asset> {
    value: Option<T>,
    import_settings: T::ImportSettings,
    path: Option<AssetPath>,
    loading: bool,
    error: Option<Arc<anyhow::Error>>,
}
// Main impl
impl<T: Asset> TypedAssetHolder<T> {
    /// Creates a new `TypedAssetHolder` with its asset being set to be loading.
    pub(super) fn loading(path: AssetPath, import_settings: T::ImportSettings) -> Self {
        Self {
            value: None,
            import_settings,
            path: Some(path),
            loading: true,
            error: None,
        }
    }

    /// Creates a new `TypedAssetHolder` with its asset being set to be ready.
    pub(super) fn ready(
        value: T,
        path: Option<AssetPath>,
        import_settings: T::ImportSettings,
    ) -> Self {
        Self {
            value: Some(value),
            import_settings,
            path,
            loading: false,
            error: None,
        }
    }

    /// Returns the asset being held inside of this `TypedAssetHolder`
    #[allow(dead_code)]
    pub(super) fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns the error (if any)
    #[allow(dead_code)]
    pub(super) fn error(&self) -> Option<&anyhow::Error> {
        self.error.as_deref()
    }

    #[allow(dead_code)]
    pub(super) fn is_loading(&self) -> bool {
        self.loading
    }

    /// Fetches the status of the asset(holder)
    pub(super) fn status(&self) -> AssetStatus {
        if self.loading {
            AssetStatus::Loading
        } else if self.value.is_some() {
            AssetStatus::Ready
        } else {
            AssetStatus::Failed(self.error.clone())
        }
    }

    /// Stores the AssetPath which was used to load the asset
    pub(super) fn path(&self) -> &Option<AssetPath> {
        &self.path
    }
}
// Impl AssetHolder
impl<T: InternalAsset> AssetHolder for TypedAssetHolder<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn inner_any(&self) -> Option<&dyn Any> {
        self.value.as_ref().map(|value| value as &dyn Any)
    }

    fn inner_any_mut(&mut self) -> Option<&mut dyn Any> {
        self.value.as_mut().map(|value| value as &mut dyn Any)
    }

    fn status(&self) -> AssetStatus {
        TypedAssetHolder::status(self)
    }

    fn path(&self) -> &Option<AssetPath> {
        TypedAssetHolder::path(self)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn begin_reload(&mut self) {
        self.loading = true;
        self.error = None;
    }

    fn update_from_bytes(&mut self, ctx: &mut GeeseContextHandle<AssetSystem>, bytes: &[u8]) {
        if let Some(mut prev) = self.value.take() {
            match prev.update_from_bytes(ctx, bytes, &self.import_settings) {
                Ok(()) => {
                    self.loading = false;
                    self.error = None;
                }
                Err(e) => {
                    self.value = Some(prev);
                    self.loading = false;
                    self.error = Some(Arc::new(e));
                }
            };
        } else {
            match T::create_from_bytes(ctx, bytes, &self.import_settings) {
                Ok(val) => {
                    self.value = Some(val);
                    self.loading = false;
                    self.error = None;
                }
                Err(e) => {
                    self.value = None;
                    self.loading = false;
                    self.error = Some(Arc::new(e));
                }
            }
        }
    }

    fn fail(&mut self, error: Arc<anyhow::Error>) {
        self.loading = false;
        self.error = Some(error);
    }
}
