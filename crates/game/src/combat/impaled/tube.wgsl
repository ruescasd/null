// Tubes from their points (tube.rs): each corner of the mesh is only its
// ring (x) and its side (y); its ring is laid round its point, from the
// buffer. The same for the main pass and the prepass (depth, shadows).

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#else
#import bevy_pbr::forward_io::{Vertex, VertexOutput}
#endif

// Two a point: where it is and how thick (w), and which way its ring's
// first side faces.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<storage, read> points: array<vec4<f32>>;

const SIDES: f32 = 5.0;
const TAU: f32 = 6.28318531;

struct Placed {
    local: vec3<f32>,
    normal: vec3<f32>,
}

// A ring's corner: along its strand from the point before to the one after,
// its first side carried as it came (but kept square to its way here).
fn placed(ring: u32, side: f32) -> Placed {
    let last = arrayLength(&points) / 2u - 1u;
    let here = points[2u * ring];
    let before = points[2u * max(ring, 1u) - 2u].xyz;
    let after = points[2u * min(ring + 1u, last)].xyz;
    var along = after - before;
    along = select(vec3(0.0, 1.0, 0.0), normalize(along), dot(along, along) > 1e-12);
    var up = points[2u * ring + 1u].xyz;
    up -= along * dot(up, along);
    let other = select(vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), abs(along.x) > 0.9);
    up = select(normalize(cross(along, other)), normalize(up), dot(up, up) > 1e-12);
    let a = side / SIDES * TAU;
    let out = up * cos(a) + cross(along, up) * sin(a);
    return Placed(here.xyz + out * here.w, out);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let p = placed(u32(vertex.position.x + 0.5), vertex.position.y);
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4(p.local, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);

#ifdef PREPASS_PIPELINE
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(p.normal, vertex.instance_index);
#endif
#ifdef MOTION_VECTOR_PREPASS
    // (Where its points were is not kept: only its whole moving.)
    let previous = mesh_functions::get_previous_world_from_local(vertex.instance_index);
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(previous, vec4(p.local, 1.0));
#endif
#else
    out.world_normal = mesh_functions::mesh_normal_local_to_world(p.normal, vertex.instance_index);
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
