@export struct Params {
    // xy = position, zw = size
    viewport_rect: vec4f,
    surface_size: vec2f,
};

@group(0) @binding(0)
var<uniform> params: Params;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};


@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2f, 6>(
        vec2f(0.0, 0.0),
        vec2f(1.0, 0.0),
        vec2f(1.0, 1.0),

        vec2f(0.0, 0.0),
        vec2f(1.0, 1.0),
        vec2f(0.0, 1.0),
    );

    var uvs = array<vec2f, 6>(
        vec2f(0.0, 0.0),
        vec2f(1.0, 0.0),
        vec2f(1.0, 1.0),

        vec2f(0.0, 0.0),
        vec2f(1.0, 1.0),
        vec2f(0.0, 1.0),
    );

    let local = positions[index];
    let pixel_position =
        params.viewport_rect.xy +
        local * params.viewport_rect.zw;
    let ndc = vec2f(
        pixel_position.x / params.surface_size.x * 2.0 - 1.0,
        1.0 - pixel_position.y / params.surface_size.y * 2.0,
    );
    var output: VertexOutput;
    output.position = vec4f(ndc, 0.0, 1.0);
    output.uv = uvs[index];

    return output;
}

@group(1) @binding(0)
var game_texture: texture_2d<f32>;
@group(1) @binding(1)
var game_texture_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(
        game_texture,
        game_texture_sampler,
        input.uv,
    );
}