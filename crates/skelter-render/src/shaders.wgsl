// Derived from zed-industries/zed crates/gpui_wgpu/src/shaders.wgsl and shaders_storage.wgsl
// @ a5c4ca8bf6c4717a328a4a61b42d6d1d38949bcc
// Copyright 2022 - 2025 Zed Industries, Inc. Licensed under the Apache License, Version 2.0.
// Modifications by the svelters authors: straight sRGB-encoded RGBA instead of HSLA, per-instance
// affine transforms (geometry in pixel-local space), a rounded clip per instance, up to 8 gradient
// stops with CSS angles, dotted borders, offset/spread shadows, capped and dashed lines, image and
// surface sprites with source rects, MSAA path meshes, and a scissored clear.
//
// skelter's renderer shaders (architecture §8.1). Every pipeline draws a 4-vertex triangle strip per
// instance (paths: triangle lists), reading its records from `@group(1)`: one storage buffer bound
// once per kind (bindings 0..=6), so no two declarations alias a binding. Output is
// premultiplied; targets are non-sRGB, so blending happens in sRGB-encoded space (architecture §8.3).

// Contrast and gamma correction adapted from https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.hlsl
// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
fn color_brightness(color: vec3<f32>) -> f32 {
    // REC. 601 luminance coefficients for perceived brightness
    return dot(color, vec3<f32>(0.30, 0.59, 0.11));
}

fn light_on_dark_contrast(enhancedContrast: f32, color: vec3<f32>) -> f32 {
    let brightness = color_brightness(color);
    let multiplier = saturate(4.0 * (0.75 - brightness));
    return enhancedContrast * multiplier;
}

fn enhance_contrast(alpha: f32, k: f32) -> f32 {
    return alpha * (k + 1.0) / (alpha * k + 1.0);
}

fn apply_alpha_correction(a: f32, b: f32, g: vec4<f32>) -> f32 {
    let brightness_adjustment = g.x * b + g.y;
    let correction = brightness_adjustment * a + (g.z * b + g.w);
    return a + a * (1.0 - a) * correction;
}

fn apply_contrast_and_gamma_correction(sample: f32, color: vec3<f32>, enhanced_contrast_factor: f32, gamma_ratios: vec4<f32>) -> f32 {
    let enhanced_contrast = light_on_dark_contrast(enhanced_contrast_factor, color);
    let brightness = color_brightness(color);

    let contrasted = enhance_contrast(sample, enhanced_contrast);
    return apply_alpha_correction(contrasted, brightness, gamma_ratios);
}
// End of the Windows Terminal code.

struct Globals {
    viewport: vec4<f32>,
    clear: vec4<f32>,
    gamma_ratios: vec4<f32>,
    text: vec4<f32>,
}

struct Xform {
    m: vec4<f32>,
    t: vec4<f32>,
}

struct Clip {
    rect: vec4<f32>,
    rrect: vec4<f32>,
    radii: vec4<f32>,
}

struct Bounds {
    origin: vec2<f32>,
    size: vec2<f32>,
}

struct Corners {
    top_left: f32,
    top_right: f32,
    bottom_right: f32,
    bottom_left: f32,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(2) @binding(0) var t_sprite: texture_2d<f32>;
@group(2) @binding(1) var s_sprite: sampler;

const M_PI_F: f32 = 3.1415926;
// Wavy lines, as multiples of the line width (`flatten.rs` sizes the geometry with the same amplitude).
const WAVE_AMPLITUDE: f32 = 1.0;
const WAVE_LENGTH: f32 = 6.0;

fn unit_vertex(vertex_id: u32) -> vec2<f32> {
    return vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
}

fn bounds_of(v: vec4<f32>) -> Bounds {
    return Bounds(v.xy, v.zw);
}

fn corners_of(v: vec4<f32>) -> Corners {
    return Corners(v.x, v.y, v.z, v.w);
}

fn apply_xform(x: Xform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(x.m.x * p.x + x.m.z * p.y + x.t.x, x.m.y * p.x + x.m.w * p.y + x.t.y);
}

fn is_rotated(x: Xform) -> bool {
    return x.m.y != 0.0 || x.m.z != 0.0;
}

fn to_device_position_impl(position: vec2<f32>) -> vec4<f32> {
    let device_position = position / globals.viewport.xy * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    return vec4<f32>(device_position, 0.0, 1.0);
}

/// The pixel-local position of a unit-quad vertex, grown by `pad` pixels (antialiasing room under
/// rotation, where the edges don't follow pixel rows).
fn local_vertex(unit: vec2<f32>, b: vec4<f32>, pad: f32) -> vec2<f32> {
    return b.xy - vec2<f32>(pad) + unit * (b.zw + vec2<f32>(2.0 * pad));
}

// Selects corner radius based on quadrant.
fn pick_corner_radius(center_to_point: vec2<f32>, radii: Corners) -> f32 {
    if (center_to_point.x < 0.0) {
        if (center_to_point.y < 0.0) {
            return radii.top_left;
        } else {
            return radii.bottom_left;
        }
    } else {
        if (center_to_point.y < 0.0) {
            return radii.top_right;
        } else {
            return radii.bottom_right;
        }
    }
}

// Signed distance of the point to the quad's border - positive outside the
// border, and negative inside.
fn quad_sdf(point: vec2<f32>, bounds: Bounds, corner_radii: Corners) -> f32 {
    let half_size = bounds.size / 2.0;
    let center = bounds.origin + half_size;
    let center_to_point = point - center;
    let corner_radius = pick_corner_radius(center_to_point, corner_radii);
    let corner_to_point = abs(center_to_point) - half_size;
    let corner_center_to_point = corner_to_point + corner_radius;
    return quad_sdf_impl(corner_center_to_point, corner_radius);
}

fn quad_sdf_impl(corner_center_to_point: vec2<f32>, corner_radius: f32) -> f32 {
    if (corner_radius == 0.0) {
        // Fast path for unrounded corners.
        return max(corner_center_to_point.x, corner_center_to_point.y);
    } else {
        // Signed distance of the point from a quad that is inset by corner_radius.
        // It is negative inside this quad, and positive outside.
        let signed_distance_to_inset_quad =
            // 0 inside the inset quad, and positive outside.
            length(max(vec2<f32>(0.0), corner_center_to_point)) +
            // 0 outside the inset quad, and negative inside.
            min(0.0, max(corner_center_to_point.x, corner_center_to_point.y));

        return signed_distance_to_inset_quad - corner_radius;
    }
}

/// Coverage of the device pixel at `p` by an instance's clip: hard rect, antialiased rounded rect.
fn clip_alpha(p: vec2<f32>, c: Clip) -> f32 {
    if (p.x < c.rect.x || p.y < c.rect.y || p.x >= c.rect.z || p.y >= c.rect.w) {
        return 0.0;
    }
    if (c.rrect.z <= 0.0) {
        return 1.0;
    }
    return saturate(0.5 - quad_sdf(p, bounds_of(c.rrect), corners_of(c.radii)));
}

/// Straight colour × coverage → premultiplied output.
fn blend_color(color: vec4<f32>, alpha_factor: f32) -> vec4<f32> {
    let alpha = color.a * alpha_factor;
    return vec4<f32>(color.rgb * alpha, alpha);
}

fn over(below: vec4<f32>, above: vec4<f32>) -> vec4<f32> {
    let alpha = above.a + below.a * (1.0 - above.a);
    if (alpha <= 0.0) {
        return vec4<f32>(0.0);
    }
    let color = (above.rgb * above.a + below.rgb * below.a * (1.0 - above.a)) / alpha;
    return vec4<f32>(color, alpha);
}

// A standard gaussian function, used for weighting samples
fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * M_PI_F) * sigma);
}

// This approximates the error function, needed for the gaussian integral
fn erf(v: vec2<f32>) -> vec2<f32> {
    let s = sign(v);
    let a = abs(v);
    let r1 = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
    let r2 = r1 * r1;
    return s - s / (r2 * r2);
}

fn blur_along_x(x: f32, y: f32, sigma: f32, corner: f32, half_size: vec2<f32>) -> f32 {
    let delta = min(half_size.y - corner - abs(y), 0.0);
    let curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    let integral = 0.5 + 0.5 * erf((x + vec2<f32>(-curved, curved)) * (sqrt(0.5) / sigma));
    return integral.y - integral.x;
}

// Modulus that has the same sign as `a`.
fn fmod(a: f32, b: f32) -> f32 {
    return a - b * trunc(a / b);
}

// --- quads --- //

struct Quad {
    xform: Xform,
    clip: Clip,
    bounds: vec4<f32>,
    radii: vec4<f32>,
    // top, right, bottom, left
    borders: vec4<f32>,
    fill: vec4<f32>,
    border_color: vec4<f32>,
    // fill kind (0 solid, 1 linear), border style (0 solid, 1 dashed, 2 dotted), stop count
    info: vec4<u32>,
    // gradient angle (radians, CSS: 0 = to top, clockwise)
    params: vec4<f32>,
    stop_colors: array<u32, 8>,
    stop_offsets: array<f32, 8>,
}

@group(1) @binding(0) var<storage, read> b_quads: array<Quad>;

/// The CSS gradient colour at `t`, interpolated in premultiplied sRGB-encoded space (straight result).
fn gradient_at(id: u32, t: f32) -> vec4<f32> {
    let n = b_quads[id].info.z;
    var prev_color = unpack4x8unorm(b_quads[id].stop_colors[0]);
    var prev_offset = b_quads[id].stop_offsets[0];
    if (t <= prev_offset || n < 2u) {
        return prev_color;
    }
    for (var i = 1u; i < n; i += 1u) {
        let color = unpack4x8unorm(b_quads[id].stop_colors[i]);
        let offset = b_quads[id].stop_offsets[i];
        if (t <= offset) {
            let f = select(1.0, (t - prev_offset) / (offset - prev_offset), offset > prev_offset);
            let a = vec4<f32>(prev_color.rgb * prev_color.a, prev_color.a);
            let b = vec4<f32>(color.rgb * color.a, color.a);
            let p = mix(a, b, f);
            if (p.a <= 0.0) {
                return vec4<f32>(0.0);
            }
            return vec4<f32>(p.rgb / p.a, p.a);
        }
        prev_color = color;
        prev_offset = offset;
    }
    return prev_color;
}

fn quad_fill(id: u32, point: vec2<f32>) -> vec4<f32> {
    let quad = b_quads[id];
    if (quad.info.x == 0u) {
        return quad.fill;
    }
    // CSS gradient line: through the centre at `angle`, long enough that the corners hit 0% and 100%.
    let angle = quad.params.x;
    let direction = vec2<f32>(sin(angle), -cos(angle));
    let size = quad.bounds.zw;
    let line_length = abs(size.x * sin(angle)) + abs(size.y * cos(angle));
    let center = quad.bounds.xy + size * 0.5;
    let t = dot(point - center, direction) / max(line_length, 1e-6) + 0.5;
    return gradient_at(id, t);
}

struct QuadVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) quad_id: u32,
}

@vertex
fn vs_quad(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> QuadVarying {
    let quad = b_quads[instance_id];
    let pad = select(0.0, 1.0, is_rotated(quad.xform));
    let local = local_vertex(unit_vertex(vertex_id), quad.bounds, pad);

    var out = QuadVarying();
    out.position = to_device_position_impl(apply_xform(quad.xform, local));
    out.local = local;
    out.quad_id = instance_id;
    return out;
}

@fragment
fn fs_quad(input: QuadVarying) -> @location(0) vec4<f32> {
    let quad = b_quads[input.quad_id];
    let clip = clip_alpha(input.position.xy, quad.clip);
    if (clip <= 0.0) {
        return vec4<f32>(0.0);
    }

    let background_color = quad_fill(input.quad_id, input.local);

    let corner_radii = corners_of(quad.radii);
    let unrounded = corner_radii.top_left == 0.0 &&
        corner_radii.bottom_left == 0.0 &&
        corner_radii.top_right == 0.0 &&
        corner_radii.bottom_right == 0.0;

    let border_top = quad.borders.x;
    let border_right = quad.borders.y;
    let border_bottom = quad.borders.z;
    let border_left = quad.borders.w;

    // Fast path when the quad is not rounded, has no border and its edges follow pixel rows.
    if (border_top == 0.0 && border_left == 0.0 && border_right == 0.0 && border_bottom == 0.0 &&
            unrounded && !is_rotated(quad.xform)) {
        return blend_color(background_color, clip);
    }

    let size = quad.bounds.zw;
    let half_size = size / 2.0;
    let point = input.local - quad.bounds.xy;
    let center_to_point = point - half_size;

    // Signed distance field threshold for inclusion of pixels. 0.5 is the
    // minimum distance between the center of the pixel and the edge.
    let antialias_threshold = 0.5;

    // Radius of the nearest corner
    let corner_radius = pick_corner_radius(center_to_point, corner_radii);

    // Width of the nearest borders
    let border = vec2<f32>(
        select(border_right, border_left, center_to_point.x < 0.0),
        select(border_bottom, border_top, center_to_point.y < 0.0));

    // 0-width borders are reduced so that `inner_sdf >= antialias_threshold`.
    // The purpose of this is to not draw antialiasing pixels in this case.
    let reduced_border =
        vec2<f32>(select(border.x, -antialias_threshold, border.x == 0.0),
                  select(border.y, -antialias_threshold, border.y == 0.0));

    // Vector from the corner of the quad bounds to the point, after mirroring
    // the point into the bottom right quadrant. Both components are <= 0.
    let corner_to_point = abs(center_to_point) - half_size;

    // Vector from the point to the center of the rounded corner's circle, also
    // mirrored into bottom right quadrant.
    let corner_center_to_point = corner_to_point + corner_radius;

    // Whether the nearest point on the border is rounded
    let is_near_rounded_corner =
            corner_center_to_point.x >= 0 &&
            corner_center_to_point.y >= 0;

    // Vector from straight border inner corner to point.
    let straight_border_inner_corner_to_point = corner_to_point + reduced_border;

    // Whether the point is beyond the inner edge of the straight border.
    let is_beyond_inner_straight_border =
            straight_border_inner_corner_to_point.x > 0 ||
            straight_border_inner_corner_to_point.y > 0;

    // Whether the point is far enough inside the quad, such that the pixels are
    // not affected by the straight border.
    let is_within_inner_straight_border =
        straight_border_inner_corner_to_point.x < -antialias_threshold &&
        straight_border_inner_corner_to_point.y < -antialias_threshold;

    // Fast path for points that must be part of the background.
    if (is_within_inner_straight_border && !is_near_rounded_corner) {
        return blend_color(background_color, clip);
    }

    // Signed distance of the point to the outside edge of the quad's border. It
    // is positive outside this edge, and negative inside.
    let outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);

    // Approximate signed distance of the point to the inside edge of the quad's
    // border. It is negative outside this edge (within the border), and
    // positive inside.
    //
    // This is not always an accurate signed distance:
    // * The rounded portions with varying border width use an approximation of
    //   nearest-point-on-ellipse.
    // * When it is quickly known to be outside the edge, -1.0 is used.
    var inner_sdf = 0.0;
    if (corner_center_to_point.x <= 0 || corner_center_to_point.y <= 0) {
        // Fast paths for straight borders.
        inner_sdf = -max(straight_border_inner_corner_to_point.x,
                         straight_border_inner_corner_to_point.y);
    } else if (is_beyond_inner_straight_border) {
        // Fast path for points that must be outside the inner edge.
        inner_sdf = -1.0;
    } else if (reduced_border.x == reduced_border.y) {
        // Fast path for circular inner edge.
        inner_sdf = -(outer_sdf + reduced_border.x);
    } else {
        let ellipse_radii = max(vec2<f32>(0.0), corner_radius - reduced_border);
        inner_sdf = quarter_ellipse_sdf(corner_center_to_point, ellipse_radii);
    }

    // Negative when inside the border
    let border_sdf = max(inner_sdf, outer_sdf);

    var color = background_color;
    if (border_sdf < antialias_threshold) {
        var border_color = quad.border_color;

        // Dashed (1) and dotted (2) borders
        if (quad.info.y != 0u) {
            // Position along the perimeter in "dash space", where each dash
            // period has length 1
            var t = 0.0;

            // Total number of dash periods, so that the dash spacing can be
            // adjusted to evenly divide it
            var max_t = 0.0;

            // Border width is proportional to dash size. This is the behavior
            // used by browsers, but also avoids dashes from different segments
            // overlapping when dash size is smaller than the border width.
            //
            // Dashed: (2 * border width) dash, (1 * border width) gap.
            // Dotted: (1 * border width) dot, (1 * border width) gap.
            let dash_length_per_width = select(2.0, 1.0, quad.info.y == 2u);
            let dash_gap_per_width = 1.0;
            let dash_period_per_width = dash_length_per_width + dash_gap_per_width;

            // Since the dash size is determined by border width, the density of
            // dashes varies. Multiplying a pixel distance by this returns a
            // position in dash space - it has units (dash period / pixels). So
            // a dash velocity of (1 / 10) is 1 dash every 10 pixels.
            var dash_velocity = 0.0;

            // Dividing this by the border width gives the dash velocity
            let dv_numerator = 1.0 / dash_period_per_width;

            if (unrounded) {
                // When corners aren't rounded, the dashes are separately laid
                // out on each straight line, rather than around the whole
                // perimeter. This way each line starts and ends with a dash.
                let is_horizontal =
                        corner_center_to_point.x <
                        corner_center_to_point.y;

                // When applying dashed borders to just some, not all, the sides.
                // The way we chose border widths above sometimes comes with a 0 width value.
                // So we choose again to avoid division by zero.
                let dashed_border = vec2<f32>(
                        max(border_bottom, border_top),
                        max(border_right, border_left));

                let border_width = select(dashed_border.y, dashed_border.x, is_horizontal);
                dash_velocity = dv_numerator / border_width;
                t = select(point.y, point.x, is_horizontal) * dash_velocity;
                max_t = select(size.y, size.x, is_horizontal) * dash_velocity;
            } else {
                // When corners are rounded, the dashes are laid out clockwise
                // around the whole perimeter.

                let r_tr = corner_radii.top_right;
                let r_br = corner_radii.bottom_right;
                let r_bl = corner_radii.bottom_left;
                let r_tl = corner_radii.top_left;

                let w_t = border_top;
                let w_r = border_right;
                let w_b = border_bottom;
                let w_l = border_left;

                // Straight side dash velocities
                let dv_t = select(dv_numerator / w_t, 0.0, w_t <= 0.0);
                let dv_r = select(dv_numerator / w_r, 0.0, w_r <= 0.0);
                let dv_b = select(dv_numerator / w_b, 0.0, w_b <= 0.0);
                let dv_l = select(dv_numerator / w_l, 0.0, w_l <= 0.0);

                // Straight side lengths in dash space
                let s_t = (size.x - r_tl - r_tr) * dv_t;
                let s_r = (size.y - r_tr - r_br) * dv_r;
                let s_b = (size.x - r_br - r_bl) * dv_b;
                let s_l = (size.y - r_bl - r_tl) * dv_l;

                let corner_dash_velocity_tr = corner_dash_velocity(dv_t, dv_r);
                let corner_dash_velocity_br = corner_dash_velocity(dv_b, dv_r);
                let corner_dash_velocity_bl = corner_dash_velocity(dv_b, dv_l);
                let corner_dash_velocity_tl = corner_dash_velocity(dv_t, dv_l);

                // Corner lengths in dash space
                let c_tr = r_tr * (M_PI_F / 2.0) * corner_dash_velocity_tr;
                let c_br = r_br * (M_PI_F / 2.0) * corner_dash_velocity_br;
                let c_bl = r_bl * (M_PI_F / 2.0) * corner_dash_velocity_bl;
                let c_tl = r_tl * (M_PI_F / 2.0) * corner_dash_velocity_tl;

                // Cumulative dash space upto each segment
                let upto_tr = s_t;
                let upto_r = upto_tr + c_tr;
                let upto_br = upto_r + s_r;
                let upto_b = upto_br + c_br;
                let upto_bl = upto_b + s_b;
                let upto_l = upto_bl + c_bl;
                let upto_tl = upto_l + s_l;
                max_t = upto_tl + c_tl;

                if (is_near_rounded_corner) {
                    let radians = atan2(corner_center_to_point.y,
                                        corner_center_to_point.x);
                    let corner_t = radians * corner_radius;

                    if (center_to_point.x >= 0.0) {
                        if (center_to_point.y < 0.0) {
                            dash_velocity = corner_dash_velocity_tr;
                            // Subtracted because radians is pi/2 to 0 when
                            // going clockwise around the top right corner,
                            // since the y axis has been flipped
                            t = upto_r - corner_t * dash_velocity;
                        } else {
                            dash_velocity = corner_dash_velocity_br;
                            // Added because radians is 0 to pi/2 when going
                            // clockwise around the bottom-right corner
                            t = upto_br + corner_t * dash_velocity;
                        }
                    } else {
                        if (center_to_point.y >= 0.0) {
                            dash_velocity = corner_dash_velocity_bl;
                            // Subtracted because radians is pi/2 to 0 when
                            // going clockwise around the bottom-left corner,
                            // since the x axis has been flipped
                            t = upto_l - corner_t * dash_velocity;
                        } else {
                            dash_velocity = corner_dash_velocity_tl;
                            // Added because radians is 0 to pi/2 when going
                            // clockwise around the top-left corner, since both
                            // axis were flipped
                            t = upto_tl + corner_t * dash_velocity;
                        }
                    }
                } else {
                    // Straight borders
                    let is_horizontal =
                            corner_center_to_point.x <
                            corner_center_to_point.y;
                    if (is_horizontal) {
                        if (center_to_point.y < 0.0) {
                            dash_velocity = dv_t;
                            t = (point.x - r_tl) * dash_velocity;
                        } else {
                            dash_velocity = dv_b;
                            t = upto_bl - (point.x - r_bl) * dash_velocity;
                        }
                    } else {
                        if (center_to_point.x < 0.0) {
                            dash_velocity = dv_l;
                            t = upto_tl - (point.y - r_tl) * dash_velocity;
                        } else {
                            dash_velocity = dv_r;
                            t = upto_r + (point.y - r_tr) * dash_velocity;
                        }
                    }
                }
            }

            let dash_length = dash_length_per_width / dash_period_per_width;
            let desired_dash_gap = dash_gap_per_width / dash_period_per_width;

            // Straight borders should start and end with a dash, so max_t is
            // reduced to cause this.
            max_t -= select(0.0, dash_length, unrounded);
            if (max_t >= 1.0) {
                // Adjust dash gap to evenly divide max_t.
                let dash_count = floor(max_t);
                let dash_period = max_t / dash_count;
                border_color.a *= dash_alpha(
                    t,
                    dash_period,
                    dash_length,
                    dash_velocity,
                    antialias_threshold);
            } else if (unrounded) {
                // When there isn't enough space for the full gap between the
                // two start / end dashes of a straight border, reduce gap to
                // make them fit.
                let dash_gap = max_t - dash_length;
                if (dash_gap > 0.0) {
                    let dash_period = dash_length + dash_gap;
                    border_color.a *= dash_alpha(
                        t,
                        dash_period,
                        dash_length,
                        dash_velocity,
                        antialias_threshold);
                }
            }
        }

        // Blend the border on top of the background and then linearly interpolate
        // between the two as we slide inside the background.
        let blended_border = over(background_color, border_color);
        color = mix(background_color, blended_border,
                    saturate(antialias_threshold - inner_sdf));
    }

    return blend_color(color, saturate(antialias_threshold - outer_sdf) * clip);
}

// Returns the dash velocity of a corner given the dash velocity of the two
// sides, by returning the slower velocity (larger dashes).
//
// Since 0 is used for dash velocity when the border width is 0 (instead of
// +inf), this returns the other dash velocity in that case.
fn corner_dash_velocity(dv1: f32, dv2: f32) -> f32 {
    if (dv1 == 0.0) {
        return dv2;
    } else if (dv2 == 0.0) {
        return dv1;
    } else {
        return min(dv1, dv2);
    }
}

// Returns alpha used to render antialiased dashes.
// `t` is within the dash when `fmod(t, period) < length`.
fn dash_alpha(t: f32, period: f32, length: f32, dash_velocity: f32, antialias_threshold: f32) -> f32 {
    let half_period = period / 2;
    let half_length = length / 2;
    // Value in [-half_period, half_period].
    // The dash is in [-half_length, half_length].
    let centered = fmod(t + half_period - half_length, period) - half_period;
    // Signed distance for the dash, negative values are inside the dash.
    let signed_distance = abs(centered) - half_length;
    // Antialiased alpha based on the signed distance.
    return saturate(antialias_threshold - signed_distance / dash_velocity);
}

// This approximates distance to the nearest point to a quarter ellipse in a way
// that is sufficient for anti-aliasing when the ellipse is not very eccentric.
// The components of `point` are expected to be positive.
//
// Negative on the outside and positive on the inside.
fn quarter_ellipse_sdf(point: vec2<f32>, radii: vec2<f32>) -> f32 {
    // Scale the space to treat the ellipse like a unit circle.
    let circle_vec = point / radii;
    let unit_circle_sdf = length(circle_vec) - 1.0;
    // Approximate up-scaling of the length by using the average of the radii.
    return unit_circle_sdf * (radii.x + radii.y) * -0.5;
}

// --- shadows --- //

struct Shadow {
    xform: Xform,
    clip: Clip,
    // The shadow rect for drop shadows; the "hole" rect for inset shadows.
    bounds: vec4<f32>,
    radii: vec4<f32>,
    color: vec4<f32>,
    // The element's own box: inset shadows are clipped to it.
    element: vec4<f32>,
    element_radii: vec4<f32>,
    // sigma, inset (0 / 1)
    params: vec4<f32>,
}

@group(1) @binding(1) var<storage, read> b_shadows: array<Shadow>;

struct ShadowVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) shadow_id: u32,
}

@vertex
fn vs_shadow(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> ShadowVarying {
    let shadow = b_shadows[instance_id];

    var geometry: vec4<f32>;
    if (shadow.params.y != 0.0) {
        geometry = shadow.element;
    } else {
        // Leave room for the gaussian tail outside the shadow rect.
        let margin = 3.0 * shadow.params.x;
        geometry = vec4<f32>(shadow.bounds.xy - vec2<f32>(margin), shadow.bounds.zw + vec2<f32>(2.0 * margin));
    }
    let pad = select(0.0, 1.0, is_rotated(shadow.xform));
    let local = local_vertex(unit_vertex(vertex_id), geometry, pad);

    var out = ShadowVarying();
    out.position = to_device_position_impl(apply_xform(shadow.xform, local));
    out.local = local;
    out.shadow_id = instance_id;
    return out;
}

@fragment
fn fs_shadow(input: ShadowVarying) -> @location(0) vec4<f32> {
    let shadow = b_shadows[input.shadow_id];
    let clip = clip_alpha(input.position.xy, shadow.clip);
    if (clip <= 0.0) {
        return vec4<f32>(0.0);
    }

    let bounds = bounds_of(shadow.bounds);
    let radii = corners_of(shadow.radii);
    let blur_radius = shadow.params.x;
    let half_size = bounds.size / 2.0;
    let center = bounds.origin + half_size;
    let center_to_point = input.local - center;

    let corner_radius = pick_corner_radius(center_to_point, radii);

    var alpha: f32;
    if (blur_radius == 0.0) {
        let distance = quad_sdf(input.local, bounds, radii);
        alpha = saturate(0.5 - distance);
    } else {
        // The signal is only non-zero in a limited range, so don't waste samples
        let low = center_to_point.y - half_size.y;
        let high = center_to_point.y + half_size.y;
        let start = clamp(-3.0 * blur_radius, low, high);
        let end = clamp(3.0 * blur_radius, low, high);

        // Accumulate samples (we can get away with surprisingly few samples)
        let step = (end - start) / 4.0;
        var y = start + step * 0.5;
        alpha = 0.0;
        for (var i = 0; i < 4; i += 1) {
            let blur = blur_along_x(center_to_point.x, center_to_point.y - y,
                blur_radius, corner_radius, half_size);
            alpha += blur * gaussian(y, blur_radius) * step;
            y += step;
        }
    }

    if (shadow.params.y != 0.0) {
        // The inset shadow is the complement of the (blurred) hole rect, clipped to the element.
        alpha = 1.0 - alpha;
        let element_distance = quad_sdf(input.local, bounds_of(shadow.element), corners_of(shadow.element_radii));
        alpha *= saturate(0.5 - element_distance);
    }

    return blend_color(shadow.color, alpha * clip);
}

// --- lines --- //

struct Line {
    xform: Xform,
    clip: Clip,
    // The drawn quad in line space (x along the segment from 0 to length, y across).
    bounds: vec4<f32>,
    color: vec4<f32>,
    // length, half width, cap (0 butt, 1 round, 2 square), wavy (0 / 1)
    geom: vec4<f32>,
    // dash on, dash off (on = 0: solid)
    dash: vec4<f32>,
}

@group(1) @binding(2) var<storage, read> b_lines: array<Line>;

struct LineVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) line_id: u32,
}

@vertex
fn vs_line(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> LineVarying {
    let line = b_lines[instance_id];
    let local = local_vertex(unit_vertex(vertex_id), line.bounds, 0.0);

    var out = LineVarying();
    out.position = to_device_position_impl(apply_xform(line.xform, local));
    out.local = local;
    out.line_id = instance_id;
    return out;
}

/// Signed distance to the segment [a, b] on the x axis with half width `hw` and cap `cap`.
fn segment_sdf(p: vec2<f32>, a: f32, b: f32, hw: f32, cap: f32) -> f32 {
    var lo = a;
    var hi = b;
    if (cap == 2.0) {
        lo -= hw;
        hi += hw;
    }
    if (cap == 1.0) {
        let dx = max(lo - p.x, 0.0) + max(p.x - hi, 0.0);
        return length(vec2<f32>(dx, p.y)) - hw;
    }
    return max(max(lo - p.x, p.x - hi), abs(p.y) - hw);
}

@fragment
fn fs_line(input: LineVarying) -> @location(0) vec4<f32> {
    let line = b_lines[input.line_id];
    let clip = clip_alpha(input.position.xy, line.clip);
    if (clip <= 0.0) {
        return vec4<f32>(0.0);
    }

    let len = line.geom.x;
    let hw = line.geom.y;
    let cap = line.geom.z;
    var p = input.local;

    if (line.geom.w != 0.0) {
        // Distance across a sine wave, corrected by its slope.
        let amplitude = WAVE_AMPLITUDE * 2.0 * hw;
        let k = 2.0 * M_PI_F / (WAVE_LENGTH * 2.0 * hw);
        let wave = amplitude * sin(k * p.x);
        let slope = amplitude * k * cos(k * p.x);
        p.y = (p.y - wave) / sqrt(1.0 + slope * slope);
    }

    var distance: f32;
    let on = line.dash.x;
    let off = line.dash.y;
    if (on > 0.0) {
        let period = on + off;
        let start = p.x - fmod(fmod(p.x, period) + period, period);
        let current = segment_sdf(p, max(start, 0.0), min(start + on, len), hw, cap);
        let next = segment_sdf(p, max(start + period, 0.0), min(start + period + on, len), hw, cap);
        distance = min(current, select(1e6, next, start + period < len));
    } else {
        distance = segment_sdf(p, 0.0, len, hw, cap);
    }

    return blend_color(line.color, saturate(0.5 - distance) * clip);
}

// --- mask sprites (glyphs, monochrome icons) --- //

struct MaskSprite {
    xform: Xform,
    clip: Clip,
    bounds: vec4<f32>,
    // atlas tile (x, y, w, h) in texels
    uv: vec4<f32>,
    color: vec4<f32>,
    // gamma-correct (0 / 1)
    params: vec4<f32>,
}

@group(1) @binding(3) var<storage, read> b_masks: array<MaskSprite>;

struct MaskVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) tile_position: vec2<f32>,
    @location(1) @interpolate(flat) sprite_id: u32,
}

@vertex
fn vs_mask(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> MaskVarying {
    let sprite = b_masks[instance_id];
    let unit = unit_vertex(vertex_id);
    let local = local_vertex(unit, sprite.bounds, 0.0);
    let atlas_size = vec2<f32>(textureDimensions(t_sprite, 0));

    var out = MaskVarying();
    out.position = to_device_position_impl(apply_xform(sprite.xform, local));
    out.tile_position = (sprite.uv.xy + unit * sprite.uv.zw) / atlas_size;
    out.sprite_id = instance_id;
    return out;
}

@fragment
fn fs_mask(input: MaskVarying) -> @location(0) vec4<f32> {
    let sample = textureSample(t_sprite, s_sprite, input.tile_position).r;
    let sprite = b_masks[input.sprite_id];
    var coverage = sample;
    if (sprite.params.x != 0.0) {
        coverage = apply_contrast_and_gamma_correction(sample, sprite.color.rgb, globals.text.x, globals.gamma_ratios);
    }
    // Clip after sampling (derivatives need uniform control flow).
    let clip = clip_alpha(input.position.xy, sprite.clip);
    return blend_color(sprite.color, saturate(coverage) * clip);
}

// --- image sprites (images, colour glyphs and icons, surfaces) --- //

struct ImageSprite {
    xform: Xform,
    clip: Clip,
    bounds: vec4<f32>,
    // source (x, y, w, h) in texels
    uv: vec4<f32>,
    // sampling clamp (x0, y0, x1, y1) in texels
    uv_clamp: vec4<f32>,
    radii: vec4<f32>,
    // opacity
    params: vec4<f32>,
}

@group(1) @binding(4) var<storage, read> b_images: array<ImageSprite>;

struct ImageVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) texel: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) @interpolate(flat) sprite_id: u32,
}

@vertex
fn vs_image(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> ImageVarying {
    let sprite = b_images[instance_id];
    let unit = unit_vertex(vertex_id);
    let local = local_vertex(unit, sprite.bounds, 0.0);

    var out = ImageVarying();
    out.position = to_device_position_impl(apply_xform(sprite.xform, local));
    out.texel = sprite.uv.xy + unit * sprite.uv.zw;
    out.local = local;
    out.sprite_id = instance_id;
    return out;
}

@fragment
fn fs_image(input: ImageVarying) -> @location(0) vec4<f32> {
    let sprite = b_images[input.sprite_id];
    let atlas_size = vec2<f32>(textureDimensions(t_sprite, 0));
    let texel = clamp(input.texel, sprite.uv_clamp.xy, sprite.uv_clamp.zw);
    let sample = textureSampleLevel(t_sprite, s_sprite, texel / atlas_size, 0.0);
    let clip = clip_alpha(input.position.xy, sprite.clip);
    let distance = quad_sdf(input.local, bounds_of(sprite.bounds), corners_of(sprite.radii));
    // The texture is premultiplied.
    return sample * (sprite.params.x * saturate(0.5 - distance) * clip);
}

// --- path rasterization (into the MSAA intermediate) --- //

struct PathInstance {
    xform: Xform,
    clip: Clip,
    color: vec4<f32>,
    // the intermediate's device rect (x, y, w, h)
    viewport_rect: vec4<f32>,
}

@group(1) @binding(5) var<storage, read> b_paths: array<PathInstance>;

struct PathRasterVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) path_id: u32,
}

@vertex
fn vs_path_raster(@location(0) pos: vec2<f32>, @builtin(instance_index) instance_id: u32) -> PathRasterVarying {
    let path = b_paths[instance_id];
    let device = apply_xform(path.xform, pos) - path.viewport_rect.xy;

    var out = PathRasterVarying();
    out.position = vec4<f32>(device / path.viewport_rect.zw * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.path_id = instance_id;
    return out;
}

@fragment
fn fs_path_raster(input: PathRasterVarying) -> @location(0) vec4<f32> {
    let path = b_paths[input.path_id];
    let clip = clip_alpha(input.position.xy + path.viewport_rect.xy, path.clip);
    return blend_color(path.color, clip);
}

// --- path composite --- //

struct PathSprite {
    bounds: vec4<f32>,
}

@group(1) @binding(6) var<storage, read> b_path_sprites: array<PathSprite>;

struct PathSpriteVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) sprite_id: u32,
}

@vertex
fn vs_path_sprite(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> PathSpriteVarying {
    let sprite = b_path_sprites[instance_id];
    let local = local_vertex(unit_vertex(vertex_id), sprite.bounds, 0.0);

    var out = PathSpriteVarying();
    out.position = to_device_position_impl(local);
    out.sprite_id = instance_id;
    return out;
}

@fragment
fn fs_path_sprite(input: PathSpriteVarying) -> @location(0) vec4<f32> {
    let sprite = b_path_sprites[input.sprite_id];
    let texel = vec2<i32>(floor(input.position.xy - sprite.bounds.xy));
    // Already premultiplied.
    return textureLoad(t_sprite, texel, 0);
}

// --- scissored clear --- //

@vertex
fn vs_clear(@builtin(vertex_index) vertex_id: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((vertex_id << 1u) & 2u), f32(vertex_id & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_clear() -> @location(0) vec4<f32> {
    return globals.clear;
}
