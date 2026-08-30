use super::Asset;

impl Asset for wgpu::ShaderModule {
    type LoadSettings = ();
}
