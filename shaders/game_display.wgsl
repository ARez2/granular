struct Params {
    surface_pixels_to_clip: mat4x4f,
    // xy: top-left, zw: size, in physical surface pixels. Tells WHERE to render the game render target
    viewport_rect: vec4f,
    // xy: first UV, zw: extent. determines WHICH PART of the game render target is shown
    uv_rect: vec4f,
};
@group(0) @binding(0) var<uniform> params: Params;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
        vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0),
    );
    let local = corners[index];
    let surface_pixel = params.viewport_rect.xy + local * params.viewport_rect.zw;
    var out: VertexOutput;
    // Shared CPU-generated projection: no independent Y-flip formula here.
    out.position = params.surface_pixels_to_clip * vec4f(surface_pixel, 0.0, 1.0);
    out.uv = params.uv_rect.xy + local * params.uv_rect.zw;
    return out;
}
@group(1) @binding(0) var game_texture: texture_2d<f32>;
@group(1) @binding(1) var game_texture_sampler: sampler;
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    return textureSample(game_texture, game_texture_sampler, input.uv);
}
