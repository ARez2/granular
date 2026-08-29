use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::{graphics::Texture2D, utils::*};

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

pub struct TextureSystem {
    next_texture_id: u32,
    texture_storage: FxHashMap<TextureHandle, Box<dyn Texture2D>>,
}
impl TextureSystem {
    fn get_next_texture_id(&mut self) -> u32 {
        let id = self.next_texture_id;
        self.next_texture_id += 1;
        id
    }

    /// Stores a new texture in the GraphicsSystem and returns a handle to it, which can be used to retrieve the texture
    pub fn create_texture(&mut self, texture: Box<dyn Texture2D>) -> TextureHandle {
        let handle = TextureHandle::new(self.get_next_texture_id(), 0);
        self.texture_storage.insert(handle.clone(), texture);
        handle
    }

    /// Uses the handle to get a reference to the texture
    pub fn get_texture(&self, handle: &TextureHandle) -> Option<&dyn Texture2D> {
        self.texture_storage.get(handle).map(|v| &**v)
    }

    /// Uses the handle to get a mutable reference to the texture
    pub fn get_texture_mut(&mut self, handle: &TextureHandle) -> Option<&mut Box<dyn Texture2D>> {
        self.texture_storage.get_mut(handle)
    }
}
impl GeeseSystem for TextureSystem {
    fn new(_ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            next_texture_id: 0,
            texture_storage: FxHashMap::default(),
        }
    }
}
