use wgpu::{
    Device, Extent3d, Queue, Sampler, SamplerDescriptor, TexelCopyBufferLayout, Texture,
    TextureDescriptor, TextureView, TextureViewDescriptor,
};

use crate::{graphics::Texture2D, utils::*};

#[derive(Debug)]
pub struct TextureBundle {
    texture: wgpu::Texture,
    data_layout: TexelCopyBufferLayout,
    view: TextureView,
    sampler: Sampler,
}
impl TextureBundle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &Device,
        queue: &Queue,
        label: &str,
        mut tex_descriptor: TextureDescriptor,
        view_descriptor: &TextureViewDescriptor,
        sampler_descriptor: &SamplerDescriptor,
        data: &[u8],
        mut data_layout: TexelCopyBufferLayout,
    ) -> Self {
        let mut converted_data: Option<Vec<u8>> = None;
        if let Ok(img) = image::load_from_memory(data) {
            converted_data = crate::graphics::convert_image(&img, tex_descriptor.format).ok();
            let auto_extent = Extent3d {
                width: img.width(),
                height: img.height(),
                depth_or_array_layers: 1,
            };
            if auto_extent != tex_descriptor.size {
                trace!(
                    "Overriding texture size for newly created texture as given size of {:?} does not seem to fit. New size: {:?}",
                    tex_descriptor.size, auto_extent
                );
                tex_descriptor.size = auto_extent;
                data_layout = wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        crate::graphics::bytes_per_pixel(tex_descriptor.format).unwrap_or(4)
                            * auto_extent.width,
                    ),
                    rows_per_image: Some(auto_extent.height),
                };
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            ..tex_descriptor
        });
        let view = texture.create_view(view_descriptor);
        let sampler = device.create_sampler(sampler_descriptor);

        let upload_data = converted_data.as_deref().unwrap_or(data);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            upload_data,
            data_layout,
            texture.size(),
        );

        Self {
            texture,
            data_layout,
            view,
            sampler,
        }
    }

    pub fn default(device: &Device, queue: &Queue, extent: Extent3d, data: &[u8]) -> Self {
        let tex_descriptor = wgpu::TextureDescriptor {
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: None,
            view_formats: &[],
        };

        let view_descriptor = TextureViewDescriptor::default();

        let sampler_descriptor = wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        };
        let data_layout = wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(
                crate::graphics::bytes_per_pixel(tex_descriptor.format).unwrap_or(4) * extent.width,
            ),
            rows_per_image: Some(extent.height),
        };

        Self::new(
            device,
            queue,
            "New default texture",
            tex_descriptor,
            &view_descriptor,
            &sampler_descriptor,
            data,
            data_layout,
        )
    }

    pub fn view(&self) -> &TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    pub fn texture(&self) -> &Texture {
        &self.texture
    }

    pub fn data_layout(&self) -> TexelCopyBufferLayout {
        self.data_layout
    }

    pub fn width(&self) -> u32 {
        self.texture.size().width
    }
    pub fn height(&self) -> u32 {
        self.texture.size().height
    }
    pub fn extent(&self) -> Extent3d {
        self.texture.size()
    }
}
impl Texture2D for TextureBundle {
    fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}
