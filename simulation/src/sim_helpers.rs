/// Helper to write into StagingBuffers
pub(super) struct EncaseStaging<'a>(pub(super) &'a mut wgpu::QueueWriteBufferView);
impl encase::internal::BufferMut for EncaseStaging<'_> {
    fn capacity(&self) -> usize {
        self.0.len()
    }

    fn write<const N: usize>(&mut self, offset: usize, value: &[u8; N]) {
        self.0.slice(offset..offset + N).copy_from_slice(value);
    }

    fn write_slice(&mut self, offset: usize, value: &[u8]) {
        self.0
            .slice(offset..offset + value.len())
            .copy_from_slice(value);
    }
}
