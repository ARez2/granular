@export struct Params {
    surface_size: vec2f,
    time: f32,
    _pad: f32
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

    let local = positions[index];

    // Fullscreen, instead of viewport_rect.
    let pixel_position = local * params.surface_size;
    let ndc = vec2f(
        pixel_position.x / params.surface_size.x * 2.0 - 1.0,
        1.0 - pixel_position.y / params.surface_size.y * 2.0,
    );

    var output: VertexOutput;
    output.position = vec4f(ndc, 0.0, 1.0);
    output.uv = local;
    return output;
}

fn hash33(p: vec3f) -> vec3f {
    let q = vec3f(
        dot(p, vec3f(127.1, 311.7, 74.7)),
        dot(p, vec3f(269.5, 183.3, 246.1)),
        dot(p, vec3f(113.5, 271.9, 124.6))
    );

    return -1.0 + 2.0 * fract(sin(q) * 43758.5453123);
}
fn tetraNoise(o: vec2f) -> f32 {
    let p0 = vec3f(
        o.x + 0.008 * params.time,
        o.y + 0.004 * params.time,
        0.005 * params.time
    );

    var p = p0;

    var i = floor(
        p + dot(p, vec3f(0.33333))
    );

    p -= i - dot(i, vec3f(0.16666));

    let i1 = step(p.yzx, p);

    let i2 = max(
        i1,
        1.0 - i1.zxy
    );

    // GLSL:
    // i1 = min(i1, 1.0-i1.zxy);
    let i1b = min(
        i1,
        1.0 - i1.zxy
    );

    let p1 = p - i1b + 0.16666;
    let p2 = p - i2  + 0.33333;
    let p3 = p - 0.5;

    let v = max(
        vec4f(0.5) - vec4f(
            dot(p,  p),
            dot(p1, p1),
            dot(p2, p2),
            dot(p3, p3)
        ),
        vec4f(0.0)
    );

    let d = vec4f(
        dot(p,  hash33(i)),
        dot(p1, hash33(i + i1b)),
        dot(p2, hash33(i + i2)),
        dot(p3, hash33(i + 1.0))
    );

    let n = clamp(
        dot(d, v * v * v * 8.0) * 1.732 + 0.5,
        0.0,
        1.0
    );

    return n;
}
fn topologize(noise: f32) -> f32 {
    let smoothFloor0 = noise * 12.0;

    var fracU = vec2f(
        smoothFloor0,
        fwidth(smoothFloor0) * 1.3
    );

    fracU.x = fract(fracU.x);

    fracU += (
        1.0 - 2.0 * fracU
    ) * step(
        fracU.y,
        fracU.x
    );

    let smoothFloor =
        smoothFloor0 -
        clamp(
            1.0 - fracU.x / fracU.y,
            0.0,
            1.0
        );

    return noise * 0.25 +
           smoothFloor * 0.75 / 11.0;
}


@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    let resolution = params.surface_size;
    let fragCoordXY = input.uv * resolution;

    // Convert coordinates
    let p =
        (fragCoordXY * 2.5 - resolution) /
        (resolution.y * 0.5 + resolution.x * 0.5);

    // Sample distance
    let e = vec2f(
        10.0 / (resolution.y + resolution.x),
        0.0
    );

    // Four samples
    let fxl = topologize(
        tetraNoise(p + e.xy)
    );

    let fxr = topologize(
        tetraNoise(p - e.xy)
    );

    let fyu = topologize(
        tetraNoise(p + e.yx)
    );

    let fyd = topologize(
        tetraNoise(p - e.yx)
    );

    // Edge detection
    let weight = clamp(
        (
            max(
                abs(fxl - fxr),
                abs(fyu - fyd)
            ) - 0.01
        ) * 12.0,
        0.0,
        1.0
    );


    let color = mix(
        vec3f(0.11),
        vec3f(0.18),
        weight
    );

    return vec4f(color, 1.0);
}