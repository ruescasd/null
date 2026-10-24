// The void's cores (the ground lab's `void_inlay`), where the grooves cut
// round the cylinders show them: dark and matte, lit faintly from within,
// the light gathered to a groove's middle height (evenly over it, the wall
// itself seemed to light up, not something inside it, the user); and pulses
// climbing up through each core, as the canals' run along their floors (the
// user's), each a sharp head and a tail below it, each core's at its own
// phase: seen where the grooves show it, groove after groove lighting up
// the cylinder. The standard surface otherwise: lit, shadowed and hazed as
// the stone is.
//
// (The user: the machine shown, with restraint, in what the cut exposes.)
//
// And the channels down the walkways' middles: each pulse, where a core's
// reaches a walkway's level, running along it to the next core, there
// climbing on (the void's network, its phases set so); a faint light
// between them. Told apart by the tone: a channel's below 0 (its phase -1,
// the less); a core's 2 where no pulse climbs it (below where it is fed).

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

#import bevy_pbr::mesh_view_bindings::globals

// x: the light between pulses, y: a pulse's peak, z: their speed up (m/s),
// w: how far apart (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> light: vec4<f32>;
// x: the core's own tone (dark), y: the steady light's spread about the
// groove's middle height (of its height), z: a pulse's tail (m), w: its
// head (m).
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> surface: vec4<f32>;
// The grooves: x the lowest's floor (m), y how far apart, z how high.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> grooves: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // The vertex's tone: its pulse's phase (see above).
    var tone = 0.0;
#ifdef VERTEX_COLORS
    tone = in.color.r;
#endif
    let channel = tone < 0.0;
    let phase = select(tone, -tone - 1.0, channel);
    let pulsing = tone < 2.0;
    pbr_input.material.base_color = vec4<f32>(vec3<f32>(surface.x), 1.0);
    // How far up its groove (0 its floor, 1 its ceiling): the steady light
    // gathered about the middle; a pulse's only eased off at the floor and
    // the ceiling.
    let y = in.world_position.y - grooves.x;
    let up = (y % grooves.y) / grooves.z;
    let gathered = exp(-0.5 * pow((up - 0.5) / surface.y, 2.0));
    let eased = smoothstep(0.0, 0.15, up) * smoothstep(1.0, 0.85, up);
    // The pulses: how far this is above the nearest head below it (which
    // it is ahead of), and below the nearest above (which has passed it);
    // lit at a head's front, and along the tail behind it.
    let ahead = fract((y - globals.time * light.z) / light.w + phase) * light.w;
    let behind = light.w - ahead;
    let pulse = select(exp(-behind / surface.z), 0.0, behind > light.w * 0.5) + exp(-ahead * ahead / (surface.w * surface.w));
    // (A channel's world height its level's: its pulse as far up the
    // climb as the vertex's phase puts it.)
    let core = light.x * gathered + light.y * pulse * eased * select(0.0, 1.0, pulsing);
    let glow = select(core, light.x * 0.25 + light.y * pulse, channel);
    pbr_input.material.emissive = vec4<f32>(vec3<f32>(glow), 1.0);
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
