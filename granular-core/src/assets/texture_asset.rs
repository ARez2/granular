use std::path::PathBuf;
use wgpu::{Extent3d, Sampler, Texture, TextureView};
use geese::GeeseContextHandle;

use crate::graphics::GraphicsSystem;
use super::{Asset, AssetServer};



#[derive(Debug)]
pub struct TextureAsset {
    extent: Extent3d,
    texture: Texture,
    view: TextureView,
    sampler: Sampler
}
impl TextureAsset {
    pub fn view(&self) -> &TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    pub fn width(&self) -> u32 {
        self.extent.width
    }
    pub fn height(&self) -> u32 {
        self.extent.height
    }
    pub fn extent(&self) -> &Extent3d {
        &self.extent
    }
}
impl Asset for TextureAsset {
    fn from_path(ctx: &GeeseContextHandle<AssetServer>, path: &PathBuf) -> Self {
        let sys = ctx.get::<GraphicsSystem>();
        let device = sys.device();
        let queue = sys.queue();

        let img = image::open(path).unwrap().to_rgba8();
        let extent = Extent3d {width: img.width(), height: img.height(), depth_or_array_layers: 1};
        let texture_descriptor = wgpu::TextureDescriptor {
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: None,
            view_formats: &[],
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: path.to_str(),
            view_formats: &[],
            ..texture_descriptor
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * extent.width),
                rows_per_image: Some(extent.height),
            },
            extent,
        );
        Self {
            extent,
            texture,
            view,
            sampler
        }
    }
}