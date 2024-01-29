struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}
@vertex
fn vert_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = model.tex_coords;
    out.clip_position = vec4<f32>(model.position, 1.0);
    return out;
}

@group(0) @binding(0)
var display_texture: texture_2d<f32>;
@group(0) @binding(1)
var display_sampler: sampler;


@fragment
fn uniform_main(fragment: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(display_texture, display_sampler, fragment.tex_coords);
}