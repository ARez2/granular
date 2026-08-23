use anyhow::anyhow;

use crate::{
    assets::Asset,
    graphics::{GraphicsSystem, TextureBundle, TextureHandle},
    utils::*,
};

/// Settings you might want to set when loading a texture. Not complete
#[derive(Debug, PartialEq, Eq)]
pub struct TextureAssetImportSettings {
    pub size: wgpu::Extent3d,
    pub format: wgpu::TextureFormat,
    pub filtering: wgpu::FilterMode,
}
impl Default for TextureAssetImportSettings {
    fn default() -> Self {
        Self {
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            filtering: wgpu::FilterMode::Nearest,
        }
    }
}

/// A TextureAsset can store any texture via a TextureHandle, which is used with the GraphicsSystem to retrieve the underlying texture.
#[derive(Debug)]
pub struct TextureAsset {
    texture_handle: TextureHandle,
}
impl TextureAsset {
    pub fn handle(&mut self) -> TextureHandle {
        self.texture_handle.clone()
    }

    fn create_bundle_from_import_settings(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        import_settings: &TextureAssetImportSettings,
        bytes: &[u8],
    ) -> TextureBundle {
        TextureBundle::new(
            device,
            queue,
            "TextureAsset",
            wgpu::TextureDescriptor {
                label: Some("TextureAsset Desc"),
                size: import_settings.size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: import_settings.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
            &wgpu::TextureViewDescriptor::default(),
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: import_settings.filtering,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: match import_settings.filtering {
                    wgpu::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
                    wgpu::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
                },
                ..Default::default()
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(
                    crate::graphics::bytes_per_pixel(import_settings.format).unwrap_or(4)
                        * import_settings.size.width,
                ),
                rows_per_image: Some(import_settings.size.height),
            },
        )
    }
}
impl Asset for TextureAsset {
    type ImportSettings = TextureAssetImportSettings;

    fn create_from_bytes(
        ctx: &mut GeeseContextHandle<super::AssetSystem>,
        bytes: &[u8],
        import_settings: &Self::ImportSettings,
    ) -> anyhow::Result<Self>
    where
        Self: std::marker::Sized,
    {
        let mut graphics_sys = ctx.get_mut::<GraphicsSystem>();
        let bundle = Self::create_bundle_from_import_settings(
            graphics_sys.device(),
            graphics_sys.queue(),
            import_settings,
            bytes,
        );
        let texture_handle = graphics_sys.create_texture(Box::new(bundle));
        Ok(Self { texture_handle })
    }

    fn update_from_bytes(
        &mut self,
        ctx: &mut GeeseContextHandle<super::AssetSystem>,
        bytes: &[u8],
        import_settings: &Self::ImportSettings,
    ) -> anyhow::Result<()> {
        let graphics_sys = ctx.get_mut::<GraphicsSystem>();
        let queue = graphics_sys.queue();

        let tex = graphics_sys
            .get_texture(&self.texture_handle)
            .ok_or(anyhow!("Texture not found in GraphicsSystem."))?
            .texture();
        let extent = tex.size();

        let data_layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(
                crate::graphics::bytes_per_pixel(import_settings.format).unwrap_or(4)
                    * extent.width,
            ),
            rows_per_image: Some(extent.height),
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            data_layout,
            extent,
        );

        Ok(())
    }
}
