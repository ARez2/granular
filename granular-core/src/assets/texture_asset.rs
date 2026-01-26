#![allow(unused)]

use geese::GeeseContextHandle;
use std::path::Path;
use wgpu::{Extent3d, Sampler, Texture, TextureView};

use super::{Asset, AssetSystem};
use crate::graphics::{GraphicsSystem, TextureBundle};

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
    fn from_path(ctx: &GeeseContextHandle<AssetSystem>, path: &Path) -> Self {
        let sys = ctx.get::<GraphicsSystem>();
        let device = sys.device();
        let queue = sys.queue();

        let img = image::open(path).unwrap().to_rgba8();
        let extent = Extent3d {
            width: img.width(),
            height: img.height(),
            depth_or_array_layers: 1,
        };

        Self {
            texture: TextureBundle::default(device, queue, extent, &img),
        }
    }
}
