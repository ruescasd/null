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

/// Something of a brute's field: whose, and what.
#[derive(Component)]
pub(in crate::combat) struct Part {
    pub brute: Entity,
    kind: Kind,
}

#[derive(Clone, Copy)]
pub(in crate::combat) enum Kind {
    Beams,
    Air(usize),
    Hum,
}

/// The beams, as they are: their mesh (made again each frame) and their
/// look.
#[derive(Component)]
pub(in crate::combat) struct Beams {
    /// Drawn in shards (else in curves).
    shards: bool,
    mesh: Handle<Mesh>,
    plume: Handle<super::plume::Plume>,
}

/// The meshes, noise and sounds its field and its crash share, made once.
#[derive(Resource)]
pub(in crate::combat) struct Kit {
    /// The bent air's cone, without its ends (seen from inside, a
    /// frustum's ends cut a band across the beams).
    open: Handle<Mesh>,
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
        ring: meshes.add(ring),
        shimmer: images.add(shimmer()),
        hum: server.load("embedded://game/sounds/impaled_hum.wav"),
        thud: server.load("embedded://game/sounds/stalker_impact.wav"),
    });
}

/// A mesh to be made again each frame, as it starts: one triangle, all at
/// one point (a mesh with nothing in it troubled the renderer's allocator,
/// "use-after-free"). With the attributes the beams and cables write.
pub(in crate::combat) fn placeholder() -> Mesh {
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3])
        .with_inserted_indices(bevy::mesh::Indices::U32(vec![0, 1, 2]))
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
pub(in crate::combat) fn make(commands: &mut Commands, kit: &Kit, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, plumes: &mut Assets<super::plume::Plume>, brute: Entity, shards: bool, sound: bool) {
    let mesh = meshes.add(placeholder());
    let plume = plumes.add(super::plume::Plume::new(if shards { super::plume::Look::Shards } else { super::plume::Look::Curves }, FLARE.0));
    commands.spawn((
        Part { brute, kind: Kind::Beams },
        Beams { shards, mesh: mesh.clone(), plume: plume.clone() },
        Mesh3d(mesh),
        MeshMaterial3d(plume),
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
        bevy::camera::visibility::NoFrustumCulling,
        Transform::default(),
        Visibility::Inherited,
    ));
    // (Bent air: see-through, its shimmer bending what is behind it; only
    // its far side: its near side, written into depth, hid the beam's start
    // inside it.)
    let air = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        specular_transmission: 1.0,
        thickness: 0.8,
        ior: 1.08,
        perceptual_roughness: 0.12,
        reflectance: 0.0,
        normal_map_texture: Some(kit.shimmer.clone()),
        double_sided: true,
        cull_mode: Some(bevy::render::render_resource::Face::Front),
        ..default()
    });
    for k in 0..SOLES.len() {
        commands.spawn((Part { brute, kind: Kind::Air(k) }, Mesh3d(kit.open.clone()), MeshMaterial3d(air.clone()), bevy::light::NotShadowCaster, Transform::default(), Visibility::Hidden));
    }
    if sound {
        commands.spawn((Part { brute, kind: Kind::Hum }, AudioPlayer::new(kit.hum.clone()), PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), spatial: true, ..default() }, Transform::default()));
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
    mut parts: Query<(Entity, &Part, &mut Transform, &mut Visibility, Option<&MeshMaterial3d<StandardMaterial>>, Option<&mut SpatialAudioSink>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs();
    for (entity, part, mut t, mut visible, material, sink) in &mut parts {
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
                if let Some(mut m) = material.and_then(|m| materials.get_mut(&m.0)) {
                    m.uv_transform = bevy::math::Affine2::from_translation(Vec2::new(now * 0.05, now * (0.5 + 0.6 * s)));
                    m.thickness = 0.4 + 0.8 * s;
                }
            }
            Kind::Beams => {}
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
/// strains. Each a centreline in pieces, made a ribbon facing the eye in
/// `plume.wgsl`.
pub(in crate::combat) fn beams(
    time: Res<Time>,
    pushes: Res<Pushes>,
    ground: crate::ground::Ground,
    parts: Query<(&Part, &Beams)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plumes: ResMut<Assets<super::plume::Plume>>,
) {
    let now = time.elapsed_secs();
    for (part, b) in &parts {
        let Some((push, _)) = pushes.0.get(&part.brute) else { continue };
        let s = push.strain.clamp(0.0, 1.6);
        // (How much there is: none below a third of its weight, all of it at
        // a hover, more as it strains.)
        let on = ((s - 0.35) / 0.6).clamp(0.0, 1.0);
        let hard = ((s - 1.0).max(0.0) / 0.45).min(1.0);
        let down = push.rot * Vec3::NEG_Y;
        let (mut pos, mut along, mut uv, mut flow, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (k, sole) in SOLES.iter().enumerate() {
            let from = push.at + push.rot * *sole + down * AIR_GAP;
            // (To the ground along its down: as far as that falls, the
            // ground under its foot found again where it lands.)
            let fall = (from.y - ground.at(from)).max(0.05);
            let length = fall / (-down.y).max(0.3);
            let flicker = 1.0 + 0.08 * (now * 31.0 + k as f32 * 2.1).sin() * (now * 13.0 + k as f32).sin();
            let wide = (0.35 + 0.65 * on) * (1.0 + 0.3 * hard) * flicker;
            // (How far the streaks have run: faster as it strains.)
            let ran = now * (FLOW_SPEED.0 + FLOW_SPEED.1 * (s - 1.0).max(0.0)) + k as f32 * 3.7;
            let width = |t: f32| {
                let flare = ((t - FLARE.0) / (1.0 - FLARE.0)).clamp(0.0, 1.0);
                // (Born narrow where it starts below its soles.)
                let born = 1.0 - (1.0 - (length * t / BEAM_GROW).min(1.0)).powi(2);
                (BEAM_WIDE.0 + (BEAM_WIDE.1 - BEAM_WIDE.0) * t.sqrt()) * wide * (1.0 + (FLARE.1 - 1.0) * flare * flare) * (0.45 + 0.55 * born)
            };
            // (Its outline, in shards, in straight runs between a few kinks:
            // its birth, the middle, where it flares, near the ground, the
            // ground.)
            let mut kinks = [0.0, (BEAM_GROW / length).min(0.5), 0.35, FLARE.0, 0.82, 1.0];
            kinks.sort_by(f32::total_cmp);
            for j in 0..=BEAM_PIECES {
                let t = j as f32 / BEAM_PIECES as f32;
                let w = if b.shards {
                    let k = kinks.windows(2).find(|k| t <= k[1]).unwrap_or(&kinks[4..]);
                    let f = if k[1] > k[0] { (t - k[0]) / (k[1] - k[0]) } else { 1.0 };
                    width(k[0]) + (width(k[1]) - width(k[0])) * f
                } else {
                    width(t)
                };
                let i = pos.len() as u32;
                for side in [-1.0, 1.0] {
                    pos.push((from + down * (length * t)).to_array());
                    along.push(down.to_array());
                    uv.push([side, w]);
                    flow.push([length * t - ran, t]);
                }
                if j < BEAM_PIECES {
                    idx.extend([i, i + 1, i + 3, i, i + 3, i + 2]);
                }
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&b.mesh) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, along);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, flow);
            mesh.insert_indices(bevy::mesh::Indices::U32(idx));
        }
        if let Some(mut plume) = plumes.get_mut(&b.plume) {
            plume.set(on, WHITE * on);
        }
    }
}
