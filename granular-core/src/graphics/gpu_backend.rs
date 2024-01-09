use std::{future::Future, sync::Arc};

use geese::*;
use log::warn;
use wgpu::{Instance, Adapter, DeviceType, Backends, AdapterInfo, Backend, Device, Queue, DeviceDescriptor, Features, Limits};

type Adapters = Vec<(Arc<Adapter>, GpuInfo)>;

mod events {
    use wgpu::{Device, Queue, RequestDeviceError};

    use super::Adapters;
    
    pub struct LoadedAllAdapters {
        adapters: Adapters
    }
    impl LoadedAllAdapters {
        pub fn new(adapters: Adapters) -> Self {
            Self {
                adapters
            }
        }

        pub fn get(&self) -> Adapters {
            self.adapters
        }
    }

    /// Instructs the `GpuBackend` to update its device and queue.
    pub struct SetDeviceQueue {
        /// The chosen adapter index.
        pub adapter: usize,
        /// Whether the program should abort if this adapter fails to load.
        pub abort_on_fail: bool,
        /// The result of loading the device and queue.
        result: Result<(Device, Queue), RequestDeviceError>
    }
    impl SetDeviceQueue {
        /// Creates a new event.
        pub fn new(adapter: usize, abort_on_fail: bool, res: Result<(Device, Queue), RequestDeviceError>) -> Self {
            Self {
                adapter,
                abort_on_fail,
                result: res
            }
        }

        /// Obtains the load result.
        pub fn get(&self) -> Result<(Device, Queue), RequestDeviceError> {
            self.result
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
struct GpuID {
    id: u32,
    backend_id: u8
}

/// Identifies and describes an available GPU device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuInfo {
    /// The unique identifier of the GPU.
    pub id: GpuID,
    /// Metadata about the GPU adapter.
    pub info: AdapterInfo
}

impl From<AdapterInfo> for GpuInfo {
    fn from(value: AdapterInfo) -> Self {
        Self {
            id: GpuID { id: value.device, backend_id: value.backend as u8 },
            info: value
        }
    }
}



pub struct GPUBackend {
    ctx: GeeseContextHandle<Self>,
    instance: Instance,
    adapters: Adapters,
    selected_adapter_idx: usize,
    pub loaded_device_queue: Option<(Device, Queue)>,
}
impl GPUBackend {
    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    /// Retrieves information about the current adapter.
    pub fn selected_adapter(&self) -> &GpuInfo {
        &self.adapters[self.selected_adapter_idx].1
    }

    fn gpu_preference_key(device_type: DeviceType) -> u8 {
        match device_type {
            DeviceType::Other => 3,
            DeviceType::IntegratedGpu => 1,
            DeviceType::DiscreteGpu => 0,
            DeviceType::VirtualGpu => 2,
            DeviceType::Cpu => 4,
        }
    }

    /// Begins loading all available adapters for the given instance.
    fn load_all_adapters(instance: &Instance) -> impl Future<Output = events::LoadedAllAdapters> {
        let mut result = instance
            .enumerate_adapters(Backends::all())
            .map(|ad| {
                let a = Arc::new(ad);
                (a.clone(), Self::update_adapter_info_names(a.clone().get_info().into()))
            })
            .collect::<Vec<_>>();
        result.sort_by_key(|(_, info)| Self::gpu_preference_key(info.info.device_type));
        async move { events::LoadedAllAdapters::new(result) }
    }

    /// Responds to the LoadedAllAdapters event and sets it in GPUBackend
    fn set_all_adapters(&mut self, event: &events::LoadedAllAdapters) {
        self.adapters = event.get();
        println!("{:?}", self.adapters);
        // let chosen_gpu = self.ctx.get::<Store<GameSettings>>().gpu;
        // self.selected_adapter = self.adapter_infos.iter().position(|x| x.id == chosen_gpu).unwrap_or(usize::MAX);
        self.selected_adapter_idx = 0;
    }

    /// Adds names to adapters which do not have any.
    fn update_adapter_info_names(mut info: GpuInfo) -> GpuInfo {
        if info.info.name == "" {
            info.info.name = match info.info.backend {
                Backend::Empty => "Empty",
                Backend::Vulkan => "Vulkan",
                Backend::Metal => "Metal",
                Backend::Dx12 => "DirectX 12",
                Backend::Gl => "OpenGL",
                Backend::BrowserWebGpu => "Browser WebGPU",
            }.to_string();
        }

        info
    }

    
    /// Begins loading a device and queue from the provided adapter.
    pub(crate) fn load_device_queue(&mut self, index: usize, force_load: bool) {
        if index != self.selected_adapter_idx || force_load {
            self.selected_adapter_idx = index;
            
            let limits = self.adapters[index].0.limits();
            let device_queue_future = self.adapters[index].0.request_device(&DeviceDescriptor {
                label: Some("Device"),
                required_features: Features::empty(),
                required_limits: Limits {
                    max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                    max_buffer_size: limits.max_buffer_size,
                    ..Default::default()
                }
            }, None);
            let res = pollster::block_on(device_queue_future);
            self.ctx.raise_event(events::SetDeviceQueue::new(index, false, res));
        }
    }


    /// Completes loading a new device and queue, or records an error if loading failed.
    fn set_device_queue(&mut self, event: &events::SetDeviceQueue) {
        match event.get() {
            Ok((device, queue)) => {
                #[cfg(not(debug_assertions))]
                {
                    let needs_reset = self.needs_reset.clone();
                    device.on_uncaptured_error(Box::new(move |x| {
                        error!("GPU device error: {x}");
                        needs_reset.store(true, Ordering::Release);
                    }));
                }
                self.loaded_device_queue = Some((device, queue));
            },
            Err(x) => {
                if event.abort_on_fail {
                    panic!("Failed to load device from adapter: {x:?}");
                }
                else {
                    warn!("Failed to load device from adapter {:?}; falling back to adapter {:?}. Reason: {x:?}", self.adapters[event.adapter].1, self.adapters[self.selected_adapter_idx]);
                    //self.ctx.get_mut::<Store<GameSettings>>().gpu = self.adapter_infos[self.selected_adapter].id;
                }
            }
        }
        
    }

}
impl GeeseSystem for GPUBackend {
    const EVENT_HANDLERS: EventHandlers<Self> = event_handlers()
        .with(Self::set_all_adapters)
        .with(Self::set_device_queue);

    fn new(ctx: GeeseContextHandle<Self>) -> Self {
        let instance = Instance::new(
            wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                ..Default::default()
            });
        let loaded_device_queue = None;
        pollster::block_on(Self::load_all_adapters(&instance));
        
        Self {
            ctx,
            instance,
            adapters: vec![],
            selected_adapter_idx: usize::MAX,
            loaded_device_queue,
        }
    }
}