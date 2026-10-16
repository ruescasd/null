// The light in a canal's grooves while it is on: a steady glow, and pulses
// running along it the way it flows. Bent down with distance as the terrain
// is (a drop of d^2 / 2R), so it stays in its grooves; fading out with
// distance before it thins to shimmer (the beam carries the canal further).

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
}

// x: curvature 1 / 2R, y: how far it is seen (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> shape: vec4<f32>;
// x: steady glow, y: a pulse's peak, z: pulses' speed (m/s), w: how far
// apart they are (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> light: vec4<f32>;
// x: a pulse's length (m), y: how much is left of the sky (the swarm's
// darkness: 0 none).
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> extra: vec4<f32>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // x: across the strip (-1, 1), y: how far along the flow (m).
    @location(2) uv: vec2<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) distance: f32,
}

@vertex
fn vertex(in: Vertex) -> Out {
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    var p = (world_from_local * vec4<f32>(in.position, 1.0)).xyz;
    let c = view.world_position;
    let d = p.xz - c.xz;
    p.y -= dot(d, d) * shape.x;
    var out: Out;
    out.position = position_world_to_clip(p);
    out.uv = in.uv;
    out.distance = length(p - c);
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // Brightest along its middle.
    let core = exp(-2.0 * in.uv.x * in.uv.x);
    // Pulses: each a sharp head and a tail behind it, running along.
    let phase = fract((in.uv.y - globals.time * light.z) / light.w) * light.w;
    let behind = light.w - phase;
    let pulse = select(exp(-behind / extra.x), 0.0, behind > light.w * 0.5) + exp(-phase * phase / (0.5 * 0.5));
    let fade = 1.0 - smoothstep(0.6 * shape.y, shape.y, in.distance);
    let value = core * (light.x + light.y * pulse) * fade * extra.y;
    return vec4<f32>(vec3<f32>(value), 0.0);
}
