// A tumbler's damage (scars.rs; the user, 2026-11-13: damage shown in its
// glow, cracks accumulating): cracks spread from where shards struck it, in
// its part's own units (so they stay where they are as it moves). Its
// blades are dark and the light inside it shows through their cracks: faint
// at rest, bright as it winds up to fire and when struck. Its body glows
// whole as it winds up, and there its cracks split the glow, dark.
//
// Its parts are flat (their faces square to z), so a crack reaches across
// that plane (struck through, on both its faces; where a shard struck is
// on its hit shape, off its surface).
//
// A crack is struck cracks, as in glass: jagged rays out from where it
// struck, each reaching its own way out, thinning to its end, and further
// out some branching off between them. (Cell edges were tried first: a
// mesh of closed rings, not cracks.)

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_functions,
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

// x: the light inside it now (emissive), y: its cracks' at rest, z: 1 if
// its whole surface glows with it (its body), w: how many cracks.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> light: vec4<f32>;
// x: how finely a crack zigzags (its units), y: a crack's width (its
// units), z: how dark its cracks split its body's glow (0..1), w: how much
// of the light inside it leaks through its blades' cracks.
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> look: vec4<f32>;
// Each crack: where it struck (its part's own units), how far it has spread.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> cracks: array<vec4<f32>, 16>;

fn hash3(p: vec3<f32>) -> vec3<f32> {
    var q = vec3<u32>(vec3<i32>(floor(p)) + vec3<i32>(4096));
    q = q * vec3(1597334673u, 3812015801u, 2798796415u);
    q = (q.x ^ q.y ^ q.z) * vec3(1597334673u, 3812015801u, 2798796415u);
    return vec3<f32>(q) / 4294967295.0;
}

fn hash1(p: vec3<f32>) -> f32 {
    return hash3(p).x;
}

// Smooth value noise (0..1).
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash1(i), hash1(i + vec3(1.0, 0.0, 0.0)), u.x), mix(hash1(i + vec3(0.0, 1.0, 0.0)), hash1(i + vec3(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash1(i + vec3(0.0, 0.0, 1.0)), hash1(i + vec3(1.0, 0.0, 1.0)), u.x), mix(hash1(i + vec3(0.0, 1.0, 1.0)), hash1(i + vec3(1.0, 1.0, 1.0)), u.x), u.y),
        u.z);
}

const TAU: f32 = 6.2831853;
// Its rays, and its branches further out (as many, between them).
const RAYS: f32 = 7.0;

// How many of its widths `p` is from a crack's nearest ray or branch (big:
// none near). `c`: where it struck and how far it has spread; `i`: which.
fn rays(p: vec2<f32>, c: vec4<f32>, i: f32) -> f32 {
    let d = p - c.xy;
    let r = length(d);
    if r >= c.w || c.w <= 0.0 {
        return 1e4;
    }
    let seed = vec3(i * 17.0, floor(c.x * 911.0), floor(c.y * 613.0));
    let a = atan2(d.y, d.x);
    var best = 1e4;
    for (var level = 0; level < 2; level++) {
        let shift = hash1(seed + vec3(f32(level) * 31.0, 0.0, 0.0));
        let s = a / TAU * RAYS + shift * RAYS;
        let k = floor(s);
        // (Each ray zigzagging its own way: pushed aside so far, not turned
        // so far, so it is as jagged near where it struck as at its end;
        // all alike, they swirled as a pinwheel.)
        let z = vec3(r / look.x, k * 5.3 + i * 3.7, f32(level) * 11.0);
        let aside = (noise(z) - 0.5) * look.x * 1.2 + (noise(z * vec3(3.1, 1.0, 1.0) + 50.0) - 0.5) * look.x * 0.4;
        let off = abs((fract(s) - 0.5) * TAU / RAYS * r - aside);
        let h = hash3(seed + vec3(k, f32(level) * 7.0 + 3.0, 0.0));
        // (A ray from where it struck, out so far; a branch, half of
        // them, from partway out.)
        let start = select(0.0, c.w * (0.25 + 0.3 * h.y), level == 1);
        let end = select(c.w * (0.45 + 0.55 * h.x), c.w * (0.55 + 0.45 * h.x), level == 1);
        let there = level == 0 || h.z > 0.5;
        if there && r >= start && r < end {
            let thin = select(1.0, 0.6, level == 1) * (0.25 + 0.75 * sqrt(1.0 - (r - start) / (end - start)));
            best = min(best, off / (look.y * thin));
        }
    }
    return best;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = (mesh_functions::get_local_from_world(in.instance_index) * vec4(in.world_position.xyz, 1.0)).xyz;
    var q = 1e4;
    let n = u32(light.w);
    for (var i = 0u; i < n; i++) {
        q = min(q, rays(p.xy, cracks[i], f32(i)));
    }
    let aa = max(fwidth(q), 0.001);
    let line = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, q);
    // (Its body's glow split wider than its cracks: thin dark lines are
    // lost in its glow's bloom.)
    let split = 1.0 - smoothstep(2.0, 6.0 + aa, q);
    let inner = light.x;
    let leak = max(inner * look.w, light.y) * line;
    // (Its body's cracks glowing faintly at rest, as its blades', gone as
    // its body lights: left on, they were white on white, unseen.)
    let resting = light.y * line * clamp(1.0 - inner / (2.0 * light.y + 0.001), 0.0, 1.0);
    let glow = select(leak, inner * (1.0 - look.z * split) + resting, light.z > 0.5);
    // (Its alpha 0: not scaled by the exposure, as the standard material.)
    pbr_input.material.emissive = vec4<f32>(vec3<f32>(glow), 0.0);
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
