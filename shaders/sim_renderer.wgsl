// Vertex shader bindings

struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) position: vec4<f32>,
}

struct Globals {
    canvas_transform: mat4x4f,
}

@group(0) @binding(2)
var<uniform> globals: Globals;


@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = globals.canvas_transform * vec4<f32>(position, 0.0, 1.0);
    var p = vec2<f32>(out.position.x, out.position.y);
    out.tex_coord = fma(p, vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5));
    return out;
}

// Fragment shader bindings

@group(0) @binding(0) var r_tex_color: texture_2d<f32>;
@group(0) @binding(1) var r_tex_sampler: sampler;

@fragment
fn fs_main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(r_tex_color, r_tex_sampler, tex_coord);
}