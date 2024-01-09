use geese::*;
use log::{info, debug};
use wgpu::{Device, Queue, Limits};

use super::GPUBackend;


pub struct GPU {
    device: Device,
    queue: Queue,
    limits: Limits
}
impl GeeseSystem for GPU {
    const DEPENDENCIES: Dependencies = dependencies()
        .with::<GPUBackend>();
    fn new(mut ctx: GeeseContextHandle<Self>) -> Self {
        let mut backend = ctx.get_mut::<GPUBackend>();
        let (device, queue) = backend.loaded_device_queue.take().expect("Device and queue were not loaded.");
        let limits = device.limits();
        info!("GPU loaded for {:?}: {:?}", std::thread::current().id(), backend.selected_adapter().info);
        drop(backend);
        debug!("GPU device limits: {limits:?}");
        Self {
            device,
            queue,
            limits
        }
    }
}