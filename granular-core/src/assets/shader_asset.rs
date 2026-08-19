use std::borrow::Cow;
use wgpu::{ShaderModule, ShaderModuleDescriptor};

use crate::{graphics::GraphicsSystem, utils::*};

use super::{Asset, AssetSystem};

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
    fn from_bytes(ctx: &GeeseContextHandle<AssetSystem>, bytes: &[u8]) -> anyhow::Result<Self> {
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let shader_src = str::from_utf8(bytes)?;
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
        });

        Ok(Self { module })
    }
}
