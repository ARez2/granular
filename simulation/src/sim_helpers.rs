use naga::valid::{Capabilities, ValidationFlags, Validator};

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

pub fn validate_wgsl(source: &str) -> Result<(), String> {
    let module =
        naga::front::wgsl::parse_str(source).map_err(|error| error.emit_to_string(source))?;

    Validator::new(ValidationFlags::all(), Capabilities::default())
        .validate(&module)
        .map_err(|error| {
            let msg = error.emit_to_string(source);
            enrich_naga_error(msg)
        })?;

    Ok(())
}

/// Naga errors sometimes suck. So lets try to add some hints to some confusing errors I've encountered
fn enrich_naga_error(mut error: String) -> String {
    let mut hints = Vec::new();

    if error.contains("doesn't match the type stored") {
        hints.push("This could mean that the value is later used as a different type");
    }

    if error.contains("Loading of")
        && error.contains("can't be done")
        && error.contains("arrayLength")
    {
        hints.push(
            "`arrayLength` expects a pointer to a runtime-sized array. \
             Try `arrayLength(&array)`.",
        );
    }

    if error.contains("condition") && error.contains("is not a boolean scalar") {
        hints.push(
            "A comparison operator of a vector in WGSL also returns a vector. \
        Try 'all(a == b)' or 'select(a != b)'.",
        )
    }

    if !hints.is_empty() {
        error.push_str("\n\n");
        for hint in hints {
            error.push_str("Hint: ");
            error.push_str(hint);
            error.push('\n');
        }
    }

    error
}
