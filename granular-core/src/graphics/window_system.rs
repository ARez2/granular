use std::sync::Arc;

use winit::{
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

use crate::utils::*;

pub mod events {
    use std::sync::Arc;
    use winit::window::Window;

    pub struct WindowCreated(pub Arc<Window>);
}

pub struct WindowSystem {
    ctx: GeeseContextHandle<Self>,
    windows: Vec<Arc<Window>>,
}
impl WindowSystem {
    pub fn window_handle(&self) -> Arc<Window> {
        if self.windows.is_empty() {
            panic!("Tried getting a window handle but no windows exist.");
        }
        self.windows[0].clone()
    }

    pub fn init(&mut self, event_loop: &ActiveEventLoop) {
        let mut window_attributes = WindowAttributes::default()
            .with_title("Default Granular Window")
            .with_visible(false)
            .with_resizable(true)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
            .with_position(winit::dpi::PhysicalPosition::new(1500, 100));

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();
            window_attributes = window_attributes.with_canvas(Some(canvas));
        }

        let result = event_loop.create_window(window_attributes);
        if let Ok(window) = result {
            self.windows.push(Arc::new(window));
        } else if let Err(e) = result {
            panic!("OS Error while creating a new window: {}", e);
        }

        self.ctx
            .raise_event(events::WindowCreated(self.windows.last().unwrap().clone()));
    }
}
impl GeeseSystem for WindowSystem {
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,
            windows: vec![],
        }
    }
}
