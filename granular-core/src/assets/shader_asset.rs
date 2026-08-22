use std::borrow::Cow;
use wgpu::{ShaderModule, ShaderModuleDescriptor};

use crate::{graphics::GraphicsSystem, utils::*};

use super::{Asset, AssetSystem};

/// Asset to hold a wgpu::ShaderModule
#[derive(Debug)]
pub struct ShaderAsset {
    module: ShaderModule,
}
impl ShaderAsset {
    pub fn module(&self) -> &ShaderModule {
        &self.module
    }
}
impl Asset for ShaderAsset {
    // could be used, if shaders need more settings
    type ImportSettings = ();

    fn create_from_bytes(
        ctx: &mut GeeseContextHandle<AssetSystem>,
        bytes: &[u8],
        _import_settings: &Self::ImportSettings,
    ) -> anyhow::Result<Self> {
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let shader_src = str::from_utf8(bytes)?;
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
        });

        Ok(Self { module })
    }

    fn update_from_bytes(
        &mut self,
        ctx: &mut GeeseContextHandle<AssetSystem>,
        bytes: &[u8],
        _import_settings: &Self::ImportSettings,
    ) -> anyhow::Result<()> {
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let shader_src = str::from_utf8(bytes)?;
        self.module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
        });
        Ok(())
    }
}
