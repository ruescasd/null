// The bent air round the brute's beams (`haze.rs`): the screen behind it,
// as drawn before it, looked at a little aside by soft lumps running down
// it; as thick as it strains; the bending fading to nothing towards its
// outline (where it turns side on) and at its ends (at the sole and at the
// ground), as heat haze has no edge.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_functions,
    mesh_view_bindings::{view, globals, view_transmission_texture, view_transmission_sampler},
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var lumps: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var lumps_sampler: sampler;

// How far aside it bends what is behind it at its thickest (m, at a metre's
// thickness), and over how much of its turn to side-on and of its ends the
// bending fades.
const BEND: f32 = 0.12;
const OUTLINE: vec2<f32> = vec2<f32>(0.1, 0.55);
const ENDS: f32 = 0.2;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tag = mesh_functions::get_tag(in.instance_index);
    let ran = f32(tag & 0xffffu) / 65535.0;
    let strain = f32(tag >> 16u) / 1000.0;
    let lump = textureSample(lumps, lumps_sampler, in.uv + vec2<f32>(fract(globals.time * 0.05), ran)).xy * 2.0 - 1.0;
    let to_eye = normalize(view.world_position - in.world_position.xyz);
    let facing = abs(dot(normalize(in.world_normal), to_eye));
    let outline = smoothstep(OUTLINE.x, OUTLINE.y, facing);
    let ends = smoothstep(0.0, ENDS, in.uv.y) * (1.0 - smoothstep(1.0 - ENDS, 1.0, in.uv.y));
    let thick = 0.4 + 0.8 * strain;
    // (Metres aside, made a share of the screen at its distance.)
    let depth = max(-(view.view_from_world * in.world_position).z, 0.3);
    let aside = lump * BEND * thick * outline * ends * view.clip_from_view[1][1] / depth * vec2<f32>(0.5 * view.viewport.w / view.viewport.z, -0.5);
    let at = (in.position.xy - view.viewport.xy) / view.viewport.zw + aside;
    let seen = textureSampleLevel(view_transmission_texture, view_transmission_sampler, at, 0.0);
    return vec4<f32>(seen.rgb, 1.0);
}
