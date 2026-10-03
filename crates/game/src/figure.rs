//! An experiment: a creature made of the world's fractal language. A
//! procedural skeleton walks on the real terrain; each bone's volume is
//! filled by a fractal fill of small fragments (cubes, wedges and shards,
//! from the same rule as the structures), and every fragment springs towards
//! its place on its bone instead of being fixed to it, so the body is held
//! together rather than solid.
//!
//! Two anatomies share the same skeleton: a menacing humanoid (human
//! proportions, but hunched, head pushed forward and low, shoulders raised,
//! knees bent, arms forward with elbows out) and, with `--opt creature`, a
//! feral creature (long low neck and elongated skull, long clawed arms,
//! digitigrade legs). Every part tapers, and each is filled as a dense core
//! with irregular gaps plus a sparse outer layer of fragments, so the
//! outline frays.
//!
//! A core of faintly glowing shards sits deep in the chest and skull, with
//! a small light among them: the body is lit from inside, through its own
//! gaps, so it reads against the black sky from any side, and it pulses.
//!
//! The walk is procedural: a foot stays planted until it is too far from
//! where it should be, then steps there along an arc; legs bend with
//! two-bone IK. For now the figure walks towards the player and stops a few
//! metres away (`--opt nofigures`).

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
            .add_systems(Update, (walk, follow, pulse).chain().after(crate::terrain::StreamSet));
    }
}

const WALK_SPEED: f32 = 1.8;
/// A foot steps when it is this far from where it should be.
const STEP_TRIGGER: f32 = 0.8;
const STEP_TIME: f32 = 0.4;
const STEP_HEIGHT: f32 = 0.35;
/// Brightness of the glowing core (cd/m², before the pulse) and the light
/// output of the lamp inside the chest (lumens).
const GLOW: f32 = 20000.0;
const CORE_LIGHT: f32 = 60000.0;

/// Which body plan the figure has.
#[derive(Clone, Copy, PartialEq)]
enum Anatomy {
    /// Human proportions with a menacing, hunched, forward stance.
    Humanoid,
    /// Long low neck, elongated skull, long clawed arms, digitigrade legs.
    Creature,
}

/// Limb lengths and widths of an anatomy, in metres.
struct Build {
    pelvis_height: f32,
    hip_width: f32,
    thigh: f32,
    shin: f32,
    /// The foot: for the creature the raised section from ankle to toe.
    foot: f32,
    shoulder_width: f32,
    upper_arm: f32,
    forearm: f32,
}

impl Anatomy {
    fn build(self) -> Build {
        match self {
            Anatomy::Humanoid => Build {
                pelvis_height: 1.58,
                hip_width: 0.19,
                thigh: 0.94,
                shin: 0.92,
                foot: 0.24,
                shoulder_width: 0.38,
                upper_arm: 0.74,
                forearm: 0.7,
            },
            Anatomy::Creature => Build {
                pelvis_height: 1.5,
                hip_width: 0.2,
                thigh: 0.78,
                shin: 0.82,
                foot: 0.55,
                shoulder_width: 0.4,
                upper_arm: 0.8,
                forearm: 0.8,
            },
        }
    }
}
/// How close the figure comes before stopping.
const KEEP_AWAY: f32 = 5.0;
/// Spring holding each fragment to its place: stiffness and damping.
const STIFFNESS: f32 = 220.0;
const DAMPING: f32 = 18.0;

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

#[derive(Clone, Copy)]
enum Bone {
    Pelvis,
    Waist,
    Chest,
    Neck,
    Skull,
    UpperArm(Side),
    Forearm(Side),
    Claw(Side),
    Thigh(Side),
    Shin(Side),
    Metatarsal(Side),
    Toe(Side),
}

const BONES: [Bone; 19] = [
    Bone::Pelvis,
    Bone::Waist,
    Bone::Chest,
    Bone::Neck,
    Bone::Skull,
    Bone::UpperArm(Side::Left),
    Bone::UpperArm(Side::Right),
    Bone::Forearm(Side::Left),
    Bone::Forearm(Side::Right),
    Bone::Claw(Side::Left),
    Bone::Claw(Side::Right),
    Bone::Thigh(Side::Left),
    Bone::Thigh(Side::Right),
    Bone::Shin(Side::Left),
    Bone::Shin(Side::Right),
    Bone::Metatarsal(Side::Left),
    Bone::Metatarsal(Side::Right),
    Bone::Toe(Side::Left),
    Bone::Toe(Side::Right),
];

/// A part's volume: width and depth at its start and at its end (it tapers
/// between them), its nominal length, and how finely it is split.
struct Part {
    start: (f32, f32),
    end: (f32, f32),
    length: f32,
    levels: u32,
}

impl Bone {
    fn part(self, anatomy: Anatomy) -> Part {
        let p = |start, end, length, levels| Part { start, end, length, levels };
        let b = anatomy.build();
        if anatomy == Anatomy::Humanoid {
            return match self {
                Bone::Pelvis => p((0.46, 0.32), (0.42, 0.3), 0.26, 2),
                Bone::Waist => p((0.38, 0.28), (0.32, 0.26), 0.34, 2),
                // A broad V of a torso, shoulders wide and high.
                Bone::Chest => p((0.4, 0.3), (0.82, 0.46), 0.6, 2),
                Bone::Neck => p((0.17, 0.18), (0.14, 0.16), 0.22, 1),
                Bone::Skull => p((0.24, 0.28), (0.18, 0.22), 0.34, 2),
                Bone::UpperArm(_) => p((0.21, 0.21), (0.12, 0.12), b.upper_arm, 2),
                Bone::Forearm(_) => p((0.13, 0.13), (0.08, 0.09), b.forearm, 1),
                // Hands narrowing to clawed fingers.
                Bone::Claw(_) => p((0.12, 0.05), (0.03, 0.02), 0.3, 1),
                Bone::Thigh(_) => p((0.3, 0.32), (0.15, 0.16), b.thigh, 2),
                Bone::Shin(_) => p((0.14, 0.16), (0.09, 0.11), b.shin, 1),
                Bone::Metatarsal(_) => p((0.1, 0.08), (0.09, 0.06), b.foot, 1),
                Bone::Toe(_) => p((0.09, 0.05), (0.03, 0.02), 0.12, 1),
            };
        }
        match self {
            Bone::Pelvis => p((0.44, 0.34), (0.4, 0.3), 0.26, 2),
            Bone::Waist => p((0.36, 0.3), (0.28, 0.26), 0.36, 2),
            // A barrel ribcage, widest at the shoulders, hunched.
            Bone::Chest => p((0.34, 0.3), (0.86, 0.56), 0.62, 2),
            Bone::Neck => p((0.2, 0.22), (0.13, 0.15), 0.42, 1),
            // An elongated skull tapering to a point.
            Bone::Skull => p((0.26, 0.32), (0.05, 0.07), 0.6, 2),
            Bone::UpperArm(_) => p((0.24, 0.24), (0.12, 0.12), b.upper_arm, 2),
            Bone::Forearm(_) => p((0.15, 0.15), (0.08, 0.09), b.forearm, 1),
            Bone::Claw(_) => p((0.13, 0.05), (0.02, 0.02), 0.42, 1),
            Bone::Thigh(_) => p((0.36, 0.38), (0.15, 0.16), b.thigh, 2),
            Bone::Shin(_) => p((0.13, 0.15), (0.08, 0.1), b.shin, 1),
            Bone::Metatarsal(_) => p((0.08, 0.1), (0.07, 0.12), b.foot, 1),
            Bone::Toe(_) => p((0.1, 0.06), (0.02, 0.02), 0.22, 1),
        }
    }
}

/// A bone's pose: where its part starts, the part's length, and its frame
/// (local +y runs along the bone, +x across it).
#[derive(Clone, Copy, Default)]
struct Pose {
    start: Vec3,
    rotation: Quat,
    length: f32,
}

#[derive(Clone, Copy)]
struct Foot {
    /// Where the toe touches the ground.
    planted: Vec3,
    from: Vec3,
    to: Vec3,
    /// Progress of a step in 0..1, or None while planted.
    step: Option<f32>,
}

#[derive(Component)]
struct Figure {
    anatomy: Anatomy,
    position: Vec3,
    heading: f32,
    speed: f32,
    feet: [Foot; 2],
    poses: Vec<Pose>,
}

/// The glowing core's material (pulsed) and the lamp inside the chest.
#[derive(Resource)]
struct Glow(Handle<StandardMaterial>);

#[derive(Component)]
struct CoreLight;

/// One fragment of the body, held to a place on a bone by a spring.
#[derive(Component)]
struct Element {
    bone: usize,
    /// Centre in the bone's frame, with y as a fraction of the bone's
    /// length so parts follow the skeleton.
    offset: Vec3,
    rotation: Quat,
    velocity: Vec3,
    /// A per-fragment phase for the shiver.
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
    // About 22 m ahead of the default spawn point, facing it.
    let (x, z) = (1214.0, 883.0);
    let position = Vec3::new(x, world.ground_height(x, z), z);
    let heading = 140f32.to_radians();
    let forward = Vec3::new(heading.sin(), 0.0, heading.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    let anatomy = if args.opt("creature") { Anatomy::Creature } else { Anatomy::Humanoid };
    let hip_width = anatomy.build().hip_width;
    let foot = |side: f32| {
        let p = position + right * side * hip_width + forward * 0.2;
        let p = Vec3::new(p.x, world.ground_height(p.x, p.z), p.z);
        Foot { planted: p, from: p, to: p, step: None }
    };

    // Fragments are cubes, wedges (triangular prisms) and pointed shards,
    // all spanning -1..1 so a fragment's scale is its half extents.
    let shapes = [
        meshes.add(Cuboid::new(2.0, 2.0, 2.0)),
        meshes.add(faceted(
            &[[-1., -1., -1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.], [1., -1., 1.], [-1., 1., 1.]],
            &[&[0, 2, 1], &[3, 4, 5], &[0, 1, 4, 3], &[0, 3, 5, 2], &[1, 2, 5, 4]],
        )),
        meshes.add(faceted(
            &[[-1., -1., -1.], [1., -1., -0.6], [-0.2, -1., 1.], [0.3, 1., 0.1]],
            &[&[0, 1, 2], &[0, 3, 1], &[1, 3, 2], &[2, 3, 0]],
        )),
    ];
    // Three finishes, from pale to dark, so the body reads against both the
    // pale ground and the black sky (all-dark fragments vanish against it).
    let finishes = [(0.22, 0.35, 0.7), (0.09, 0.55, 0.5), (0.03, 0.2, 0.9)].map(|(tone, rough, refl)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(tone, tone, tone),
            perceptual_roughness: rough,
            reflectance: refl,
            ..default()
        })
    });

    // The glowing core: emissive shards, pulsed by `pulse`.
    let glow = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(GLOW, GLOW, GLOW),
        ..default()
    });
    commands.insert_resource(Glow(glow.clone()));
    commands.spawn((
        CoreLight,
        PointLight { intensity: CORE_LIGHT, range: 5.0, shadow_maps_enabled: false, ..default() },
        Transform::from_translation(position + Vec3::Y * 2.0),
    ));

    commands.spawn((
        Figure {
            anatomy,
            position,
            heading,
            speed: 0.0,
            feet: [foot(-1.0), foot(1.0)],
            poses: vec![Pose::default(); BONES.len()],
        },
        Transform::default(),
        Visibility::default(),
    ));

    let mut count = 0;
    for (index, bone) in BONES.iter().enumerate() {
        let part = bone.part(anatomy);
        let (wide, deep) = (part.start.0.max(part.end.0), part.start.1.max(part.end.1));
        // Generated one unit long at the part's widest, then tapered.
        let root = Block {
            center: Vec3::Y * 0.5,
            rotation: Quat::IDENTITY,
            half: Vec3::new(wide * 0.5, 0.5, deep * 0.5),
            level: 0,
        };
        // A dense core, then a sparse outer layer pushed out from the
        // surface, so the outline frays.
        let core = Rule {
            divisions: [2, 4, 2],
            keep: Keep::Random(0.9),
            depth: part.levels + 1,
            gap: 1.0,
            twist: Quat::IDENTITY,
            stop_chance: 0.3,
            lift: 0.0,
            min_size: 0.0,
        };
        let fray = Rule { divisions: [3, 6, 3], keep: Keep::Random(0.3), depth: 1, stop_chance: 0.0, ..core };
        for (layer, rule) in [core, fray].iter().enumerate() {
            let leaves = ifs::generate(rule, root, 41 + index as u32 * 7 + layer as u32, 1200);
            for (n, leaf) in leaves.iter().enumerate() {
                let b = leaf.block;
                let r = |k: i32| hash01(index as i32 * 2 + layer as i32, n as i32, k, 0xf16);
                // Taper: width and depth follow the part along its length.
                let t = b.center.y.clamp(0.0, 1.0);
                let fx = (part.start.0 + (part.end.0 - part.start.0) * t) / wide;
                let fz = (part.start.1 + (part.end.1 - part.start.1) * t) / deep;
                // Irregular gaps: each fragment shrunk by its own amount.
                let shrink = Vec3::new(0.5 + 0.45 * r(1), 0.5 + 0.45 * r(2), 0.5 + 0.45 * r(3));
                let mut half = b.half * shrink * Vec3::new(fx, part.length, fz);
                let mut offset = Vec3::new(b.center.x * fx, b.center.y, b.center.z * fz);
                if layer == 1 {
                    // Pushed out beyond the surface, and smaller.
                    let out = 1.25 + 0.45 * r(4);
                    offset.x *= out;
                    offset.z *= out;
                    half *= 0.55;
                }
                let tilt = Quat::from_euler(EulerRot::YXZ, (r(5) - 0.5) * 0.7, (r(6) - 0.5) * 0.5, (r(7) - 0.5) * 0.5);
                let shape = match r(8) {
                    x if x < 0.5 => 0,
                    x if x < 0.78 => 1,
                    _ => 2,
                };
                // Shards inside the chest, along the spine and in the skull
                // glow, close enough to the surface to show through the gaps.
                let inside = layer == 0
                    && matches!(bone, Bone::Chest | Bone::Waist | Bone::Neck | Bone::Skull)
                    && (b.center.x / (wide * 0.5)).abs() < 0.75
                    && (b.center.z / (deep * 0.5)).abs() < 0.75;
                let material = if inside && r(11) < 0.3 {
                    glow.clone()
                } else {
                    finishes[(r(10) * 3.0) as usize % 3].clone()
                };
                commands.spawn((
                    Element { bone: index, offset, rotation: b.rotation * tilt, velocity: Vec3::ZERO, phase: r(9) * 100.0 },
                    Mesh3d(shapes[shape].clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(position + Vec3::Y * 2.0).with_scale(half),
                ));
                count += 1;
            }
        }
    }
    info!("figure: {count} fragments");
}

/// Rotation whose +y runs along `along` and whose +x is as close to `right`
/// as possible.
fn frame(along: Vec3, right: Vec3) -> Quat {
    let y = along.normalize_or(Vec3::Y);
    let x = (right - y * right.dot(y)).normalize_or(Vec3::X);
    let z = x.cross(y);
    Quat::from_mat3(&Mat3::from_cols(x, y, z))
}

/// Two-bone IK: the middle joint between `root` and `end`, bending towards
/// `pole`.
fn middle_joint(root: Vec3, end: Vec3, a: f32, b: f32, pole: Vec3) -> Vec3 {
    let to_end = end - root;
    let d = to_end.length().clamp(0.05, a + b - 1e-3);
    let dir = to_end.normalize_or(Vec3::NEG_Y);
    let cos = ((a * a + d * d - b * b) / (2.0 * a * d)).clamp(-1.0, 1.0);
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Z);
    root + dir * a * cos + bend * a * (1.0 - cos * cos).sqrt()
}

/// `dir` turned by `angle` radians towards `towards` (both unit, roughly
/// perpendicular).
fn tip(dir: Vec3, towards: Vec3, angle: f32) -> Vec3 {
    (dir * angle.cos() + towards * angle.sin()).normalize()
}

fn walk(
    time: Res<Time>,
    args: Res<Args>,
    world: Res<WorldGen>,
    camera: Single<&Transform, (With<FlyCam>, Without<Figure>)>,
    mut figures: Query<&mut Figure>,
) {
    let dt = time.delta_secs().min(0.05);
    let ground = |p: Vec3| Vec3::new(p.x, world.ground_height(p.x, p.z), p.z);
    for mut f in &mut figures {
        // Head for the player; stop a few metres away.
        let to_player = camera.translation - f.position;
        let flat = Vec3::new(to_player.x, 0.0, to_player.z);
        // `--opt statue` keeps it still where it spawned, for looking at.
        let distance = if args.opt("statue") { 0.0 } else { flat.length() };
        if distance > 0.1 {
            let target = flat.x.atan2(flat.z);
            let turn = (target - f.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            f.heading += turn.clamp(-1.4 * dt, 1.4 * dt);
        }
        let wanted = if distance > KEEP_AWAY && distance < 120.0 { WALK_SPEED } else { 0.0 };
        f.speed += (wanted - f.speed).clamp(-2.0 * dt, 2.0 * dt);
        let forward = Vec3::new(f.heading.sin(), 0.0, f.heading.cos());
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        f.position = ground(f.position + forward * f.speed * dt);
        let body = f.anatomy.build();
        let creature = f.anatomy == Anatomy::Creature;

        // Feet: step when too far from where they belong, one at a time.
        for i in 0..2 {
            let other_stepping = f.feet[1 - i].step.is_some();
            let side = if i == 0 { -1.0 } else { 1.0 };
            let home = ground(f.position + right * side * body.hip_width + forward * (0.2 + f.speed * 0.4));
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

        // Pose: crouched, hunched, head low and forward.
        let lift = f.feet.iter().map(|ft| ft.step.map_or(0.0, |t| (t * std::f32::consts::PI).sin())).sum::<f32>();
        let pace = f.speed / WALK_SPEED;
        let bob = -0.07 * (1.0 - lift.min(1.0)) * pace;
        let pelvis = f.position + Vec3::Y * (body.pelvis_height + bob);
        let hunch = 0.15 * pace;
        // The creature folds far forward; the humanoid leans, hunched, with
        // its head pushed forward and down.
        let (waist_lean, chest_lean) = if creature { (0.45, 1.0) } else { (0.25, 0.55) };
        let waist = pelvis + tip(Vec3::Y, forward, waist_lean + hunch) * 0.35;
        let shoulders = waist + tip(Vec3::Y, forward, chest_lean + hunch) * 0.6;
        let neck_end = if creature {
            shoulders + tip(forward, Vec3::Y, 0.25) * 0.42
        } else {
            shoulders + tip(Vec3::Y, forward, 1.0) * 0.22
        };
        let skull_dir = if creature { tip(forward, Vec3::NEG_Y, 0.3) } else { tip(Vec3::Y, forward, 0.75) };
        let swing = |s: f32| {
            let leg = if s < 0.0 { 0 } else { 1 };
            -(f.feet[leg].planted - f.position).dot(forward) * 0.7
        };

        let mut poses = Vec::with_capacity(BONES.len());
        for bone in BONES {
            let pose = |a: Vec3, b: Vec3| Pose { start: a, rotation: frame(b - a, right), length: a.distance(b) };
            let p = match bone {
                Bone::Pelvis => pose(pelvis - Vec3::Y * 0.13, pelvis + Vec3::Y * 0.13),
                Bone::Waist => pose(pelvis, waist),
                Bone::Chest => pose(waist, shoulders),
                Bone::Neck => pose(shoulders - forward * 0.05, neck_end),
                Bone::Skull if creature => pose(neck_end - skull_dir * 0.08, neck_end + skull_dir * 0.52),
                Bone::Skull => pose(neck_end - skull_dir * 0.04, neck_end + skull_dir * 0.32),
                Bone::UpperArm(side) | Bone::Forearm(side) | Bone::Claw(side) => {
                    let s = side.sign();
                    // Shoulders raised on the humanoid, as if braced.
                    let raise = if creature { -0.05 } else { 0.07 };
                    let shoulder = shoulders + right * s * body.shoulder_width + Vec3::Y * raise;
                    // Arms forward from the hunched shoulders, elbows out.
                    let reach = if creature { 0.45 } else { 0.32 };
                    let hang = (Vec3::NEG_Y + forward * (reach + swing(s)) + right * s * 0.18).normalize();
                    let hand = shoulder + hang * (body.upper_arm + body.forearm) * if creature { 0.9 } else { 0.86 };
                    let elbow = middle_joint(shoulder, hand, body.upper_arm, body.forearm, -forward + right * s * 0.7);
                    match bone {
                        Bone::UpperArm(_) => pose(shoulder, elbow),
                        Bone::Forearm(_) => pose(elbow, hand),
                        _ => {
                            let along = (hand - elbow).normalize();
                            let claw = if creature { 0.42 } else { 0.3 };
                            pose(hand, hand + tip(along, Vec3::NEG_Y, 0.4) * claw)
                        }
                    }
                }
                Bone::Thigh(side) | Bone::Shin(side) | Bone::Metatarsal(side) | Bone::Toe(side) => {
                    let s = side.sign();
                    let leg = if s < 0.0 { 0 } else { 1 };
                    let hip = pelvis + right * s * body.hip_width;
                    let toe = f.feet[leg].planted;
                    // The creature walks on its toes with the heel raised
                    // high behind; the humanoid's heel is on the ground.
                    let ankle = if creature {
                        toe + tip(Vec3::Y, -forward, 0.55) * body.foot
                    } else {
                        toe - forward * body.foot + Vec3::Y * 0.1
                    };
                    let knee = middle_joint(hip, ankle, body.thigh, body.shin, forward + right * s * 0.2);
                    let toe_length = if creature { 0.22 } else { 0.12 };
                    match bone {
                        Bone::Thigh(_) => pose(hip, knee),
                        Bone::Shin(_) => pose(knee, ankle),
                        Bone::Metatarsal(_) => pose(ankle, toe),
                        _ => pose(toe, toe + forward * toe_length - Vec3::Y * 0.02),
                    }
                }
            };
            poses.push(p);
        }
        f.poses = poses;
    }
}

/// Moves every fragment towards its place on its bone through a spring, with
/// a faint shiver.
fn follow(
    time: Res<Time>,
    figure: Single<&Figure>,
    mut elements: Query<(&mut Element, &mut Transform), Without<CoreLight>>,
    mut light: Single<&mut Transform, With<CoreLight>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    // The lamp sits in the middle of the chest.
    let chest = figure.poses[2];
    light.translation = chest.start + chest.rotation * Vec3::Y * chest.length * 0.55;
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

/// A slow, uneven pulse of the glowing core and the lamp inside it.
fn pulse(
    time: Res<Time>,
    glow: Option<Res<Glow>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut light: Single<&mut PointLight, With<CoreLight>>,
) {
    let Some(glow) = glow else { return };
    let t = time.elapsed_secs();
    let beat = 0.55 + 0.45 * ((t * 1.6).sin() * 0.7 + (t * 0.53).sin() * 0.3).max(-1.0);
    if let Some(mut material) = materials.get_mut(&glow.0) {
        let g = GLOW * beat;
        material.emissive = LinearRgba::rgb(g, g, g);
    }
    light.intensity = CORE_LIGHT * beat;
}
