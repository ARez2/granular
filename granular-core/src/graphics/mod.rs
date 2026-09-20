mod graphics_system;
pub(super) use graphics_system::GraphicsState;
pub use graphics_system::{GraphicsSystem, RenderContext, events};

mod graphics_helpers;
pub use graphics_helpers::*;

mod texture2d;
pub use texture2d::Texture2D;

mod vertex;

mod texture_bundle;
pub use texture_bundle::{TextureBundle, TextureBundleLoadSettings};

pub(super) mod window_system;
pub use window_system::WindowSystem;

mod camera;
pub use camera::Camera;

mod batchrenderer;
pub use batchrenderer::{BatchRenderer, QuadTex};
mod draw_space;
pub use draw_space::DrawSpace;
mod quad_geometry;
pub mod view_mapping;
pub use view_mapping::{
    Camera2D, CameraMotion, GamePixelPos, Presentation, RenderView, ScalingMode, SurfacePos, UiPos,
    WorldPos,
};

mod game_renderer;
pub(crate) use game_renderer::GameRenderer;

// mod simulation_renderer;
// pub use simulation_renderer::SimulationRenderer;

pub type TextureHandle = AssetHandle<TextureBundle>;

mod texture_atlas;
pub use texture_atlas::{AtlasSubregionHandle, *};

mod debug_draw;
pub use debug_draw::*;

use crate::assets::AssetHandle;

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
