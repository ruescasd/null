//! The stalker's gun (the user: an oversized carried weapon, against its
//! fragile look, strange and frightening; it fires a beam, the grid's
//! comet). A long heavy lance, carried resting on its shoulder, planted on a
//! monopod to fire, with a cage near the muzzle that spins up as it charges
//! and stops dead at the lock. Heavy-duty and worn from use, never makeshift
//! (as the shotgun: dark metal, grease where it turns, soot at the muzzle,
//! engineered parts). Built once (`build`), in its own frame: the grip at
//! the origin, +Y up, -Z towards the muzzle; placed each frame from the
//! stalker's rig (`place`, see `rig::GunFrame`).

use bevy::prelude::*;

use super::hunter::Hunter;
use crate::rig::{GUN_BORE, GUN_HINGE};

/// Along it from the grip (m): the butt's end, the receiver, the shroud,
/// the cage, the muzzle's end. Some 3.5 m in all, longer than the stalker
/// is tall.
const BUTT: f32 = -0.85;
const RECEIVER: (f32, f32) = (-0.4, 0.6);
const SHROUD: (f32, f32) = (0.6, 1.75);
const CAGE: (f32, f32) = (1.75, 2.45);
const MUZZLE: f32 = 2.75;
/// The scope on the receiver: along it from the grip (m, its back and
/// front), its radius, how high its axis over the bore. Its lens flares as
/// the gun points at you (`LENS_GLOW`, sharper as it is aimed straight at
/// you): the stalker's warning, on the gun where a scope belongs (on its
/// head, an optic read as a torch stuck in it, the user).
const SCOPE: (f32, f32) = (-0.15, 0.45);
const SCOPE_RADIUS: f32 = 0.05;
const SCOPE_HEIGHT: f32 = 0.22;
const LENS_GLOW: f32 = 6000.0;
/// The cage's spin at full charge (radians a second), and how fast it
/// gathers speed.
const SPIN: f32 = 14.0;
const SPIN_RATE: f32 = 2.5;

/// The gun's parts, built once: its body, its cage (turning about its bore),
/// and its monopod (a unit rod along +Y).
#[derive(Clone)]
pub(super) struct Kit {
    body: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>,
    cage: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>,
    rod: (Handle<Mesh>, Handle<StandardMaterial>),
    /// The scope's lens: its mesh and where (each gun its own glass).
    lens: (Handle<Mesh>, Transform),
}

/// A stalker's gun: whose, its cage, its monopod, and how the cage turns
/// (where it is, how fast).
#[derive(Component)]
pub(super) struct Lance {
    hunter: Entity,
    cage: Entity,
    rod: Entity,
    turn: f32,
    spin: f32,
    /// Its lens's glass (flaring as it points at you).
    lens: Handle<StandardMaterial>,
}

/// Builds the gun's parts.
pub(super) fn build(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Kit {
    // (The shotgun's finishes: dark but matte enough that its facets catch
    // the light; cord and rubber; grease, dark and glossy; worn bare metal;
    // soot.)
    let metal = materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.12, 0.12), perceptual_roughness: 0.62, reflectance: 0.4, ..default() });
    let wrap = materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.95, reflectance: 0.2, ..default() });
    let grease = materials.add(StandardMaterial { base_color: Color::srgb(0.035, 0.035, 0.035), perceptual_roughness: 0.12, reflectance: 0.7, ..default() });
    let worn = materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.15, 0.15), perceptual_roughness: 0.75, reflectance: 0.35, ..default() });
    let soot = materials.add(StandardMaterial { base_color: Color::srgb(0.025, 0.025, 0.025), perceptual_roughness: 0.9, reflectance: 0.2, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::BLACK, perceptual_roughness: 1.0, reflectance: 0.0, ..default() });
    // (Its core: the pale of the creatures' markings.)
    let pale = materials.add(StandardMaterial { base_color: Color::srgb(0.82, 0.82, 0.8), perceptual_roughness: 0.5, ..default() });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let tube = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(8));
    let round = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16));
    let prong = meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4));
    // A block `size` centred `along` it, at height `y` over the grip, `x`
    // aside, tilted about its across axis.
    let block = |size: Vec3, along: f32, y: f32, x: f32, tilt: f32| Transform::from_xyz(x, y, -along).with_rotation(Quat::from_rotation_x(tilt)).with_scale(size);
    // A tube of radius `r` from `a0` to `a1` along it, at height `y`, `x` aside.
    let piped = |r: f32, a0: f32, a1: f32, y: f32, x: f32| {
        Transform::from_xyz(x, y, -(a0 + a1) * 0.5).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(r, a1 - a0, r))
    };
    let mut body = Vec::new();
    let mut part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, t: Transform| body.push((mesh.clone(), material.clone(), t));
    // The stock, its pad.
    part(&cube, &metal, block(Vec3::new(0.1, 0.2, -BUTT + RECEIVER.0), (BUTT + RECEIVER.0) * 0.5, 0.09, 0.0, 0.0));
    part(&cube, &wrap, block(Vec3::new(0.13, 0.25, 0.06), BUTT, 0.09, 0.0, 0.0));
    // The receiver: broad and deep, a narrower keel under it (a chamfer's
    // read), a rail on top.
    let receiver = RECEIVER.1 - RECEIVER.0;
    part(&cube, &metal, block(Vec3::new(0.24, 0.28, receiver), (RECEIVER.0 + RECEIVER.1) * 0.5, GUN_BORE, 0.0, 0.0));
    part(&cube, &metal, block(Vec3::new(0.17, 0.07, receiver * 0.92), (RECEIVER.0 + RECEIVER.1) * 0.5, GUN_BORE - 0.17, 0.0, 0.0));
    part(&cube, &metal, block(Vec3::new(0.08, 0.05, receiver * 0.9), (RECEIVER.0 + RECEIVER.1) * 0.5, GUN_BORE + 0.165, 0.0, 0.0));
    // A grooved plate on either side (as the shotgun's, which the user
    // liked).
    for side in [-1.0f32, 1.0] {
        part(&cube, &metal, block(Vec3::new(0.014, 0.2, receiver * 0.7), (RECEIVER.0 + RECEIVER.1) * 0.5 + 0.05, GUN_BORE, side * 0.126, 0.0));
        for y in [-0.06, 0.0, 0.06] {
            part(&cube, &dark, block(Vec3::new(0.016, 0.012, receiver * 0.6), (RECEIVER.0 + RECEIVER.1) * 0.5 + 0.05, GUN_BORE + y, side * 0.127, 0.0));
        }
    }
    // The grip, raked back, wrapped.
    part(&cube, &wrap, block(Vec3::new(0.07, 0.26, 0.09), 0.0, -0.08, 0.0, -0.3));
    // The shroud, and the collar its monopod is hinged on.
    part(&tube, &metal, piped(0.09, SHROUD.0, SHROUD.1, GUN_BORE, 0.0));
    part(&round, &worn, piped(0.115, GUN_HINGE - 0.07, GUN_HINGE + 0.07, GUN_BORE, 0.0));
    // A hose clamped along its side, from the receiver to the cage.
    part(&tube, &wrap, piped(0.022, RECEIVER.1 - 0.1, CAGE.0, GUN_BORE - 0.06, 0.12));
    for a in [0.85, 1.5] {
        part(&cube, &worn, block(Vec3::new(0.06, 0.06, 0.03), a, GUN_BORE - 0.06, 0.12, 0.0));
    }
    // Greased collars either side of the cage, where it turns.
    for a in [CAGE.0, CAGE.1] {
        part(&round, &grease, piped(0.12, a - 0.035, a + 0.035, GUN_BORE, 0.0));
    }
    // The muzzle: sooted, a crown of prongs.
    part(&tube, &soot, piped(0.1, CAGE.1, MUZZLE, GUN_BORE, 0.0));
    for k in 0..6 {
        let a = (k as f32 + 0.5) / 6.0 * std::f32::consts::TAU;
        part(
            &prong,
            &soot,
            Transform::from_xyz(a.cos() * 0.085, GUN_BORE + a.sin() * 0.085, -(MUZZLE + 0.12)).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(0.028, 0.25, 0.028)),
        );
    }
    // The scope: a tube on two mounts over the receiver, a sooted hood.
    part(&tube, &metal, piped(SCOPE_RADIUS, SCOPE.0, SCOPE.1, GUN_BORE + SCOPE_HEIGHT, 0.0));
    part(&tube, &soot, piped(SCOPE_RADIUS * 1.2, SCOPE.1 - 0.04, SCOPE.1 + 0.03, GUN_BORE + SCOPE_HEIGHT, 0.0));
    for a in [SCOPE.0 + 0.1, SCOPE.1 - 0.15] {
        part(&cube, &worn, block(Vec3::new(0.05, SCOPE_HEIGHT - 0.12, 0.05), a, GUN_BORE + 0.14 + (SCOPE_HEIGHT - 0.12) * 0.5, 0.0, 0.0));
    }
    let lens = (
        round.clone(),
        Transform::from_xyz(0.0, GUN_BORE + SCOPE_HEIGHT, -(SCOPE.1 + 0.035)).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(SCOPE_RADIUS * 0.9, 0.01, SCOPE_RADIUS * 0.9)),
    );
    // The cage: six greased rails round a pale core, in its own frame on the
    // bore (it turns about it).
    let mut cage = Vec::new();
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        cage.push((tube.clone(), grease.clone(), piped(0.016, CAGE.0, CAGE.1, a.sin() * 0.1, a.cos() * 0.1)));
    }
    cage.push((round.clone(), pale, piped(0.04, CAGE.0, CAGE.1, 0.0, 0.0)));
    Kit { body, cage, rod: (meshes.add(Cylinder::new(0.03, 1.0).mesh().resolution(6)), metal), lens }
}

/// Spawns a stalker's gun.
pub(super) fn spawn(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, kit: &Kit, hunter: Entity, at: Vec3) {
    let gun = commands.spawn((Transform::from_translation(at), Visibility::default())).id();
    let lens = materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.8, 0.8), perceptual_roughness: 0.4, fog_enabled: false, ..default() });
    commands.spawn((Mesh3d(kit.lens.0.clone()), MeshMaterial3d(lens.clone()), kit.lens.1, ChildOf(gun)));
    for (mesh, material, t) in &kit.body {
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), *t, ChildOf(gun)));
    }
    let cage = commands.spawn((Transform::from_xyz(0.0, GUN_BORE, 0.0), Visibility::default(), ChildOf(gun))).id();
    for (mesh, material, t) in &kit.cage {
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), *t, ChildOf(cage)));
    }
    let rod = commands.spawn((Mesh3d(kit.rod.0.clone()), MeshMaterial3d(kit.rod.1.clone()), Transform::from_translation(at))).id();
    commands.entity(gun).insert(Lance { hunter, cage, rod, turn: 0.0, spin: 0.0, lens });
}

/// Each gun where its stalker holds it, its monopod from its hinge to its
/// foot, its cage turning as it is aimed.
#[allow(clippy::type_complexity)]
pub(super) fn place(
    mut commands: Commands,
    time: Res<Time>,
    hunters: Query<&Hunter>,
    mut lances: Query<(Entity, &mut Lance, &mut Transform)>,
    mut parts: Query<&mut Transform, (Without<Lance>, Without<crate::camera::FlyCam>)>,
    camera: Single<&Transform, (With<crate::camera::FlyCam>, Without<Lance>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut lance, mut transform) in &mut lances {
        let Ok(h) = hunters.get(lance.hunter) else {
            commands.entity(lance.rod).despawn();
            commands.entity(entity).despawn();
            continue;
        };
        let rig = h.rig();
        let (Some(frame), Some(hold)) = (rig.gun_frame, rig.gun) else { continue };
        let back = -frame.axis;
        transform.translation = frame.grip;
        transform.rotation = Quat::from_mat3(&Mat3::from_cols(frame.up.cross(back).normalize_or(Vec3::X), frame.up, back));
        // (Spinning up as it is aimed; for now, as it is planted.)
        let want = SPIN * hold.aim.clamp(0.0, 1.0);
        lance.spin += (want - lance.spin) * (1.0 - (-dt * SPIN_RATE).exp());
        lance.turn = (lance.turn + lance.spin * dt) % std::f32::consts::TAU;
        // Its scope's lens flaring as it points at you.
        let facing = frame.axis.dot((camera.translation - frame.grip).normalize_or(Vec3::Y)).max(0.0).powi(8);
        if let Some(mut m) = materials.get_mut(&lance.lens) {
            m.emissive = LinearRgba::rgb(1.0, 1.0, 1.0) * LENS_GLOW * facing;
        }
        if let Ok(mut cage) = parts.get_mut(lance.cage) {
            cage.rotation = Quat::from_rotation_z(lance.turn);
        }
        if let Ok(mut rod) = parts.get_mut(lance.rod) {
            let along = frame.foot - frame.hinge;
            let length = along.length().max(0.01);
            *rod = Transform::from_translation((frame.hinge + frame.foot) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, along / length)).with_scale(Vec3::new(1.0, length, 1.0));
        }
    }
}
