// SPIKE (the user, 2026-11-17, `--opt rt`): the suns' shadows on the
// terrain traced, not sampled from maps. Each pixel's point, from this
// frame's depth, one ray towards each sun against the terrain drawn (see
// rt_shadow.rs): lit (1) or not (0), in `mask`'s red and green.

enable wgpu_ray_query;

#import bevy_render::view::View

struct Params {
    // Towards each sun (xyz), whether it is up (w).
    suns: array<vec4<f32>, 2>,
    // x: the bend round the camera (1 / 2R; see terrain_vertex.wgsl).
    bend: vec4<f32>,
    // DEBUG: the pixel probed (xy), on (z 1).
    probe: vec4<f32>,
    // Where the traced scene's origin is in the world (xyz): every column
    // placed relative to it, near the camera, for precision (see
    // rt_shadow.rs, `SHIFT`).
    shift: vec4<f32>,
}

@group(0) @binding(0) var tlas: acceleration_structure;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var mask: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> view: View;
@group(0) @binding(4) var<uniform> params: Params;
// DEBUG: the probed pixel's results (see rt_shadow.rs, `read_probe`).
@group(0) @binding(5) var<storage, read_write> probe: array<vec4<f32>, 8>;

// How far behind its surface a point may be (m) besides: the sag of the
// largest triangle drawn, 400 m across, under the bend (1 / 2R x L^2 / 4 =
// 0.5 m; see `trace`).
const REACH: f32 = 0.5;
// How far off its surface a ray starts (m), and more for each metre from
// the eye: a few of single precision's steps a few kilometres from the
// scene's origin (see `Params::shift`), and of the depth's own.
const LIFT: f32 = 0.002;
const LIFT_PER_M: f32 = 0.000002;
// How far behind its surface a point rebuilt from the depth may be, for
// each metre from the eye: the coarsest level's depth bias (see `trace`)
// is about 0.0003 of the distance.
const BEHIND_PER_M: f32 = 0.001;
// How far below the scene's surface a coarser copy fading may be drawn (m):
// the far level's lowering (8.1 m, `sink`) and its planes' tolerance (6.4
// m), to spare.
const DROP: f32 = 16.0;

// Lit (1) or not, and (DEBUG) how far the hit is.
fn lit(origin: vec3<f32>, towards: vec3<f32>) -> vec2<f32> {
    var rq: ray_query;
    rayQueryInitialize(&rq, tlas, RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT, 0xFFu, 0.0, 20000.0, origin, towards));
    rayQueryProceed(&rq);
    let hit = rayQueryGetCommittedIntersection(&rq);
    if hit.kind == RAY_QUERY_INTERSECTION_NONE {
        return vec2<f32>(1.0, 1.0);
    }
    return vec2<f32>(0.0, hit.t);
}

// A pixel's point, unbent, in the traced scene's frame (see `trace`); its
// depth 0 for the sky.
fn point_at(pixel: vec2<i32>, eye: vec3<f32>) -> vec4<f32> {
    let dims = vec2<i32>(textureDimensions(depth));
    let q = clamp(pixel, vec2<i32>(0), dims - 1);
    let d = textureLoad(depth, q, 0);
    let uv = (vec2<f32>(q) + 0.5) / vec2<f32>(dims);
    let v = view.view_from_clip * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let offset = (view.world_from_view * vec4<f32>(v.xyz / v.w, 0.0)).xyz;
    var p = eye + offset;
    p.y += dot(offset.xz, offset.xz) * params.bend.x;
    return vec4<f32>(p, d);
}

@compute @workgroup_size(8, 8, 1)
fn trace(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = min(textureDimensions(mask), textureDimensions(depth));
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let pixel = vec2<i32>(id.xy);
    let d = textureLoad(depth, pixel, 0);
    if d <= 0.0 {
        // (Nothing drawn: the sky.)
        textureStore(mask, pixel, vec4<f32>(1.0, 1.0, 0.0, 1.0));
        return;
    }
    // In the traced scene's frame, near the camera (see `Params::shift`),
    // relative to the camera first (through one matrix with the camera's
    // place in it, 16 km out, points came out decimetres off); unbent (the
    // terrain is drawn bent round the camera; traced flat).
    let eye = view.world_position - params.shift.xyz;
    let p = point_at(pixel, eye).xyz;
    let far = distance(p, eye);
    // Onto the surface the pixel shows. The point rebuilt from the depth is
    // behind it, never in front: the bend is applied at triangles' corners
    // and each drawn flat between them, under the true curve (its own
    // triangle's sag, c L^2 / 4: centimetres; round blobs, the spike's), and
    // the coarser levels' depth is biased back so a finer level always
    // wins (`LOD_DEPTH_BIAS`: a few parts in ten thousand of the distance,
    // 0.7 m at 2.3 km; the far rim's wall dark), and while a column fades
    // into another, the coarser of the two, lowered under the finer (see
    // terrain.rs, `sink`), shows through every other pixel, metres below
    // the scene's surface there (squares blinking on the ground, spikes on
    // the slanted walls, the user). Nothing drawn is between the pixel's
    // surface and the eye (it is what the pixel shows), so back towards
    // the eye, the first surface of the scene met is that place's, at the
    // level traced: reached at any angle, down to `DROP` below it. (Searched
    // along a facing from the depth, the bias was out of reach; within a
    // metre or so, the lowering.)
    let to_eye = normalize(eye - p);
    let lift = LIFT + far * LIFT_PER_M;
    var origin = p + to_eye * lift;
    var rq: ray_query;
    let reach = min(far, REACH + far * BEHIND_PER_M + DROP / max(abs(to_eye.y), 0.01));
    rayQueryInitialize(&rq, tlas, RayDesc(RAY_FLAG_NONE, 0xFFu, 0.0, reach, p, to_eye));
    rayQueryProceed(&rq);
    let surface = rayQueryGetCommittedIntersection(&rq);
    if surface.kind != RAY_QUERY_INTERSECTION_NONE {
        origin = p + to_eye * (surface.t + lift);
    }
    if params.probe.z > 0.5 && all(id.xy == vec2<u32>(params.probe.xy)) {
        var rq: ray_query;
        rayQueryInitialize(&rq, tlas, RayDesc(RAY_FLAG_NONE, 0xFFu, 0.0, 20000.0, origin, params.suns[0].xyz));
        while rayQueryProceed(&rq) {}
        let hit = rayQueryGetCommittedIntersection(&rq);
        probe[0] = vec4<f32>(d, f32(size.x), f32(size.y), 0.0);
        probe[1] = vec4<f32>(p, far);
        probe[2] = vec4<f32>(origin, surface.t);
        probe[3] = vec4<f32>(hit.t, f32(hit.instance_index), f32(hit.primitive_index), f32(hit.kind));
        probe[4] = vec4<f32>(to_eye, select(0.0, 1.0, hit.front_face));
        probe[5] = vec4<f32>(origin + params.suns[0].xyz * hit.t, 0.0);
        probe[6] = vec4<f32>(hit.object_to_world[3], f32(hit.instance_custom_data));
        probe[7] = vec4<f32>(lit(origin, params.suns[0].xyz), lit(origin, params.suns[1].xyz));
    }
    var out = vec4<f32>(1.0, 1.0, 0.0, 1.0);
    for (var k = 0; k < 2; k++) {
        if params.suns[k].w > 0.5 {
            let r = lit(origin, params.suns[k].xyz);
            out[k] = r.x;
            if k == 0 && r.x < 0.5 {
                // (DEBUG: how far the main sun's ray went before a hit,
                // 0 to 1 over 0 to 100 m, in blue.)
                out.b = clamp(r.y / 100.0, 0.0, 1.0);
            }
        }
    }
    textureStore(mask, pixel, out);
}
