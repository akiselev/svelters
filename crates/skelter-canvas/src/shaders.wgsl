// skelter-canvas shaders (architecture §9.5): analytic shapes, tessellated polygons, Slug glyphs and
// the static-cache blit. Colours arrive straight-alpha sRGB-encoded and leave premultiplied; blending
// is premultiplied over, in sRGB-encoded space (architecture §8.3).
//
// `@group(0)` is one `Draw` per chunk draw (a dynamic offset into a per-frame uniform buffer):
// chunk-local world offsets → device pixels of the target being drawn into.

struct Draw {
    // Column-major 2×2 [a, b, c, d]: device = m · local + t.
    m: vec4<f32>,
    // tx, ty, device pixels per world unit, y sign (+1: world y points down on screen).
    t: vec4<f32>,
    // Target width and height in pixels, minimum stroke width in pixels, opacity.
    target_size: vec4<f32>,
    // The target's origin in the frame's device space (static-cache passes), antialiasing width in
    // pixels, unused.
    origin: vec4<f32>,
};

@group(0) @binding(0) var<uniform> draw: Draw;

fn to_clip(local: vec2<f32>) -> vec4<f32> {
    let dev = vec2<f32>(
        draw.m.x * local.x + draw.m.z * local.y + draw.t.x,
        draw.m.y * local.x + draw.m.w * local.y + draw.t.y,
    ) - draw.origin.xy;
    let ndc = dev / draw.target_size.xy * 2.0 - 1.0;
    return vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
}

fn corner(vi: u32) -> vec2<f32> {
    return vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
}

fn premul(c: vec4<f32>, coverage: f32) -> vec4<f32> {
    let a = c.a * coverage * draw.target_size.w;
    return vec4<f32>(c.rgb * a, a);
}

// ---------------------------------------------------------------------------------------------------
// Shapes

const RECT: u32 = 0u;
const SEGMENT: u32 = 1u;
const CIRCLE: u32 = 2u;
const ARC: u32 = 3u;
const PI: f32 = 3.14159265358979;

struct ShapeIn {
    @location(0) p0: vec2<f32>,
    @location(1) p1: vec2<f32>,
    @location(2) params: vec4<f32>,
    @location(3) color: u32,
    @location(4) kind: u32,
};

struct ShapeOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) p0: vec2<f32>,
    @location(2) @interpolate(flat) p1: vec2<f32>,
    // Shape parameters with widths already raised to the minimum stroke.
    @location(3) @interpolate(flat) params: vec4<f32>,
    @location(4) @interpolate(flat) color: vec4<f32>,
    @location(5) @interpolate(flat) kind: u32,
};

// A width in world units, at least the minimum stroke width on screen.
fn stroke(w: f32) -> f32 {
    return max(w, draw.target_size.z / draw.t.z);
}

@vertex
fn vs_shape(@builtin(vertex_index) vi: u32, s: ShapeIn) -> ShapeOut {
    var out: ShapeOut;
    let aa = draw.origin.z / draw.t.z;
    var params = s.params;
    var c = s.p0;
    var u = vec2<f32>(1.0, 0.0);
    var half = vec2<f32>(0.0, 0.0);
    switch s.kind {
        case RECT: {
            if params.y > 0.0 {
                params.y = stroke(params.y);
            }
            c = (s.p0 + s.p1) * 0.5;
            half = abs(s.p1 - s.p0) * 0.5 + vec2<f32>(params.y * 0.5);
        }
        case SEGMENT: {
            params.x = stroke(params.x);
            let d = s.p1 - s.p0;
            let len = length(d);
            if len > 0.0 {
                u = d / len;
            }
            c = (s.p0 + s.p1) * 0.5;
            var ext = len * 0.5;
            if params.y != 0.0 {
                ext = ext + params.x * 0.5;
            }
            half = vec2<f32>(ext, params.x * 0.5);
        }
        case CIRCLE: {
            if params.y > 0.0 {
                params.y = stroke(params.y);
            }
            half = vec2<f32>(params.x + params.y * 0.5);
        }
        default: {
            params.y = stroke(params.y);
            half = vec2<f32>(params.x + params.y * 0.5);
        }
    }
    let v = vec2<f32>(-u.y, u.x);
    let k = corner(vi) * 2.0 - 1.0;
    let local = c + u * (half.x + aa) * k.x + v * (half.y + aa) * k.y;
    out.pos = to_clip(local);
    out.local = local;
    out.p0 = s.p0;
    out.p1 = s.p1;
    out.params = params;
    out.color = unpack4x8unorm(s.color);
    out.kind = s.kind;
    return out;
}

fn sd_round_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let rr = min(r, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(rr);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

fn sd_box(p: vec2<f32>, half: vec2<f32>) -> f32 {
    let q = abs(p) - half;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

@fragment
fn fs_shape(in: ShapeOut) -> @location(0) vec4<f32> {
    var d = 0.0;
    switch in.kind {
        case RECT: {
            let c = (in.p0 + in.p1) * 0.5;
            let half = abs(in.p1 - in.p0) * 0.5;
            d = sd_round_box(in.local - c, half, in.params.x);
            if in.params.y > 0.0 {
                d = abs(d) - in.params.y * 0.5;
            }
        }
        case SEGMENT: {
            let w = in.params.x;
            let ab = in.p1 - in.p0;
            let len = length(ab);
            if in.params.y == 1.0 || len == 0.0 {
                let h = clamp(dot(in.local - in.p0, ab) / max(dot(ab, ab), 1e-30), 0.0, 1.0);
                d = length(in.local - in.p0 - ab * h) - w * 0.5;
            } else {
                let u = ab / len;
                let v = vec2<f32>(-u.y, u.x);
                let p = in.local - (in.p0 + in.p1) * 0.5;
                var ext = len * 0.5;
                if in.params.y == 2.0 {
                    ext = ext + w * 0.5;
                }
                d = sd_box(vec2<f32>(dot(p, u), dot(p, v)), vec2<f32>(ext, w * 0.5));
            }
        }
        case CIRCLE: {
            d = length(in.local - in.p0) - in.params.x;
            if in.params.y > 0.0 {
                d = abs(d) - in.params.y * 0.5;
            }
        }
        default: {
            // An arc: rotate so its middle points along +y, then fold it onto x ≥ 0 (Quílez's sdArc).
            let r = in.params.x;
            let w = in.params.y;
            let half_sweep = abs(in.params.w) * 0.5;
            let p = in.local - in.p0;
            if half_sweep >= PI {
                d = abs(length(p) - r) - w * 0.5;
            } else {
                let phi = PI * 0.5 - (in.params.z + in.params.w * 0.5);
                let cs = cos(phi);
                let sn = sin(phi);
                var q = vec2<f32>(p.x * cs - p.y * sn, p.x * sn + p.y * cs);
                q.x = abs(q.x);
                let sc = vec2<f32>(sin(half_sweep), cos(half_sweep));
                if sc.y * q.x > sc.x * q.y {
                    d = length(q - sc * r);
                } else {
                    d = abs(length(q) - r);
                }
                d = d - w * 0.5;
            }
        }
    }
    let coverage = clamp(0.5 - d * draw.t.z / max(draw.origin.z, 1e-3), 0.0, 1.0);
    if coverage <= 0.0 {
        discard;
    }
    return premul(in.color, coverage);
}

// ---------------------------------------------------------------------------------------------------
// Polygons (tessellated at build time, drawn without antialiasing)

struct PolyIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: u32,
};

struct PolyOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
};

@vertex
fn vs_poly(v: PolyIn) -> PolyOut {
    var out: PolyOut;
    out.pos = to_clip(v.pos);
    out.color = unpack4x8unorm(v.color);
    return out;
}

@fragment
fn fs_poly(in: PolyOut) -> @location(0) vec4<f32> {
    return premul(in.color, 1.0);
}

// ---------------------------------------------------------------------------------------------------
// Slug glyphs: coverage from the quadratic outline, per pixel (see slug.rs, which mirrors this).

struct GlyphRecord {
    bbox: vec4<f32>,
    // Band header offset, horizontal bands, vertical bands, flags (bit 0: empty).
    info: vec4<u32>,
};

@group(1) @binding(0) var<storage, read> glyph_records: array<GlyphRecord>;
@group(1) @binding(1) var<storage, read> glyph_curves: array<vec4<f32>>;
@group(1) @binding(2) var<storage, read> glyph_bands: array<u32>;

struct GlyphIn {
    @location(0) anchor: vec2<f32>,
    @location(1) pen: vec2<f32>,
    @location(2) rot: vec2<f32>,
    @location(3) size: f32,
    @location(4) glyph: u32,
    @location(5) color: u32,
};

struct GlyphOut {
    @builtin(position) pos: vec4<f32>,
    // Em coordinates (y up) of this pixel.
    @location(0) em: vec2<f32>,
    @location(1) @interpolate(flat) glyph: u32,
    @location(2) @interpolate(flat) color: vec4<f32>,
};

@vertex
fn vs_glyph(@builtin(vertex_index) vi: u32, g: GlyphIn) -> GlyphOut {
    var out: GlyphOut;
    let rec = glyph_records[g.glyph];
    out.glyph = g.glyph;
    out.color = unpack4x8unorm(g.color);
    if (rec.info.w & 1u) != 0u {
        out.pos = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        out.em = vec2<f32>(0.0);
        return out;
    }
    // Dilate by the antialiasing width so edge pixels are shaded (Slug's dynamic dilation, simplified
    // to a uniform pad in device pixels).
    let pad = draw.origin.z / max(g.size * draw.t.z, 1e-30);
    let k = corner(vi);
    let em = mix(rec.bbox.xy - vec2<f32>(pad), rec.bbox.zw + vec2<f32>(pad), k);
    let text = (g.pen + em) * g.size;
    let r = vec2<f32>(g.rot.x * text.x - g.rot.y * text.y, g.rot.y * text.x + g.rot.x * text.y);
    // Text space is y up; flip into world space so labels stay upright for either y direction.
    let local = g.anchor + vec2<f32>(r.x, -draw.t.w * r.y);
    out.pos = to_clip(local);
    out.em = em;
    return out;
}

fn root_code(y1: f32, y2: f32, y3: f32) -> u32 {
    let i1 = bitcast<u32>(y1) >> 31u;
    let i2 = bitcast<u32>(y2) >> 30u;
    let i3 = bitcast<u32>(y3) >> 29u;
    var shift = (i2 & 2u) | (i1 & ~2u);
    shift = (i3 & 4u) | (shift & ~4u);
    return (0x2E74u >> shift) & 0x0101u;
}

// The x of both roots of y(t) = 0 for the quadratic (p1, p2, p3).
fn solve_x(p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>) -> vec2<f32> {
    let a = p1 - p2 * 2.0 + p3;
    let b = p1 - p2;
    let ra = 1.0 / a.y;
    let rb = 0.5 / b.y;
    let d = sqrt(max(b.y * b.y - a.y * p1.y, 0.0));
    var t1 = (b.y - d) * ra;
    var t2 = (b.y + d) * ra;
    if abs(a.y) < 1.0 / 65536.0 {
        t1 = p1.y * rb;
        t2 = t1;
    }
    return vec2<f32>((a.x * t1 - b.x * 2.0) * t1 + p1.x, (a.x * t2 - b.x * 2.0) * t2 + p1.x);
}

fn band_of(v: f32, lo: f32, hi: f32, n: u32) -> u32 {
    let size = (hi - lo) / f32(n);
    if size <= 0.0 {
        return 0u;
    }
    return min(u32(max(floor((v - lo) / size), 0.0)), n - 1u);
}

@fragment
fn fs_glyph(in: GlyphOut) -> @location(0) vec4<f32> {
    let rec = glyph_records[in.glyph];
    let ppem = 1.0 / max(fwidth(in.em), vec2<f32>(1e-30));
    let header = rec.info.x;
    let nh = rec.info.y;
    let nv = rec.info.z;

    var xcov = 0.0;
    var xwgt = 0.0;
    let hb = header + 2u * band_of(in.em.y, rec.bbox.y, rec.bbox.w, nh);
    let hstart = glyph_bands[hb];
    let hcount = glyph_bands[hb + 1u];
    for (var i = 0u; i < hcount; i = i + 1u) {
        let ci = glyph_bands[hstart + i];
        let c0 = glyph_curves[2u * ci];
        let c1 = glyph_curves[2u * ci + 1u];
        let p1 = c0.xy - in.em;
        let p2 = c0.zw - in.em;
        let p3 = c1.xy - in.em;
        if max(max(p1.x, p2.x), p3.x) * ppem.x < -0.5 {
            break;
        }
        let code = root_code(p1.y, p2.y, p3.y);
        if code != 0u {
            let r = solve_x(p1, p2, p3) * ppem.x;
            if (code & 1u) != 0u {
                xcov = xcov + clamp(r.x + 0.5, 0.0, 1.0);
                xwgt = max(xwgt, clamp(1.0 - abs(r.x) * 2.0, 0.0, 1.0));
            }
            if code > 1u {
                xcov = xcov - clamp(r.y + 0.5, 0.0, 1.0);
                xwgt = max(xwgt, clamp(1.0 - abs(r.y) * 2.0, 0.0, 1.0));
            }
        }
    }

    var ycov = 0.0;
    var ywgt = 0.0;
    let vb = header + 2u * nh + 2u * band_of(in.em.x, rec.bbox.x, rec.bbox.z, nv);
    let vstart = glyph_bands[vb];
    let vcount = glyph_bands[vb + 1u];
    for (var i = 0u; i < vcount; i = i + 1u) {
        let ci = glyph_bands[vstart + i];
        let c0 = glyph_curves[2u * ci];
        let c1 = glyph_curves[2u * ci + 1u];
        let p1 = c0.xy - in.em;
        let p2 = c0.zw - in.em;
        let p3 = c1.xy - in.em;
        if max(max(p1.y, p2.y), p3.y) * ppem.y < -0.5 {
            break;
        }
        let code = root_code(p1.x, p2.x, p3.x);
        if code != 0u {
            let r = solve_x(p1.yx, p2.yx, p3.yx) * ppem.y;
            // A ray a quarter turn round crosses the other way.
            if (code & 1u) != 0u {
                ycov = ycov - clamp(r.x + 0.5, 0.0, 1.0);
                ywgt = max(ywgt, clamp(1.0 - abs(r.x) * 2.0, 0.0, 1.0));
            }
            if code > 1u {
                ycov = ycov + clamp(r.y + 0.5, 0.0, 1.0);
                ywgt = max(ywgt, clamp(1.0 - abs(r.y) * 2.0, 0.0, 1.0));
            }
        }
    }

    let weighted = abs(xcov * xwgt + ycov * ywgt) / max(xwgt + ywgt, 1.0 / 65536.0);
    let coverage = clamp(max(weighted, min(abs(xcov), abs(ycov))), 0.0, 1.0);
    if coverage <= 0.0 {
        discard;
    }
    return premul(in.color, coverage);
}

// ---------------------------------------------------------------------------------------------------
// The static cache: a texture of the plane drawn earlier, placed back with the draw's transform
// (`m` = the rectangle's size, `t` = its origin).

@group(1) @binding(0) var cache_texture: texture_2d<f32>;
@group(1) @binding(1) var cache_sampler: sampler;

struct BlitOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_blit(@builtin(vertex_index) vi: u32) -> BlitOut {
    var out: BlitOut;
    let k = corner(vi);
    out.pos = to_clip(k);
    out.uv = k;
    return out;
}

@fragment
fn fs_blit(in: BlitOut) -> @location(0) vec4<f32> {
    return textureSample(cache_texture, cache_sampler, in.uv) * draw.target_size.w;
}
