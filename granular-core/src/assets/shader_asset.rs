use std::borrow::Cow;

use log::error;
use wgpu::{ShaderModule, ShaderModuleDescriptor};

use crate::graphics::GraphicsSystem;

use super::Asset;



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
    fn from_path(ctx: &geese::GeeseContextHandle<super::AssetServer>, path: &std::path::PathBuf) -> Self {
        let graphics_sys = ctx.get::<GraphicsSystem>();
        let device = graphics_sys.device();

        let shader_contents = std::fs::read_to_string(path);
        let shader_src = match shader_contents {
            Ok(data) => {data},
            Err(e) => {
                error!("Error while reading shader: {:?}", e);
                String::new()
            }
        };
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(path.to_str().unwrap()),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_src)),
        });

        Self {
            module,
        }
    }
}