// The stalker's shot: a ray of light, a ribbon turned to face the camera,
// bright at its core and soft to its edges, at its own depth. At least
// `shape.y` pixels wide, dimmer as it thins. (Being hit by it is the comet,
// `envelop.wgsl`.)

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}

// x: width (m), y: least width drawn (px), z: dark (over a half: it
// darkens what is behind it, multiplied, instead of lighting it).
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> shape: vec4<f32>;
// x: brightness.
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> light: vec4<f32>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // A point on its centreline.
    @location(0) position: vec3<f32>,
    // Along it.
    @location(1) normal: vec3<f32>,
    // x: which side of the ribbon (-1, 1).
    @location(2) uv: vec2<f32>,
}

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) side: f32,
    @location(1) kept: f32,
}

@vertex
fn vertex(in: Vertex) -> Out {
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let p = (world_from_local * vec4<f32>(in.position, 1.0)).xyz;
    let along = normalize((world_from_local * vec4<f32>(in.normal, 0.0)).xyz);
    let to_camera = view.world_position - p;
    let distance = max(length(to_camera), 1e-3);
    let side = normalize(cross(along, to_camera / distance));
    let pixel = 2.0 * distance / (view.clip_from_view[1][1] * view.viewport.w);
    let half = 0.5 * shape.x;
    let drawn = max(half, 0.5 * shape.y * pixel);
    var out: Out;
    out.position = position_world_to_clip(p + side * drawn * in.uv.x);
    out.side = in.uv.x;
    out.kept = sqrt(half / drawn);
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let glow = exp(-4.0 * in.side * in.side);
    if shape.z > 0.5 {
        return vec4<f32>(vec3<f32>(1.0 - light.x * in.kept * glow), 1.0);
    }
    return vec4<f32>(vec3<f32>(light.x * in.kept * glow), 0.0);
}
