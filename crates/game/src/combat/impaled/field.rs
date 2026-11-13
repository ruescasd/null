//! What holds the brute up, seen and heard (step 3; the user, 2026-11-09:
//! "make the propulsion apparent but not from where", following the
//! unexplained theme): two thick beams from under its soles to the ground
//! (`plume.rs`), along the way its thrust pushes, white-hot where they start
//! a little below its soles and fading to black outwards and downwards (the
//! user's picture of a plume), in curves or in hard straight-edged shards
//! (`plume=1|2`), bent all along, bent air round them; flaring where they
//! hit the ground. They follow its thrust (straining as it climbs or
//! catches itself, gone in a stutter). And a hum (a placeholder), its pitch
//! and loudness its thrust's.
//!
//! The way here (the user's calls, 2026-11-09 and 10): a haze of bent air
//! under it, a glow with grit drawn up, rings pressed into the ground (the
//! first three, removed: the user chose the beams); fractal tendrils
//! hanging from it (fixed, they read as tree roots, not a flux of energy);
//! dark lightning and a dark Voronoi cage ("looks cool, but doesn't
//! work"); dark beams, fuzzy sticks; dark pulses down them, a stack of
//! tyres; dark with white streaks (removed with the first three). What
//! they taught: thrust for something this heavy must be thick (thin lines
//! read as no force at all), simple, moving, and stand out from its black
//! body.
//!
//! Nothing of it is made or changed again each frame (the user,
//! 2026-11-12: a brute cost a fifth of the frame, most of it the renderer
//! taking in meshes and materials changed each frame): its beams and air are
//! meshes made once, placed by their transforms, what changes in them
//! carried in their `MeshTag`s to their shaders (`plume.wgsl`, `haze.wgsl`).

use bevy::audio::{PlaybackMode, SpatialAudioSink, Volume};
use bevy::math::primitives::Annulus;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use worldgen::noise::hash01;

/// The beams: where they come from in its own frame (m, at its height: its
/// soles); how wide at its feet and at the ground (m), before they flare;
/// from how far down they flare (of the way) and how much wider they are
/// at the ground for it (spreading as thrust does where it hits); how fast
/// their streaks run at a hover and how much faster as it strains (m/s);
/// how bright the streaks; in how many pieces (each its own width; in few,
/// the edges were saw-toothed).
///
/// (Soft-edged, in the stalker's dark beam, they read as fuzzy sticks, the
/// user, 2026-11-09; dark pulses down them as a stack of tyres, and they
/// did not stand out from its black body, 2026-11-10: hence a hard edge,
/// white streaks, the flare.)
// (Its feet's middles as `tools/impaled_mesh.py` measures them, at its
// height: the right foot stands back.)
const SOLES: [Vec3; 2] = [Vec3::new(-0.44, -1.25, 0.04), Vec3::new(0.43, -1.25, -0.18)];
const BEAM_WIDE: (f32, f32) = (1.4, 0.9);
const FLARE: (f32, f32) = (0.6, 2.8);
const FLOW_SPEED: (f32, f32) = (6.0, 6.0);
const WHITE: f32 = 1.0;
const BEAM_PIECES: usize = 64;
/// Where how far its streaks have run wraps (m: its tag holds 20 bits of
/// millimetres).
const RAN_WRAP: f32 = 1048.575;
/// How far below its soles its beams start (m); how far down
/// they grow from narrow to their full width (m); how wide the bent air
/// round them, from the sole to the ground, across at its foot (m; the
/// column is half as wide at the sole). (Half a metre's gap and the air
/// only in it, the two fought each other instead of being one source; the
/// air only round its start, the user wanted the whole beam bent; at
/// 0.15 m, a bit lower, 2026-11-10.)
const AIR_GAP: f32 = 0.35;
const BEAM_GROW: f32 = 0.3;
const AIR_WIDE: f32 = 1.8;

/// Something of a brute's field: whose, what, and how far what runs down
/// it has run (a beam's streaks, m; its air's lumps, of their tile).
#[derive(Component)]
pub(in crate::combat) struct Part {
    pub brute: Entity,
    kind: Kind,
    ran: f32,
}

/// A beam or its air, under which sole; its hum.
#[derive(Clone, Copy)]
pub(in crate::combat) enum Kind {
    Beam(usize),
    Air(usize),
    Hum,
}

/// The meshes, noise and sounds its field and its crash share, made once.
#[derive(Resource)]
pub(in crate::combat) struct Kit {
    /// The bent air's cone, without its ends (seen from inside, a
    /// frustum's ends cut a band across the beams).
    open: Handle<Mesh>,
    /// A beam, one metre down its local -y, to be shaped in `plume.wgsl`.
    beam: Handle<Mesh>,
    /// A ring one metre across, lying flat (its crash's, `attack.rs`).
    pub(in crate::combat) ring: Handle<Mesh>,
    shimmer: Handle<Image>,
    hum: Handle<AudioSource>,
    /// A brute landing, dead (a placeholder: the stalker's impact).
    pub(in crate::combat) thud: Handle<AudioSource>,
}

pub(in crate::combat) fn setup(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>) {
    let ring = Mesh::from(Annulus::new(0.42, 0.5)).rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    commands.insert_resource(Kit {
        open: meshes.add(open_cone(0.55, 1.1)),
        beam: meshes.add(beam()),
        ring: meshes.add(ring),
        shimmer: images.add(shimmer()),
        hum: server.load("embedded://game/sounds/impaled_hum.wav"),
        thud: server.load("embedded://game/sounds/stalker_impact.wav"),
    });
}

/// A cone's side one metre high, `top` and `bottom` its radii, its middle
/// at 0, no ends.
fn open_cone(top: f32, bottom: f32) -> Mesh {
    const ROUND: u32 = 32;
    let (mut pos, mut normal, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let slope = bottom - top;
    for i in 0..=ROUND {
        let a = i as f32 / ROUND as f32 * std::f32::consts::TAU;
        let (sin, cos) = a.sin_cos();
        let n = Vec3::new(cos, slope, sin).normalize();
        for (y, r, v) in [(0.5, top, 0.0), (-0.5, bottom, 1.0)] {
            pos.push([cos * r, y, sin * r]);
            normal.push(n.to_array());
            uv.push([i as f32 / ROUND as f32, v]);
        }
        if i < ROUND {
            let k = i * 2;
            idx.extend([k, k + 2, k + 1, k + 1, k + 2, k + 3]);
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(bevy::mesh::Indices::U32(idx))
        .with_generated_tangents()
        .expect("a cone has uvs and normals")
}

/// A beam's centreline, (0, -t, 0) for t from its source (0) to the ground
/// (1), in `BEAM_PIECES` pieces, each across both its sides (uv: side, t).
fn beam() -> Mesh {
    let (mut pos, mut normal, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for j in 0..=BEAM_PIECES {
        let t = j as f32 / BEAM_PIECES as f32;
        let i = pos.len() as u32;
        for side in [-1.0, 1.0] {
            pos.push([0.0, -t, 0.0]);
            normal.push([0.0, -1.0, 0.0]);
            uv.push([side, t]);
        }
        if j < BEAM_PIECES {
            idx.extend([i, i + 1, i + 3, i, i + 3, i + 2]);
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normal)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(bevy::mesh::Indices::U32(idx))
}

/// A tiling normal map of soft lumps, to bend what is seen through the air.
fn shimmer() -> Image {
    const N: usize = 128;
    let h = |x: i32, y: i32| {
        // (Smooth value noise, tiling at N, two octaves.)
        let o = |s: i32, k: i32| {
            let (fx, fy) = (x as f32 / s as f32, y as f32 / s as f32);
            let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            let c = N as i32 / s;
            let v = |i: i32, j: i32| hash01(i.rem_euclid(c), j.rem_euclid(c), k, 0x5f1);
            let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
            let a = v(x0, y0) + (v(x0 + 1, y0) - v(x0, y0)) * sx;
            let b = v(x0, y0 + 1) + (v(x0 + 1, y0 + 1) - v(x0, y0 + 1)) * sx;
            a + (b - a) * sy
        };
        o(32, 1) + 0.5 * o(16, 2)
    };
    let mut data = Vec::with_capacity(N * N * 4);
    for y in 0..N as i32 {
        for x in 0..N as i32 {
            let (dx, dy) = (h(x + 1, y) - h(x - 1, y), h(x, y + 1) - h(x, y - 1));
            let n = Vec3::new(-dx * 6.0, -dy * 6.0, 1.0).normalize();
            data.extend([((n.x * 0.5 + 0.5) * 255.0) as u8, ((n.y * 0.5 + 0.5) * 255.0) as u8, ((n.z * 0.5 + 0.5) * 255.0) as u8, 255]);
        }
    }
    let mut image = Image::new(Extent3d { width: N as u32, height: N as u32, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, default());
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor { address_mode_u: bevy::image::ImageAddressMode::Repeat, address_mode_v: bevy::image::ImageAddressMode::Repeat, ..bevy::image::ImageSamplerDescriptor::linear() });
    image
}

/// A brute's field made: its beams (in shards or curves), the bent air
/// round them, and the hum. (Not parented: they keep to the ground, and the
/// brute's tilt must not tilt them.)
#[allow(clippy::too_many_arguments)]
pub(in crate::combat) fn make(commands: &mut Commands, kit: &Kit, hazes: &mut Assets<super::haze::Haze>, plumes: &mut Assets<super::plume::Plume>, brute: Entity, shards: bool, sound: bool) {
    let plume = plumes.add(super::plume::Plume::new(if shards { super::plume::Look::Shards } else { super::plume::Look::Curves }, WHITE, BEAM_WIDE, FLARE, BEAM_GROW));
    let air = hazes.add(super::haze::Haze { lumps: kit.shimmer.clone() });
    for k in 0..SOLES.len() {
        commands.spawn((
            Part { brute, kind: Kind::Beam(k), ran: k as f32 * 3.7 },
            Mesh3d(kit.beam.clone()),
            MeshMaterial3d(plume.clone()),
            bevy::mesh::MeshTag(0),
            bevy::light::NotShadowCaster,
            bevy::light::NotShadowReceiver,
            bevy::camera::visibility::NoFrustumCulling,
            Transform::default(),
            Visibility::Hidden,
        ));
        commands.spawn((Part { brute, kind: Kind::Air(k), ran: 0.0 }, Mesh3d(kit.open.clone()), MeshMaterial3d(air.clone()), bevy::mesh::MeshTag(0), bevy::light::NotShadowCaster, Transform::default(), Visibility::Hidden));
    }
    if sound {
        commands.spawn((Part { brute, kind: Kind::Hum, ran: 0.0 }, AudioPlayer::new(kit.hum.clone()), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), spatial: true, ..default() }, Transform::default()));
    }
}

/// A brute as its field sees it: where its feet are, how hard it pushes
/// (its thrust over its weight).
pub(in crate::combat) struct Push {
    pub feet: Vec3,
    pub strain: f32,
    /// Where it is and how it is turned (its down is the way it pushes).
    pub at: Vec3,
    pub rot: Quat,
}

/// Each field shown as its brute is now (its bent air, its hum; its beams
/// in `beams`): `pushes` gives each brute's push and the ground under its
/// feet. Parts of brutes gone, gone.
#[allow(clippy::too_many_arguments)]
pub(in crate::combat) fn show(
    mut commands: Commands,
    time: Res<Time>,
    global: Res<bevy::audio::GlobalVolume>,
    pushes: Res<Pushes>,
    mut parts: Query<(Entity, &mut Part, &mut Transform, &mut Visibility, Option<&mut bevy::mesh::MeshTag>, Option<&mut SpatialAudioSink>)>,
) {
    let dt = time.delta_secs();
    for (entity, mut part, mut t, mut visible, tag, sink) in &mut parts {
        let Some(&(ref push, ground)) = pushes.0.get(&part.brute) else {
            commands.entity(entity).despawn();
            continue;
        };
        let s = push.strain.clamp(0.0, 1.6);
        match part.kind {
            Kind::Air(k) => {
                // From the sole to the ground along its down, round all its
                // beam, narrow at the sole (the column's top), the air in it
                // running down.
                let on = ((s - 0.35) / 0.6).clamp(0.0, 1.0);
                let down = push.rot * Vec3::NEG_Y;
                let from = push.at + push.rot * SOLES[k];
                let wide = AIR_WIDE / 2.2 * (0.6 + 0.4 * on);
                let fall = (from.y - ground).max(0.05);
                let long = fall / (-down.y).max(0.3);
                *t = Transform { translation: from + down * (long * 0.5), rotation: push.rot, scale: Vec3::new(wide, long, wide) };
                *visible = if on > 0.02 { Visibility::Inherited } else { Visibility::Hidden };
                // (Its lumps running down, faster as it strains.)
                part.ran = (part.ran + dt * (0.5 + 0.6 * s)).rem_euclid(1.0);
                if let Some(mut tag) = tag {
                    tag.0 = super::haze::tag(part.ran, s);
                }
            }
            Kind::Beam(_) => {}
            Kind::Hum => {
                t.translation = push.feet;
                if let Some(mut sink) = sink {
                    sink.set_speed(0.8 + 0.25 * s);
                    sink.set_volume(Volume::Linear((0.15 + 0.6 * s) * global.volume.to_linear()));
                }
            }
        }
    }
}

/// Each brute's push this frame, and the ground under its feet (written by
/// `impaled::fly`).
#[derive(Resource, Default)]
pub(in crate::combat) struct Pushes(pub bevy::platform::collections::HashMap<Entity, (Push, f32)>);

/// The beams drawn this frame: from each sole along its down (the way its
/// thrust pushes; tilted as it tilts) to the ground, flaring where they hit
/// it; thicker and more as it strains, thinning and fading as its thrust
/// fails, never quite steady; streaks running down them, faster as it
/// strains. Each placed by its transform (its source, along its down, as
/// long as the way to the ground), what changes in it in its tag, shaped and
/// made a ribbon facing the eye in `plume.wgsl`.
pub(in crate::combat) fn beams(time: Res<Time>, pushes: Res<Pushes>, ground: crate::ground::Ground, mut parts: Query<(&mut Part, &mut Transform, &mut bevy::mesh::MeshTag, &mut Visibility)>) {
    let dt = time.delta_secs();
    for (mut part, mut t, mut tag, mut visible) in &mut parts {
        let Kind::Beam(k) = part.kind else { continue };
        let Some((push, _)) = pushes.0.get(&part.brute) else { continue };
        let s = push.strain.clamp(0.0, 1.6);
        // (How much there is: none below a third of its weight, all of it at
        // a hover, more as it strains.)
        let on = ((s - 0.35) / 0.6).clamp(0.0, 1.0);
        let hard = ((s - 1.0).max(0.0) / 0.45).min(1.0);
        let down = push.rot * Vec3::NEG_Y;
        let from = push.at + push.rot * SOLES[k] + down * AIR_GAP;
        // (To the ground along its down: as far as that falls, the ground
        // under its foot found again where it lands.)
        let fall = (from.y - ground.at(from)).max(0.05);
        let length = fall / (-down.y).max(0.3);
        *t = Transform { translation: from, rotation: Quat::from_rotation_arc(Vec3::NEG_Y, down), scale: Vec3::new(1.0, length, 1.0) };
        *visible = if on > 0.01 { Visibility::Inherited } else { Visibility::Hidden };
        // (How far its streaks have run: faster as it strains; wrapping where
        // its tag does.)
        part.ran = (part.ran + dt * (FLOW_SPEED.0 + FLOW_SPEED.1 * (s - 1.0).max(0.0))).rem_euclid(RAN_WRAP);
        let ran = (part.ran * 1000.0) as u32 & 0xfffff;
        tag.0 = ran | (((on * 63.0).round() as u32) << 20) | (((hard * 31.0).round() as u32) << 26) | ((k as u32 & 1) << 31);
    }
}
