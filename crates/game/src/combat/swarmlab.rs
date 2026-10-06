//! The swarmer lab (`--opt swarmlab`): candidate forms for the swarmer, side
//! by side in an arc in front of you, each with a form at rest and a form when
//! looked at. Look at one and it turns into its second form; look away and it
//! relaxes. `--set gaze=0..1` holds them all at one point between the two (for
//! captures), `--set pick=N` shows only candidate N, close, and `--set
//! gloom=0..1` holds the swarm's darkness.
//!
//! 1. The bud: black petals closed round the core; looked at, they open like
//!    an eye, pale inner petals and the light inside.
//! 2. The face: a scatter of pale facets, turned away; looked at, it turns to
//!    you and the facets fall into a face, eyes lit deep under the brows.
//! 3. The hand: long jointed fingers curled shut round a light; looked at,
//!    they open and reach for you.
//! 4. The chandelier: needles hanging from a thread round a light; looked at,
//!    every needle turns its point to you.
//! 5. The figure: something standing, thin, too long in the arm, head bowed;
//!    looked at, the head lifts and tilts, one light where a face would be.

use bevy::prelude::*;
use worldgen::noise::hash01;

use crate::{
    Args,
    camera::FlyCam,
    player::tether::shard_mesh,
    terrain::{Streamer, WorldGen},
};

/// A candidate: how far it has turned into its second form.
#[derive(Component)]
pub(super) struct Candidate {
    gaze: f32,
}

/// A piece moving between its rest and looked-at forms (in its parent's
/// space).
#[derive(Component)]
pub(super) struct Morph {
    of: Entity,
    rest: Transform,
    gazed: Transform,
}

/// A light brightening as its candidate is looked at.
#[derive(Component)]
pub(super) struct Glow {
    of: Entity,
    material: Handle<StandardMaterial>,
    rest: f32,
    gazed: f32,
}

/// What the candidates are made of.
struct Kit {
    /// A blade: a flat four-sided point along +Y, unit length and half-width
    /// (scaled to size), with a short point back the other way to close it.
    blade: Handle<Mesh>,
    /// A facet: a flat diamond in the XY plane, unit half-sizes.
    facet: Handle<Mesh>,
    core: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
    pale: Handle<StandardMaterial>,
}

/// A rotation taking +Y to `dir`, with +X along `across`.
fn aim(dir: Vec3, across: Vec3) -> Quat {
    let y = dir.normalize();
    let x = (across - y * across.dot(y)).normalize_or(y.any_orthonormal_vector());
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lab(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    world: Res<WorldGen>,
    streamer: Res<Streamer>,
    camera: Single<&Transform, With<FlyCam>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gloom: ResMut<crate::look::Gloom>,
    mut spawned: Local<bool>,
    mut settled_for: Local<f32>,
    mut candidates: Query<(&GlobalTransform, &mut Candidate)>,
    mut morphs: Query<(&Morph, &mut Transform), Without<FlyCam>>,
    glows: Query<&Glow>,
) {
    if !args.opt("swarmlab") {
        return;
    }
    let held = args.num("gloom", -1.0);
    if held >= 0.0 {
        gloom.0 = held;
    }
    if !*spawned {
        // (A moment after loading, once you have come to rest.)
        *settled_for = if streamer.settled { *settled_for + time.delta_secs() } else { 0.0 };
        if *settled_for < 1.0 {
            return;
        }
        *spawned = true;
        let kit = Kit {
            blade: meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 1.0), (Vec3::NEG_Y, 0.08, 1.0)])),
            facet: meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 1.0), (Vec3::NEG_Y, 1.0, 1.0)])),
            core: meshes.add(Sphere::new(0.13).mesh().ico(1).unwrap()),
            dark: materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.35, reflectance: 0.6, ..default() }),
            pale: materials.add(StandardMaterial { base_color: Color::srgb(0.72, 0.72, 0.72), perceptual_roughness: 0.7, ..default() }),
        };
        let eye = camera.translation;
        let ahead = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::NEG_Z);
        let pick = args.num("pick", 0.0) as usize;
        for k in 1..=5 {
            if pick != 0 && pick != k {
                continue;
            }
            let (angle, distance) = if pick != 0 { (0.0, if k == 5 { 3.0 } else { 1.6 }) } else { ((k as f32 - 3.0) * 0.36, 4.0) };
            let flat = Quat::from_rotation_y(-angle) * ahead;
            // At eye height (the figure standing on the ground).
            let mut at = eye + flat * distance - Vec3::Y * 0.1;
            if k == 5 {
                at.y = world.ground_height(at.x, at.z);
            }
            // Facing you (the figure only turned, upright).
            let face = if k == 5 { Vec3::new(eye.x, at.y, eye.z) } else { eye };
            let root = commands.spawn((Candidate { gaze: 0.0 }, Transform::from_translation(at).looking_at(face, Vec3::Y), Visibility::default())).id();
            let glow = |materials: &mut Assets<StandardMaterial>, rest: f32| {
                materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: LinearRgba::rgb(rest, rest, rest),
                    fog_enabled: false,
                    ..default()
                })
            };
            match k {
                1 => bud(&mut commands, &kit, root, glow(&mut materials, 600.0)),
                2 => face_(&mut commands, &kit, root, glow(&mut materials, 10.0)),
                3 => hand(&mut commands, &kit, root, glow(&mut materials, 150.0)),
                4 => chandelier(&mut commands, &kit, root, glow(&mut materials, 2500.0)),
                _ => figure(&mut commands, &kit, root, glow(&mut materials, 30.0)),
            }
        }
        return;
    }

    // How far each has turned: held, or following your gaze (quick to turn,
    // slow to relax).
    let dt = time.delta_secs().min(0.05);
    let fixed = args.num("gaze", -1.0);
    for (transform, mut c) in &mut candidates {
        if fixed >= 0.0 {
            c.gaze = fixed;
            continue;
        }
        let to = transform.translation() + Vec3::Y * 0.2 - camera.translation;
        let looked = to.length() < 15.0 && to.normalize().dot(*camera.forward()) > 0.97;
        c.gaze = if looked { (c.gaze + dt / 1.5).min(1.0) } else { (c.gaze - dt / 2.5).max(0.0) };
    }
    for (m, mut transform) in &mut morphs {
        let Ok((_, c)) = candidates.get(m.of) else { continue };
        let g = c.gaze * c.gaze * (3.0 - 2.0 * c.gaze);
        transform.translation = m.rest.translation.lerp(m.gazed.translation, g);
        transform.rotation = m.rest.rotation.slerp(m.gazed.rotation, g);
        transform.scale = m.rest.scale.lerp(m.gazed.scale, g);
    }
    for glow in &glows {
        let Ok((_, c)) = candidates.get(glow.of) else { continue };
        if let Some(mut material) = materials.get_mut(&glow.material) {
            let e = glow.rest + (glow.gazed - glow.rest) * c.gaze;
            material.emissive = LinearRgba::rgb(e, e, e);
        }
    }
}

/// A piece under `parent`, moving between two forms.
#[allow(clippy::too_many_arguments)]
fn piece(commands: &mut Commands, parent: Entity, of: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, rest: Transform, gazed: Transform) -> Entity {
    commands.spawn((Morph { of, rest, gazed }, Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), rest, ChildOf(parent))).id()
}

/// A node with nothing drawn, moving between two forms (a joint, a neck).
fn joint(commands: &mut Commands, parent: Entity, of: Entity, rest: Transform, gazed: Transform) -> Entity {
    commands.spawn((Morph { of, rest, gazed }, rest, Visibility::default(), ChildOf(parent))).id()
}

fn light(commands: &mut Commands, kit: &Kit, parent: Entity, of: Entity, material: Handle<StandardMaterial>, rest: f32, gazed: f32, at: Transform, grown: Transform) {
    commands.spawn((
        Morph { of, rest: at, gazed: grown },
        Glow { of, material: material.clone(), rest, gazed },
        Mesh3d(kit.core.clone()),
        MeshMaterial3d(material),
        at,
        ChildOf(parent),
        bevy::light::NotShadowCaster,
    ));
}

/// Its back is to you (+Z away); the petals' bases sit behind the core.
fn bud(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    light(commands, kit, root, root, glow, 300.0, 3000.0, Transform::from_scale(Vec3::splat(0.7)), Transform::from_scale(Vec3::splat(0.85)));
    for (count, length, width, material, closed, open, base) in [
        (7, 0.36, 0.09, &kit.dark, 0.38, 0.35, 0.16),
        (5, 0.24, 0.06, &kit.pale, 0.5, -0.3, 0.09),
    ] {
        for i in 0..count {
            let a = (i as f32 + if material == &kit.pale { 0.5 } else { 0.0 }) / count as f32 * std::f32::consts::TAU;
            let (r, t) = (Vec3::new(a.cos(), a.sin(), 0.0), Vec3::new(-a.sin(), a.cos(), 0.0));
            let scale = Vec3::new(width, length, 0.012);
            // (The inner ones tucked in, shorter, while closed.)
            let tucked = if material == &kit.pale { scale * Vec3::new(1.0, 0.55, 1.0) } else { scale };
            let rest = Transform::from_translation(r * base + Vec3::Z * 0.08).with_rotation(aim(Vec3::NEG_Z - r * closed, t)).with_scale(tucked);
            let gazed = Transform::from_translation(r * base * 0.8 + Vec3::Z * 0.06).with_rotation(aim(r + Vec3::Z * open, t)).with_scale(scale);
            piece(commands, root, root, &kit.blade, material, rest, gazed);
        }
    }
    // A short stalk of spines behind.
    for i in 0..3 {
        let a = i as f32 / 3.0 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos() * 0.4, a.sin() * 0.4, 1.0);
        let t = Transform::from_translation(Vec3::Z * 0.1).with_rotation(aim(d, Vec3::X)).with_scale(Vec3::new(0.02, 0.4, 0.02));
        piece(commands, root, root, &kit.blade, &kit.dark, t, t);
    }
}

/// Turned away, it is a scatter of pale shards on a dark knot; turned to you,
/// they fall into a face.
fn face_(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    let turn = joint(commands, root, root, Transform::from_rotation(Quat::from_rotation_y(2.4)), Transform::IDENTITY);
    // The knot behind the face: spines thrown back like hair.
    for i in 0..7 {
        let r = |j: i32| hash01(i, j, 2, 0x5d1) - 0.5;
        let d = Vec3::new(r(0) * 1.4, r(1) * 1.2 + 0.2, 1.0);
        let t = Transform::from_translation(Vec3::Z * 0.02).with_rotation(aim(d, Vec3::X)).with_scale(Vec3::new(0.03, 0.3 + r(2) * 0.2, 0.02));
        piece(commands, turn, root, &kit.blade, &kit.dark, t, t);
    }
    // (centre, half-width, half-height, roll); mirrored ones once each side.
    let facets: [(Vec3, f32, f32, f32); 11] = [
        (Vec3::new(0.0, 0.15, -0.06), 0.11, 0.05, 0.0),
        (Vec3::new(0.065, 0.075, -0.1), 0.055, 0.018, -0.3),
        (Vec3::new(-0.065, 0.075, -0.1), 0.055, 0.018, 0.3),
        (Vec3::new(0.075, -0.03, -0.09), 0.04, 0.06, 0.35),
        (Vec3::new(-0.075, -0.03, -0.09), 0.04, 0.06, -0.35),
        (Vec3::new(0.0, 0.01, -0.15), 0.016, 0.06, 0.0),
        (Vec3::new(0.0, -0.12, -0.07), 0.06, 0.035, 0.0),
        (Vec3::new(0.06, -0.1, -0.04), 0.03, 0.05, 0.6),
        (Vec3::new(-0.06, -0.1, -0.04), 0.03, 0.05, -0.6),
        (Vec3::new(0.105, 0.06, -0.03), 0.02, 0.06, 0.15),
        (Vec3::new(-0.105, 0.06, -0.03), 0.02, 0.06, -0.15),
    ];
    for (i, &(at, w, h, roll)) in facets.iter().enumerate() {
        let r = |j: i32| hash01(i as i32, j, 3, 0x5d2) - 0.5;
        let scale = Vec3::new(w, h, 0.008);
        let gazed = Transform::from_translation(at).with_rotation(Quat::from_rotation_z(roll)).with_scale(scale);
        let rest = Transform::from_translation(at + Vec3::new(r(0) * 0.24, r(1) * 0.24, r(2) * 0.3))
            .with_rotation(Quat::from_euler(EulerRot::XYZ, r(3) * 1.6, r(4) * 1.6, roll + r(5) * 1.6))
            .with_scale(scale);
        piece(commands, turn, root, &kit.facet, &kit.pale, rest, gazed);
    }
    // Eyes, deep under the brows.
    for side in [-1.0, 1.0] {
        let t = Transform::from_xyz(side * 0.045, 0.045, -0.05).with_scale(Vec3::splat(0.12));
        light(commands, kit, turn, root, glow.clone(), 10.0, 2500.0, t, t);
    }
}

/// The palm faces you; the fingers curl shut over the light in it, or open
/// and reach.
fn hand(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    let core = Transform::from_xyz(0.0, 0.02, -0.02).with_scale(Vec3::splat(0.4));
    light(commands, kit, root, root, glow, 150.0, 3000.0, core, core.with_scale(Vec3::splat(0.55)));
    // The palm: a few blades; the arm: one long one back and down, from
    // nowhere.
    for (d, len, w) in [(Vec3::new(0.3, 0.2, 1.0), 0.12, 0.05), (Vec3::new(-0.3, 0.1, 1.0), 0.12, 0.05), (Vec3::new(0.0, -0.4, 1.0), 0.6, 0.035)] {
        let t = Transform::from_rotation(aim(d, Vec3::X)).with_scale(Vec3::new(w, len, 0.03));
        piece(commands, root, root, &kit.blade, &kit.dark, t, t);
    }
    // Four fingers and a thumb: (base, length factor, spread; thumb).
    for (i, (x, factor, thumb)) in [(-0.045, 0.85, false), (-0.015, 1.0, false), (0.015, 0.95, false), (0.045, 0.75, false), (0.07, 0.6, true)].into_iter().enumerate() {
        let base = if thumb { Vec3::new(x, -0.03, -0.01) } else { Vec3::new(x, 0.06, 0.0) };
        let (rest_dir, open_dir) = if thumb {
            (Vec3::new(0.2, 0.5, -0.6), Vec3::new(1.0, -0.2, -0.4))
        } else {
            (Vec3::new(x * 0.5, 1.0, -0.3), Vec3::new(x * 10.0, 1.0, -0.45))
        };
        let curl = if thumb { -1.0 } else { -1.3 };
        let mut parent = joint(
            commands,
            root,
            root,
            Transform::from_translation(base).with_rotation(aim(rest_dir, Vec3::X)),
            Transform::from_translation(base).with_rotation(aim(open_dir, Vec3::X)),
        );
        let segments: &[f32] = if thumb { &[0.14, 0.1] } else { &[0.22, 0.17, 0.13] };
        for (j, &len) in segments.iter().enumerate() {
            let len = len * factor;
            let r = hash01(i as i32, j as i32, 4, 0x5d3) - 0.5;
            let shape = Transform::from_scale(Vec3::new(0.014, len, 0.012));
            piece(commands, parent, root, &kit.blade, &kit.pale, shape, shape);
            // The next joint, at this one's end.
            let rest = Transform::from_xyz(0.0, len, 0.0).with_rotation(Quat::from_rotation_x(curl + r * 0.3));
            let gazed = Transform::from_xyz(0.0, len, 0.0).with_rotation(Quat::from_rotation_x(-0.12 + r * 0.1));
            parent = joint(commands, parent, root, rest, gazed);
        }
        // A pale claw at the tip.
        let claw = Transform::from_scale(Vec3::new(0.009, 0.06, 0.009));
        piece(commands, parent, root, &kit.blade, &kit.pale, claw, claw);
    }
}

/// Needles hanging round a light from a thread up into the dark; looked at,
/// each turns its point to you.
fn chandelier(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    let core = Transform::from_scale(Vec3::splat(0.8));
    light(commands, kit, root, root, glow, 1500.0, 5000.0, core, core.with_scale(Vec3::splat(0.9)));
    let thread = Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::new(0.004, 2.0, 0.004));
    piece(commands, root, root, &kit.blade, &kit.dark, thread, thread);
    let mut n = 0;
    for (ring, (radius, count)) in [(0.05, 6), (0.13, 9), (0.21, 11)].into_iter().enumerate() {
        for i in 0..count {
            n += 1;
            let r = |j: i32| hash01(n, j, 5, 0x5d4) - 0.5;
            let a = (i as f32 + r(0) * 0.6) / count as f32 * std::f32::consts::TAU;
            let at = Vec3::new(a.cos() * radius, -0.02 - ring as f32 * 0.05 + r(1) * 0.04, a.sin() * radius);
            let len = 0.22 + ring as f32 * 0.07 + r(2) * 0.1;
            let scale = Vec3::new(0.012, len, 0.012);
            let hang = Vec3::new(r(3) * 0.15, -1.0, r(4) * 0.15);
            // Bristling: out from the middle, and towards you.
            let point = Vec3::new(at.x, 0.0, at.z).normalize_or(Vec3::X) + Vec3::new(0.0, -0.15, -0.7);
            let rest = Transform::from_translation(at).with_rotation(aim(hang, Vec3::X)).with_scale(scale);
            let gazed = Transform::from_translation(at).with_rotation(aim(point, Vec3::X)).with_scale(scale);
            piece(commands, root, root, &kit.blade, if n % 3 == 0 { &kit.pale } else { &kit.dark }, rest, gazed);
        }
    }
}

/// Standing on the ground, facing you: legs, a thin column of a body, a bar
/// of shoulders, arms hanging to the knee, a head bowed on its neck.
fn figure(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    let blade = |from: Vec3, to: Vec3, w: f32| {
        Transform::from_translation(from).with_rotation(aim(to - from, Vec3::X)).with_scale(Vec3::new(w, from.distance(to), w * 0.8))
    };
    for t in [
        blade(Vec3::new(0.09, 0.0, 0.0), Vec3::new(0.02, 0.92, 0.0), 0.02),
        blade(Vec3::new(-0.09, 0.0, 0.0), Vec3::new(-0.02, 0.92, 0.0), 0.02),
        blade(Vec3::new(0.0, 0.86, 0.0), Vec3::new(0.0, 1.5, 0.0), 0.045),
        blade(Vec3::new(0.0, 1.0, 0.01), Vec3::new(0.03, 1.46, 0.0), 0.03),
        blade(Vec3::new(-0.19, 1.47, 0.0), Vec3::new(0.19, 1.47, 0.0), 0.02),
        blade(Vec3::new(0.18, 1.47, 0.0), Vec3::new(0.24, 0.55, 0.02), 0.018),
        blade(Vec3::new(-0.18, 1.47, 0.0), Vec3::new(-0.24, 0.55, 0.02), 0.018),
    ] {
        piece(commands, root, root, &kit.blade, &kit.pale, t, t);
    }
    // The head: bowed at rest; looked at, it lifts and tilts.
    let neck = joint(
        commands,
        root,
        root,
        Transform::from_xyz(0.0, 1.5, 0.0).with_rotation(Quat::from_rotation_x(-0.85)),
        Transform::from_xyz(0.0, 1.5, 0.0).with_rotation(Quat::from_rotation_x(0.1) * Quat::from_rotation_z(0.5)),
    );
    let stalk = Transform::from_scale(Vec3::new(0.015, 0.14, 0.015));
    piece(commands, neck, root, &kit.blade, &kit.pale, stalk, stalk);
    for i in 0..6 {
        let r = |j: i32| hash01(i, j, 6, 0x5d5) - 0.5;
        let d = Vec3::new(r(0) * 0.8, 1.0, r(1) * 0.8 + 0.2);
        let t = Transform::from_xyz(0.0, 0.12, 0.0).with_rotation(aim(d, Vec3::X)).with_scale(Vec3::new(0.06, 0.24 + r(2) * 0.06, 0.05));
        piece(commands, neck, root, &kit.blade, &kit.pale, t, t);
    }
    let eye = Transform::from_xyz(0.0, 0.22, -0.045).with_scale(Vec3::splat(0.3));
    light(commands, kit, neck, root, glow, 30.0, 3000.0, eye, eye);
}
