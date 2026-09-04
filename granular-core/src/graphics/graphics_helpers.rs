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

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct BindGroupBuilder<'a> {
    entries: HashMap<
        u32,
        (
            Option<wgpu::BindGroupLayoutEntry>,
            Option<wgpu::BindGroupEntry<'a>>,
        ),
    >,
}
impl<'a> BindGroupBuilder<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    #[must_use]
    /// Registers another binding at `binding_nr` without a resource
    pub fn add_binding(
        mut self,
        binding_nr: u32,
        visibility: wgpu::ShaderStages,
        bindtype: wgpu::BindingType,
    ) -> Self {
        if self.entries.contains_key(&binding_nr) {
            panic!(
                "There already exists a binding with binding number {}",
                binding_nr
            );
        }
        self.entries.insert(
            binding_nr,
            (
                Some(wgpu::BindGroupLayoutEntry {
                    binding: binding_nr,
                    visibility,
                    ty: bindtype,
                    count: None,
                }),
                None,
            ),
        );

        self
    }

    #[must_use]
    /// Registers another binding at `binding_nr` with a resource attached to it
    pub fn add_binding_with_resource(
        mut self,
        binding_nr: u32,
        visibility: wgpu::ShaderStages,
        bindtype: wgpu::BindingType,
        resource: wgpu::BindingResource<'a>,
    ) -> Self {
        if self.entries.contains_key(&binding_nr) {
            panic!(
                "There already exists a binding with binding number {}",
                binding_nr
            );
        }
        self.entries.insert(
            binding_nr,
            (
                Some(wgpu::BindGroupLayoutEntry {
                    binding: binding_nr,
                    visibility,
                    ty: bindtype,
                    count: None,
                }),
                Some(wgpu::BindGroupEntry {
                    binding: binding_nr,
                    resource,
                }),
            ),
        );

        self
    }

    #[must_use]
    pub fn add_resource_to_binding(
        mut self,
        binding_nr: u32,
        resource: wgpu::BindingResource<'a>,
    ) -> Self {
        if !self.entries.contains_key(&binding_nr) {
            panic!(
                "Trying to add a resource to a non existing binding with number {}",
                binding_nr
            );
        }
        self.entries.get_mut(&binding_nr).unwrap().1 = Some(wgpu::BindGroupEntry {
            binding: binding_nr,
            resource,
        });

        self
    }

    #[must_use]
    /// Creates the final bind group from the entries collected before and names the layout "<label> layout" and the bind group "<label> bind group"
    pub fn build(
        self,
        label: &str,
        device: &wgpu::Device,
    ) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
        let mut layout_entries = vec![];
        let mut bindgroup_entries = vec![];
        for (bind_nr, (layout_opt, bg_entry_opt)) in self.entries {
            if let Some(layout) = layout_opt {
                layout_entries.push(layout);
            } else {
                panic!(
                    "Binding number {} does not have a layout registered for it.",
                    bind_nr
                );
            }
            if let Some(bg_entry) = bg_entry_opt {
                bindgroup_entries.push(bg_entry);
            } else {
                panic!(
                    "Binding number {} does not have a resource registered for it.",
                    bind_nr
                );
            }
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{} layout", label)),
            entries: &layout_entries,
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &layout,
            entries: &bindgroup_entries,
        });
        (layout, bg)
    }
}

pub trait IntoGpuColor {
    fn into_gpu_color(self) -> [f32; 4];
}

impl<T> IntoGpuColor for palette::Srgb<T>
where
    palette::Srgb<T>: Copy,
    palette::LinSrgba<f32>: From<palette::Srgb<T>>,
{
    fn into_gpu_color(self) -> [f32; 4] {
        let color: palette::LinSrgba<f32> = self.into();
        palette::WithAlpha::with_alpha(color, 1.0)
            .into_components()
            .into()
    }
}

impl<T> IntoGpuColor for palette::Srgba<T>
where
    palette::Srgba<T>: Copy,
    palette::LinSrgba<f32>: From<palette::Srgba<T>>,
{
    fn into_gpu_color(self) -> [f32; 4] {
        let color: palette::LinSrgba<f32> = self.into();
        color.into_components().into()
    }
}
