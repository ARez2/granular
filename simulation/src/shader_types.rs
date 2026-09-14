use encase::ShaderType;

#[derive(ShaderType)]
pub(super) struct Params {
    pub(super) tick: u32,
}

#[derive(ShaderType)]
pub(super) struct Intent {
    pub(super) destination_index: u32,
    pub(super) encoded_key: u32,
    pub(super) intent_kind: u32,
    pub(super) _padding: u32,
}
