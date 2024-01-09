use geese::*;
use wgpu::{Instance, Adapter, Surface, InstanceDescriptor, Backends};

use super::WindowSystem;


pub struct GraphicsBackend {
    instance: Instance,
    adapter: Adapter,
    surface: Surface<'static>
}
impl GraphicsBackend {
    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    pub fn surface(&self) -> &Surface {
        &self.surface
    }
}
impl GeeseSystem for GraphicsBackend {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<WindowSystem>();

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let window = ctx.get::<WindowSystem>();

        let instance = wgpu::Instance::new(InstanceDescriptor {
            backends: Backends::VULKAN,
            ..Default::default()
        });
        let surface = instance.create_surface(window.window_handle()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })).expect("Failed to find an appropriate adapter");

        Self {
            instance,
            adapter,
            surface
        }
    }
}