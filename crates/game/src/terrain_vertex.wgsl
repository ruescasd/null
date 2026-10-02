// Main-pass vertex shader for terrain: Bevy's standard mesh transform, plus a
// drop of d^2 / 2R with horizontal distance d from the camera, which makes the
// flat world read as a planet of radius R (and gives it a real horizon).

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

// y: curvature 1 / 2R, zw: camera x and z.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> params: vec4<f32>;

fn curve(p: vec4<f32>) -> vec4<f32> {
    let d = p.xz - params.zw;
    return vec4<f32>(p.x, p.y - dot(d, d) * params.y, p.z, p.w);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif

    out.world_position = curve(
        mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0))
    );
    out.position = position_world_to_clip(out.world_position.xyz);

#ifdef VERTEX_COLORS
    out.color = vertex.color;
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
