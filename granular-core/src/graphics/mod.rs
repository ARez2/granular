mod graphics_system;
pub(super) use graphics_system::GraphicsState;
pub use graphics_system::GraphicsSystem;

mod graphics_helpers;
pub use graphics_helpers::*;

mod texture;
pub use texture::{Texture2D, TextureHandle};

mod texture_bundle;
pub use texture_bundle::TextureBundle;

pub(super) mod window_system;
pub use window_system::WindowSystem;

mod camera;
pub use camera::Camera;

mod renderer;
pub use renderer::Renderer;

mod batchrenderer;
pub use batchrenderer::{BatchRenderer, Quad};

// mod simulation_renderer;
// pub use simulation_renderer::SimulationRenderer;

mod texture_atlas;
pub use texture_atlas::TextureAtlas;

#[cfg(target_arch = "wasm32")]
pub fn get_canvas() -> web_sys::HtmlCanvasElement {
    use crate::utils::*;
    use wasm_bindgen::JsCast;
    let canvas = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .unwrap();
    canvas
}
