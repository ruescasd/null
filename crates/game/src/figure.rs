//! An experiment: a humanoid made of the world's fractal language. A simple
//! procedural skeleton walks on the real terrain; each bone's volume is
//! filled by a fractal lattice of small boxes (the same rule as the
//! structures), and every box springs towards its place on its bone instead
//! of being fixed to it, so the body is held together rather than solid: it
//! trails, sways and shivers.
//!
//! The walk is procedural: a foot stays planted until it is too far from
//! where it should be, then steps there along an arc; legs bend with
//! two-bone IK, arms swing against the legs. For now the figure walks
//! towards the player and stops a few metres away (`--opt nofigures`).

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use worldgen::{
    ifs::{self, Block, Keep, Rule},
    noise::hash01,
};

use crate::{Args, camera::FlyCam, terrain::WorldGen};

pub struct FigurePlugin;

impl Plugin for FigurePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn)
            .add_systems(Update, (walk, follow).chain().after(crate::terrain::StreamSet));
    }
}

const WALK_SPEED: f32 = 1.4;
/// A foot steps when it is this far from where it should be.
const STEP_TRIGGER: f32 = 0.55;
const STEP_TIME: f32 = 0.38;
const STEP_HEIGHT: f32 = 0.3;
const THIGH: f32 = 0.88;
const SHIN: f32 = 0.86;
const HIP_HEIGHT: f32 = 1.62;
const HIP_WIDTH: f32 = 0.17;
const SHOULDER_WIDTH: f32 = 0.3;
const UPPER_ARM: f32 = 0.66;
const FOREARM: f32 = 0.62;
/// How close the figure comes before stopping.
const KEEP_AWAY: f32 = 5.0;
/// Spring holding each element to its place: stiffness and damping.
const STIFFNESS: f32 = 220.0;
const DAMPING: f32 = 18.0;

/// Body parts, each a box along a bone from joint `a` to joint `b`:
/// (name, width, depth, lattice depth).
#[derive(Clone, Copy)]
enum Bone {
    Pelvis,
    Torso,
    Neck,
    Head,
    UpperArm(Side),
    Forearm(Side),
    Thigh(Side),
    Shin(Side),
    Foot(Side),
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

impl Side {
    fn sign(self) -> f32 {
        if self == Side::Left { -1.0 } else { 1.0 }
    }
}

const BONES: [Bone; 15] = [
    Bone::Pelvis,
    Bone::Torso,
    Bone::Neck,
    Bone::Head,
    Bone::UpperArm(Side::Left),
    Bone::UpperArm(Side::Right),
    Bone::Forearm(Side::Left),
    Bone::Forearm(Side::Right),
    Bone::Thigh(Side::Left),
    Bone::Thigh(Side::Right),
    Bone::Shin(Side::Left),
    Bone::Shin(Side::Right),
    Bone::Foot(Side::Left),
    Bone::Foot(Side::Right),
    // A second, thinner layer around the torso: the "ribcage".
    Bone::Torso,
];

impl Bone {
    /// Width and depth of the part, and the lattice rule filling it.
    fn shape(self) -> (f32, f32, u32) {
        match self {
            Bone::Pelvis => (0.56, 0.38, 2),
            Bone::Torso => (0.7, 0.44, 2),
            Bone::Neck => (0.16, 0.16, 1),
            Bone::Head => (0.38, 0.46, 2),
            Bone::UpperArm(_) => (0.22, 0.22, 2),
            Bone::Forearm(_) => (0.18, 0.18, 2),
            Bone::Thigh(_) => (0.3, 0.3, 2),
            Bone::Shin(_) => (0.22, 0.22, 2),
            Bone::Foot(_) => (0.18, 0.14, 1),
        }
    }
}

/// A bone's pose: where its part starts, the part's length, and its frame
/// (local +y runs along the bone).
#[derive(Clone, Copy, Default)]
struct Pose {
    start: Vec3,
    rotation: Quat,
    length: f32,
}

#[derive(Clone, Copy)]
struct Foot {
    planted: Vec3,
    from: Vec3,
    to: Vec3,
    /// Progress of a step in 0..1, or None while planted.
    step: Option<f32>,
}

#[derive(Component)]
struct Figure {
    position: Vec3,
    heading: f32,
    speed: f32,
    feet: [Foot; 2],
    poses: Vec<Pose>,
    clock: f32,
}

/// One box of the body, held to a place on a bone by a spring.
#[derive(Component)]
struct Element {
    bone: usize,
    /// Centre in the bone's frame, with y measured as a fraction of the
    /// bone's length so parts stretch with the skeleton.
    offset: Vec3,
    rotation: Quat,
    velocity: Vec3,
    /// A per-element phase for the shiver.
    phase: f32,
}

fn spawn(
    mut commands: Commands,
    args: Res<Args>,
    world: Res<WorldGen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if args.opt("nofigures") {
        return;
    }
    // About 22 m ahead of the default spawn point, looking back at it.
    let (x, z) = (1214.0, 883.0);
    let ground = world.ground_height(x, z);
    let position = Vec3::new(x, ground, z);
    let heading = 140f32.to_radians();
    let forward = Vec3::new(heading.sin(), 0.0, heading.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let foot = |side: f32| {
        let p = position + right * side * HIP_WIDTH;
        let p = Vec3::new(p.x, world.ground_height(p.x, p.z), p.z);
        Foot { planted: p, from: p, to: p, step: None }
    };

    // Fragments are cubes, wedges (triangular prisms) and pointed shards,
    // all spanning -1..1 so a fragment's scale is its half extents.
    let shapes = [
        meshes.add(Cuboid::new(2.0, 2.0, 2.0)),
        meshes.add(faceted(&[
            [-1., -1., -1.],
            [1., -1., -1.],
            [-1., 1., -1.],
            [-1., -1., 1.],
            [1., -1., 1.],
            [-1., 1., 1.],
        ], &[&[0, 2, 1], &[3, 4, 5], &[0, 1, 4, 3], &[0, 3, 5, 2], &[1, 2, 5, 4]])),
        meshes.add(faceted(
            &[[-1., -1., -1.], [1., -1., -0.6], [-0.2, -1., 1.], [0.3, 1., 0.1]],
            &[&[0, 1, 2], &[0, 3, 1], &[1, 3, 2], &[2, 3, 0]],
        )),
    ];
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.03, 0.03),
        perceptual_roughness: 0.35,
        reflectance: 0.7,
        ..default()
    });

    let figure = Figure {
        position,
        heading,
        speed: 0.0,
        feet: [foot(-1.0), foot(1.0)],
        poses: vec![Pose::default(); BONES.len()],
        clock: 0.0,
    };
    commands.spawn((figure, Transform::default(), Visibility::default()));

    // Fill every part with a lattice of boxes, in a unit-length frame.
    let mut count = 0;
    for (index, bone) in BONES.iter().enumerate() {
        let (width, depth, levels) = bone.shape();
        let ribcage = index == BONES.len() - 1;
        let (width, depth) = if ribcage { (width * 1.35, depth * 1.6) } else { (width, depth) };
        // Parts are generated one metre long and stretched to the bone.
        // A random fill with chunks of every size: some blocks stop splitting
        // early, the rest break into smaller pieces, with visible gaps.
        let rule = Rule {
            divisions: [2, 3, 2],
            keep: if ribcage { Keep::Random(0.3) } else { Keep::Random(0.7) },
            depth: levels,
            gap: if ribcage { 0.5 } else { 0.68 },
            twist: Quat::IDENTITY,
            stop_chance: 0.35,
            lift: 0.0,
            min_size: 0.0,
        };
        let root = Block {
            center: Vec3::Y * 0.5,
            rotation: Quat::IDENTITY,
            half: Vec3::new(width * 0.5, 0.5, depth * 0.5),
            level: 0,
        };
        for (n, leaf) in ifs::generate(&rule, root, 41 + index as u32, 600).iter().enumerate() {
            let b = leaf.block;
            // Each fragment turned and shifted a little: fragments, not a grid.
            let r = |k: i32| hash01(index as i32, n as i32, k, 0xf16) - 0.5;
            let tilt = Quat::from_euler(EulerRot::YXZ, r(1) * 0.6, r(2) * 0.4, r(3) * 0.4);
            let nudge = Vec3::new(r(4), r(5) * 0.3, r(6)) * b.half * 0.6;
            commands.spawn((
                Element {
                    bone: index,
                    offset: b.center + nudge,
                    rotation: b.rotation * tilt,
                    velocity: Vec3::ZERO,
                    phase: hash01(index as i32, n as i32, 3, 77) * 100.0,
                },
                Mesh3d(shapes[match hash01(index as i32, n as i32, 9, 0x5a9) {
                    x if x < 0.5 => 0,
                    x if x < 0.78 => 1,
                    _ => 2,
                }]
                .clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(position + Vec3::Y * 2.0).with_scale(b.half),
            ));
            count += 1;
        }
    }
    info!("figure: {count} elements");
}

/// Rotation whose +y runs along `along` and whose +x is as close to `right`
/// as possible.
fn frame(along: Vec3, right: Vec3) -> Quat {
    let y = along.normalize_or(Vec3::Y);
    let x = (right - y * right.dot(y)).normalize_or(Vec3::X);
    let z = x.cross(y);
    Quat::from_mat3(&Mat3::from_cols(x, y, z))
}

/// Two-bone IK: the knee (or elbow) between `root` and `end`, bending
/// towards `pole`.
fn knee(root: Vec3, end: Vec3, a: f32, b: f32, pole: Vec3) -> Vec3 {
    let to_end = end - root;
    let d = to_end.length().clamp(0.05, a + b - 1e-3);
    let dir = to_end.normalize_or(Vec3::NEG_Y);
    let cos = ((a * a + d * d - b * b) / (2.0 * a * d)).clamp(-1.0, 1.0);
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Z);
    root + dir * a * cos + bend * a * (1.0 - cos * cos).sqrt()
}

fn walk(
    time: Res<Time>,
    world: Res<WorldGen>,
    camera: Single<&Transform, (With<FlyCam>, Without<Figure>)>,
    mut figures: Query<&mut Figure>,
) {
    let dt = time.delta_secs().min(0.05);
    let ground = |p: Vec3| Vec3::new(p.x, world.ground_height(p.x, p.z), p.z);
    for mut f in &mut figures {
        f.clock += dt;
        // Head for the player; stop a few metres away.
        let to_player = camera.translation - f.position;
        let flat = Vec3::new(to_player.x, 0.0, to_player.z);
        let distance = flat.length();
        if distance > 0.1 {
            let target = flat.x.atan2(flat.z);
            let turn = (target - f.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            f.heading += turn.clamp(-1.2 * dt, 1.2 * dt);
        }
        let wanted = if distance > KEEP_AWAY && distance < 120.0 { WALK_SPEED } else { 0.0 };
        f.speed += (wanted - f.speed).clamp(-1.5 * dt, 1.5 * dt);
        let forward = Vec3::new(f.heading.sin(), 0.0, f.heading.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let step_to = f.position + forward * f.speed * dt;
        f.position = ground(step_to);

        // Feet: step when too far from where they belong; one at a time.
        for i in 0..2 {
            let other_stepping = f.feet[1 - i].step.is_some();
            let side = if i == 0 { -1.0 } else { 1.0 };
            let home = ground(f.position + right * side * HIP_WIDTH + forward * f.speed * 0.32);
            let foot = &mut f.feet[i];
            match foot.step {
                Some(t) => {
                    let t = (t + dt / STEP_TIME).min(1.0);
                    let arc = (t * std::f32::consts::PI).sin() * STEP_HEIGHT;
                    foot.planted = foot.from.lerp(foot.to, t) + Vec3::Y * arc;
                    foot.step = (t < 1.0).then_some(t);
                }
                None if !other_stepping && foot.planted.distance(home) > STEP_TRIGGER => {
                    foot.from = foot.planted;
                    foot.to = home;
                    foot.step = Some(0.0);
                }
                None => {}
            }
        }

        // Pose the skeleton.
        let lift = f.feet.iter().map(|ft| ft.step.map_or(0.0, |t| (t * std::f32::consts::PI).sin())).sum::<f32>();
        let bob = -0.06 * (1.0 - lift.min(1.0)) * (f.speed / WALK_SPEED);
        let lean = 0.12 * f.speed / WALK_SPEED;
        let pelvis = f.position + Vec3::Y * (HIP_HEIGHT + bob);
        let up = (Vec3::Y + forward * lean).normalize();
        let chest = pelvis + up * 0.78;
        let neck = chest + up * 0.3;
        let head = neck + (Vec3::Y + forward * 0.25).normalize() * 0.06;
        let swing = |side: f32| {
            // Arms swing against the leg on the same side.
            let leg = if side < 0.0 { 0 } else { 1 };
            let reach = (f.feet[leg].planted - f.position).dot(forward);
            -reach * 0.6
        };
        let mut poses = Vec::with_capacity(BONES.len());
        for bone in BONES {
            let pose = |a: Vec3, b: Vec3| Pose { start: a, rotation: frame(b - a, right), length: a.distance(b) };
            let p = match bone {
                Bone::Pelvis => pose(pelvis - up * 0.14, pelvis + up * 0.14),
                Bone::Torso => pose(pelvis + up * 0.1, chest),
                Bone::Neck => pose(chest, neck),
                Bone::Head => pose(head, head + (Vec3::Y + forward * 0.2).normalize() * 0.44),
                Bone::UpperArm(side) | Bone::Forearm(side) => {
                    let s = side.sign();
                    let shoulder = chest + right * s * SHOULDER_WIDTH - up * 0.05;
                    let hang = (Vec3::NEG_Y + forward * swing(s) + right * s * 0.12).normalize();
                    let hand = shoulder + hang * (UPPER_ARM + FOREARM) * 0.95;
                    let elbow = knee(shoulder, hand, UPPER_ARM, FOREARM, -forward);
                    if matches!(bone, Bone::UpperArm(_)) { pose(shoulder, elbow) } else { pose(elbow, hand) }
                }
                Bone::Thigh(side) | Bone::Shin(side) | Bone::Foot(side) => {
                    let s = side.sign();
                    let leg = if s < 0.0 { 0 } else { 1 };
                    let hip = pelvis + right * s * HIP_WIDTH;
                    let ankle = f.feet[leg].planted + Vec3::Y * 0.1;
                    let knee_at = knee(hip, ankle, THIGH, SHIN, forward);
                    match bone {
                        Bone::Thigh(_) => pose(hip, knee_at),
                        Bone::Shin(_) => pose(knee_at, ankle),
                        _ => Pose {
                            start: ankle - Vec3::Y * 0.1 - forward * 0.06,
                            rotation: frame(forward, right) * Quat::from_rotation_x(0.0),
                            length: 0.3,
                        },
                    }
                }
            };
            poses.push(p);
        }
        f.poses = poses;
    }
}

/// Moves every element towards its place on its bone through a spring, with
/// a faint shiver.
fn follow(time: Res<Time>, figure: Single<&Figure>, mut elements: Query<(&mut Element, &mut Transform)>) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (mut e, mut transform) in &mut elements {
        let pose = figure.poses[e.bone];
        if pose.length <= 0.0 {
            continue;
        }
        let local = Vec3::new(e.offset.x, e.offset.y * pose.length, e.offset.z);
        let shiver = Vec3::new((t * 7.0 + e.phase).sin(), (t * 5.3 + e.phase * 1.7).sin(), (t * 6.1 + e.phase * 0.6).sin()) * 0.004;
        let target = pose.start + pose.rotation * local + shiver;
        let to = target - transform.translation;
        // Far away (first frame, or teleported): snap.
        if to.length() > 3.0 {
            transform.translation = target;
            e.velocity = Vec3::ZERO;
        } else {
            let accel = to * STIFFNESS - e.velocity * DAMPING;
            e.velocity += accel * dt;
            transform.translation += e.velocity * dt;
        }
        let rotation = pose.rotation * e.rotation;
        transform.rotation = transform.rotation.slerp(rotation, (dt * 14.0).min(1.0));
    }
}

/// A flat-shaded convex mesh from corner points and faces (polygons of
/// indices); each face is turned to point away from the shape's centre.
fn faceted(points: &[[f32; 3]], faces: &[&[usize]]) -> Mesh {
    let center = points.iter().map(|&p| Vec3::from(p)).sum::<Vec3>() / points.len() as f32;
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for face in faces {
        let mut corners: Vec<Vec3> = face.iter().map(|&i| Vec3::from(points[i])).collect();
        let mut normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
        let middle = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
        if normal.dot(middle - center) < 0.0 {
            corners.reverse();
            normal = -normal;
        }
        let base = positions.len() as u32;
        for c in &corners {
            positions.push(c.to_array());
            normals.push(normal.to_array());
        }
        for k in 1..corners.len() as u32 - 1 {
            indices.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}
