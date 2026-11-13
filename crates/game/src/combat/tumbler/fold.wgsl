// A tumbler travelling as its shadow (tumbler.rs, `move=6`): each point of
// its body moved, as far as it has collapsed, along the light's ray to where
// the ray first meets the ground under its way (the ground's heights along
// its way, from the buffer): up a step in its way, over a ledge and down.
// Its way may curve (the user, 2026-11-13: endpoints not joined by a
// straight shadow, by stretching, not swimming): the light's rays bend
// with it, so each point keeps how far it is to the side of its way and
// lands on the curve as far along it as it would on a straight one (its
// shadow laid along the curve, a ribbon). The same for the main pass and
// the prepass (depth).
//
// The buffer: [0] where its way starts (xyz) and how far it has collapsed
// (w, 0..1); [1] the way it sets off (x, z, level), the light's ray along
// it and up (cos, -sin of its height); [2] its heights' first distance
// along its way, their spacing, how many, and how far off the ground it is
// laid; [3..] its heights, four a vector; then, one a height, its way's
// point there (x, z) and the way to its side (x, z).

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#else
#import bevy_pbr::forward_io::{Vertex, VertexOutput}
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<storage, read> fold: array<vec4<f32>>;

// How far along its ray a point is looked for at most (m), in steps of so
// much (m), then found closer (halvings).
const STEPS: i32 = 400;
const STEP: f32 = 0.08;
const HALVINGS: i32 = 8;

fn sample(i: u32) -> f32 {
    let v = fold[3u + i / 4u];
    return v[i % 4u];
}

// The ground's height `s` along its way (its heights between, the first and
// last beyond them).
fn height(s: f32) -> f32 {
    let p = fold[2];
    let last = p.z - 1.0;
    let f = clamp((s - p.x) / p.y, 0.0, last);
    let i = u32(floor(f));
    let j = min(i + 1u, u32(last));
    return mix(sample(i), sample(j), f - f32(i));
}

// Its way's point `s` along it (x, z) and the way to its side (x, z).
fn curve(s: f32) -> vec4<f32> {
    let p = fold[2];
    let last = p.z - 1.0;
    let f = clamp((s - p.x) / p.y, 0.0, last);
    let i = u32(floor(f));
    let j = min(i + 1u, u32(last));
    let base = 3u + (u32(p.z) + 3u) / 4u;
    let c = mix(fold[base + i], fold[base + j], f - f32(i));
    return vec4(c.xy, normalize(c.zw));
}

// A point (in the world) moved as far as it has collapsed along its ray.
fn folded(world: vec3<f32>) -> vec3<f32> {
    let start = fold[0];
    let k = start.w;
    if k <= 0.0 {
        return world;
    }
    let w = fold[1];
    let way = vec3(w.x, 0.0, w.y);
    let ray = vec2(w.z, w.w);
    let origin = vec2(dot(world - start.xyz, way), world.y);
    let aside = dot(world - start.xyz, vec3(way.z, 0.0, -way.x));
    // Along its ray to the ground, then nearer.
    var near = 0.0;
    var far = 0.0;
    var found = false;
    for (var n = 0; n < STEPS; n++) {
        let q = origin + ray * far;
        if q.y <= height(q.x) {
            found = true;
            break;
        }
        near = far;
        far += STEP;
    }
    if !found {
        return world;
    }
    for (var n = 0; n < HALVINGS; n++) {
        let mid = 0.5 * (near + far);
        let q = origin + ray * mid;
        if q.y <= height(q.x) {
            far = mid;
        } else {
            near = mid;
        }
    }
    // (Laid a little back along its ray, off the ground or a wall; on its
    // way's curve there, as far to its side as it was.)
    let q = origin + ray * max(far - fold[2].w, 0.0);
    let c = curve(q.x);
    let to = vec3(c.x + c.z * aside, q.y, c.y + c.w * aside);
    return mix(world, to, k);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let placed = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(vertex.position, 1.0));
    out.world_position = vec4(folded(placed.xyz), 1.0);
    out.position = position_world_to_clip(out.world_position.xyz);

#ifdef PREPASS_PIPELINE
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.previous_world_position = out.world_position;
#endif
#else
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}
