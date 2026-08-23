use std::{fmt::Debug, sync::Arc};

use crate::utils::*;

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

    pub fn id(&self) -> u32 {
        *self.id
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
pub trait Texture2D {
    /// Returns a reference to the wgpu texture
    fn texture(&self) -> &wgpu::Texture;
    /// Returns a reference to the sampler of this texture
    fn sampler(&self) -> &wgpu::Sampler;
    /// Returns a reference to the view of this texture
    fn view(&self) -> &wgpu::TextureView;
}
