// The far terrain drawn from the main sun into the cached far shadow map
// (see far_shadow.rs): depth only, bent round the camera where it stood
// when the map was drawn, as terrain_prepass.wgsl bends it for the live
// shadows.

struct Far {
    clip_from_world: mat4x4<f32>,
    // xy: the camera's x and z when drawn, z: curvature (1 / 2R).
    bend: vec4<f32>,
}

@group(0) @binding(0) var<uniform> far: Far;
// Each column's translation, by instance.
@group(0) @binding(1) var<storage, read> offsets: array<vec4<f32>>;

@vertex
fn vertex(@location(0) position: vec3<f32>, @builtin(instance_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = position + offsets[i].xyz;
    let d = p.xz - far.bend.xy;
    p.y -= dot(d, d) * far.bend.z;
    return far.clip_from_world * vec4<f32>(p, 1.0);
}
