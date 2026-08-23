use glam::{IVec2, UVec2, Vec2};
use guillotiere::*;
use rustc_hash::FxHashMap;
use wgpu::Extent3d;

use crate::{
    graphics::{Texture2D, TextureBundle, TextureHandle},
    utils::*,
};

type HashMap<K, V> = FxHashMap<K, V>;

#[derive(Debug)]
struct AtlasSubregion {
    // top left
    pub uv_start: Vec2,
    // bottom right
    pub uv_end: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasSubregionHandle(u32);

pub struct TextureAtlas {
    pub width: u32,
    pub height: u32,
    atlas_texture: TextureBundle,
    next_handle_id: u32,
    subregions: HashMap<AtlasSubregionHandle, AtlasSubregion>,
}
impl TextureAtlas {
    pub const DEFAULT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        filtering: wgpu::FilterMode,
    ) -> Self {
        let atlas_texture = TextureBundle::new(
            device,
            queue,
            &format!("TextureAtlas {}x{}", width, height),
            wgpu::TextureDescriptor {
                label: Some("TextureAsset Desc"),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::DEFAULT_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            &wgpu::TextureViewDescriptor::default(),
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filtering,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: match filtering {
                    wgpu::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
                    wgpu::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
                },
                ..Default::default()
            },
            &vec![
                0u8;
                width as usize
                    * height as usize
                    * crate::graphics::bytes_per_pixel(Self::DEFAULT_FORMAT).unwrap() as usize
            ],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(
                    crate::graphics::bytes_per_pixel(Self::DEFAULT_FORMAT).unwrap_or(4) * width,
                ),
                rows_per_image: Some(height),
            },
        );
        Self::new_from_existing(atlas_texture, width, height)
    }

    pub fn new_from_existing(atlas_texture: TextureBundle, width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            atlas_texture,
            next_handle_id: 0,
            subregions: HashMap::default(),
        }
    }

    fn get_next_handle_id(&mut self) -> u32 {
        let id = self.next_handle_id;
        self.next_handle_id += 1;
        id
    }

    pub fn register_subregion(&mut self, uv_start: Vec2, uv_end: Vec2) -> AtlasSubregionHandle {
        let handle = AtlasSubregionHandle(self.get_next_handle_id());
        self.subregions
            .insert(handle, AtlasSubregion { uv_start, uv_end });
        handle
    }

    pub fn register_tile(
        &mut self,
        tile_size_px: IVec2,
        tile_coords: IVec2,
    ) -> AtlasSubregionHandle {
        let uv_start = (tile_coords * tile_size_px).as_vec2()
            / Vec2::new(
                self.atlas_texture.width() as f32,
                self.atlas_texture.height() as f32,
            );
        let uv_end = (tile_coords * tile_size_px + tile_size_px).as_vec2()
            / Vec2::new(
                self.atlas_texture.width() as f32,
                self.atlas_texture.height() as f32,
            );
        self.register_subregion(uv_start, uv_end)
    }

    pub fn remove_subregion(&mut self, subregion: AtlasSubregionHandle) {
        self.subregions.remove(&subregion);
    }

    /// Returns the texture coords (top left, bottom right)
    pub fn get_texture_coords(&self, region: AtlasSubregionHandle) -> Option<(Vec2, Vec2)> {
        self.subregions
            .get(&region)
            .map(|reg| (reg.uv_start, reg.uv_end))
    }
}
impl Texture2D for TextureAtlas {
    fn texture(&self) -> &wgpu::Texture {
        self.atlas_texture.texture()
    }

    fn view(&self) -> &wgpu::TextureView {
        self.atlas_texture.view()
    }

    fn sampler(&self) -> &wgpu::Sampler {
        self.atlas_texture.sampler()
    }
}
impl std::fmt::Debug for TextureAtlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Texture atlas {}x{}", self.width, self.height)
    }
}

struct DynamicAtlasSubregion {
    handle: AtlasSubregionHandle,
    allocation_id: AllocId,
    pos_start: UVec2,
    #[allow(unused)]
    pos_end: UVec2,
}

/// A dynamic texture atlas which allows to add textures to it which it then renders onto an underlying texture atlas.
///
/// Doesn't support mipmaps yet, as that would require getting the wgpu::Texture for each TextureHandle to fetch mipmaps and then
/// reading back the Texture data from the GPU using a command encoder and a buffer which is an async operation and quite expensive.
pub struct DynamicTextureAtlas {
    texture_atlas: TextureAtlas,
    allocator: AtlasAllocator,
    contained_textures: HashMap<TextureHandle, DynamicAtlasSubregion>,
    dirty_textures: Vec<TextureHandle>,
}
impl DynamicTextureAtlas {
    #[allow(unused)]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        filtering: wgpu::FilterMode,
    ) -> Self {
        let allocator = AtlasAllocator::new(size2(width as i32, height as i32));
        if filtering == wgpu::FilterMode::Linear {
            warn!("Linear filtering is not supported for the DynamicTextureAtlas at the moment");
        }
        Self {
            texture_atlas: TextureAtlas::new(device, queue, width, height, filtering),
            allocator,
            contained_textures: HashMap::default(),
            dirty_textures: vec![],
        }
    }

    /// Registers another texture to use in the atlas. The texture atlas will try to allocate a place for it in the atlas
    #[allow(unused)]
    pub fn add_texture(
        &mut self,
        texture: TextureHandle,
        texture_size: UVec2,
    ) -> anyhow::Result<AtlasSubregionHandle> {
        let padding_pixels = 1;
        let padding = UVec2::new(padding_pixels, padding_pixels);
        let allocation = self
            .allocator
            .allocate(size2(
                (texture_size.x + padding_pixels * 2) as i32,
                (texture_size.y + padding_pixels * 2) as i32,
            ))
            .ok_or(anyhow::anyhow!(
                "Could not allocate space for texture. Atlas might be full"
            ))?;
        let at_size = Vec2::new(
            self.texture_atlas.width as f32,
            self.texture_atlas.height as f32,
        );
        let s = allocation.rectangle.min.to_tuple();
        let pos_start = UVec2::new(s.0 as u32, s.1 as u32) + padding;
        let s = allocation.rectangle.max.to_tuple();
        let pos_end = UVec2::new(s.0 as u32, s.1 as u32) - padding;
        let uv_start = pos_start.as_vec2() / at_size;
        let uv_end = pos_end.as_vec2() / at_size;

        assert!(pos_end - pos_start == texture_size);

        let handle = self.texture_atlas.register_subregion(uv_start, uv_end);
        self.contained_textures.insert(
            texture.clone(),
            DynamicAtlasSubregion {
                handle,
                allocation_id: allocation.id,
                pos_start,
                pos_end,
            },
        );
        self.dirty_textures.push(texture);

        Ok(handle)
    }

    /// Checks if the given texture is contained in this atlas
    pub fn contains_texture(&self, texture: &TextureHandle) -> bool {
        self.contained_textures.contains_key(texture)
    }

    /// Returns the texture coords (top left, bottom right)
    pub fn get_texture_coords(&self, texture: &TextureHandle) -> Option<(Vec2, Vec2)> {
        if let Some(region) = self.contained_textures.get(texture) {
            return self.texture_atlas.get_texture_coords(region.handle);
        }
        None
    }

    #[allow(unused)]
    pub fn remove_texture(&mut self, texture: TextureHandle) {
        if let Some(dyn_region) = self.contained_textures.remove(&texture) {
            self.allocator.deallocate(dyn_region.allocation_id);
            self.texture_atlas.remove_subregion(dyn_region.handle);
        }
    }

    /// Mark a texture to be dirty to force it to be updated in the texture atlas the next time `rebuild_atlas` is called
    #[allow(unused)]
    pub fn mark_texture_dirty(&mut self, texture: TextureHandle) {
        self.dirty_textures.push(texture);
    }

    /// Writes the textures used in the atlas to the atlas texture. Only writes those which are dirty
    #[allow(unused)]
    pub fn rebuild_atlas<'a, 'b, F: Fn(&TextureHandle) -> &'a wgpu::Texture>(
        &mut self,
        handle_to_texture_func: F,
        encoder: &'b mut wgpu::CommandEncoder,
    ) {
        self.dirty_textures
            .iter()
            .filter_map(|handle| self.contained_textures.get(handle).map(|r| (handle, r)))
            .for_each(|(tex_handle, region)| {
                let subtexture = handle_to_texture_func(tex_handle);
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfoBase {
                        texture: subtexture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfoBase {
                        texture: self.texture_atlas.texture(),
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: region.pos_start.x,
                            y: region.pos_start.y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    subtexture.size(),
                );
            });
        self.dirty_textures.clear();
    }
}
impl Texture2D for DynamicTextureAtlas {
    fn texture(&self) -> &wgpu::Texture {
        self.texture_atlas.texture()
    }

    fn view(&self) -> &wgpu::TextureView {
        self.texture_atlas.view()
    }

    fn sampler(&self) -> &wgpu::Sampler {
        self.texture_atlas.sampler()
    }
}
impl std::fmt::Debug for DynamicTextureAtlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dynamic {:?}", self.texture_atlas)
    }
}
