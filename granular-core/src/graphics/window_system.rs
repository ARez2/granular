use std::sync::{Arc, atomic::AtomicBool};

use winit::{
    dpi::PhysicalSize,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowAttributes},
};

use crate::{CustomWinitEvent, utils::*};

pub mod events {
    use std::sync::Arc;
    use winit::window::Window;

    #[allow(unused)]
    pub struct WindowCreated(pub Arc<Window>);
}

pub struct WindowSystem {
    ctx: GeeseContextHandle<Self>,
    windows: Vec<Arc<Window>>,
    eventloop_proxy: Option<EventLoopProxy<CustomWinitEvent>>,
    engine_waiting_bool: Option<Arc<AtomicBool>>,
}
impl WindowSystem {
    pub(super) fn window_handle(&self) -> Arc<Window> {
        if self.windows.is_empty() {
            panic!("Tried getting a window handle but no windows exist.");
        }
        self.windows[0].clone()
    }

    pub(crate) fn init(
        &mut self,
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<CustomWinitEvent>,
        engine_waiting_bool: Arc<AtomicBool>,
    ) {
        self.eventloop_proxy = Some(proxy);
        self.engine_waiting_bool = Some(engine_waiting_bool);

        #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
        let mut window_attributes = WindowAttributes::default()
            .with_title("Default Granular Window")
            .with_inner_size(PhysicalSize::new(512, 512))
            .with_visible(true)
            .with_resizable(true);

        #[cfg(not(target_arch = "wasm32"))]
        let window_attributes =
            window_attributes.with_position(winit::dpi::PhysicalPosition::new(1500, 100));

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = crate::graphics::get_canvas();
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

    pub fn set_title(&mut self, title: &str) {
        self.window_handle().set_title(title);
    }

    pub fn window_size(&self) -> PhysicalSize<u32> {
        self.window_handle().inner_size()
    }

    /// Sets the windows inner (canvas) size.
    pub fn set_window_size(&mut self, size: PhysicalSize<u32>) {
        let win = self.window_handle();
        // From the docs of request_inner_size():
        // > On platforms where the size is entirely controlled by the user the applied size will be returned immediately, resize event in such case may not be generated.
        // > On platforms where resizing is disallowed by the windowing system, the current inner size is returned immediately, and the user one is ignored.
        // > When None is returned, it means that the request went to the display system, and the actual size will be delivered later with the WindowEvent::Resized.
        //
        // So, we emit an event to make sure other systems know of this resize
        if size != win.inner_size() {
            let res = win.request_inner_size(size);
            if let Some(new_size) = res {
                let _ = self
                    .eventloop_proxy
                    .as_ref()
                    .unwrap()
                    .send_event(CustomWinitEvent::WindowResized(new_size));
                self.engine_waiting_bool
                    .as_mut()
                    .unwrap()
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}
impl GeeseSystem for WindowSystem {
    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        Self {
            ctx,
            windows: vec![],
            eventloop_proxy: None,
            engine_waiting_bool: None,
        }
    }
}
