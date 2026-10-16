// The canals' beams: each a ray of light from one station to the next, a few
// hundred metres up. A ribbon turned to face the camera, bent down with
// distance as the terrain is (a drop of d^2 / 2R), so a beam runs down to
// the horizon with its canal. Drawn at least `beam_px` wide, dimming as it
// thins instead of shrinking (thinner, temporal anti-aliasing erases it);
// it glows across its width rather than ending at an edge, and sinks into
// the haze as everything does, if more slowly: it is light.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}

// x: thickness (m), y: least width drawn (px), z: curvature 1 / 2R.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> shape: vec4<f32>;
// x: brightness, y: how far the haze lets them be seen (m), z: how much is
// left of the sky (the swarm's darkness: 0 none).
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> light: vec4<f32>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // A point on the beam's centreline.
    @location(0) position: vec3<f32>,
    // Along the beam.
    @location(1) normal: vec3<f32>,
    // x: which side of the ribbon (-1, 1).
    @location(2) uv: vec2<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) side: f32,
    @location(1) kept: f32,
    @location(2) distance: f32,
}

@vertex
fn vertex(in: Vertex) -> Out {
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    var p = (world_from_local * vec4<f32>(in.position, 1.0)).xyz;
    let c = view.world_position;
    let d = p.xz - c.xz;
    p.y -= dot(d, d) * shape.z;
    let along = normalize((world_from_local * vec4<f32>(in.normal, 0.0)).xyz);
    let to_camera = c - p;
    let distance = length(to_camera);
    let side = normalize(cross(along, to_camera / distance));
    // A pixel's width at this distance; the half width drawn, and the
    // light kept for drawing it wider than it is.
    let pixel = 2.0 * distance / (view.clip_from_view[1][1] * view.viewport.w);
    let half = 0.5 * shape.x;
    let drawn = max(half, 0.5 * shape.y * pixel);
    var out: Out;
    out.position = position_world_to_clip(p + side * drawn * in.uv.x);
    out.side = in.uv.x;
    out.kept = sqrt(half / drawn);
    out.distance = distance;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // Bright at its core, soft to its edges.
    let glow = exp(-3.0 * in.side * in.side);
    let haze = exp(-3.0 * (in.distance / light.y) * (in.distance / light.y));
    return vec4<f32>(vec3<f32>(glow * in.kept * haze * light.x * light.z), 0.0);
}
