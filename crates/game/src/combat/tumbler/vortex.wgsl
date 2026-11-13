// A tumbler's rhombus (tumbler.rs): a vortex on it (the user, 2026-11-13,
// from a picture of a tunnel of light streaks: grayscale), its streaks
// rushing out from a black core at its leading point, light gathering
// toward its edges. Its corners' uv are its face's own (-1..1 across and
// up), so the vortex turns with it as it spins.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::globals,
}

// x: how bright at most; y: inverted (over a half: a bright field, dark
// streaks rushing out of a white core; the user, 2026-11-13).
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> look: vec4<f32>;

const TAU: f32 = 6.28318531;

fn hash(n: f32) -> f32 {
    return fract(sin(n * 127.1) * 43758.5453);
}

// A layer of `rays` streaks round it: each its own speed and place, a
// short bright dash rushing outward along it, thin across it.
fn streaks(r: f32, a: f32, rays: f32, t: f32, seed: f32) -> f32 {
    let cell = a / TAU * rays;
    let i = floor(cell);
    let across = abs(fract(cell) - 0.5);
    let k = hash(i + seed);
    let speed = 0.6 + 1.4 * hash(i + seed + 17.0);
    let f = fract(r * (1.2 + k) - t * speed + k * 7.0);
    let dash = smoothstep(0.0, 0.08, f) * (1.0 - smoothstep(0.1, 0.55, f));
    // (Thick enough to read: thinner, the bloom of its bright field washed
    // them out, the user, 2026-11-13: not prominent enough.)
    let thin = exp(-across * across * (25.0 + 40.0 * k));
    return dash * thin * (0.4 + 0.6 * hash(i + seed + 3.0));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let r = length(uv);
    let a = atan2(uv.y, uv.x) + TAU * 0.5;
    let t = globals.time;
    var v = streaks(r, a, 48.0, t, 0.0) + 0.7 * streaks(r, a, 90.0, t * 1.3, 51.0);
    // (A clear core, the streaks rushing out from close by it, stronger
    // outward, a faint glow between.)
    let core = smoothstep(0.05, 0.28, r);
    v = (v * (0.6 + 1.2 * r) + 0.15 * r) * core;
    // (Inverted: a field just under white, its streaks dark; at the light's
    // brightness, all but the darkest streaks were white past white, and
    // it looked plain white, the user.)
    // (Just under white: over it, its bloom spilled across the streaks.)
    if look.y > 0.5 {
        v = 0.158 * (1.0 - clamp(v * 5.0, 0.0, 1.0));
    }
    return vec4<f32>(vec3<f32>(v * look.x), 1.0);
}
