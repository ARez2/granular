use std::{marker::PhantomData, mem::{align_of, size_of}, num::NonZeroU64, ops::Range};

use bytemuck::{cast_slice, cast_slice_mut};
use bytemuck::{Pod, Zeroable};
use wgpu::{BindingResource, Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, QueueWriteBufferView};

use super::graphics_system::GraphicsSystem;


/// Manages a GPU-side `Buffer` and allows it to grow automatically to accomodate new data.
/// Thanks to Douglas Dwyer for this
#[derive(Debug)]
pub struct DynamicBuffer<T: Pod + Zeroable> {
    /// The underlying buffer.
    buffer: Buffer,
    /// Whether the buffer was reallocated since its last usage.
    dirty: bool,
    /// How the buffer will be used by the GPU.
    usage: BufferUsages,
    /// Marker data.
    marker: PhantomData<T>
}

impl<T: Pod + Zeroable> DynamicBuffer<T> {
    /// Creates a new dynamic buffer on the GPU with the given usages.
    pub fn new(name: &str, gpu: &GraphicsSystem, usage: BufferUsages) -> Self {
        Self::with_capacity(name, gpu, usage, 0)
    }

    /// Creates a new dynamic buffer on the GPU with the given usages, ensuring that it
    /// can hold at least `len` instances of `T` before reallocating.
    pub fn with_capacity(name: &str, gpu: &GraphicsSystem, mut usage: BufferUsages, len: usize) -> Self {
        usage |= BufferUsages::COPY_DST | BufferUsages::COPY_SRC;
        
        let elements = (len * size_of::<T>()).next_power_of_two() as u64;
        let buffer = gpu.device().create_buffer(&BufferDescriptor {
            label: Some(name),
            size: 4.max(elements),
            usage,
            mapped_at_creation: false
        });

        Self {
            buffer,
            dirty: false,
            usage,
            marker: PhantomData
        }
    }

    /// Gets a binding for using the dynamic buffer in a shader. This binding becomes
    /// invalid when the buffer is dirty.
    pub fn as_binding(&self) -> BindingResource<'_> {
        self.buffer.as_entire_binding()
    }

    /// Gets a reference to the underlying GPU buffer which currently backs this object.
    /// The buffer may change upon reallocation.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// The current total size of the buffer.
    pub fn size(&self) -> usize {
        self.buffer.size() as usize / size_of::<T>()
    }

    /// Indicates that this buffer has been reallocated, which invalidates any former bindings of it.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Marks this buffer as clean, for future use.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Writes the provided set of data to the buffer at the given offset.
    pub fn write(&mut self, gpu: &GraphicsSystem, offset: usize, data: &[T]) {
        let start = (offset * size_of::<T>()) as u64;
        let bytes = cast_slice::<_, u8>(data);
        self.ensure_raw_size(gpu, start + bytes.len() as u64);
        gpu.queue().write_buffer(&self.buffer, start, bytes);
    }

    /// Returns a writer which can be used to overwrite this buffer's data within the provided range.
    pub fn write_with<'a>(&'a mut self, gpu: &'a GraphicsSystem, range: Range<usize>) -> DynamicBufferWrite<'a, T> {
        debug_assert!(align_of::<T>() <= 4, "Target type may only have a maximum alignment of 4 bytes.");
        let start = (range.start * size_of::<T>()) as u64;
        let end = (range.end * size_of::<T>()) as u64;
        self.ensure_raw_size(gpu, end);
        
        if let Some(size) = NonZeroU64::new(end - start) {
            DynamicBufferWrite {
                write: Some(gpu.queue().write_buffer_with(&self.buffer, start, size).expect("Failed to write to dynamic buffer.")),
                marker: PhantomData
            }
        }
        else {
            DynamicBufferWrite {
                write: None,
                marker: PhantomData
            }
        }
    }

    /// Ensures that this buffer can hold at least `size` instances of `T`.
    pub fn reserve_total(&mut self, gpu: &GraphicsSystem, size: usize) {
        self.ensure_raw_size(gpu, (size * size_of::<T>()) as u64);
    }

    /// Ensures that the underlying buffer is a certain number of bytes, reallocating if it is too small.
    fn ensure_raw_size(&mut self, gpu: &GraphicsSystem, size: u64) {
        let old_size = self.buffer.size();
        if old_size < size {
            let old_buffer = std::mem::replace(&mut self.buffer, gpu.device().create_buffer(&BufferDescriptor {
                label: Some("Dynamic buffer"),
                size: (2 * old_size).max(size.next_power_of_two()),
                usage: self.usage,
                mapped_at_creation: false
            }));
            
            let mut copy_encoder = gpu.device().create_command_encoder(&CommandEncoderDescriptor { label: Some("Dynamic buffer copy encoder") });
            copy_encoder.copy_buffer_to_buffer(&old_buffer, 0, &self.buffer, 0, old_buffer.size());
            gpu.queue().submit(Some(copy_encoder.finish()));

            self.dirty = true;
        }
    }
}

/// A view into a dynamic buffer into which data may be written.
pub struct DynamicBufferWrite<'a, T: Pod + Zeroable> {
    /// The buffer view to write data into, if any.
    write: Option<QueueWriteBufferView<'a>>,
    /// Marker data.
    marker: PhantomData<T>
}

impl<'a, T: Pod + Zeroable> std::fmt::Debug for DynamicBufferWrite<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicBufferWrite").finish()
    }
}

impl<'a, T: Pod + Zeroable> std::ops::Deref for DynamicBufferWrite<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        panic!("Reading from the dynamic buffer mapping is unsupported.")
    }
}

impl<'a, T: Pod + Zeroable> std::ops::DerefMut for DynamicBufferWrite<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let Some(write) = self.write.as_mut() {
            cast_slice_mut::<_, T>(&mut **write)
        }
        else {
            &mut []
        }
    }
}