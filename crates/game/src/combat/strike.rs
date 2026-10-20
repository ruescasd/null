//! A stalker's shot striking (the user: its impact read as geometry, not
//! damage; the whole as a science experiment, without the weight or the
//! violence of something menacing): sudden, at its height at once, then
//! messy, and leaving a mark. The game held still a moment, a white flash,
//! your view jolted (near it, more); pale stone blasted up out of the ground
//! and falling back; while its beam holds, a hot core boring in, spraying
//! sparks and stone; and after, a dark cracked scar on the ground, long
//! after all else is gone.

use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::PrimitiveTopology,
    prelude::*,
};
use worldgen::noise::hash01;

use super::{Assets3, Debris, Director, Gun};

/// The game held still (s, of real time), if you are within so far (m).
const HITSTOP: (f32, f32) = (0.06, 40.0);
/// The flash (how white at most, near), and the jolt (radians at most).
const WHITE: f32 = 0.85;
const JOLT: f32 = 0.09;
/// Stone blasted out at once (how many), and sprayed as it bores in (how
/// often, s).
const CHUNKS: usize = 18;
const SPRAY: f32 = 0.05;
/// Its core: how big (m), how bright; its light: how bright, how far.
const CORE: (f32, f32) = (0.22, 80.0);
const LIGHT: (f32, f32) = (8.0e5, 10.0);
/// Its scar: how far its cracks reach (m), how long it lasts, and over how
/// much of its end it fades (s).
const SCAR: (f32, f32, f32) = (2.4, 20.0, 4.0);
/// Its furrow, burnt back along the beam's way as it came down to the
/// ground (the user: to show, after the fact, whence it came): as long as
/// the beam's way is shallow (m, at the steepest and at most), how wide at
/// the strike (m).
const FURROW: (f32, f32, f32) = (0.6, 9.0, 0.32);
/// Stone hanging in a pillar of light (`--opt pillar`, the user's twist):
/// how many, lifted from how far round (m), up to how high (m, at least and
/// at most), how fast (of the way left, per s), turning how fast (rad/s);
/// as the pillar goes out, they fall.
const FLOATERS: usize = 24;
const FLOAT_ROUND: f32 = 2.2;
const FLOAT_HIGH: (f32, f32) = (0.5, 4.0);
const FLOAT_RISE: f32 = 0.9;
const FLOAT_SPIN: f32 = 0.8;

/// The game held still: how long yet (real s), and whether it was held by
/// this (so as not to let go of anyone else's pause).
#[derive(Resource, Default)]
pub(super) struct Hitstop {
    left: f32,
    ours: bool,
}

/// What a strike is made of: its core (a unit ball), a chunk of stone (a
/// unit cube) and its stone; and the black of stone hanging in a pillar
/// (silhouetted against it, as things against the comet; pale, it was lost
/// in it; blackening as it rose, it could not be seen, the user).
#[derive(Resource)]
pub(super) struct Kit {
    core: Handle<Mesh>,
    chunk: Handle<Mesh>,
    stone: Handle<StandardMaterial>,
    black: Handle<StandardMaterial>,
    /// A stalker's shot (its charge, then its bang; see `halo.rs`) and its
    /// strike.
    pub shot: Handle<AudioSource>,
    impact: Handle<AudioSource>,
}

/// How loud a stalker's shot and its strike, near; and from how far (m)
/// half as loud (heard far off: a sniper's).
pub(super) const LOUD: (f32, f32, f32) = (1.6, 2.2, 40.0);

/// How loud a sound from `d` m off (see `LOUD`).
pub(super) fn loudness(near: f32, d: f32) -> f32 {
    near / (1.0 + d / LOUD.2)
}

/// A strike: which way its shot came, how long it has been, how long its
/// beam holds, its core's finish (its own, to fade), when it next sprays,
/// and whether it has struck yet (its first frame).
#[derive(Component)]
pub(super) struct Strike {
    dir: Vec3,
    age: f32,
    hold: f32,
    core: Handle<StandardMaterial>,
    spray: f32,
    struck: bool,
}

/// A scar on the ground: how long it has lain, its finish (to fade).
#[derive(Component)]
pub(super) struct Scar {
    age: f32,
    material: Handle<StandardMaterial>,
}

pub(super) fn setup(mut commands: Commands, server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.init_resource::<Hitstop>();
    commands.insert_resource(Kit {
        core: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        chunk: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        stone: materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.55, 0.53), perceptual_roughness: 0.95, ..default() }),
        black: materials.add(StandardMaterial { base_color: Color::srgb(0.02, 0.02, 0.02), perceptual_roughness: 0.9, reflectance: 0.2, ..default() }),
        shot: server.load("embedded://game/sounds/stalker_shot.wav"),
        impact: server.load("embedded://game/sounds/stalker_impact.wav"),
    });
}

/// A shot strikes at `at`, from along `dir`, its beam holding `hold` s;
/// its scar on the ground there, its cracks laid on the ground's slope (its
/// up, `up`).
pub(super) fn strike(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, kit: &Kit, at: Vec3, dir: Vec3, up: Vec3, hold: f32) {
    let core = materials.add(StandardMaterial { base_color: Color::LinearRgba(LinearRgba::gray(CORE.1)), unlit: true, alpha_mode: AlphaMode::Add, fog_enabled: false, ..default() });
    commands.spawn((
        Strike { dir, age: 0.0, hold, core: core.clone(), spray: 0.0, struck: false },
        Mesh3d(kit.core.clone()),
        MeshMaterial3d(core),
        Transform::from_translation(at).with_scale(Vec3::splat(CORE.0)),
        PointLight { intensity: LIGHT.0, range: LIGHT.1, shadow_maps_enabled: false, ..default() },
        NotShadowCaster,
        NotShadowReceiver,
    ));
    let seed = (at.x * 131.0 + at.z * 977.0) as i32;
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.025, 0.025, 0.025, 0.92),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        depth_bias: 50.0,
        ..default()
    });
    commands.spawn((
        Scar { age: 0.0, material: material.clone() },
        Mesh3d(meshes.add(scar_mesh(seed, furrow(dir, up)))),
        MeshMaterial3d(material),
        Transform::from_translation(at + up * 0.03).with_rotation(Quat::from_rotation_arc(Vec3::Y, up)),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// A scar, flat (its up +Y): a ragged burnt middle, and cracks running out
/// from it, kinked, tapering.
/// A furrow back along the beam's way `dir`, in the scar's frame (its up
/// `up` the ground's): which way, and how long.
fn furrow(dir: Vec3, up: Vec3) -> (Vec3, f32) {
    let back = Quat::from_rotation_arc(up, Vec3::Y) * -dir;
    let flat = Vec3::new(back.x, 0.0, back.z);
    // (Shallow, a long furrow; straight down, a short one.)
    let slope = back.y.max(0.02) / flat.length().max(1e-3);
    (flat.normalize_or(Vec3::X), (FURROW.0 / slope).clamp(FURROW.0, FURROW.1))
}

fn scar_mesh(seed: i32, (back, long): (Vec3, f32)) -> Mesh {
    let r = |k: i32, j: i32| hash01(seed, k, j, 0x5c1);
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut tri = |a: Vec3, b: Vec3, c: Vec3| positions.extend([a.to_array(), b.to_array(), c.to_array()]);
    let n = 11;
    let rim: Vec<Vec3> = (0..n)
        .map(|k| {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let radius = 0.3 + 0.35 * r(k, 0);
            Vec3::new(a.cos() * radius, 0.0, a.sin() * radius)
        })
        .collect();
    for k in 0..n as usize {
        tri(Vec3::ZERO, rim[(k + 1) % n as usize], rim[k]);
    }
    for c in 0..8 {
        let a = (c as f32 + r(c, 1)) / 8.0 * std::f32::consts::TAU;
        let length = 0.8 + (SCAR.0 - 0.8) * r(c, 2);
        let mut at = Vec3::new(a.cos(), 0.0, a.sin()) * 0.3;
        let mut way = a;
        let steps = 5;
        for s in 0..steps {
            way += (r(c * 10 + s, 3) - 0.5) * 0.9;
            let next = at + Vec3::new(way.cos(), 0.0, way.sin()) * length / steps as f32;
            let side = Vec3::new(-way.sin(), 0.0, way.cos());
            let (w0, w1) = (0.09 * (1.0 - s as f32 / steps as f32), 0.09 * (1.0 - (s + 1) as f32 / steps as f32));
            tri(at - side * w0, next - side * w1, at + side * w0);
            tri(at + side * w0, next - side * w1, next + side * w1);
            at = next;
        }
    }
    // The furrow: from the strike back along the beam's way, wide there,
    // narrowing to nothing, a little ragged.
    let side = Vec3::new(-back.z, 0.0, back.x);
    let steps = 12;
    for s in 0..steps {
        let (t0, t1) = (s as f32 / steps as f32, (s + 1) as f32 / steps as f32);
        let w = |t: f32, k: i32| FURROW.2 * (1.0 - t).powf(0.7) * (0.75 + 0.5 * r(40 + k, 5));
        let (p0, p1) = (back * long * t0, back * long * t1);
        let (a0, a1, b0, b1) = (p0 - side * w(t0, s), p0 + side * w(t0, s + 100), p1 - side * w(t1, s + 1), p1 + side * w(t1, s + 101));
        tri(a0, b0, a1);
        tri(a1, b0, b1);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

/// Each strike: on its first frame, the game held, the flash, the jolt, the
/// stone blasted out; then its core boring in, flickering, spraying, as its
/// beam holds; then gone.
#[allow(clippy::too_many_arguments)]
pub(super) fn strikes(
    mut commands: Commands,
    time: Res<Time>,
    kit: Res<Kit>,
    assets: Res<Assets3>,
    camera: Single<&Transform, (With<crate::camera::FlyCam>, Without<Strike>)>,
    mut hitstop: ResMut<Hitstop>,
    mut director: ResMut<Director>,
    mut gun: ResMut<Gun>,
    mut strikes: Query<(Entity, &mut Strike, &mut Transform, &mut PointLight)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    for (entity, mut s, mut transform, mut light) in &mut strikes {
        let at = transform.translation;
        let back = (-s.dir + Vec3::Y * 0.8).normalize_or(Vec3::Y);
        let seed = (at.x * 131.0 + at.z * 977.0) as i32;
        if !s.struck {
            s.struck = true;
            let far = camera.translation.distance(at);
            commands.spawn((AudioPlayer::new(kit.impact.clone()), PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(loudness(LOUD.1, far)))));
            let near = (1.0 - camera.translation.distance(at) / HITSTOP.1).max(0.0);
            if near > 0.0 {
                hitstop.left = hitstop.left.max(HITSTOP.0);
                director.white = director.white.max(WHITE * near);
                gun.jolt += Vec2::new(JOLT * near, JOLT * near * (hash01(seed, 0, 0, 0x5c2) - 0.5) * 2.0);
            }
            for k in 0..CHUNKS as i32 {
                chunk(&mut commands, &kit, at, back, seed, k, 1.0);
            }
        }
        s.age += dt;
        let fade = s.hold + 0.3;
        if s.age > fade {
            materials.remove(&s.core);
            commands.entity(entity).despawn();
            continue;
        }
        // Boring in as its beam holds: flickering, spraying sparks and stone.
        let flicker = 0.7 + 0.6 * hash01(seed, (s.age / 0.03) as i32, 1, 0x5c3);
        let left = if s.age < s.hold { 1.0 } else { 1.0 - (s.age - s.hold) / 0.3 };
        transform.scale = Vec3::splat(CORE.0 * flicker * left.sqrt());
        light.intensity = LIGHT.0 * flicker * left * left;
        if let Some(mut m) = materials.get_mut(&s.core) {
            m.base_color = Color::LinearRgba(LinearRgba::gray(CORE.1 * flicker * left));
        }
        s.spray -= dt;
        if s.age < s.hold && s.spray <= 0.0 {
            s.spray += SPRAY;
            let k = (s.age / SPRAY) as i32;
            super::sparks(&mut commands, &assets, at, back, (seed + k) as u32);
            chunk(&mut commands, &kit, at, back, seed, 100 + k, 0.5);
        }
    }
}

/// A chunk of stone blasted out (`k`-th of a strike), up and back along
/// `back`; `force` of a full blast.
fn chunk(commands: &mut Commands, kit: &Kit, at: Vec3, back: Vec3, seed: i32, k: i32, force: f32) {
    let r = |j: i32| hash01(seed, k, j, 0x5c4) - 0.5;
    let out = Vec3::new(r(0), 0.0, r(1)).normalize_or(Vec3::X);
    let velocity = (out * (2.0 + 4.0 * (r(2) + 0.5)) + Vec3::Y * (5.0 + 6.0 * (r(3) + 0.5)) + back * 2.0) * force;
    let size = Vec3::new(0.06 + 0.14 * (r(4) + 0.5), 0.05 + 0.1 * (r(5) + 0.5), 0.06 + 0.14 * (r(6) + 0.5));
    let life = 1.2 + 0.6 * (r(7) + 0.5);
    commands.spawn((
        Debris { velocity, spin: Vec3::new(r(8), r(9), r(10)) * 14.0, life, total: life, size },
        Mesh3d(kit.chunk.clone()),
        MeshMaterial3d(kit.stone.clone()),
        Transform::from_translation(at + Vec3::Y * 0.05).with_scale(size),
    ));
}

/// A chunk of stone hanging in a pillar: how high it hangs over where it
/// was lifted from (m), turning how, for how long yet (s), its size.
#[derive(Component)]
pub(super) struct Floater {
    from: Vec3,
    high: f32,
    spin: Vec3,
    left: f32,
    size: Vec3,
}

/// Stone lifted off the ground round `at`, to hang in its pillar `lasts` s.
pub(super) fn levitate(commands: &mut Commands, kit: &Kit, ground: &crate::ground::Ground, at: Vec3, lasts: f32) {
    let seed = (at.x * 131.0 + at.z * 977.0) as i32;
    for k in 0..FLOATERS as i32 {
        let r = |j: i32| hash01(seed, k, j, 0x5c5);
        let (a, d) = (r(0) * std::f32::consts::TAU, FLOAT_ROUND * r(1).sqrt());
        let p = at + Vec3::new(a.cos() * d, 0.0, a.sin() * d);
        let from = p.with_y(ground.at(p + Vec3::Y * 0.5) + 0.05);
        let size = Vec3::new(0.08 + 0.22 * r(2), 0.06 + 0.14 * r(3), 0.08 + 0.22 * r(4));
        commands.spawn((
            Floater { from, high: FLOAT_HIGH.0 + (FLOAT_HIGH.1 - FLOAT_HIGH.0) * r(5), spin: Vec3::new(r(6) - 0.5, r(7) - 0.5, r(8) - 0.5) * 2.0 * FLOAT_SPIN, left: lasts, size },
            Mesh3d(kit.chunk.clone()),
            MeshMaterial3d(kit.black.clone()),
            Transform::from_translation(from).with_scale(size),
        ));
    }
}

/// Hanging stone rises slowly, turning, bobbing a little; its pillar gone,
/// it falls (as any debris).
pub(super) fn floaters(mut commands: Commands, time: Res<Time>, mut floaters: Query<(Entity, &mut Floater, &mut Transform)>) {
    let (dt, t) = (time.delta_secs(), time.elapsed_secs());
    for (entity, mut f, mut transform) in &mut floaters {
        f.left -= dt;
        if f.left <= 0.0 {
            commands.entity(entity).remove::<Floater>().insert(Debris { velocity: Vec3::ZERO, spin: f.spin, life: 2.0, total: 2.0, size: f.size });
            continue;
        }
        let bob = 0.06 * (t * 1.7 + f.from.x).sin();
        let want = f.from.y + f.high + bob;
        let y = transform.translation.y;
        transform.translation.y = y + (want - y) * (1.0 - (-dt * FLOAT_RISE).exp());
        let spin = f.spin * dt;
        transform.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
    }
}

/// Scars lie, then fade and go.
pub(super) fn scars(mut commands: Commands, time: Res<Time>, mut scars: Query<(Entity, &mut Scar)>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for (entity, mut scar) in &mut scars {
        scar.age += time.delta_secs();
        if scar.age > SCAR.1 {
            materials.remove(&scar.material);
            commands.entity(entity).despawn();
            continue;
        }
        let left = ((SCAR.1 - scar.age) / SCAR.2).min(1.0);
        if let Some(mut m) = materials.get_mut(&scar.material) {
            m.base_color = Color::srgba(0.025, 0.025, 0.025, 0.92 * left);
        }
    }
}

/// The game held still (its clock paused, by real time), then let go.
pub(super) fn hold(real: Res<Time<Real>>, mut clock: ResMut<Time<Virtual>>, mut hitstop: ResMut<Hitstop>) {
    if hitstop.left > 0.0 {
        if !clock.is_paused() {
            clock.pause();
            hitstop.ours = true;
        }
        hitstop.left -= real.delta_secs();
    } else if hitstop.ours {
        clock.unpause();
        hitstop.ours = false;
    }
}
