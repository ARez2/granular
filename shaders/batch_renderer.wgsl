const SHAPE_TEXTURED: u32 = 0u;
const SHAPE_CIRCLE: u32 = 1u;

struct VertexInput {
    @location(0) position: vec2f,
    @location(1) color: vec4f,
    @location(2) tex_coords: vec2f,
    @location(3) shape: u32, // one of SHAPE_*
    @location(4) shape_parameter: f32, // thickness for circles
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) color: vec4f,
    @location(1) tex_coords: vec2f,
    @location(2) @interpolate(flat) shape: u32, // one of SHAPE_*
    @location(4) @interpolate(flat) shape_parameter: f32, // thickness for circles
}

struct Globals {
    canvas_transform: mat4x4f,
}

@group(0) @binding(0)
var<uniform> globals: Globals;


@vertex
fn vert_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = globals.canvas_transform * vec4f(in.position, 0.0, 1.0);
    out.color = in.color;
    out.tex_coords = in.tex_coords;
    out.shape = in.shape;
    out.shape_parameter = in.shape_parameter;
    return out;
}

@group(1) @binding(0)
var texture_atlas: texture_2d<f32>;
@group(1) @binding(1)
var texture_atlas_sampler: sampler;


@fragment
fn fragment_main(in: VertexOutput) -> @location(0) vec4f {
    // Normally, this gets calculated implicitly inside of textureSample,
    // however, since we branch out in the fragment shader, this should be the same for
    // all fragments
    let uv_dx = dpdx(in.tex_coords); // Horizontal change
    let uv_dy = dpdy(in.tex_coords); // Vertical change
    
    var color: vec4f;
    if in.shape == SHAPE_TEXTURED {
        color = textureSampleGrad(
            texture_atlas,
            texture_atlas_sampler,
            in.tex_coords,
            uv_dx,
            uv_dy,
        );
    } else if in.shape == SHAPE_CIRCLE {
        let thickness = in.shape_parameter;
        let fade = 0.005;

        // Calculate distance and fill circle with white
        let distance = 1.0 - length(in.tex_coords);
        var circle_val = smoothstep(0.0, fade, distance);
        circle_val *= smoothstep(thickness + fade, thickness, distance);
        color = vec4f(vec3f(circle_val), circle_val);
    }
    return color * in.color;
}