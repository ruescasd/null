// Terrain surface: the standard PBR pipeline, plus
// - baked sky visibility (vertex colour alpha) applied to indirect light only;
//   direct sunlight is left to the shadow maps, so a crevice in full sun is
//   still lit;
// - procedural grain: albedo mottling and a faint micro-relief, fixed in the
//   world so it streams past as you move. It tiles with the world's wrap
//   period and fades out with distance before it can shimmer;
// - procedural panelling: faces layered in bands that run their whole width
//   (plates, ribs, conduits, rows of openings, vents, light strips), divided
//   into bays by a regular rhythm of frames; walls streaked with weathering
//   below every band.
//   It is all in the shading (normal and albedo), no geometry; each detail
//   fades out before it gets near the pixel size.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

// x: strength of the baked occlusion (0 disables it), y: curvature 1 / 2R,
// zw: camera x and z.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> params: vec4<f32>;
// x: albedo grain strength, y: world wrap period in metres, z: relief strength.
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> grain: vec4<f32>;
// Panelling. x: strength (0 off), y: inlay glow, z: largest panel (m),
// w: seam width (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> detail: vec4<f32>;

fn hash(cell: vec2<i32>, period: i32) -> f32 {
    // Wrap the lattice so the pattern tiles with the world.
    let c = bitcast<vec2<u32>>(((cell % period) + period) % period);
    var h = (c.x * 0x8da6b343u) ^ (c.y * 0xd8163841u);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777216.0;
}

// Value noise in 0..1 with `cells_per_metre` lattice cells per metre.
fn value_noise(p: vec2<f32>, cells_per_metre: f32) -> f32 {
    let q = p * cells_per_metre;
    let i = vec2<i32>(floor(q));
    let f = fract(q);
    let u = f * f * (3.0 - 2.0 * f);
    let period = i32(round(grain.y * cells_per_metre));
    let a = hash(i, period);
    let b = hash(i + vec2(1, 0), period);
    let c = hash(i + vec2(0, 1), period);
    let d = hash(i + vec2(1, 1), period);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Mottling and relief height (metres) on a surface plane, with each octave
// faded out once it gets close to the pixel size.
struct Grain {
    mottle: f32,
    height: f32,
}

fn sample_grain(p: vec2<f32>, pixel: f32) -> Grain {
    // Octaves: 25 cm speckle, 1 m mottling, 4 m blotches (powers of two, so
    // every lattice fits the wrap period exactly).
    let freqs = vec3<f32>(4.0, 1.0, 0.25);
    let mottle_weights = vec3<f32>(0.35, 0.4, 0.25);
    let heights = vec3<f32>(0.004, 0.012, 0.03);
    var g = Grain(0.0, 0.0);
    for (var k = 0; k < 3; k++) {
        let cell = 1.0 / freqs[k];
        let fade = saturate(cell / (pixel * 3.0) - 1.0);
        let n = value_noise(p, freqs[k]) - 0.5;
        g.mottle += n * mottle_weights[k] * fade;
        g.height += n * heights[k] * fade;
    }
    return g;
}

// The panelling around a point on a surface plane. Faces are layered in
// bands that run their whole width (a recursive split of the height, the
// same all along the face), and a regular rhythm of frames divides the
// bands into bays, so detail has direction: conduits and light strips run
// on through the frames, ribs and openings repeat in step.
struct Panel {
    // Position across the band (up the face) and along it, and the band's
    // height.
    across: f32,
    along: f32,
    width: f32,
    // Distance to the nearest frame, and the bay's length.
    frame: f32,
    bay: f32,
    // What kind of band, 0..1; its tone, and the bay's.
    kind: f32,
    tone: f32,
    bay_tone: f32,
    // Which bay (x) in which band (y), for per-bay choices.
    id: vec2<i32>,
}

fn panel_at(uv: vec2<f32>) -> Panel {
    let s0 = detail.z;
    let period = max(i32(round(grain.y / s0)), 1);
    // Bands: split the height recursively.
    let cell = i32(floor(uv.y / s0));
    let f = uv.y / s0 - f32(cell);
    var lo = 0.0;
    var hi = 1.0;
    var path = 1;
    for (var k = 0; k < 5; k++) {
        let stop = hash(vec2<i32>(path * 31 + k, cell), period);
        if (k > 0 && stop < 0.2) || (hi - lo) * s0 < 1.2 {
            break;
        }
        let m = mix(lo, hi, 0.25 + 0.5 * hash(vec2<i32>(path * 71 + 11, cell), period));
        if f < m { hi = m; path = path * 2; } else { lo = m; path = path * 2 + 1; }
    }
    let band = vec2<i32>(path * 97 + 5, cell);
    // Frames: a few bays per stretch of twice the largest panel, the same
    // through every band.
    let stretch = s0 * 2.0;
    let stretches = max(i32(round(grain.y / stretch)), 1);
    let sc = i32(floor(uv.x / stretch));
    let pick = min(i32(hash(vec2<i32>(sc, 401), stretches) * 4.0), 3);
    let count = f32(select(select(select(8, 6, pick == 2), 4, pick == 1), 3, pick == 0));
    let bay = stretch / count;
    let x = uv.x - f32(sc) * stretch;
    let bi = floor(x / bay);
    var p: Panel;
    p.across = (f - lo) * s0;
    p.width = (hi - lo) * s0;
    p.along = x - bi * bay;
    p.bay = bay;
    p.frame = min(p.along, bay - p.along);
    p.kind = hash(band, period);
    p.tone = hash(band + vec2<i32>(13, 0), period);
    p.id = vec2<i32>(sc * 8 + i32(bi), path * 1024 + cell);
    p.bay_tone = hash(p.id + vec2<i32>(0, 77), 1 << 20);
    return p;
}

// The panelling's height (metres, negative is recessed), albedo factor and
// glow at a point, with details smaller than `pixel` faded out.
struct Relief {
    height: f32,
    albedo: f32,
    glow: f32,
}

fn relief_at(uv: vec2<f32>, pixel: f32) -> Relief {
    let p = panel_at(uv);
    let w = detail.w;
    let seam_fade = saturate(w / (pixel * 2.0) - 0.5);
    var r = Relief(0.0, 0.85 + 0.3 * p.tone, 0.0);
    // Seams between bands.
    let band_edge = min(p.across, p.width - p.across);
    let band_groove = 1.0 - smoothstep(0.0, w, band_edge);
    r.height -= 0.08 * band_groove * seam_fade;
    r.albedo *= 1.0 - 0.6 * band_groove * seam_fade;
    // Frames: a raised rib between bays, across the bands that stop at
    // them; conduits and light strips run on over them.
    let runs_on = (p.kind >= 0.36 && p.kind < 0.56) || (p.kind >= 0.76 && p.kind < 0.82);
    let half = 0.15 + 0.25 * fract(f32(p.id.x >> 3u) * 0.618);
    let rib_fade = saturate(half / pixel - 0.5);
    let rib = 1.0 - smoothstep(half, half + max(w, pixel), p.frame);
    let inside_band = smoothstep(w, w * 2.0, band_edge);
    if !runs_on {
        r.height += 0.12 * rib * rib_fade;
        r.albedo *= 1.0 + 0.15 * rib * rib_fade;
    }
    let edge = select(min(band_edge, p.frame - half), band_edge, runs_on);
    let inside = smoothstep(w, w * 2.0, edge) * select(1.0 - rib, 1.0, runs_on);
    if p.kind < 0.2 {
        // Plates, one per bay, recessed or proud by turns.
        let sign = select(-1.0, 1.0, p.bay_tone < 0.5);
        r.height += 0.05 * sign * smoothstep(w, w + 0.2, edge);
        r.albedo *= 0.9 + 0.2 * p.bay_tone * inside;
    } else if p.kind < 0.36 {
        // Ribbed: close ridges across the band, in step with the frames.
        let n = max(round(p.bay / (0.25 + 0.5 * p.tone)), 2.0);
        let pitch = p.bay / n;
        let fade = saturate(pitch / (pixel * 4.0) - 0.5);
        let x = abs(fract(p.along / pitch) - 0.5) * 2.0;
        r.height += 0.04 * (0.5 - x) * fade * inside;
        r.albedo *= 1.0 - 0.25 * x * fade * inside;
    } else if p.kind < 0.56 {
        // Conduits: rounded pipes laid along the band, running on over the
        // frames, thick and few or thin and many.
        let count = 1.0 + floor(p.tone * p.tone * 8.0);
        let pitch = p.width / count;
        let fade = saturate(pitch / (pixel * 3.0) - 0.5);
        let x = (fract(p.across / pitch) - 0.5) * 2.0;
        let bulge = sqrt(max(1.0 - x * x, 0.0));
        r.height += min(0.08 * pitch, 0.3) * bulge * fade * inside_band;
        r.albedo *= 1.0 - 0.5 * (1.0 - bulge) * fade;
        // Collars where they pass a frame.
        r.height += 0.03 * rib * bulge * fade * rib_fade;
    } else if p.kind < 0.68 {
        // Perforated: rows of small dark openings in step with the bays;
        // now and then one is lit. Below a pixel they fade to their
        // average darkness.
        let size = vec2<f32>(0.6 + 0.8 * p.tone, 0.35 + 0.3 * p.tone);
        let nx = max(round(p.bay / (size.x * 2.0)), 1.0);
        let ny = max(round(p.width / (size.y * 2.5)), 1.0);
        let pitch = vec2<f32>(p.bay / nx, p.width / ny);
        let q = vec2<f32>(p.along, p.across);
        let local = abs(fract(q / pitch) - 0.5) * pitch;
        let hole = (1.0 - smoothstep(size.x * 0.5 - pixel, size.x * 0.5 + pixel, local.x))
            * (1.0 - smoothstep(size.y * 0.5 - pixel, size.y * 0.5 + pixel, local.y));
        let resolved = saturate(min(size.x, size.y) / (pixel * 2.0) - 0.25);
        let average = size.x * size.y / (pitch.x * pitch.y);
        let dark = mix(average, hole, resolved) * inside;
        r.height -= 0.08 * hole * resolved * inside;
        r.albedo *= 1.0 - 0.85 * dark;
        let id = vec2<i32>(floor(q / pitch)) + p.id * 64;
        let lit = step(hash(id, 1 << 20), 0.03);
        r.glow = lit * dark * 0.5;
    } else if p.kind < 0.76 {
        // Vents: dense grooves along the band.
        let pitch = 0.08;
        let fade = saturate(pitch / (pixel * 4.0) - 0.5);
        let x = abs(fract(p.across / pitch) - 0.5) * 2.0;
        r.height -= 0.015 * smoothstep(0.3, 0.6, x) * fade * inside;
        r.albedo *= 1.0 - 0.35 * inside;
    } else if p.kind < 0.82 {
        // A light strip along the middle of the band, running on over the
        // frames, out in a bay now and then. Wide enough to read from afar;
        // below a pixel it fades with the share it covers.
        let half = 0.06 + 0.06 * p.tone;
        let coverage = saturate(2.0 * half / pixel);
        let d = abs(p.across - p.width * 0.5);
        let channel = 1.0 - smoothstep(max(half - pixel, 0.0), half + pixel * 0.5, d);
        let on = step(0.45, p.bay_tone);
        r.height -= 0.05 * channel * coverage;
        r.glow = on * max(channel, coverage * 0.5 * step(d, half + pixel));
        r.albedo *= 1.0 - 0.6 * channel;
    } else {
        // Plain, deep set.
        r.height -= 0.08 * smoothstep(w, w + 0.3, edge);
        r.albedo *= 0.8;
    }
    return r;
}

// Weathering on walls: dark streaks run down from the top of each band, in
// narrow columns of their own lengths, as if something had seeped from the
// ledges for a very long time. Albedo factor; below a pixel the columns
// blend to their average.
fn streaks_at(uv: vec2<f32>, pixel: f32) -> f32 {
    let p = panel_at(uv);
    let column = 0.35;
    let c = floor(uv.x / column);
    let h = hash(vec2<i32>(i32(c), p.id.y), 1 << 20);
    let h2 = hash(vec2<i32>(i32(c) + 7919, p.id.y), 1 << 20);
    let length = 0.5 + 9.0 * h * h;
    let t = saturate((p.width - p.across) / length);
    let x = fract(uv.x / column);
    let profile = smoothstep(0.0, 0.35, x) * smoothstep(1.0, 0.65, x);
    let streak = (1.0 - t) * (1.0 - t) * step(0.45, h2) * profile * (0.4 + 0.6 * h2);
    let resolved = saturate(column / (pixel * 3.0) - 0.3);
    let s = mix(0.08, streak, resolved);
    return 1.0 - 0.45 * s;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_COLORS
    let visibility = mix(1.0, in.color.a, params.x);
    pbr_input.material.base_color = vec4<f32>(in.color.rgb, 1.0);
    pbr_input.diffuse_occlusion *= visibility;
    pbr_input.specular_occlusion *= visibility;
#endif

    // Undo the curvature so the grain stays fixed to the ground as the camera
    // moves.
    var p = in.world_position.xyz;
    let d = p.xz - params.zw;
    p.y += dot(d, d) * params.y;

    // Project onto the surface's own plane: tops use x/z, walls run along
    // their face and up.
    let n = normalize(in.world_normal);
    var u_axis = vec3<f32>(1.0, 0.0, 0.0);
    var v_axis = vec3<f32>(0.0, 0.0, 1.0);
    if abs(n.y) < 0.7 {
        u_axis = normalize(vec3<f32>(-n.z, 0.0, n.x));
        v_axis = vec3<f32>(0.0, 1.0, 0.0);
    }
    let uv = vec2<f32>(dot(p, u_axis), dot(p, v_axis));
    let pixel = max(length(fwidth(uv)), 1e-4);

    let g = sample_grain(uv, pixel);
    pbr_input.material.base_color = vec4<f32>(
        pbr_input.material.base_color.rgb * max(1.0 + grain.x * 2.0 * g.mottle, 0.0),
        1.0,
    );

    // Panelling: its own relief, albedo and glow, on top of the grain.
    if detail.x > 0.0 {
        let r = relief_at(uv, pixel);
        let e = max(0.02, pixel);
        let ru = relief_at(uv + vec2<f32>(e, 0.0), pixel).height - r.height;
        let rv = relief_at(uv + vec2<f32>(0.0, e), pixel).height - r.height;
        let slope = (u_axis * ru + v_axis * rv) / e;
        pbr_input.N = normalize(pbr_input.N - slope * detail.x);
        let base = pbr_input.material.base_color.rgb;
        var albedo = r.albedo;
        if abs(n.y) < 0.7 {
            albedo *= streaks_at(uv, pixel);
        }
        pbr_input.material.base_color = vec4<f32>(base * mix(1.0, albedo, detail.x), 1.0);
        pbr_input.material.emissive = vec4<f32>(vec3<f32>(r.glow * detail.y), 1.0);
    }

    // Micro-relief: tilt the normal by the height field's slope, found with
    // two extra samples a little way along the plane.
    if grain.z > 0.0 {
        let e = 0.04;
        let gu = sample_grain(uv + vec2<f32>(e, 0.0), pixel).height - g.height;
        let gv = sample_grain(uv + vec2<f32>(0.0, e), pixel).height - g.height;
        let slope = (u_axis * gu + v_axis * gv) / e;
        pbr_input.N = normalize(pbr_input.N - slope * grain.z);
    }

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
