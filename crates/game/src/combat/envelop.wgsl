// Shot: you are in the beam, as in the old grid's comet (the user, flying
// into a canal's beam): the sky one flat pale sheet behind everything, and
// everything dark against it. Two layers over the whole screen, a hair
// nearer than the sky: one drawn only where the sky shows (its depth test
// passes there), adding pale; one only where something is, darkening it.

#import bevy_pbr::mesh_view_bindings::view

// x: how much (0..1), y: 1 the sky's layer, 0 the world's; z: how pale the
// sky, w: how dark the world.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> tone: vec4<f32>;

struct Vertex {
    // The screen's corners (-1..1).
    @location(0) position: vec3<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vertex(in: Vertex) -> Out {
    var out: Out;
    // (Reversed depth: the sky at 0; this a hair nearer.)
    out.position = vec4<f32>(in.position.xy, 1e-7, 1.0);
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    if tone.y > 0.5 {
        return vec4<f32>(vec3<f32>(tone.x * tone.z), 0.0);
    }
    // (Multiplied: what is there, times this.)
    return vec4<f32>(vec3<f32>(1.0 - tone.x * tone.w), 1.0);
}
