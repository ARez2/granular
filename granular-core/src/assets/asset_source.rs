#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum AssetSource {
    Embedded {
        name: &'static str,
        bytes: &'static [u8],
    },

    #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
    File { path: PathBuf },
}
impl AssetSource {
    pub(super) fn read(&self) -> anyhow::Result<Vec<u8>> {
        match self {
            Self::Embedded { name: _, bytes } => Ok(bytes.to_vec()),
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            Self::File { path } => {
                let bytes = std::fs::read(path)?;
                Ok(bytes)
            }
        }
    }
}
impl std::fmt::Display for AssetSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            AssetSource::Embedded { name, bytes: _ } => write!(f, "{name}"),
            #[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
            AssetSource::File { path } => write!(f, "{}", super::pathbuf_to_string(path.clone())),
        }
    }
}
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
#[macro_export]
macro_rules! asset_source {
    ($path:literal) => {{
        $crate::validate_asset!($path);

        $crate::AssetSource::File {
            path: std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path)),
        }
    }};
}

#[cfg(any(target_arch = "wasm32", not(debug_assertions)))]
#[macro_export]
macro_rules! asset_source {
    ($path:literal) => {
        $crate::AssetSource::Embedded {
            name: $path,
            bytes: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path)),
        }
    };
}
