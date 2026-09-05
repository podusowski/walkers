struct Uniform {
    // Turns a tile coordinate into a point on the screen.
    scale: vec2<f32>,
    offset: vec2<f32>,

    // The part of the screen being drawn to. Clip space is relative to it, so the vertices
    // have to be too - a host may well have narrowed the viewport down to one tile.
    viewport_origin: vec2<f32>,
    viewport_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> settings: Uniform;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Straight out of egui's own shader, so that what is drawn here matches what it draws.
fn linear_from_gamma(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOut {
    // Tile coordinates to points on the screen...
    let point = position * settings.scale + settings.offset;

    // ...to points inside the viewport...
    let local = point - settings.viewport_origin;

    // ...and on to clip space.
    var out: VertexOut;
    out.position = vec4<f32>(
        2.0 * local.x / settings.viewport_size.x - 1.0,
        1.0 - 2.0 * local.y / settings.viewport_size.y,
        0.0,
        1.0,
    );

    // Colours stay in gamma space here, exactly as egui leaves them.
    out.color = color;
    return out;
}

// A target which is not sRGB-aware wants what egui already has.
@fragment
fn fs_main_gamma_framebuffer(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}

// An sRGB target converts back on write, so the colour has to be linear going in.
@fragment
fn fs_main_linear_framebuffer(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(linear_from_gamma(in.color.rgb), in.color.a);
}
