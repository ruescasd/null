// Terrain surface: the standard PBR pipeline, plus
// - baked sky visibility (vertex colour alpha) applied to indirect light only;
//   direct sunlight is left to the shadow maps, so a crevice in full sun is
//   still lit;
// - procedural grain: albedo mottling and a faint micro-relief, fixed in the
//   world so it streams past as you move. It tiles with the world's wrap
//   period and fades out with distance before it can shimmer;
// - light from pieces of geometry the mesh marks as lit (glowing etchings
//   and seams);
// - paving (an experiment): in some regions of the ground, a pattern of
//   tiles with cut joints, a ruler underfoot and something to stream past.

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
// strength.
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> grain: vec4<f32>;
// x: brightness of lit geometry.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> glow: vec4<f32>;
// x: pattern (0 none, 1 checkerboard, 2 warped, 3 fractal, 4 warped
// fractal; plus 100: whole, not broken at steps), y: tile size (m), z:
// contrast, w: share of the ground paved.
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> paving: vec4<f32>;

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

// The paving region a point falls in (cells of a Voronoi pattern 512 m
// apart, which fits the wrap period): whether it is paved, its centre, and a
// number of its own.
struct Region {
    paved: bool,
    centre: vec2<f32>,
    id: f32,
}

fn region(p: vec2<f32>) -> Region {
    let size = 512.0;
    let period = i32(round(grain.y / size));
    let base = vec2<i32>(floor(p / size));
    var best = 1e9;
    var r = Region(false, vec2<f32>(0.0), 0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = base + vec2<i32>(x, y);
            let f = (vec2<f32>(c) + vec2<f32>(hash(c, period), hash(c + vec2(17, 31), period)) * 0.8 + 0.1) * size;
            let d = distance(p, f);
            if d < best {
                best = d;
                let id = hash(c + vec2(53, 7), period);
                r = Region(id < paving.w, f, id);
            }
        }
    }
    return r;
}

// Distance (in tile units) to the nearest joint of a unit square tiling at
// `q`, and the tile it is in.
fn tile_edge(q: vec2<f32>) -> f32 {
    let f = fract(q);
    let e = min(f, 1.0 - f);
    return min(e.x, e.y);
}

// The paving at `p` on a top surface: a tone (added to albedo, about -1..1)
// and how much of a joint is there (0..1), antialiased with `pixel`.
struct Paving {
    tone: f32,
    joint: f32,
}

fn pave(p: vec2<f32>, height: f32, pixel: f32) -> Paving {
    var out = Paving(0.0, 0.0);
    let r = region(p);
    if !r.paved {
        return out;
    }
    let tile = paving.y;
    let whole = paving.x > 99.5;
    let mode = i32(round(paving.x)) % 100;
    // Turned by the region's own angle; and, unless whole, each flat piece
    // of ground at its own height its own turn and offset, so the pattern
    // breaks at every step, a floor heaved apart.
    var a = r.id * 6.2831;
    var shift = vec2<f32>(0.0);
    if !whole {
        let level = vec2<i32>(i32(round(height * 4.0)), 977);
        a += hash(level, 1 << 20) * 6.2831;
        shift = vec2<f32>(hash(level + vec2(0, 13), 1 << 20), hash(level + vec2(0, 29), 1 << 20)) * tile * 8.0;
    }
    let rot = mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a));
    var z = rot * (p - r.centre) + shift;
    let joint_w = 0.025; // in tile units: 5 cm on a 2 m tile
    if mode == 2 || mode == 4 {
        // Warped: z + a sin(z / L), a conformal map, so every tile stays
        // square while the lines bend over distance.
        let l = 150.0;
        let k = 0.2;
        let w = z / l;
        let s = vec2<f32>(sin(w.x) * cosh(w.y), cos(w.x) * sinh(w.y));
        z = z + s * (k * l);
    }
    if mode == 1 || mode == 2 {
        let q = z / tile;
        let i = vec2<i32>(floor(q));
        let checker = f32((i.x + i.y) & 1) * 2.0 - 1.0;
        let aa = max(pixel / tile, 1e-4);
        out.tone = checker;
        out.joint = 1.0 - smoothstep(joint_w - aa, joint_w + aa, tile_edge(q));
        // Joints thinner than a pixel fade rather than shimmer.
        out.joint *= saturate(joint_w / aa);
    } else if mode == 3 || mode == 4 {
        // Fractal: tiles four times the tile size split again and again (in
        // two each way), down to a quarter of it; each finished tile its own
        // tone; the joints thinner the smaller the tiles.
        var size = tile * 4.0;
        var edge_m = 1e9;
        var tone = 0.0;
        let period = 1 << 20;
        for (var level = 0; level < 6; level++) {
            let q = z / size;
            let i = vec2<i32>(floor(q));
            let w = 0.06 * pow(size / (tile * 4.0), 0.35);
            edge_m = min(edge_m, tile_edge(q) * size - w);
            let h = hash(i + vec2(level * 1013, level * 7919), period);
            tone = hash(i + vec2(level * 331 + 5, 91), period) * 2.0 - 1.0;
            if h > 0.7 || size < tile * 0.5 {
                break;
            }
            size *= 0.5;
        }
        out.tone = tone;
        out.joint = 1.0 - smoothstep(-pixel, pixel, edge_m);
    }
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    var lit = 0.0;
#ifdef VERTEX_COLORS
    let visibility = mix(1.0, in.color.a, params.x);
    pbr_input.material.base_color = vec4<f32>(vec3<f32>(in.color.r), 1.0);
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

    // Paving, on top surfaces in paved regions.
    if paving.x > 0.5 && n.y > 0.85 {
        let pv = pave(p.xz, p.y, pixel);
        let c = pbr_input.material.base_color.rgb * (1.0 + pv.tone * paving.z) * (1.0 - 0.55 * pv.joint);
        pbr_input.material.base_color = vec4<f32>(c, 1.0);
    }

    // Lit pieces of geometry (glowing etchings and seams).
    if lit > 0.0 {
        pbr_input.material.emissive = vec4<f32>(pbr_input.material.emissive.rgb + vec3<f32>(lit * glow.x), 1.0);
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
