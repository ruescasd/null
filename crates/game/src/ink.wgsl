// Ink lines from the prepass: silhouettes where depth jumps, creases where
// the surface turns. See ink.rs.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var normals: texture_2d<f32>;

struct Ink {
    // strength, width (px), depth sensitivity, crease sensitivity
    a: vec4<f32>,
    // near plane, fade distance, ink tone, unused
    b: vec4<f32>,
}
@group(0) @binding(3) var<uniform> ink: Ink;

// Distance from the camera along its axis (reverse-Z, infinite far plane):
// 0 for the sky.
fn view_depth(p: vec2<i32>) -> f32 {
    let size = vec2<i32>(textureDimensions(depth));
    let raw = textureLoad(depth, clamp(p, vec2<i32>(0), size - 1), 0);
    if raw <= 0.0 {
        return 1e7;
    }
    return ink.b.x / raw;
}

fn normal_at(p: vec2<i32>) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(normals));
    let n = textureLoad(normals, clamp(p, vec2<i32>(0), size - 1), 0).xyz * 2.0 - 1.0;
    return normalize(n + vec3<f32>(1e-5));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.position.xy);
    let color = textureLoad(screen, p, 0);
    if ink.a.x <= 0.0 {
        return color;
    }
    let w = max(i32(round(ink.a.y)), 1);
    let d = view_depth(p);
    let n = normal_at(p);
    let offsets = array<vec2<i32>, 4>(vec2(w, 0), vec2(-w, 0), vec2(0, w), vec2(0, -w));
    var depths: array<f32, 4>;
    var edge = 0.0;
    for (var k = 0; k < 4; k++) {
        depths[k] = view_depth(p + offsets[k]);
        // A crease: the surface turns.
        let m = normal_at(p + offsets[k]);
        let turn = 1.0 - dot(n, m);
        edge = max(edge, smoothstep(0.15, 0.35, turn * ink.a.w));
    }
    // A silhouette: depth is not a plane across the pixel (the second
    // difference, relative to the depth, so slopes and distance do not draw).
    let near = min(d, min(min(depths[0], depths[1]), min(depths[2], depths[3])));
    let bend = max(abs(depths[0] + depths[1] - 2.0 * d), abs(depths[2] + depths[3] - 2.0 * d)) / max(near, 0.1);
    edge = max(edge, smoothstep(0.04, 0.12, bend * ink.a.z));
    // Nothing on the sky; fading with distance into the haze.
    let fade = 1.0 - smoothstep(ink.b.y * 0.3, ink.b.y, near);
    let k = edge * ink.a.x * fade * select(1.0, 0.0, near > 1e6);
    let tone = vec3<f32>(ink.b.z);
    return vec4<f32>(mix(color.rgb, tone, k), color.a);
}
