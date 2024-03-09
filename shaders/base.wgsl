struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) tex_index: i32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) tex_index: i32,
}

struct Globals {
    transform: mat4x4f,
}

@group(0) @binding(0)
var<uniform> globals: Globals;


@vertex
fn vert_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // out.clip_position = globals.view_proj * globals.transform * vec4<f32>(in.position, 1.0);
    out.clip_position = globals.transform * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    out.tex_coords = in.tex_coords;
    out.tex_index = in.tex_index;
    return out;
}



@group(0) @binding(1)
var textures: binding_array<texture_2d<f32>>;
@group(0) @binding(2)
var samplers: binding_array<sampler>;


@fragment
fn uniform_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var index: i32 = in.tex_index;
    return textureSample(textures[index], samplers[index], in.tex_coords) * in.color;
}