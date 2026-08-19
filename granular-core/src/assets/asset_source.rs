use anyhow::anyhow;
use std::{path::PathBuf, pin::Pin, sync::Arc};

use super::AssetPath;

pub type AssetLoadResult = (u64, anyhow::Result<Vec<u8>, Arc<anyhow::Error>>);
pub type AssetFuture<'a> = Pin<Box<dyn Future<Output = AssetLoadResult> + 'a + Send>>;

pub trait AssetSource: Send + Sync {
    fn load<'a>(&'a self, asset_id: u64, path: &'a AssetPath) -> AssetFuture<'a>;

    fn make_assetpath_absolute(&self, path: &AssetPath) -> AssetPath;
}

pub struct FsAssetSource {
    pub base_path: PathBuf,
}

impl AssetSource for FsAssetSource {
    fn load<'a>(&'a self, asset_id: u64, path: &'a AssetPath) -> AssetFuture<'a> {
        Box::pin(async move {
            let path = self.make_assetpath_absolute(path);
            let bytes = std::fs::read(path.as_str()).map_err(|e| Arc::new(anyhow!(e)));
            (asset_id, bytes)
        })
    }

    fn make_assetpath_absolute(&self, path: &AssetPath) -> AssetPath {
        AssetPath::new(self.base_path.join(path.as_str()).to_str().unwrap())
    }
}

pub struct WebAssetSource {
    pub base_url: String,
}

impl AssetSource for WebAssetSource {
    fn load<'a>(&'a self, asset_id: u64, path: &'a AssetPath) -> AssetFuture<'a> {
        Box::pin(async move {
            let url = format!("{}/{}", self.base_url, path.as_str());

            // let response = reqwest::get(url).await?;
            // let bytes = response.bytes().await?;
            todo!();

            // Ok(bytes.to_vec())
        })
    }

    fn make_assetpath_absolute(&self, path: &AssetPath) -> AssetPath {
        let mut p = self.base_url.clone();
        p.push_str(path.as_str());
        AssetPath::new(p)
    }
}
