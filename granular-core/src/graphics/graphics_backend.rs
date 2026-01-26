use geese::*;
use wgpu::{Adapter, Backends, Instance, InstanceDescriptor, RequestAdapterOptions};

use super::WindowSystem;

pub struct GraphicsBackend {
    instance: Instance,
    adapter: Adapter,
}
impl GraphicsBackend {
    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    pub(super) fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    pub(super) fn set_adapter(&mut self, adapter: Adapter) {
        self.adapter = adapter;
    }
}
impl GeeseSystem for GraphicsBackend {
    const DEPENDENCIES: Dependencies = dependencies().with::<WindowSystem>();

    fn new(_ctx: GeeseContextHandle<Self>) -> Self {
        let instance = wgpu::Instance::new(InstanceDescriptor {
            backends: Backends::VULKAN,
            ..Default::default()
        });
        let adapter =
            pollster::block_on(instance.request_adapter(&RequestAdapterOptions::default()))
                .expect("Cannot request any adapter");

        Self { instance, adapter }
    }
}
