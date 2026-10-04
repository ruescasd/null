// Terrain surface: the standard PBR pipeline, plus
// - baked sky visibility (vertex colour alpha) applied to indirect light only;
//   direct sunlight is left to the shadow maps, so a crevice in full sun is
//   still lit;
// - procedural grain: albedo mottling and a faint micro-relief, fixed in the
//   world so it streams past as you move. It tiles with the world's wrap
//   period and fades out with distance before it can shimmer;
// - procedural panelling: faces layered in bands that run their whole width
//   (plates, ribs, conduits, rows of openings, vents, channels), divided
//   into bays by a regular rhythm of frames; walls streaked with weathering
//   below every band. Geometry leads: the panelling is low in contrast and
//   scales with the face it is on (the mesh gives each face's size), so a
//   narrow face keeps only fine seams and a vast wall gets the full set.
//   Light only deep in recesses, and from pieces of geometry the mesh marks
//   as lit (light filaments along buried conduits).
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
// x: albedo grain strength, y: world wrap period in metres, z: relief
// strength, w: 1 for the etched network instead of the panelling.
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

fn relief_at(uv: vec2<f32>, pixel: f32, face: f32) -> Relief {
    let p = panel_at(uv);
    // How much of the panelling a face this size carries: bands' contents
    // from a couple of metres up, frames on broad faces only.
    let amount = smoothstep(1.5, 6.0, face);
    let framed = smoothstep(4.0, 10.0, face);
    let w = detail.w;
    let seam_fade = saturate(w / (pixel * 2.0) - 0.5);
    var r = Relief(0.0, 0.94 + 0.12 * p.tone * amount, 0.0);
    // Seams between bands.
    let band_edge = min(p.across, p.width - p.across);
    let band_groove = 1.0 - smoothstep(0.0, w, band_edge);
    r.height -= 0.08 * band_groove * seam_fade;
    r.albedo *= 1.0 - 0.4 * band_groove * seam_fade;
    // Frames: a raised rib between bays, across the bands that stop at
    // them; conduits and light strips run on over them.
    let runs_on = (p.kind >= 0.36 && p.kind < 0.56) || (p.kind >= 0.76 && p.kind < 0.82);
    let half = 0.15 + 0.25 * fract(f32(p.id.x >> 3u) * 0.618);
    let rib_fade = saturate(half / pixel - 0.5) * framed;
    let rib = 1.0 - smoothstep(half, half + max(w, pixel), p.frame);
    let inside_band = smoothstep(w, w * 2.0, band_edge);
    if !runs_on {
        r.height += 0.12 * rib * rib_fade;
        r.albedo *= 1.0 + 0.08 * rib * rib_fade;
    }
    let framed_edge = mix(band_edge, min(band_edge, p.frame - half), framed);
    let edge = select(framed_edge, band_edge, runs_on);
    let inside = smoothstep(w, w * 2.0, edge) * select(1.0 - rib * framed, 1.0, runs_on) * amount;
    if p.kind < 0.2 {
        // Plates, one per bay, recessed or proud by turns.
        let sign = select(-1.0, 1.0, p.bay_tone < 0.5);
        r.height += 0.05 * sign * smoothstep(w, w + 0.2, edge);
        r.albedo *= 1.0 + 0.1 * (p.bay_tone - 0.5) * inside;
    } else if p.kind < 0.36 {
        // Ribbed: close ridges across the band, in step with the frames.
        let n = max(round(p.bay / (0.25 + 0.5 * p.tone)), 2.0);
        let pitch = p.bay / n;
        let fade = saturate(pitch / (pixel * 4.0) - 0.5);
        let x = abs(fract(p.along / pitch) - 0.5) * 2.0;
        r.height += 0.04 * (0.5 - x) * fade * inside;
        r.albedo *= 1.0 - 0.12 * x * fade * inside;
    } else if p.kind < 0.56 {
        // Conduits: rounded pipes laid along the band, running on over the
        // frames, thick and few or thin and many.
        let count = 1.0 + floor(p.tone * p.tone * 8.0);
        let pitch = p.width / count;
        let fade = saturate(pitch / (pixel * 3.0) - 0.5);
        let x = (fract(p.across / pitch) - 0.5) * 2.0;
        let bulge = sqrt(max(1.0 - x * x, 0.0));
        r.height += min(0.08 * pitch, 0.3) * bulge * fade * inside_band * amount;
        r.albedo *= 1.0 - 0.25 * (1.0 - bulge) * fade * amount;
        // Collars where they pass a frame.
        r.height += 0.03 * rib * bulge * fade * rib_fade * amount;
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
        r.albedo *= 1.0 - 0.6 * dark;
    } else if p.kind < 0.76 {
        // Vents: dense grooves along the band.
        let pitch = 0.08;
        let fade = saturate(pitch / (pixel * 4.0) - 0.5);
        let x = abs(fract(p.across / pitch) - 0.5) * 2.0;
        r.height -= 0.015 * smoothstep(0.3, 0.6, x) * fade * inside;
        r.albedo *= 1.0 - 0.15 * inside;
    } else if p.kind < 0.82 {
        // A dark channel along the middle of the band, running on over the
        // frames. Below a pixel it fades with the share it covers.
        let half = 0.06 + 0.06 * p.tone;
        let coverage = saturate(2.0 * half / pixel);
        let d = abs(p.across - p.width * 0.5);
        let channel = 1.0 - smoothstep(max(half - pixel, 0.0), half + pixel * 0.5, d);
        r.height -= 0.05 * channel * coverage * amount;
        r.albedo *= 1.0 - 0.4 * channel * amount;
    } else {
        // Deep set; in some bays of broad faces a faint light shows from a
        // slot at the bottom of the recess, the only light on the walls.
        let deep = smoothstep(w, w + 0.3, edge);
        r.height -= 0.08 * deep;
        r.albedo *= 1.0 - 0.15 * amount;
        let half = 0.04 + 0.04 * p.tone;
        let coverage = saturate(2.0 * half / pixel);
        let slot = 1.0 - smoothstep(half, half + pixel, abs(p.across - (w * 2.0 + 0.3 + half)));
        let on = step(0.9, p.bay_tone) * framed * step(0.3, p.width);
        r.glow = 0.6 * on * slot * coverage * inside;
    }
    return r;
}

// Etched network: thin grooves cut into the surface along routes like the
// pipe network's. The face is divided into lanes (`lane` metres wide) and
// rows (`cell` metres high); a lane carries a groove for a run of rows at a
// time; in each row it may bend over into a neighbouring lane in a soft S
// (merging with whatever runs there), and where it bends it may also carry
// straight on (a split). A route is a bundle of one to four grooves. x: how much of the groove covers the point, y: how
// near its middle, z: whether its route is lit.
fn etch_layer(uv: vec2<f32>, pixel: f32, lane: f32, cell: f32, half: f32, salt: i32) -> vec3<f32> {
    let j = floor(uv.y / cell);
    let t = uv.y / cell - j;
    let k0 = floor(uv.x / lane);
    let ji = i32(j);
    var best = 1e9;
    var lit = 0.0;
    for (var dk = -2; dk <= 2; dk++) {
        let ki = i32(k0) + dk;
        // Lanes carry grooves in runs of four rows.
        let run = vec2<i32>(ki * 7 + salt, ji >> 2u);
        if hash(run, 1 << 20) > 0.45 {
            continue;
        }
        let h = hash(vec2<i32>(ki + salt * 3, ji * 5 + 1), 1 << 20);
        let shift = select(select(0.0, 1.0, h > 0.84), -1.0, h < 0.16);
        let s = t * t * (3.0 - 2.0 * t);
        let x = (f32(ki) + 0.5) * lane + shift * lane * s;
        let slope = shift * lane * 6.0 * t * (1.0 - t) / cell;
        // A bundle of one to four grooves side by side.
        let strands = 1.0 + floor(hash(run + vec2<i32>(0, 57), 1 << 20) * 4.0);
        let spacing = half * 3.2;
        let offset = uv.x - x;
        let nearest = clamp(round(offset / spacing + (strands - 1.0) * 0.5), 0.0, strands - 1.0);
        let strand_x = (nearest - (strands - 1.0) * 0.5) * spacing;
        var d = abs(offset - strand_x) / sqrt(1.0 + slope * slope);
        // Where it bends it may also carry straight on.
        if shift != 0.0 && hash(vec2<i32>(ki * 11 + salt, ji * 3 + 2), 1 << 20) < 0.35 {
            d = min(d, abs(uv.x - (f32(ki) + 0.5) * lane));
        }
        if d < best {
            best = d;
            lit = step(hash(run + vec2<i32>(91, 0), 1 << 20), 0.07) * step(nearest, 0.5);
        }
    }
    let coverage = saturate(2.0 * half / pixel);
    let groove = (1.0 - smoothstep(half - pixel * 0.5, half + pixel * 0.5, best)) * select(coverage, 1.0, half > pixel);
    let middle = 1.0 - smoothstep(0.0, half * 0.45 + pixel * 0.5, best);
    return vec3<f32>(groove, middle * coverage, lit);
}

fn etch_at(uv: vec2<f32>, pixel: f32, face: f32) -> Relief {
    // Fine etchings everywhere but on the narrowest faces; wide grooves on
    // broad ones.
    let fine = etch_layer(uv, pixel, 0.9, 4.0, 0.045, 3) * smoothstep(0.6, 2.0, face);
    let wide = etch_layer(uv, pixel, 3.4, 13.0, 0.2, 17) * smoothstep(3.0, 8.0, face);
    var r = Relief(0.0, 1.0, 0.0);
    r.height = -0.035 * fine.x - 0.18 * wide.x;
    r.albedo = 1.0 - 0.35 * max(fine.x, wide.x);
    // A faint light deep in some wide grooves.
    r.glow = wide.z * wide.y * 0.12;
    return r;
}

// The surface detail at a point: the etched network, or the panelling.
fn surface_at(uv: vec2<f32>, pixel: f32, face: f32) -> Relief {
    if grain.w > 0.5 {
        return etch_at(uv, pixel, face);
    }
    return relief_at(uv, pixel, face);
}

// Weathering on walls: dark streaks run down from the top of each band, in
// narrow columns of their own lengths, as if something had seeped from the
// ledges for a very long time. Albedo factor; below a pixel the columns
// blend to their average.
fn streaks_at(uv: vec2<f32>, pixel: f32, face: f32) -> f32 {
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
    return 1.0 - 0.22 * s * smoothstep(2.0, 8.0, face);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    var face = 1000.0;
    var lit = 0.0;
#ifdef VERTEX_COLORS
    let visibility = mix(1.0, in.color.a, params.x);
    pbr_input.material.base_color = vec4<f32>(vec3<f32>(in.color.r), 1.0);
    face = in.color.g;
    lit = in.color.b;
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
        let r = surface_at(uv, pixel, face);
        let e = max(0.02, pixel);
        let ru = surface_at(uv + vec2<f32>(e, 0.0), pixel, face).height - r.height;
        let rv = surface_at(uv + vec2<f32>(0.0, e), pixel, face).height - r.height;
        let slope = (u_axis * ru + v_axis * rv) / e;
        pbr_input.N = normalize(pbr_input.N - slope * detail.x);
        let base = pbr_input.material.base_color.rgb;
        var albedo = r.albedo;
        if abs(n.y) < 0.7 {
            albedo *= streaks_at(uv, pixel, face);
        }
        pbr_input.material.base_color = vec4<f32>(base * mix(1.0, albedo, detail.x), 1.0);
        pbr_input.material.emissive = vec4<f32>(vec3<f32>(r.glow * detail.y), 1.0);
    }

    // Lit pieces of geometry (light filaments): as bright as the light in
    // the recesses.
    if lit > 0.0 {
        pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + vec3<f32>(lit * detail.y), 1.0);
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
