use geese::*;
use wgpu::{Instance, Adapter, InstanceDescriptor, Backends};

use super::WindowSystem;


pub struct GraphicsBackend {
    instance: Instance,
    adapters: Vec<Adapter>,
    chosen_adapter_index: usize
}
impl GraphicsBackend {
    pub fn adapter(&self) -> &Adapter {
        &self.adapters[self.chosen_adapter_index]
    }

    pub fn instance(&self) -> &Instance {
        &self.instance
    }
}
impl GeeseSystem for GraphicsBackend {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<WindowSystem>();

    fn new(_ctx: GeeseContextHandle<Self>) -> Self {
        let instance = wgpu::Instance::new(InstanceDescriptor {
            backends: Backends::VULKAN,
            ..Default::default()
        });
        let adapters = instance.enumerate_adapters(Backends::VULKAN);
        let chosen = {
            if adapters.len() == 1 {
                0
            } else {
                0 // TODO: do more here?
            }
        };

        Self {
            instance,
            adapters,
            chosen_adapter_index: chosen,
        }
    }
}