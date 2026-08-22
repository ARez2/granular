use crate::{graphics::TextureBundle, utils::*};
use std::{fmt::Debug, sync::Arc};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct TextureHandle {
    id: Arc<u32>,
    generation: u32,
}
impl TextureHandle {
    pub fn new(id: u32, generation: u32) -> Self {
        Self {
            id: Arc::new(id),
            generation,
        }
    }
}
impl Clone for TextureHandle {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            generation: self.generation,
        }
    }
}

/// Trait that all different textures need to implement.
pub trait Texture2D: Debug {
    /// Returns a reference to the wgpu texture
    fn texture(&self) -> &wgpu::Texture;
    /// Returns a reference to the sampler of this texture
    fn sampler(&self) -> &wgpu::Sampler;
    /// Returns a reference to the view of this texture
    fn view(&self) -> &wgpu::TextureView;
}

pub struct AtlasHandle {
    pub name: String,
    id: usize,
}

pub struct AtlasTexture {
    atlas_texture: TextureBundle,
}
impl AtlasTexture {
    pub fn new(atlas_texture: TextureBundle) -> Self {
        Self { atlas_texture }
    }

    // pub fn register_subregion()
}
