struct VertexInput {
    @location(0) position: vec2<i32>,
    @location(1) color: vec4<f32>,
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct Globals {
    canvas_transform: mat4x4f,
}

@group(0) @binding(0)
var<uniform> globals: Globals;


@vertex
fn vert_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // out.clip_position = globals.view_proj * globals.transform * vec4<f32>(in.position, 1.0);
    out.clip_position = globals.canvas_transform * vec4<f32>(vec2<f32>(in.position), 0.0, 1.0);
    out.color = in.color;
    out.tex_coords = in.tex_coords;
    return out;
}

@group(1) @binding(0)
var texture_atlas: texture_2d<f32>;
@group(1) @binding(1)
var texture_atlas_sampler: sampler;


@fragment
fn fragment_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(
        texture_atlas,
        texture_atlas_sampler,
        vec2<f32>(
            in.tex_coords.x,
            in.tex_coords.y
        )
    );
    return tex_color * in.color;
}