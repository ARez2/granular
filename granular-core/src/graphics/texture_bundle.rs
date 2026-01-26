use wgpu::{
    Device, Extent3d, ImageDataLayout, Queue, Sampler, SamplerDescriptor, Texture,
    TextureDescriptor, TextureView, TextureViewDescriptor,
};

#[derive(Debug)]
pub struct TextureBundle {
    extent: Extent3d,
    texture: Texture,
    data_layout: ImageDataLayout,
    view: TextureView,
    sampler: Sampler,
}
impl TextureBundle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &Device,
        queue: &Queue,
        label: &str,
        extent: Extent3d,
        tex_descriptor: TextureDescriptor,
        view_descriptor: &TextureViewDescriptor,
        sampler_descriptor: &SamplerDescriptor,
        data: &[u8],
        data_layout: ImageDataLayout,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            view_formats: &[],
            ..tex_descriptor
        });
        let view = texture.create_view(view_descriptor);
        let sampler = device.create_sampler(sampler_descriptor);

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            data_layout,
            extent,
        );

        Self {
            texture,
            data_layout,
            extent,
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
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        };
        let data_layout = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * extent.width),
            rows_per_image: Some(extent.height),
        };

        Self::new(
            device,
            queue,
            "New default texture",
            extent,
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

    pub fn data_layout(&self) -> ImageDataLayout {
        self.data_layout
    }

    pub fn width(&self) -> u32 {
        self.extent.width
    }
    pub fn height(&self) -> u32 {
        self.extent.height
    }
    pub fn extent(&self) -> Extent3d {
        self.extent
    }
}
impl PartialEq for TextureBundle {
    fn eq(&self, other: &Self) -> bool {
        self.extent == other.extent
            && self.texture.global_id() == other.texture.global_id()
            && self.view.global_id() == other.view.global_id()
            && self.sampler.global_id() == other.sampler.global_id()
    }
}
