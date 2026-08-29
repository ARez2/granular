#[allow(unused)]
use crate::utils::*;

/// Trait that all different textures need to implement.
pub trait Texture2D {
    /// Returns a reference to the wgpu texture
    fn texture(&self) -> &wgpu::Texture;
    /// Returns a reference to the sampler of this texture
    fn sampler(&self) -> &wgpu::Sampler;
    /// Returns a reference to the view of this texture
    fn view(&self) -> &wgpu::TextureView;
}
