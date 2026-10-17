// Terrain surface: the standard PBR pipeline, plus
// - baked sky visibility (vertex colour alpha) applied to indirect light only;
//   direct sunlight is left to the shadow maps, so a crevice in full sun is
//   still lit;
// - light from pieces of geometry the mesh marks as lit (glowing etchings
//   and seams);
// - paving (off by default): in some regions of the ground, a pattern of
//   tiles with cut joints, a ruler underfoot and something to stream past.
// - the plates' subdivision (`--opt nosubdiv` for none): each plate split
//   on below what the geometry draws, by the same rule, into flush pieces
//   with grooved joints (see `subdivide`). What the ground has instead of a
//   texture: a sense of speed from the ground's own make-up.
// The ground is otherwise plain. (Its grain, value-noise mottling, read as
// camouflage, a texture gone wrong, and the flecks and cracks tried after it
// as cosmetic: the user, 2026-10-17; see docs/removed.md. A sense of speed
// has to come from built things.)

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
// x: world wrap period in metres; the plates' subdivision: y on (1) or off
// (0), z the chance a piece splits again, w the largest pieces drawn (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> wrap: vec4<f32>;
// The plates' subdivision: x the smallest pieces drawn (m), y the slope of
// the joints' sides (degrees; 0, flat dark lines).
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> pieces: vec4<f32>;
// x: brightness of lit geometry; the plates' subdivision: y each piece's
// tone (a share of the albedo either way), z its tilt (degrees), w how dark
// the joints are.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> glow: vec4<f32>;
// x: pattern (0 none, 1 checkerboard, 2 fractal), y: tile size (m), z:
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

// A Voronoi diagram of sites `size` m apart (jittered as the plates' are),
// its own for each `seed`: the cell `p` is in, how far `p` is from that
// cell's border (m), and which way that border lies.
struct Cell {
    id: vec2<i32>,
    edge: f32,
    dir: vec2<f32>,
}

fn voronoi(p: vec2<f32>, size: f32, seed: i32) -> Cell {
    let q = p / size;
    let i = vec2<i32>(floor(q));
    let period = i32(round(wrap.x / size));
    let s = vec2<i32>(seed, seed * 7 + 3);
    var best = 1e9;
    var near = vec2<f32>(0.0);
    var id = i;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = i + vec2<i32>(x, y);
            let at = vec2<f32>(c) + 0.5 + (vec2<f32>(hash(c + s, period), hash(c + s + vec2(17, 31), period)) - 0.5) * 0.6;
            let d = dot(at - q, at - q);
            if d < best {
                best = d;
                near = at;
                id = c;
            }
        }
    }
    // (The distance to the border with each neighbour, along the line
    // between the two sites.)
    var edge = 1e9;
    var dir = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = id + vec2<i32>(x, y);
            if x == 0 && y == 0 {
                continue;
            }
            let at = vec2<f32>(c) + 0.5 + (vec2<f32>(hash(c + s, period), hash(c + s + vec2(17, 31), period)) - 0.5) * 0.6;
            let across = normalize(at - near);
            let e = dot((near + at) * 0.5 - q, across);
            if e < edge {
                edge = e;
                dir = across;
            }
        }
    }
    return Cell(id, edge * size, dir);
}

// A plate split on below what the geometry draws, by the plates' own rule:
// a piece splits into the pieces of the next, finer Voronoi diagram that fall
// inside it, and those may split again; every piece flush, but its own tone
// and tilt (lit as if tilted, as the canal's stones), with a fine joint round
// it. Pieces of 32, 8, 2 and 0.5 m (the plates are 128, 32 and 8); each
// scale fades out before its pieces near a pixel. `level` is the plate's own
// (0..=2), `id` its number.
struct Sub {
    tone: f32,
    tilt: vec2<f32>,
    joint: f32,
    // The joint's side sloping down to the border, as a groove's would: the
    // way it leans, faded where the groove is narrower than a pixel.
    groove: vec2<f32>,
}

fn subdivide(p: vec2<f32>, pixel: f32, level: i32, id: i32) -> Sub {
    var out = Sub(0.0, vec2<f32>(0.0), 0.0, vec2<f32>(0.0));
    let sizes = array<f32, 4>(32.0, 8.0, 2.0, 0.5);
    let plate_size = 128.0 / f32(1 << u32(2 * level));
    var seed = id * 131 + level * 977;
    for (var k = 0; k < 4; k++) {
        let size = sizes[k];
        if size >= plate_size || size > wrap.w {
            continue;
        }
        if size < pieces.x {
            break;
        }
        let fade = saturate(size / (pixel * 6.0) - 1.0);
        if fade <= 0.0 {
            break;
        }
        let c = voronoi(p, size, seed);
        let period = i32(round(wrap.x / size));
        // Joints thinner the smaller the pieces: 10 cm round 32 m pieces,
        // 3 cm round 8 m, 1 cm round 2 m; thinner than a pixel, fainter.
        let w = 0.003 * size + 0.004;
        let drawn = max(w, pixel);
        let joint = saturate(1.0 - (c.edge - drawn * 0.5) / pixel) * (w / drawn) * fade;
        if joint > out.joint {
            out.joint = joint;
            out.groove = c.dir * joint;
        }
        let a = hash(c.id + vec2(seed, 5), period) * 6.2831;
        out.tone = mix(out.tone, hash(c.id + vec2(seed, 3), period) * 2.0 - 1.0, fade);
        out.tilt = mix(out.tilt, vec2<f32>(cos(a), sin(a)) * hash(c.id + vec2(seed, 7), period), fade);
        if hash(c.id + vec2(seed, 11), period) >= wrap.z {
            break;
        }
        seed = i32(hash(c.id + vec2(seed, 13), period) * 65536.0);
    }
    return out;
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
    let period = i32(round(wrap.x / size));
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
    let mode = i32(round(paving.x));
    // Each flat piece of ground at its own height its own turn and offset of
    // the pattern, so it breaks at every step: laid plate by plate.
    let level = vec2<i32>(i32(round(height * 4.0)), 977);
    let a = (r.id + hash(level, 1 << 20)) * 6.2831;
    let shift = vec2<f32>(hash(level + vec2(0, 13), 1 << 20), hash(level + vec2(0, 29), 1 << 20)) * tile * 8.0;
    let rot = mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a));
    let z = rot * (p - r.centre) + shift;
    let joint_w = 0.025; // in tile units: 5 cm on a 2 m tile
    if mode == 1 {
        let q = z / tile;
        let i = vec2<i32>(floor(q));
        let checker = f32((i.x + i.y) & 1) * 2.0 - 1.0;
        let aa = max(pixel / tile, 1e-4);
        out.tone = checker;
        out.joint = 1.0 - smoothstep(joint_w - aa, joint_w + aa, tile_edge(q));
        // Joints thinner than a pixel fade rather than shimmer.
        out.joint *= saturate(joint_w / aa);
    } else if mode == 2 {
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
    // A plate's top: its level and number (see `plate_code` in worldgen).
    var plate = -1;
#ifdef VERTEX_COLORS
    if in.color.g > 0.5 {
        plate = i32(round(in.color.g)) - 1;
    }
    let visibility = mix(1.0, in.color.a, params.x);
    pbr_input.material.base_color = vec4<f32>(vec3<f32>(in.color.r), 1.0);
    lit = in.color.b;
    pbr_input.diffuse_occlusion *= visibility;
    pbr_input.specular_occlusion *= visibility;
#endif

    // Undo the curvature so the paving stays fixed to the ground as the
    // camera moves.
    var p = in.world_position.xyz;
    let d = p.xz - params.zw;
    p.y += dot(d, d) * params.y;

    let n = normalize(in.world_normal);
    // (The size of a pixel on the ground, in metres: outside the branch,
    // since derivatives need every pixel nearby to take the same path.)
    let pixel = max(length(fwidth(p.xz)), 1e-4);

    // The plates' subdivision, on their tops.
    if wrap.y > 0.5 && plate >= 0 && n.y > 0.85 {
        let sub = subdivide(p.xz, pixel, plate % 4, plate / 4);
        // (Grooved, the joint is shaded by its slope, and darkened less.)
        let dark = select(glow.w, glow.w * 0.4, pieces.y > 0.0);
        let c = pbr_input.material.base_color.rgb * (1.0 + sub.tone * glow.y) * (1.0 - dark * sub.joint);
        pbr_input.material.base_color = vec4<f32>(c, 1.0);
        let t = tan(radians(glow.z));
        let g = tan(radians(pieces.y));
        pbr_input.N = normalize(pbr_input.N + vec3<f32>(sub.tilt.x, 0.0, sub.tilt.y) * t + vec3<f32>(sub.groove.x, 0.0, sub.groove.y) * g);
    }

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
