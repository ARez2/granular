// Darken
fn darken(color_target: vec3f, blend: vec3f) -> vec3f {
    return min(color_target, blend);
}

// Multiply
fn multiply(color_target: vec3f, blend: vec3f) -> vec3f {
    return color_target * blend;
}

// Color Burn
fn colorBurn(color_target: vec3f, blend: vec3f) -> vec3f {
    return 1.0 - (1.0 - color_target) / blend;
}

// Linear Burn
fn linearBurn(color_target: vec3f, blend: vec3f) -> vec3f {
    return color_target + blend - 1.0;
}

// Lighten
fn lighten(color_target: vec3f, blend: vec3f) -> vec3f {
    return max(color_target, blend);
}

// Screen
fn screen(color_target: vec3f, blend: vec3f) -> vec3f {
    return 1.0 - (1.0 - color_target) * (1.0 - blend);
}

// Color Dodge
fn colorDodge(color_target: vec3f, blend: vec3f) -> vec3f {
    return color_target / (1.0 - blend);
}

// Linear Dodge
fn linearDodge(color_target: vec3f, blend: vec3f) -> vec3f {
    return color_target + blend;
}

// Overlay
fn overlay(color_target: vec3f, blend: vec3f) -> vec3f {
    var temp = vec3f(0.0);

    temp.x = select(
        (2.0 * color_target.x) * blend.x,
        1.0 - (1.0 - 2.0 * (color_target.x - 0.5)) * (1.0 - blend.x),
        color_target.x > 0.5
    );

    temp.y = select(
        (2.0 * color_target.y) * blend.y,
        1.0 - (1.0 - 2.0 * (color_target.y - 0.5)) * (1.0 - blend.y),
        color_target.y > 0.5
    );

    temp.z = select(
        (2.0 * color_target.z) * blend.z,
        1.0 - (1.0 - 2.0 * (color_target.z - 0.5)) * (1.0 - blend.z),
        color_target.z > 0.5
    );

    return temp;
}

// Soft Light
fn softLight(color_target: vec3f, blend: vec3f) -> vec3f {
    var temp = vec3f(0.0);

    temp.x = select(
        color_target.x * (blend.x + 0.5),
        1.0 - (1.0 - color_target.x) * (1.0 - (blend.x - 0.5)),
        blend.x > 0.5
    );

    temp.y = select(
        color_target.y * (blend.y + 0.5),
        1.0 - (1.0 - color_target.y) * (1.0 - (blend.y - 0.5)),
        blend.y > 0.5
    );

    temp.z = select(
        color_target.z * (blend.z + 0.5),
        1.0 - (1.0 - color_target.z) * (1.0 - (blend.z - 0.5)),
        blend.z > 0.5
    );

    return temp;
}

// Hard Light
fn hardLight(color_target: vec3f, blend: vec3f) -> vec3f {
    var temp = vec3f(0.0);

    temp.x = select(
        color_target.x * (2.0 * blend.x),
        1.0 - (1.0 - color_target.x) * (1.0 - 2.0 * (blend.x - 0.5)),
        blend.x > 0.5
    );

    temp.y = select(
        color_target.y * (2.0 * blend.y),
        1.0 - (1.0 - color_target.y) * (1.0 - 2.0 * (blend.y - 0.5)),
        blend.y > 0.5
    );

    temp.z = select(
        color_target.z * (2.0 * blend.z),
        1.0 - (1.0 - color_target.z) * (1.0 - 2.0 * (blend.z - 0.5)),
        blend.z > 0.5
    );

    return temp;
}

// Vivid Light
fn vividLight(color_target: vec3f, blend: vec3f) -> vec3f {
    var temp = vec3f(0.0);

    temp.x = select(
        color_target.x / (1.0 - 2.0 * blend.x),
        1.0 - (1.0 - color_target.x) / (2.0 * (blend.x - 0.5)),
        blend.x > 0.5
    );

    temp.y = select(
        color_target.y / (1.0 - 2.0 * blend.y),
        1.0 - (1.0 - color_target.y) / (2.0 * (blend.y - 0.5)),
        blend.y > 0.5
    );

    temp.z = select(
        color_target.z / (1.0 - 2.0 * blend.z),
        1.0 - (1.0 - color_target.z) / (2.0 * (blend.z - 0.5)),
        blend.z > 0.5
    );

    return temp;
}

// Linear Light
fn linearLight(color_target: vec3f, blend: vec3f) -> vec3f {
    var temp = vec3f(0.0);

    temp.x = select(
        color_target.x + (2.0 * blend.x - 1.0),
        color_target.x + (2.0 * (blend.x - 0.5)),
        blend.x > 0.5
    );

    temp.y = select(
        color_target.y + (2.0 * blend.y - 1.0),
        color_target.y + (2.0 * (blend.y - 0.5)),
        blend.y > 0.5
    );

    temp.z = select(
        color_target.z + (2.0 * blend.z - 1.0),
        color_target.z + (2.0 * (blend.z - 0.5)),
        blend.z > 0.5
    );

    return temp;
}

// Pin Light
fn pinLight(color_target: vec3f, blend: vec3f) -> vec3f {
    var temp = vec3f(0.0);

    temp.x = select(
        min(color_target.x, 2.0 * blend.x),
        max(color_target.x, 2.0 * (blend.x - 0.5)),
        blend.x > 0.5
    );

    temp.y = select(
        min(color_target.y, 2.0 * blend.y),
        max(color_target.y, 2.0 * (blend.y - 0.5)),
        blend.y > 0.5
    );

    temp.z = select(
        min(color_target.z, 2.0 * blend.z),
        max(color_target.z, 2.0 * (blend.z - 0.5)),
        blend.z > 0.5
    );

    return temp;
}

// Difference
fn difference(color_target: vec3f, blend: vec3f) -> vec3f {
    return abs(color_target - blend);
}

// Exclusion
fn exclusion(color_target: vec3f, blend: vec3f) -> vec3f {
    return 0.5 - 2.0 * (color_target - 0.5) * (blend - 0.5);
}

// Subtract
fn subtract(color_target: vec3f, blend: vec3f) -> vec3f {
    return color_target - blend;
}

// Divide
fn divide(color_target: vec3f, blend: vec3f) -> vec3f {
    return color_target / blend;
}