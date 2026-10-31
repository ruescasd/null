//! The stalker's gun (the user: an oversized weapon, against its
//! fragile look, strange and frightening; it fires a beam, the grid's
//! comet). Dragged behind it, its tip on the ground (weak, struggling with
//! it, the user's idea), heaved up to fire, its other hand under the barrel
//! (on a monopod, it did not work, the user), with a part that spins up as
//! it charges, and a glass that glints as it points
//! at you, the warning. Heavy-duty and worn from use, never makeshift
//! (as the shotgun: dark metal, grease where it turns, soot at the muzzle,
//! engineered parts). Built once (`build`), in its own frame: the grip at
//! the origin, +Y up, -Z towards the muzzle; placed each frame from the
//! stalker's rig (`place`, see `rig::GunFrame`).
//!
//! Its design, the user's pick of five (see docs/removed.md): tines, four
//! long blades round the beam's square cavity, some 2.6 m.

use bevy::prelude::*;

use super::hunter::Hunter;
use crate::rig::{GUN_BORE, GUN_FORE};

/// The glass's glow aimed straight at you (sharper as it is aimed straight
/// at you, see `place`), and how many pixels across it is drawn at least
/// (far off, a speck too small to see; the stalker's warning at range).
const LENS_GLOW: f32 = 6000.0;
const LENS_PX: f32 = 5.0;
/// The spinning part's spin at full charge (radians a second), and how fast
/// it gathers speed.
const SPIN: f32 = 14.0;
const SPIN_RATE: f32 = 2.5;

/// The gun's parts, built once: its body, its spinning part (turning about
/// its bore), and its glass.
#[derive(Clone)]
pub(super) struct Kit {
    body: Parts,
    cage: Parts,
    /// The glass: its mesh and where (each gun its own glass).
    lens: (Handle<Mesh>, Transform),
}

/// A stalker's gun: whose, its spinning part, and how the part turns (where
/// it is, how fast).
#[derive(Component)]
pub(super) struct Lance {
    hunter: Entity,
    cage: Entity,
    turn: f32,
    spin: f32,
    /// Its glass (flaring as it points at you): its finish, its entity, and
    /// its place on the gun.
    lens: Handle<StandardMaterial>,
    glass: Entity,
    glass_at: Transform,
}

/// The finishes and shapes it is made of.
struct Shop {
    cube: Handle<Mesh>,
    tube: Handle<Mesh>,
    round: Handle<Mesh>,
    hex: Handle<Mesh>,
    prong: Handle<Mesh>,
    metal: Handle<StandardMaterial>,
    wrap: Handle<StandardMaterial>,
    grease: Handle<StandardMaterial>,
    worn: Handle<StandardMaterial>,
    soot: Handle<StandardMaterial>,
    pale: Handle<StandardMaterial>,
}

type Parts = Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>;

/// A point `along` it from the grip, `y` up, `x` aside.
fn at(along: f32, y: f32, x: f32) -> Vec3 {
    Vec3::new(x, y, -along)
}

/// A block `size` centred `along` it, at height `y` over the grip, `x`
/// aside, tilted about its across axis.
fn block(size: Vec3, along: f32, y: f32, x: f32, tilt: f32) -> Transform {
    Transform::from_translation(at(along, y, x)).with_rotation(Quat::from_rotation_x(tilt)).with_scale(size)
}

/// A cylinder of radius `r` from `a0` to `a1` along it, at height `y`, `x`
/// aside.
fn piped(r: f32, a0: f32, a1: f32, y: f32, x: f32) -> Transform {
    Transform::from_translation(at((a0 + a1) * 0.5, y, x)).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(r, a1 - a0, r))
}

/// A cylinder of radius `r` from `a` to `b`.
fn strut(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    Transform::from_translation((a + b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y))).with_scale(Vec3::new(r, d.length(), r))
}


/// A point (a four-sided cone) `long`, `r` round, from `along` forward.
fn point(along: f32, y: f32, r: f32, long: f32) -> Transform {
    Transform::from_translation(at(along + long * 0.5, y, 0.0)).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(r, long, r))
}

/// A glass of radius `r` facing forward, `along` it at height `y`.
fn glass(r: f32, along: f32, y: f32) -> Transform {
    Transform::from_translation(at(along, y, 0.0)).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(r, 0.01, r))
}

/// How far its tip is along its bore from the grip (m): where it drags on
/// the ground (see `rig::GunHold`).
pub(super) const TIP: f32 = 2.18;

/// Where its shot leaves from, along its bore from the grip (m): inside it,
/// the pale needle at the back of its blades' cavity, so the beam comes out
/// of it (from the prongs' tips, it formed apart before the gun, the user).
pub(super) const EMITTER: f32 = 0.64;

/// Builds the gun's parts.
pub(super) fn build(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Kit {
    // (The shotgun's finishes: dark but matte enough that its facets catch
    // the light; cord and rubber; grease, dark and glossy; worn bare metal;
    // soot; and the pale of the creatures' markings.)
    let shop = Shop {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        tube: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(8)),
        round: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16)),
        hex: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(6)),
        prong: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4)),
        metal: materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.12, 0.12), perceptual_roughness: 0.62, reflectance: 0.4, ..default() }),
        wrap: materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.95, reflectance: 0.2, ..default() }),
        grease: materials.add(StandardMaterial { base_color: Color::srgb(0.035, 0.035, 0.035), perceptual_roughness: 0.12, reflectance: 0.7, ..default() }),
        worn: materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.15, 0.15), perceptual_roughness: 0.75, reflectance: 0.35, ..default() }),
        soot: materials.add(StandardMaterial { base_color: Color::srgb(0.025, 0.025, 0.025), perceptual_roughness: 0.9, reflectance: 0.2, ..default() }),
        pale: materials.add(StandardMaterial { base_color: Color::srgb(0.82, 0.82, 0.8), perceptual_roughness: 0.5, ..default() }),
    };
    let (mut body, mut cage) = (Parts::new(), Parts::new());
    let lens = tines(&shop, &mut body, &mut cage);
    Kit { body, cage, lens: (shop.round.clone(), lens) }
}

/// The rear of the newer designs: a round housing on the bore from `a0` to
/// `a1` (radius `r`), a greased collar at its front, a thin stock back to a
/// pad, and the grip under it.
fn rear(s: &Shop, body: &mut Parts, r: f32, a0: f32, a1: f32) {
    body.push((s.round.clone(), s.metal.clone(), piped(r, a0, a1, GUN_BORE, 0.0)));
    body.push((s.round.clone(), s.grease.clone(), piped(r * 1.15, a1 - 0.03, a1 + 0.02, GUN_BORE, 0.0)));
    body.push((s.tube.clone(), s.metal.clone(), piped(0.025, -0.55, a0, GUN_BORE - 0.02, 0.0)));
    body.push((s.round.clone(), s.wrap.clone(), piped(0.07, -0.6, -0.54, GUN_BORE - 0.02, 0.0)));
    body.push((s.cube.clone(), s.wrap.clone(), block(Vec3::new(0.06, 0.24, 0.08), 0.0, -0.06, 0.0, -0.3)));
}

/// Tines. Four long flat blades round the bore, its top, bottom and
/// sides, the beam's square cavity between them (open at its corners, a
/// slit along each), narrowing to points; bound by square clamps; a stack
/// of hex discs spinning behind them; the glass at the back of the cavity,
/// on a pale needle. (Two blades, over and under, read as a fork, the
/// user: four, a square cavity.)
fn tines(s: &Shop, body: &mut Parts, cage: &mut Parts) -> Transform {
    rear(s, body, 0.075, -0.2, 0.32);
    let (from, to, gap) = (0.52, 2.0, 0.065);
    // (Each blade made as the top one, from the bore, then turned about it.)
    let about = |t: Transform, k: usize| {
        let q = Quat::from_rotation_z(k as f32 * std::f32::consts::FRAC_PI_2);
        Transform { translation: q * t.translation + Vec3::Y * GUN_BORE, rotation: q * t.rotation, scale: t.scale }
    };
    for k in 0..4 {
        // Deep at the back, thinner on, a point at the tip.
        body.push((s.cube.clone(), s.metal.clone(), about(block(Vec3::new(0.075, 0.022, 0.8), from + 0.4, gap + 0.011, 0.0, 0.0), k)));
        body.push((s.cube.clone(), s.metal.clone(), about(block(Vec3::new(0.06, 0.016, to - from - 0.8), (from + 0.8 + to) * 0.5, gap + 0.008, 0.0, 0.0), k)));
        body.push((s.prong.clone(), s.soot.clone(), about(point(to, gap + 0.008, 0.02, 0.18), k)));
        // Its inner face sooted by the beam.
        body.push((s.cube.clone(), s.soot.clone(), about(block(Vec3::new(0.055, 0.004, to - from - 0.1), (from + to) * 0.5 + 0.05, gap - 0.001, 0.0, 0.0), k)));
        // Square clamps round the four (one where its other hand holds it).
        for a in [GUN_FORE, 1.05, 1.6] {
            body.push((s.cube.clone(), s.worn.clone(), about(block(Vec3::new(gap * 2.0 + 0.07, 0.014, 0.045), a, gap + 0.03, 0.0, 0.0), k)));
        }
    }
    // The square flange the blades root in, at the housing's front.
    body.push((s.cube.clone(), s.metal.clone(), block(Vec3::new(gap * 2.0 + 0.06, gap * 2.0 + 0.06, 0.1), from - 0.05, GUN_BORE, 0.0, 0.0)));
    // A hose from the housing along the bottom blade.
    body.push((s.tube.clone(), s.wrap.clone(), strut(at(0.2, GUN_BORE - 0.07, 0.06), at(0.95, GUN_BORE - gap - 0.045, 0.03), 0.016)));
    // The disc stack, spinning behind the blades.
    for k in 0..3 {
        let a = 0.345 + k as f32 * 0.025;
        cage.push((s.hex.clone(), if k == 1 { s.grease.clone() } else { s.metal.clone() }, piped(0.11 - k as f32 * 0.01, a, a + 0.02, 0.0, 0.0)));
    }
    // The pale needle the beam leaves from, its glass at its tip.
    body.push((s.round.clone(), s.pale.clone(), piped(0.012, from - 0.02, from + 0.12, GUN_BORE, 0.0)));
    glass(0.02, from + 0.125, GUN_BORE)
}




/// Spawns a stalker's gun.
pub(super) fn spawn(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, kit: &Kit, hunter: Entity, at: Vec3) {
    let gun = commands.spawn((Transform::from_translation(at), Visibility::default())).id();
    let lens = materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.8, 0.8), perceptual_roughness: 0.4, fog_enabled: false, ..default() });
    let glass = commands.spawn((Mesh3d(kit.lens.0.clone()), MeshMaterial3d(lens.clone()), kit.lens.1, ChildOf(gun))).id();
    for (mesh, material, t) in &kit.body {
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), *t, ChildOf(gun)));
    }
    let cage = commands.spawn((Transform::from_xyz(0.0, GUN_BORE, 0.0), Visibility::default(), ChildOf(gun))).id();
    for (mesh, material, t) in &kit.cage {
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), *t, ChildOf(cage)));
    }
    commands.entity(gun).insert(Lance { hunter, cage, turn: 0.0, spin: 0.0, lens, glass, glass_at: kit.lens.1 });
}

/// Each gun where its stalker holds it, its spinning part turning as it is
/// aimed.
#[allow(clippy::type_complexity)]
pub(super) fn place(
    mut commands: Commands,
    time: Res<Time>,
    stutter: Res<crate::look::Stutter>,
    hunters: Query<&Hunter>,
    mut lances: Query<(Entity, &mut Lance, &mut Transform)>,
    mut parts: Query<&mut Transform, (Without<Lance>, Without<crate::camera::FlyCam>)>,
    camera: Single<&Transform, (With<crate::camera::FlyCam>, Without<Lance>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut lance, mut transform) in &mut lances {
        let Ok(h) = hunters.get(lance.hunter) else {
            commands.entity(entity).despawn();
            continue;
        };
        let rig = h.rig();
        let (Some(frame), Some(hold)) = (rig.gun_frame, rig.gun) else { continue };
        let back = -frame.axis;
        transform.translation = frame.grip;
        transform.rotation = Quat::from_mat3(&Mat3::from_cols(frame.up.cross(back).normalize_or(Vec3::X), frame.up, back));
        // (Spinning up as it is aimed.)
        let want = SPIN * hold.aim.clamp(0.0, 1.0);
        lance.spin += (want - lance.spin) * (1.0 - (-dt * SPIN_RATE).exp());
        lance.turn = (lance.turn + lance.spin * dt) % std::f32::consts::TAU;
        // Its glass flaring as it points at you.
        let facing = frame.axis.dot((camera.translation - frame.grip).normalize_or(Vec3::Y)).max(0.0).powi(8);
        if let Some(mut m) = materials.get_mut(&lance.lens) {
            m.emissive = LinearRgba::rgb(1.0, 1.0, 1.0) * LENS_GLOW * facing * stutter.0;
        }
        // (Far off, drawn larger, at least `LENS_PX` across: a pixel is
        // about a thousandth of the distance.)
        let far = camera.translation.distance(frame.grip) * 0.00107 * LENS_PX * 0.5;
        let grow = (far / lance.glass_at.scale.x.max(1e-4)).max(1.0);
        if let Ok(mut glass) = parts.get_mut(lance.glass) {
            glass.scale = lance.glass_at.scale * Vec3::new(grow, 1.0, grow);
        }
        if let Ok(mut cage) = parts.get_mut(lance.cage) {
            cage.rotation = Quat::from_rotation_z(lance.turn);
        }
    }
}
