#![allow(unused)]

use std::path::Path;
use wgpu::{Extent3d, Sampler, Texture, TextureView};

use super::{Asset, AssetPath, AssetSystem};
use crate::{
    graphics::{GraphicsSystem, TextureBundle},
    utils::*,
};

/// Asset which holds a TextureBundle
#[derive(Debug, PartialEq)]
pub struct TextureAsset {
    texture: TextureBundle,
}
impl TextureAsset {
    pub fn texture(&self) -> &TextureBundle {
        &self.texture
    }
}
impl From<TextureBundle> for TextureAsset {
    fn from(value: TextureBundle) -> Self {
        Self { texture: value }
    }
}
impl Asset for TextureAsset {
    fn from_bytes(ctx: &GeeseContextHandle<AssetSystem>, bytes: &[u8]) -> anyhow::Result<Self> {
        let sys = ctx.get::<GraphicsSystem>();
        let device = sys.device();
        let queue = sys.queue();

        let img = image::load_from_memory(bytes)?.to_rgba8();
        let extent = Extent3d {
            width: img.width(),
            height: img.height(),
            depth_or_array_layers: 1,
        };

        Ok(Self {
            texture: TextureBundle::default(device, queue, extent, &img),
        })
    }
}
