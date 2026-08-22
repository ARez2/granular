pub fn bytes_per_pixel(format: wgpu::TextureFormat) -> anyhow::Result<u32> {
    match format {
        wgpu::TextureFormat::R8Unorm => Ok(1),
        wgpu::TextureFormat::Rg8Unorm => Ok(2),

        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => Ok(4),

        wgpu::TextureFormat::R16Unorm => Ok(2),
        wgpu::TextureFormat::Rg16Unorm => Ok(4),
        wgpu::TextureFormat::Rgba16Unorm => Ok(8),

        wgpu::TextureFormat::R32Float => Ok(4),
        wgpu::TextureFormat::Rg32Float => Ok(8),
        wgpu::TextureFormat::Rgba32Float => Ok(16),

        wgpu::TextureFormat::R16Float => Ok(2),
        wgpu::TextureFormat::Rg16Float => Ok(4),
        wgpu::TextureFormat::Rgba16Float => Ok(8),

        _ => anyhow::bail!("Format has no simple bytes-per-pixel"),
    }
}

pub fn convert_image(
    img: &image::DynamicImage,
    format: wgpu::TextureFormat,
) -> anyhow::Result<Vec<u8>> {
    match format {
        wgpu::TextureFormat::R8Unorm => Ok(img.to_luma8().into_raw()),

        wgpu::TextureFormat::Rg8Unorm => Ok(img.to_luma_alpha8().into_raw()),

        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            Ok(img.to_rgba8().into_raw())
        }

        wgpu::TextureFormat::R16Unorm => Ok(img
            .to_luma16()
            .into_raw()
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect()),

        wgpu::TextureFormat::Rg16Unorm => Ok(img
            .to_luma_alpha16()
            .into_raw()
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect()),

        wgpu::TextureFormat::Rgba16Unorm => Ok(img
            .to_rgba16()
            .into_raw()
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect()),

        _ => anyhow::bail!(
            "Unsupported image conversion to wgpu texture format {:?}",
            format
        ),
    }
}
