struct Uniform {
    // Turns a tile coordinate into a point on the screen.
    scale: vec2<f32>,
    offset: vec2<f32>,

    // The part of the screen being drawn to. Clip space is relative to it, so the vertices
    // have to be too - a host may well have narrowed the viewport down to one tile.
    viewport_origin: vec2<f32>,
    viewport_size: vec2<f32>,

    @size(16) transparency: f32,
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
fn vs_fill(
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
    out.color = color * settings.transparency;
    return out;
}

// A target which is not sRGB-aware wants what egui already has.
@fragment
fn fs_fill_gamma_framebuffer(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}

// An sRGB target converts back on write, so the colour has to be linear going in.
@fragment
fn fs_fill_linear_framebuffer(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(linear_from_gamma(in.color.rgb), in.color.a);
}

struct LineOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,

    /// How far from the middle of the line this is, in pixels.
    @location(1) distance: f32,

    /// Where the line's real edge is, which is half a pixel inside what was drawn.
    @location(2) half_width: f32,

    /// How far along the line this is, in pixels.
    @location(3) along: f32,

    @location(4) @interpolate(flat) dasharray: vec4<f32>,
};

@vertex
fn vs_line(
    @location(0) position: vec2<f32>,
    @location(1) extrude: vec2<f32>,
    @location(2) side: f32,
    @location(3) color: vec4<f32>,
    @location(4) along: f32,
    @location(5) dasharray: vec4<f32>,
) -> LineOut {
    // The line is pushed out sideways after the map's transform, so its width is in screen
    // pixels however far the map is zoomed.
    let point = position * settings.scale + settings.offset + extrude;
    let local = point - settings.viewport_origin;

    var out: LineOut;
    out.position = vec4<f32>(
        2.0 * local.x / settings.viewport_size.x - 1.0,
        1.0 - 2.0 * local.y / settings.viewport_size.y,
        0.0,
        1.0,
    );
    let drawn_half = length(extrude);

    out.color = color * settings.transparency;
    out.distance = side * drawn_half;
    out.half_width = drawn_half - 0.5;
    out.along = along * settings.scale.x;
    out.dasharray = dasharray;
    return out;
}

// How much of the line's length up to `x` is dashes rather than gaps.
fn dashed_up_to(x: f32, pattern: vec4<f32>, period: f32) -> f32 {
    let repeats = floor(x / period);
    let rest = x - repeats * period;
    let first = min(rest, pattern.x);
    let second = clamp(rest - pattern.x - pattern.y, 0.0, pattern.z);
    return repeats * (pattern.x + pattern.z) + first + second;
}

// How much of this pixel, along the line, falls on dashes rather than gaps. Dashes shorter than
// a pixel blend into a fainter line, instead of each pixel being all dash or all gap. A pattern
// which adds up to nothing is a solid line.
fn dash_coverage(in: LineOut) -> f32 {
    let pattern = in.dasharray;
    let period = pattern.x + pattern.y + pattern.z + pattern.w;
    if period <= 0.0 {
        return 1.0;
    }

    return dashed_up_to(in.along + 0.5, pattern, period)
        - dashed_up_to(in.along - 0.5, pattern, period);
}

// How much of this pixel the line covers. Full in the middle, fading over the last half pixel
// at each edge, and never more than the line's own width - which is what keeps a road thinner
// than a pixel looking thin rather than disappearing or turning into a solid one.
fn line_alpha(in: LineOut) -> f32 {
    return dash_coverage(in) * clamp(in.half_width - abs(in.distance) + 0.5, 0.0, 1.0);
}

@fragment
fn fs_line_gamma_framebuffer(in: LineOut) -> @location(0) vec4<f32> {
    // Premultiplied, so the colour fades with the alpha.
    return in.color * line_alpha(in);
}

@fragment
fn fs_line_linear_framebuffer(in: LineOut) -> @location(0) vec4<f32> {
    let faded = in.color * line_alpha(in);
    return vec4<f32>(linear_from_gamma(faded.rgb), faded.a);
}
