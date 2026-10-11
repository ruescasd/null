//! The orb (`--opt orb` in the room lab): a sphere of the creature's
//! substance hanging in the middle of a rotunda. A fractal fill of small
//! fragments (cubes, wedges, shards, as the creature's), kept within a
//! sphere: a shell of dark fragments with irregular gaps, a sparser layer
//! under it, a core of glowing shards, and a few fragments adrift outside.
//! A light in the core casts shadows, so what light leaves it leaves through
//! the gaps, across the columns round it. It turns slowly, breathes, its
//! fragments shiver, and its core pulses as the creature's chest does.

use bevy::prelude::*;
use worldgen::{
    ifs::{self, Block, Keep, Rule},
    noise::hash01,
};

use crate::figure::faceted;

/// A fragment of an orb: where it rests (from the orb's centre), its own
/// turn, a phase for its motion, and whether it is adrift outside.
#[derive(Component)]
pub(crate) struct Piece {
    orb: Entity,
    rest: Vec3,
    rotation: Quat,
    phase: f32,
    adrift: bool,
}

/// An orb: its centre and radius, and its glowing core's material.
#[derive(Component)]
pub(crate) struct Orb {
    centre: Vec3,
    radius: f32,
    glow: Handle<StandardMaterial>,
}

/// The core's brightness (before the pulse) and the light in it (lumens).
const GLOW: f32 = 20000.0;
const CORE_LIGHT: f32 = 2.0e6;

/// Builds an orb of `radius` at `centre`.
pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, centre: Vec3, radius: f32) {
    let shapes = [
        meshes.add(Cuboid::new(2.0, 2.0, 2.0)),
        meshes.add(faceted(
            &[[-1., -1., -1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.], [1., -1., 1.], [-1., 1., 1.]],
            &[&[0, 2, 1], &[3, 4, 5], &[0, 1, 4, 3], &[0, 3, 5, 2], &[1, 2, 5, 4]],
        )),
        meshes.add(faceted(&[[-1., -1., -1.], [1., -1., -0.6], [-0.2, -1., 1.], [0.3, 1., 0.1]], &[&[0, 1, 2], &[0, 3, 1], &[1, 3, 2], &[2, 3, 0]])),
    ];
    // (The luminous creature's finishes, pale to dark.)
    let finishes = [(0.22, 0.35, 0.7), (0.09, 0.55, 0.5), (0.03, 0.2, 0.9)].map(|(tone, rough, refl)| {
        materials.add(StandardMaterial { base_color: Color::srgb(tone, tone, tone), perceptual_roughness: rough, reflectance: refl, ..default() })
    });
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(GLOW), ..default() });
    let orb = commands.spawn((Orb { centre, radius, glow: glow.clone() }, Transform::from_translation(centre), Visibility::default())).id();
    commands.spawn((
        CoreLight(orb),
        PointLight { intensity: CORE_LIGHT, range: radius * 14.0, radius: radius * 0.15, shadow_maps_enabled: true, ..default() },
        Transform::from_translation(centre),
    ));
    // (Every cell kept and split to the end: the gaps are made fragment by
    // fragment below, never a whole block of the sphere at once.)
    let rule = Rule { divisions: [3, 3, 3], keep: Keep::All, depth: 3, gap: 1.0, twist: Quat::IDENTITY, stop_chance: 0.0, lift: 0.0, min_size: 0.0 };
    let root = Block { center: Vec3::ZERO, rotation: Quat::IDENTITY, half: Vec3::splat(radius), level: 0 };
    let mut count = 0;
    for (n, leaf) in ifs::generate(&rule, root, 0x0b5, 30000).iter().enumerate() {
        let b = leaf.block;
        let r = |k: i32| hash01(n as i32, k, 0, 0x0b6);
        let d = b.center.length() / radius;
        // The shell, a sparser layer under it, the glowing core.
        let (keep, glows) = match d {
            d if d > 1.0 => (false, false),
            d if d > 0.72 => (r(1) < 0.62, false),
            d if d > 0.42 => (r(1) < 0.25, false),
            _ => (r(1) < 0.75, r(2) < 0.85),
        };
        if !keep {
            continue;
        }
        let shrink = Vec3::new(0.55 + 0.4 * r(3), 0.55 + 0.4 * r(4), 0.55 + 0.4 * r(5));
        let tilt = Quat::from_euler(EulerRot::YXZ, (r(6) - 0.5) * 0.9, (r(7) - 0.5) * 0.7, (r(8) - 0.5) * 0.7);
        let shape = match r(9) {
            x if x < 0.45 => 0,
            x if x < 0.75 => 1,
            _ => 2,
        };
        let material = if glows { glow.clone() } else { finishes[(r(10) * 3.0) as usize % 3].clone() };
        commands.spawn((
            Piece { orb, rest: b.center, rotation: b.rotation * tilt, phase: r(11) * 100.0, adrift: false },
            Mesh3d(shapes[shape].clone()),
            MeshMaterial3d(material),
            Transform::from_translation(centre + b.center).with_scale(b.half * shrink),
        ));
        count += 1;
    }
    // A few fragments adrift outside, larger and slower.
    for k in 0..90 {
        let r = |j: i32| hash01(k, j, 1, 0x0b7);
        let dir = Vec3::new(r(1) - 0.5, (r(2) - 0.5) * 0.7, r(3) - 0.5).normalize_or(Vec3::X);
        let at = dir * radius * (1.15 + 0.5 * r(4));
        let size = radius * (0.02 + 0.03 * r(5));
        commands.spawn((
            Piece { orb, rest: at, rotation: Quat::from_euler(EulerRot::YXZ, r(6) * 6.3, r(7) * 6.3, r(8) * 6.3), phase: r(9) * 100.0, adrift: true },
            Mesh3d(shapes[(r(10) * 3.0) as usize % 3].clone()),
            MeshMaterial3d(finishes[(r(11) * 3.0) as usize % 3].clone()),
            Transform::from_translation(centre + at).with_scale(Vec3::new(size, size * (0.6 + r(12)), size * (0.5 + r(13)))),
        ));
    }
    info!("the orb: {count} fragments, 90 adrift");
}

/// The light in an orb's core.
#[derive(Component)]
pub(crate) struct CoreLight(Entity);

/// The orbs' motion: each turns slowly about a leaning axis and breathes
/// (each fragment's distance from the centre swells and settles, out of
/// step with the others'); fragments shiver; those adrift circle round it
/// on their own; the core pulses.
pub(crate) fn animate(
    time: Res<Time>,
    orbs: Query<&Orb>,
    mut pieces: Query<(&Piece, &mut Transform), Without<CoreLight>>,
    mut lights: Query<(&CoreLight, &mut PointLight)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let turn = |t: f32| Quat::from_axis_angle(Vec3::new(0.2, 1.0, 0.1).normalize(), t * 0.06);
    for (p, mut transform) in &mut pieces {
        let Ok(orb) = orbs.get(p.orb) else { continue };
        let d = p.rest.length() / orb.radius;
        let breathe = 1.0 + 0.035 * (t * 0.45 - d * 3.0 + p.phase * 0.02).sin();
        let shiver = Vec3::new((t * 7.0 + p.phase).sin(), (t * 5.3 + p.phase * 1.7).sin(), (t * 6.1 + p.phase * 0.6).sin()) * orb.radius * 0.002;
        let (at, rotation) = if p.adrift {
            let own = Quat::from_rotation_y(t * (0.05 + (p.phase % 1.0) * 0.08));
            let bob = Vec3::Y * (t * 0.3 + p.phase).sin() * orb.radius * 0.05;
            (own * p.rest + bob, own * Quat::from_rotation_x(t * 0.1 + p.phase) * p.rotation)
        } else {
            (turn(t) * (p.rest * breathe) + shiver, turn(t) * p.rotation)
        };
        transform.translation = orb.centre + at;
        transform.rotation = rotation;
    }
    // (The creature's beat: slow, uneven.)
    let beat = 0.55 + 0.45 * ((t * 1.6).sin() * 0.7 + (t * 0.53).sin() * 0.3).max(-1.0);
    for (core, mut light) in &mut lights {
        let Ok(orb) = orbs.get(core.0) else { continue };
        light.intensity = CORE_LIGHT * beat;
        if let Some(mut m) = materials.get_mut(&orb.glow) {
            m.emissive = LinearRgba::gray(GLOW * beat);
        }
    }
}
