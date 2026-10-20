//! The stalker's gun (the user: an oversized carried weapon, against its
//! fragile look, strange and frightening; it fires a beam, the grid's
//! comet). Carried resting on its shoulder, shouldered to fire, its other
//! hand under the barrel (on a monopod, it did not work, the user), with a
//! part that spins up as it charges, and a glass that glints as it points
//! at you, the warning. Heavy-duty and worn from use, never makeshift
//! (as the shotgun: dark metal, grease where it turns, soot at the muzzle,
//! engineered parts). Built once (`build`), in its own frame: the grip at
//! the origin, +Y up, -Z towards the muzzle; placed each frame from the
//! stalker's rig (`place`, see `rig::GunFrame`).
//!
//! Designs, to judge (`--set lance=N`; the first, a long box receiver and
//! shroud some 3.6 m long, too large even for oversized and too blocky, the
//! user): 0 that gun a quarter shorter; 1 tines, four long blades round the
//! beam's square cavity; 2 a spine, a rod threaded with vertebrae; 3 a
//! truss, three rails braced to a point; 4 lenses, rings held between two
//! rails. Each some 2.6 m.

use bevy::prelude::*;

use super::hunter::Hunter;
use crate::rig::{GUN_BORE, GUN_FORE, GUN_HOLD};

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

/// The finishes and shapes the designs are made of.
struct Shop {
    cube: Handle<Mesh>,
    tube: Handle<Mesh>,
    round: Handle<Mesh>,
    hex: Handle<Mesh>,
    prong: Handle<Mesh>,
    ring: Handle<Mesh>,
    metal: Handle<StandardMaterial>,
    wrap: Handle<StandardMaterial>,
    grease: Handle<StandardMaterial>,
    worn: Handle<StandardMaterial>,
    soot: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
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

/// A ring of radius `r` round the bore's line `along` it, at height `y`.
fn ringed(r: f32, along: f32, y: f32) -> Transform {
    Transform::from_translation(at(along, y, 0.0)).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::splat(r))
}

/// A point (a four-sided cone) `long`, `r` round, from `along` forward.
fn point(along: f32, y: f32, r: f32, long: f32) -> Transform {
    Transform::from_translation(at(along + long * 0.5, y, 0.0)).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(r, long, r))
}

/// A glass of radius `r` facing forward, `along` it at height `y`.
fn glass(r: f32, along: f32, y: f32) -> Transform {
    Transform::from_translation(at(along, y, 0.0)).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(r, 0.01, r))
}

/// How far a design's tip is along its bore from the grip (m): where it
/// drags on the ground (see `rig::GunHold`).
pub(super) fn tip(design: u32) -> f32 {
    match design {
        1 => 2.18,
        2 => 2.25,
        3 => 2.24,
        4 => 2.0,
        _ => 2.25,
    }
}

/// Where in a design its shot leaves from, along its bore from the grip
/// (m): inside it, so the beam comes out of it (from the prongs' tips, it
/// formed apart before the gun, the user): the four blades' pale needle at
/// the back of their cavity, the truss's crystal, behind the rings, in the
/// others' muzzles.
pub(super) fn emitter(design: u32) -> f32 {
    match design {
        1 => 0.64,
        2 => 1.95,
        3 => 0.62,
        4 => 0.35,
        _ => 1.9,
    }
}

/// Builds the gun's parts, as design `design` (see the module's notes).
pub(super) fn build(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, design: u32) -> Kit {
    // (The shotgun's finishes: dark but matte enough that its facets catch
    // the light; cord and rubber; grease, dark and glossy; worn bare metal;
    // soot; and the pale of the creatures' markings.)
    let shop = Shop {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        tube: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(8)),
        round: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16)),
        hex: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(6)),
        prong: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(4)),
        ring: meshes.add(Torus::new(0.84, 1.0)),
        metal: materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.12, 0.12), perceptual_roughness: 0.62, reflectance: 0.4, ..default() }),
        wrap: materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.95, reflectance: 0.2, ..default() }),
        grease: materials.add(StandardMaterial { base_color: Color::srgb(0.035, 0.035, 0.035), perceptual_roughness: 0.12, reflectance: 0.7, ..default() }),
        worn: materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.15, 0.15), perceptual_roughness: 0.75, reflectance: 0.35, ..default() }),
        soot: materials.add(StandardMaterial { base_color: Color::srgb(0.025, 0.025, 0.025), perceptual_roughness: 0.9, reflectance: 0.2, ..default() }),
        dark: materials.add(StandardMaterial { base_color: Color::BLACK, perceptual_roughness: 1.0, reflectance: 0.0, ..default() }),
        pale: materials.add(StandardMaterial { base_color: Color::srgb(0.82, 0.82, 0.8), perceptual_roughness: 0.5, ..default() }),
    };
    let (mut body, mut cage) = (Parts::new(), Parts::new());
    let lens = match design {
        1 => tines(&shop, &mut body, &mut cage),
        2 => spine(&shop, &mut body, &mut cage),
        3 => truss(&shop, &mut body, &mut cage),
        4 => lenses(&shop, &mut body, &mut cage),
        _ => boxed(&shop, &mut body, &mut cage),
    };
    // Every design: the lug its other hand holds, under the bore (the
    // tines' a clamp round them).
    if design != 1 {
        body.push((shop.cube.clone(), shop.worn.clone(), block(Vec3::new(0.05, GUN_HOLD, 0.07), GUN_FORE, GUN_BORE - GUN_HOLD * 0.5, 0.0, 0.0)));
    }
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

/// 0: the first gun, a quarter shorter: a box receiver with grooved plates,
/// a scope, a shroud, a cage of rails round a pale core, a crowned muzzle.
fn boxed(s: &Shop, body: &mut Parts, cage: &mut Parts) -> Transform {
    let (butt, receiver, shroud, spun, muzzle) = (-0.62, (-0.3, 0.45), (0.45, 1.35), (1.35, 1.85), 2.05);
    let mid = (receiver.0 + receiver.1) * 0.5;
    let long = receiver.1 - receiver.0;
    let mut part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, t: Transform| body.push((mesh.clone(), material.clone(), t));
    part(&s.cube, &s.metal, block(Vec3::new(0.08, 0.16, -butt + receiver.0), (butt + receiver.0) * 0.5, 0.09, 0.0, 0.0));
    part(&s.cube, &s.wrap, block(Vec3::new(0.11, 0.2, 0.05), butt, 0.09, 0.0, 0.0));
    part(&s.cube, &s.metal, block(Vec3::new(0.19, 0.22, long), mid, GUN_BORE, 0.0, 0.0));
    part(&s.cube, &s.metal, block(Vec3::new(0.13, 0.06, long * 0.92), mid, GUN_BORE - 0.14, 0.0, 0.0));
    for side in [-1.0f32, 1.0] {
        part(&s.cube, &s.metal, block(Vec3::new(0.012, 0.16, long * 0.7), mid + 0.04, GUN_BORE, side * 0.1, 0.0));
        for y in [-0.05, 0.0, 0.05] {
            part(&s.cube, &s.dark, block(Vec3::new(0.014, 0.01, long * 0.6), mid + 0.04, GUN_BORE + y, side * 0.101, 0.0));
        }
    }
    part(&s.cube, &s.wrap, block(Vec3::new(0.06, 0.22, 0.08), 0.0, -0.06, 0.0, -0.3));
    part(&s.tube, &s.metal, piped(0.075, shroud.0, shroud.1, GUN_BORE, 0.0));
    part(&s.round, &s.worn, piped(0.095, GUN_FORE - 0.06, GUN_FORE + 0.06, GUN_BORE, 0.0));
    part(&s.tube, &s.wrap, piped(0.018, receiver.1 - 0.08, spun.0, GUN_BORE - 0.05, 0.1));
    for a in [spun.0, spun.1] {
        part(&s.round, &s.grease, piped(0.1, a - 0.03, a + 0.03, GUN_BORE, 0.0));
    }
    part(&s.tube, &s.soot, piped(0.08, spun.1, muzzle, GUN_BORE, 0.0));
    for k in 0..6 {
        let a = (k as f32 + 0.5) / 6.0 * std::f32::consts::TAU;
        part(
            &s.prong,
            &s.soot,
            Transform::from_translation(at(muzzle + 0.1, GUN_BORE + a.sin() * 0.07, a.cos() * 0.07)).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(0.022, 0.2, 0.022)),
        );
    }
    let (scope, height, r) = ((-0.12, 0.35), 0.18, 0.04);
    part(&s.tube, &s.metal, piped(r, scope.0, scope.1, GUN_BORE + height, 0.0));
    part(&s.tube, &s.soot, piped(r * 1.2, scope.1 - 0.03, scope.1 + 0.03, GUN_BORE + height, 0.0));
    for a in [scope.0 + 0.08, scope.1 - 0.12] {
        part(&s.cube, &s.worn, block(Vec3::new(0.04, height - 0.1, 0.04), a, GUN_BORE + 0.11 + (height - 0.1) * 0.5, 0.0, 0.0));
    }
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        cage.push((s.tube.clone(), s.grease.clone(), piped(0.013, spun.0, spun.1, a.sin() * 0.08, a.cos() * 0.08)));
    }
    cage.push((s.round.clone(), s.pale.clone(), piped(0.032, spun.0, spun.1, 0.0, 0.0)));
    glass(r * 0.9, scope.1 + 0.035, GUN_BORE + height)
}

/// 1: tines. Four long flat blades round the bore, its top, bottom and
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

/// 2: a spine. One thin rod from the drum to the tip threaded with
/// vertebrae, hex discs shrinking towards the muzzle, that spin up as it
/// charges; the glass on its point.
fn spine(s: &Shop, body: &mut Parts, cage: &mut Parts) -> Transform {
    rear(s, body, 0.085, -0.22, 0.3);
    let tip = 2.05;
    body.push((s.round.clone(), s.metal.clone(), piped(0.02, 0.3, tip, GUN_BORE, 0.0)));
    body.push((s.round.clone(), s.worn.clone(), piped(0.045, GUN_FORE - 0.05, GUN_FORE + 0.05, GUN_BORE, 0.0)));
    body.push((s.prong.clone(), s.soot.clone(), point(tip, GUN_BORE, 0.035, 0.2)));
    // The vertebrae (clear of the hand's collar), every third greased,
    // closer and smaller towards the tip.
    let span = tip - 0.52;
    let (mut a, mut k) = (0.42, 0);
    while a < tip - 0.1 {
        let t = ((a - 0.42) / span).clamp(0.0, 1.0);
        if (a - GUN_FORE).abs() > 0.09 {
            cage.push((s.hex.clone(), if k % 3 == 2 { s.grease.clone() } else { s.metal.clone() }, piped(0.12 - 0.07 * t, a, a + 0.022, 0.0, 0.0)));
            k += 1;
        }
        a += 0.12 - 0.04 * t;
    }
    // A pale cord threaded through them, over the rod.
    body.push((s.tube.clone(), s.pale.clone(), piped(0.006, 0.4, tip - 0.1, GUN_BORE + 0.03, 0.0)));
    glass(0.012, tip + 0.205, GUN_BORE)
}

/// 3: a truss. Three rails round the bore converging to a point, braced in
/// a zigzag (a lattice, as its head); a pale crystal spinning inside its
/// widest bay; the glass at the point.
fn truss(s: &Shop, body: &mut Parts, cage: &mut Parts) -> Transform {
    rear(s, body, 0.08, -0.2, 0.35);
    let (from, tip, wide) = (0.35, 2.1, 0.12);
    let radius = |a: f32| wide * (1.0 - (a - from) / (tip - from)) + 0.008;
    let rail = |a: f32, k: usize| {
        let t = std::f32::consts::FRAC_PI_2 + k as f32 / 3.0 * std::f32::consts::TAU;
        at(a, GUN_BORE + t.sin() * radius(a), t.cos() * radius(a))
    };
    for k in 0..3 {
        body.push((s.tube.clone(), s.metal.clone(), strut(rail(from, k), rail(tip, k), 0.011)));
    }
    // The braces: from each rail to the next, zigzagging along.
    let bays = 9;
    for b in 0..bays {
        let (a0, a1) = (from + (tip - from) * b as f32 / bays as f32, from + (tip - from) * (b + 1) as f32 / bays as f32);
        for k in 0..3 {
            body.push((s.tube.clone(), s.worn.clone(), strut(rail(a0, k), rail(a1, (k + 1) % 3), 0.006)));
        }
    }
    // The hand's collar round the three rails, a point at the tip.
    body.push((s.ring.clone(), s.worn.clone(), ringed(radius(GUN_FORE) + 0.012, GUN_FORE, GUN_BORE)));
    body.push((s.prong.clone(), s.soot.clone(), point(tip, GUN_BORE, 0.02, 0.14)));
    // The crystal: two long four-sided points back to back, and its fins.
    let middle = 0.62;
    for way in [-1.0f32, 1.0] {
        cage.push((
            s.prong.clone(),
            s.pale.clone(),
            Transform::from_translation(at(middle + way * 0.12, 0.0, 0.0)).with_rotation(Quat::from_rotation_x(-way * std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(0.035, 0.24, 0.035)),
        ));
    }
    for k in 0..3 {
        let t = k as f32 / 3.0 * std::f32::consts::TAU;
        cage.push((s.cube.clone(), s.grease.clone(), Transform::from_translation(at(middle, t.sin() * 0.045, t.cos() * 0.045)).with_rotation(Quat::from_rotation_z(t)).with_scale(Vec3::new(0.03, 0.006, 0.16))));
    }
    glass(0.01, tip + 0.142, GUN_BORE)
}

/// 4: lenses. A row of rings shrinking towards the muzzle, held between two
/// rails, open between (an instrument taken apart); knobs on the rings that
/// spin; the glass in the front ring.
fn lenses(s: &Shop, body: &mut Parts, cage: &mut Parts) -> Transform {
    rear(s, body, 0.09, -0.2, 0.32);
    let rings = [(0.45, 0.12), (0.75, 0.105), (1.0, 0.09), (1.4, 0.075), (1.7, 0.062), (1.98, 0.05)];
    let side = 0.135;
    let rail = |a: f32| side * (0.8 - 0.25 * (a - 0.32) / (1.98 - 0.32));
    for x in [-1.0f32, 1.0] {
        body.push((s.tube.clone(), s.metal.clone(), strut(at(0.32, GUN_BORE, x * rail(0.32)), at(1.98, GUN_BORE, x * rail(1.98)), 0.012)));
    }
    for (a, r) in rings {
        body.push((s.ring.clone(), s.metal.clone(), ringed(r, a, GUN_BORE)));
        // Each held to the rails by a spoke either side.
        for x in [-1.0f32, 1.0] {
            body.push((s.tube.clone(), s.worn.clone(), strut(at(a, GUN_BORE, x * r), at(a, GUN_BORE, x * rail(a)), 0.008)));
        }
        // Knobs round it, spinning.
        for k in 0..3 {
            let t = (k as f32 + 0.25) / 3.0 * std::f32::consts::TAU;
            cage.push((s.cube.clone(), s.grease.clone(), block(Vec3::new(0.02, 0.02, 0.025), a, t.sin() * r, t.cos() * r, 0.0)));
        }
    }
    // A bar under the rings where the hand holds it, and soot round the
    // front ring.
    body.push((s.cube.clone(), s.worn.clone(), block(Vec3::new(side * 1.4, 0.02, 0.04), GUN_FORE, GUN_BORE - 0.06, 0.0, 0.0)));
    body.push((s.ring.clone(), s.soot.clone(), ringed(0.058, 2.0, GUN_BORE)));
    glass(0.035, 1.98, GUN_BORE)
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
            m.emissive = LinearRgba::rgb(1.0, 1.0, 1.0) * LENS_GLOW * facing;
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
