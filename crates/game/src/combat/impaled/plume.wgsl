// The impaled brute's thrust (`plume.rs`): a ribbon turned to face the
// camera, white-hot at its source, fading to black outwards and downwards
// (the user's picture of a plume), in curves or in hard-edged shards, faint
// streaks in its core, bent all along. Blended, not
// multiplied: dark on the pale ground by day, white on its dark ground at
// night and against its own black body.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
}

// x: how bright its streaks, z: which look (1 in curves, 2 in shards), w:
// from how far down it flares and fades over the ground (of the way).
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> look: vec4<f32>;
// Its shape: x, y how wide at its source and at the ground before it flares
// (m), z how much wider it flares there, w how far down it grows from narrow
// to its width (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> shape: vec4<f32>;

// One beam, made once (`field.rs`) and shaped here, so that nothing of it is
// made again each frame (the user, 2026-11-12: a brute cost a fifth of the
// frame, most of it the renderer taking in what was made again): its
// centreline runs down its local -y over its length (its transform scales
// y to that, and turns it along its thrust); its tag (`MeshTag`) carries how
// far its streaks have run (bits 0-19, mm, wrapping), how much of it there
// is (20-25), how hard it strains (26-30), which sole (31).
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // A point on its centreline, (0, -t, 0).
    @location(0) position: vec3<f32>,
    // x: which side (-1, 1); y: how far down it (0 at its source, 1 at the
    // ground).
    @location(2) uv: vec2<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) side: f32,
    @location(1) flow: f32,
    @location(2) down: f32,
    @location(3) on: f32,
}

// Its width `t` of the way down, `long` metres long, `wide` times its own.
fn width(t: f32, long: f32, wide: f32) -> f32 {
    let flare = clamp((t - look.w) / (1.0 - look.w), 0.0, 1.0);
    // (Born narrow where it starts below its soles.)
    let born = 1.0 - pow(1.0 - min(long * t / shape.w, 1.0), 2.0);
    return (shape.x + (shape.y - shape.x) * sqrt(t)) * wide * (1.0 + (shape.z - 1.0) * flare * flare) * (0.45 + 0.55 * born);
}

@vertex
fn vertex(in: Vertex) -> Out {
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let tag = mesh_functions::get_tag(in.instance_index);
    let ran = f32(tag & 0xfffffu) * 0.001;
    let on = f32((tag >> 20u) & 63u) / 63.0;
    let hard = f32((tag >> 26u) & 31u) / 31.0;
    let k = f32(tag >> 31u);
    let reach = (world_from_local * vec4<f32>(0.0, -1.0, 0.0, 0.0)).xyz;
    let long = max(length(reach), 1e-4);
    let along = reach / long;
    let t = in.uv.y;
    let p = (world_from_local * vec4<f32>(in.position, 1.0)).xyz;
    // (Never quite steady.)
    let flicker = 1.0 + 0.08 * sin(globals.time * 31.0 + k * 2.1) * sin(globals.time * 13.0 + k);
    let wide = (0.35 + 0.65 * on) * (1.0 + 0.3 * hard) * flicker;
    var w = width(t, long, wide);
    if look.z > 1.5 {
        // (Its outline in shards: straight runs between a few kinks, its
        // birth, the middle, where it flares, near the ground, the ground.)
        var kinks = array<f32, 6>(0.0, min(shape.w / long, 0.5), 0.35, look.w, 0.82, 1.0);
        for (var i = 1; i < 6; i++) {
            for (var j = i; j > 0 && kinks[j - 1] > kinks[j]; j--) {
                let x = kinks[j];
                kinks[j] = kinks[j - 1];
                kinks[j - 1] = x;
            }
        }
        var a = kinks[4];
        var b = kinks[5];
        for (var i = 0; i < 5; i++) {
            if t <= kinks[i + 1] {
                a = kinks[i];
                b = kinks[i + 1];
                break;
            }
        }
        let f = select(1.0, (t - a) / (b - a), b > a);
        w = width(a, long, wide) + (width(b, long, wide) - width(a, long, wide)) * f;
    }
    let to_camera = normalize(view.world_position - p);
    let side = normalize(cross(along, to_camera));
    var out: Out;
    out.position = position_world_to_clip(p + side * 0.5 * w * in.uv.x);
    out.side = in.uv.x;
    out.flow = long * t - ran;
    out.down = t;
    out.on = on;
    return out;
}

fn hash(n: f32) -> f32 {
    return fract(sin(n * 127.1 + 311.7) * 43758.5453);
}

// Streaks across it, each in its own lane, of its own length and pace.
const LANES: f32 = 9.0;
// How many steps its heat falls in (the gradient's look).
const STEPS: f32 = 4.0;

// A wander from -1 to 1 in straight runs, turning at each whole `x`.
fn kinked(x: f32, seed: f32) -> f32 {
    let i = floor(x);
    return mix(hash(i + seed), hash(i + 1.0 + seed), x - i) * 2.0 - 1.0;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // (Faded where it spreads over the ground.)
    let flare = clamp((in.down - look.w) / (1.0 - look.w), 0.0, 1.0);
    let shows = 1.0 - 0.8 * pow(flare, 1.5);
    let u = (in.side * 0.5 + 0.5) * LANES;
    let lane = floor(u);
    let x = abs(fract(u) - 0.5);
    let r = hash(lane);
    let long = 0.5 + 0.9 * hash(lane + 17.0);
    // (Its head downstream, bright, its tail fading behind it.)
    let d = fract(in.flow * (0.75 + 0.5 * r) / long + hash(lane + 41.0));
    let dash = smoothstep(0.35, 0.92, d) * (1.0 - smoothstep(0.94, 1.0, d));
    let line = 1.0 - smoothstep(0.08, 0.22, x);
    let lit = select(1.0, 0.0, hash(lane + 73.0) < 0.25);
    let streak = line * dash * lit * look.x * in.on * shows * (1.0 - smoothstep(0.7, 0.95, abs(in.side)));
    if look.z < 1.5 {
        // (White-hot in a core narrow at its source, widening and cooling
        // as it goes down; its streaks faint, in that core; all of it bent,
        // as the air round it is, the bending running down with it.)
        let bend = 0.07 * sin(in.flow * 9.0 + in.side * 2.0) + 0.05 * sin(in.flow * 17.0 - in.side * 5.0 + 1.3);
        let side = in.side + bend * (0.4 + in.down);
        let core = 0.22 + 0.7 * in.down;
        let heat = exp(-3.0 * pow(side / core, 2.0)) * exp(-2.4 * in.down);
        let white = clamp(heat * (0.8 + 0.5 * streak), 0.0, 1.0) * in.on * shows;
        let bent = exp(-4.0 * pow(abs(side), 6.0)) * in.on * shows;
        let alpha = clamp(max(bent * 0.94, white), 0.0, 1.0);
        return vec4<f32>(vec3<f32>(white * 1.8), alpha);
    }
    // (The same in shards: white-hot at its source, widening and cooling as it goes down,
    // in flat steps bounded by straight lines meeting at angles: shards,
    // not curves, the user, 2026-11-10. All of it bent, as the air round
    // it is, in kinked straight runs running down with it; a hard edge.)
    let bend = 0.09 * kinked(in.flow * 2.2, 3.0) + 0.05 * kinked(in.flow * 4.7, 11.0) + 0.06 * kinked(in.flow * 3.1, 23.0) * in.side;
    let side = abs(in.side + bend * (0.4 + in.down));
    let heat = min(1.0 - 3.5 * side + 0.8 * in.down, 1.0 - 1.3 * in.down - 1.4 * side);
    let step = clamp(floor(heat * STEPS + 0.5) / STEPS, 0.0, 1.0);
    let white = clamp(step * (0.85 + 0.4 * streak), 0.0, 1.0) * in.on * shows;
    let aa = fwidth(side);
    let inside = 1.0 - smoothstep(0.86 - aa, 0.86 + aa, side);
    let alpha = clamp(max(inside * in.on * shows * 0.94, white), 0.0, 1.0);
    return vec4<f32>(vec3<f32>(white * 1.8), alpha);
}
