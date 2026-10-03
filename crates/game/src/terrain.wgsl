// Terrain surface: the standard PBR pipeline, plus
// - baked sky visibility (vertex colour alpha) applied to indirect light only;
//   direct sunlight is left to the shadow maps, so a crevice in full sun is
//   still lit;
// - procedural grain: albedo mottling and a faint micro-relief, fixed in the
//   world so it streams past as you move. It tiles with the world's wrap
//   period and fades out with distance before it can shimmer.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

// x: strength of the baked occlusion (0 disables it), y: curvature 1 / 2R,
// zw: camera x and z.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> params: vec4<f32>;
// x: albedo grain strength, y: world wrap period in metres, z: relief strength.
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> grain: vec4<f32>;

fn hash(cell: vec2<i32>, period: i32) -> f32 {
    // Wrap the lattice so the pattern tiles with the world.
    let c = bitcast<vec2<u32>>(((cell % period) + period) % period);
    var h = (c.x * 0x8da6b343u) ^ (c.y * 0xd8163841u);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777216.0;
}

// Value noise in 0..1 with `cells_per_metre` lattice cells per metre.
fn value_noise(p: vec2<f32>, cells_per_metre: f32) -> f32 {
    let q = p * cells_per_metre;
    let i = vec2<i32>(floor(q));
    let f = fract(q);
    let u = f * f * (3.0 - 2.0 * f);
    let period = i32(round(grain.y * cells_per_metre));
    let a = hash(i, period);
    let b = hash(i + vec2(1, 0), period);
    let c = hash(i + vec2(0, 1), period);
    let d = hash(i + vec2(1, 1), period);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Mottling and relief height (metres) on a surface plane, with each octave
// faded out once it gets close to the pixel size.
struct Grain {
    mottle: f32,
    height: f32,
}

fn sample_grain(p: vec2<f32>, pixel: f32) -> Grain {
    // Octaves: 25 cm speckle, 1 m mottling, 4 m blotches (powers of two, so
    // every lattice fits the wrap period exactly).
    let freqs = vec3<f32>(4.0, 1.0, 0.25);
    let mottle_weights = vec3<f32>(0.35, 0.4, 0.25);
    let heights = vec3<f32>(0.004, 0.012, 0.03);
    var g = Grain(0.0, 0.0);
    for (var k = 0; k < 3; k++) {
        let cell = 1.0 / freqs[k];
        let fade = saturate(cell / (pixel * 3.0) - 1.0);
        let n = value_noise(p, freqs[k]) - 0.5;
        g.mottle += n * mottle_weights[k] * fade;
        g.height += n * heights[k] * fade;
    }
    return g;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_COLORS
    let visibility = mix(1.0, in.color.a, params.x);
    pbr_input.material.base_color = vec4<f32>(in.color.rgb, 1.0);
    pbr_input.diffuse_occlusion *= visibility;
    pbr_input.specular_occlusion *= visibility;
#endif

    // Undo the curvature so the grain stays fixed to the ground as the camera
    // moves.
    var p = in.world_position.xyz;
    let d = p.xz - params.zw;
    p.y += dot(d, d) * params.y;

    // Project onto the surface's own plane: tops use x/z, walls run along
    // their face and up.
    let n = normalize(in.world_normal);
    var u_axis = vec3<f32>(1.0, 0.0, 0.0);
    var v_axis = vec3<f32>(0.0, 0.0, 1.0);
    if abs(n.y) < 0.7 {
        u_axis = normalize(vec3<f32>(-n.z, 0.0, n.x));
        v_axis = vec3<f32>(0.0, 1.0, 0.0);
    }
    let uv = vec2<f32>(dot(p, u_axis), dot(p, v_axis));
    let pixel = max(length(fwidth(uv)), 1e-4);

    let g = sample_grain(uv, pixel);
    pbr_input.material.base_color = vec4<f32>(
        pbr_input.material.base_color.rgb * max(1.0 + grain.x * 2.0 * g.mottle, 0.0),
        1.0,
    );

    // Micro-relief: tilt the normal by the height field's slope, found with
    // two extra samples a little way along the plane.
    if grain.z > 0.0 {
        let e = 0.04;
        let gu = sample_grain(uv + vec2<f32>(e, 0.0), pixel).height - g.height;
        let gv = sample_grain(uv + vec2<f32>(0.0, e), pixel).height - g.height;
        let slope = (u_axis * gu + v_axis * gv) / e;
        pbr_input.N = normalize(pbr_input.N - slope * grain.z);
    }

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
