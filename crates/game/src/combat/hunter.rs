//! The swarm assembling into a hunter. When enough free swarmers bunch up
//! they fly together (with a grinding you can hear from afar) and, unless
//! enough of them are broken first, become a creature: a tall biped with
//! long arms, or a beast the size of a horse built like a big cat (`--opt
//! biped`, `--opt beast` to choose). Its body moves on a procedural rig (see
//! `rig.rs`): feet that plant and step, a spine that leads and follows, a
//! head that tracks you, a tail. What it is made of, dark shards and a few
//! glowing cores, hangs on the rig's bones by springs, so it lags, sways and
//! settles. Its head is one faceted skull, a muzzle and a hinged jaw that
//! opens as it crouches and strikes. Glow is kept for rare, powerful
//! creatures: these are dark with thin pale markings (stripes across the
//! torso, a brow over the eyes, bands above the paws), and their pale eyes
//! flare only when they look straight at you, like eyeshine. It stalks and attacks
//! after a crouch you can see coming: the
//! biped dashes, the beast pounces. It takes hits as a whole: each shard
//! jolts the part it strikes and staggers the body, and when its health is
//! gone the whole body bursts at once.

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};

use super::*;
use crate::rig::{Intent, Plan, Rig};

pub(super) mod cables;
pub(super) use cables::run as cables;

/// How many swarmers, settled and waiting near you (within this, near still),
/// weave themselves into a hunter, how long it takes, and how near you it
/// forms at most. Break one and the weave fails.
const GATHER_COUNT: usize = 3;
const SETTLED_WITHIN: f32 = 10.0;
const GATHER_TIME: f32 = 3.5;
const GATHER_NEAREST: f32 = 6.0;
/// Seconds between assemblies.
const GATHER_COOLDOWN: f32 = 8.0;

const RECOVER: f32 = 1.1;
const ATTACK: f32 = 30.0;
/// Shards it takes to break (about four good shots at close range).
const HUNTER_HEALTH: f32 = 45.0;
/// The body's pieces: how stiffly they follow their bones (limbs stiffer,
/// so they stay limbs), and how they settle.
const STIFFNESS: f32 = 160.0;
const LIMB_STIFFNESS: f32 = 600.0;
/// No spring at all: fixed to its bone.
const RIGID: f32 = 0.0;
const DAMPING: f32 = 13.0;
/// How much of its bone's movement a piece is carried along with each frame
/// before its spring acts (the rest it trails behind).
const CARRY: f32 = 0.9;

#[derive(Component)]
pub(super) struct Assembly {
    centre: Vec3,
    time: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Biped,
    Beast,
}

#[derive(Clone, Copy, PartialEq)]
enum Stance {
    Stalk,
    Windup,
    /// Dashing (the biped) or in a pounce (the beast).
    Attack(Vec3),
    Recover,
    /// The beast: still, low, head locked on you.
    Freeze,
    /// The beast: building to a gallop before it pounces.
    Charge,
}

#[derive(Component)]
pub(super) struct Hunter {
    kind: Kind,
    rig: Rig,
    /// Its bones last frame (the pieces on them are carried along with
    /// them; see `flesh`).
    last_bones: Vec<crate::rig::Bone>,
    stance: Stance,
    timer: f32,
    health: f32,
    stun: f32,
    /// Hits this frame: the bone struck and the shot's direction (the pieces
    /// on that bone are jolted).
    hits: Vec<(usize, Vec3)>,
    struck: bool,
    /// How far its jaw is open (radians).
    jaw: f32,
    /// What it is doing beyond walking, and when it next does something.
    gesture: Gesture,
    gesture_in: f32,
}

/// Gestures that break up a prowl.
#[derive(Clone, Copy, PartialEq)]
enum Gesture {
    None,
    /// Stopped, staring, the head slowly tilting over (radians).
    Tilt(f32, f32),
    /// The head snapping aside and back (-1 left, 1 right).
    Twitch(f32, f32),
    /// Very low and slow.
    Creep(f32),
    /// A short trot.
    Burst(f32),
}

impl Hunter {
    /// A shard struck bone `bone`, flying along `dir`.
    pub(super) fn hurt(&mut self, bone: usize, dir: Vec3) {
        self.health -= 1.0;
        self.rig.knock = (self.rig.knock + Vec3::new(dir.x, 0.0, dir.z) * 1.2).clamp_length_max(9.0);
        self.stun = self.stun.max(0.12);
        self.hits.push((bone, dir));
    }

    /// The nearest bone a ray hits within `max`, and how far.
    pub(super) fn ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, usize)> {
        self.rig
            .bones
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.ray(origin, dir, max).map(|d| (d, i)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }
}

/// A piece of a hunter's body, hung on one of its bones.
#[derive(Component)]
pub(super) struct Part {
    hunter: Entity,
    bone: usize,
    /// Where along the bone (0..1), and out from it in the bone's frame.
    along: f32,
    offset: Vec3,
    rotation: Quat,
    velocity: Vec3,
    stiffness: f32,
}

/// A hunter's voice, kept at its head.
#[derive(Component)]
pub(super) struct Voice {
    hunter: Entity,
    /// Seconds until it may growl again.
    next: f32,
    count: u32,
}

/// A hunter's lower jaw: it opens about its hinge.
#[derive(Component)]
pub(super) struct Jaw;

/// The skulls and jaws, built once.
#[derive(Resource)]
pub(super) struct Heads {
    /// The beasts' heads: the hound's, the angler's.
    beast: Vec<(Handle<Mesh>, Handle<Mesh>)>,
    biped: (Handle<Mesh>, Handle<Mesh>),
}

/// Where a beast head's eyes and brow sit: out from the bone and up, in
/// head radii, and along it (0..1).
const HEAD_EYES: [(f32, f32, f32); 2] = [(0.62, 0.32, 0.5), (0.86, 0.9, 0.3)];

/// A faceted solid from rings along +Y (y, half width, half height, how far
/// up): flat shaded, closed at both ends. "Up" is -Z, as in a bone's frame
/// on a level head.
fn rings_mesh(rings: &[(f32, f32, f32, f32)]) -> Mesh {
    const SIDES: usize = 8;
    let ring = |&(y, w, h, up): &(f32, f32, f32, f32)| -> Vec<Vec3> {
        (0..SIDES)
            .map(|k| {
                let a = (k as f32 + 0.5) / SIDES as f32 * std::f32::consts::TAU;
                Vec3::new(a.cos() * w, y, -(a.sin() * h + up))
            })
            .collect()
    };
    let pts: Vec<Vec<Vec3>> = rings.iter().map(ring).collect();
    let (mut positions, mut normals): (Vec<[f32; 3]>, Vec<[f32; 3]>) = (Vec::new(), Vec::new());
    let centre = |r: &[Vec3]| r.iter().copied().sum::<Vec3>() / r.len() as f32;
    let mut tri = |a: Vec3, b: Vec3, c: Vec3, inside: Vec3| {
        let mut n = (b - a).cross(c - a).normalize_or(Vec3::Y);
        let (b, c) = if n.dot((a + b + c) / 3.0 - inside) < 0.0 {
            n = -n;
            (c, b)
        } else {
            (b, c)
        };
        for p in [a, b, c] {
            positions.push(p.to_array());
            normals.push(n.to_array());
        }
    };
    for r in 0..pts.len() - 1 {
        let inside = (centre(&pts[r]) + centre(&pts[r + 1])) * 0.5;
        for k in 0..SIDES {
            let k2 = (k + 1) % SIDES;
            tri(pts[r][k], pts[r][k2], pts[r + 1][k2], inside);
            tri(pts[r][k], pts[r + 1][k2], pts[r + 1][k], inside);
        }
    }
    for r in [0, pts.len() - 1] {
        let c = centre(&pts[r]);
        let inside = c + Vec3::Y * if r == 0 { 1.0 } else { -1.0 };
        for k in 0..SIDES {
            tri(c, pts[r][k], pts[r][(k + 1) % SIDES], inside);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // A long, narrow, angular head: a ridge of cranium behind, the brow low
    // over the eyes, a long muzzle; nothing round about it.
    let beast = rings_mesh(&[(0.0, 0.5, 0.65, 0.1), (0.2, 0.68, 0.8, 0.22), (0.42, 0.55, 0.5, 0.06), (0.72, 0.38, 0.34, -0.04), (1.0, 0.16, 0.18, -0.1)]);
    let beast_jaw = rings_mesh(&[(0.0, 0.5, 0.2, 0.0), (0.55, 0.36, 0.15, 0.0), (1.0, 0.14, 0.08, 0.0)]);
    // A shorter, rounder head with a jutting brow.
    let biped = rings_mesh(&[(0.0, 0.75, 0.8, 0.0), (0.3, 1.0, 1.0, 0.1), (0.6, 0.85, 0.75, 0.05), (0.85, 0.55, 0.5, -0.1), (1.0, 0.3, 0.3, -0.15)]);
    let biped_jaw = rings_mesh(&[(0.0, 0.6, 0.25, 0.0), (0.6, 0.45, 0.2, 0.0), (1.0, 0.25, 0.12, 0.0)]);
    // An angler fish's: a broad flat skull, its front the upper lip, raised
    // over a gaping mouth; under it a huge jaw jutting past the snout and
    // curling up at the front.
    let angler = rings_mesh(&[(0.0, 0.85, 0.75, 0.15), (0.3, 1.05, 0.8, 0.35), (0.6, 1.0, 0.55, 0.3), (0.85, 0.85, 0.35, 0.22), (1.0, 0.7, 0.2, 0.18)]);
    // (The jaw is deep: a heavy faceted chin and throat bulging below the
    // teeth, reaching back under the skull.)
    let angler_jaw = rings_mesh(&[(-0.2, 0.7, 0.55, -0.35), (0.3, 1.0, 0.78, -0.48), (0.75, 1.1, 0.62, -0.32), (1.05, 1.05, 0.42, -0.08), (1.25, 0.9, 0.25, 0.25)]);
    let beast_jaw = meshes.add(beast_jaw);
    commands.insert_resource(Heads {
        beast: vec![(meshes.add(beast), beast_jaw), (meshes.add(angler), meshes.add(angler_jaw))],
        biped: (meshes.add(biped), meshes.add(biped_jaw)),
    });
}

/// One of a hunter's eyes (each hunter's pair shares a material, which
/// flares as it looks at you).
#[derive(Component)]
pub(super) struct Eye {
    hunter: Entity,
    /// Where on the head: across and up (head radii), and along it (0..1).
    at: Vec3,
    material: Handle<StandardMaterial>,
}

/// Starts assemblies, draws their members together, and turns a finished
/// one into a hunter (or lets it fall apart if too many were broken).
#[allow(clippy::too_many_arguments)]
pub(super) fn gather(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    world: Res<WorldGen>,
    assets: Res<Assets3>,
    heads: Res<Heads>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut director: ResMut<Director>,
    player: Single<&Transform, (With<Player>, Without<Swarmer>)>,
    mut assemblies: Query<(Entity, &mut Assembly)>,
    hunters: Query<(), With<Hunter>>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    if director.alive <= 0.0 {
        return;
    }
    director.next_gather -= dt;

    // `--opt hunters`: no swarm, no assembly; a hunter, already built,
    // appears a little way off whenever there is none (`--set crowd=N`: N of
    // them at once, round you, for measuring their cost).
    // `--opt specimen`: one hunter standing still in front of you, to look
    // at (`--set specimen_at` metres ahead, `specimen_yaw` degrees turned
    // from facing you, `jaw` radians open, `stretch` 0..1).
    if args.opt("specimen") {
        if hunters.is_empty() && director.next_gather <= 0.0 {
            let ahead = player.forward();
            let at = player.translation + Vec3::new(ahead.x, 0.0, ahead.z).normalize_or(Vec3::Z) * args.num("specimen_at", 3.5);
            let face = at + Quat::from_rotation_y(args.num("specimen_yaw", 0.0).to_radians()) * (player.translation - at);
            spawn_hunter(&mut commands, &args, &world, &assets, &heads, &mut materials, 7, at, face);
            director.next_gather = f32::MAX;
        }
        return;
    }
    if args.opt("hunters") {
        if hunters.is_empty() && director.next_gather <= 0.0 {
            let crowd = args.num("crowd", 1.0).max(1.0) as u32;
            for n in 0..crowd {
                let k = (time.elapsed_secs() * 1000.0) as u32 + n * 7919;
                let a = hash01(k as i32, 1, 0, 0x6b8) * std::f32::consts::TAU;
                let at = player.translation + Vec3::new(a.cos(), 0.0, a.sin()) * 25.0;
                spawn_hunter(&mut commands, &args, &world, &assets, &heads, &mut materials, k, at, player.translation);
            }
            director.next_gather = 4.0;
        }
        return;
    }

    // Start one: enough free swarmers settled and waiting near you (behind
    // you, as they stalk). (`--opt swarm`: never; the swarm alone.)
    let allowed = 1 + (director.alive / 90.0) as usize;
    if !args.opt("swarm") && assemblies.is_empty() && hunters.iter().count() < allowed && director.next_gather <= 0.0 {
        let near: Vec<Entity> = swarm
            .iter()
            .filter(|(_, t, s)| s.mode == Mode::Free && s.velocity.length() < 1.5 && t.translation.distance(player.translation) < SETTLED_WITHIN)
            .map(|(e, ..)| e)
            .take(GATHER_COUNT)
            .collect();
        if near.len() >= GATHER_COUNT {
            let mut centre = near.iter().filter_map(|&e| swarm.get(e).ok()).map(|(_, t, _)| t.translation).sum::<Vec3>() / near.len() as f32;
            // Not on top of the player: a little way off.
            let away = Vec3::new(centre.x - player.translation.x, 0.0, centre.z - player.translation.z);
            if away.length() < GATHER_NEAREST {
                centre = player.translation + away.normalize_or(Vec3::X) * GATHER_NEAREST;
            }
            centre.y = world.ground_height(centre.x, centre.z) + 2.0;
            let assembly = commands.spawn((Assembly { centre, time: 0.0 }, Transform::from_translation(centre))).id();
            commands.spawn((
                AudioPlayer::new(assets.assemble.clone()),
                PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.6)),
                Transform::from_translation(centre),
            ));
            for &e in &near {
                if let Ok((_, _, mut s)) = swarm.get_mut(e) {
                    s.mode = Mode::Gather(assembly);
                }
            }
            director.next_gather = GATHER_COOLDOWN;
            info!("the swarm assembles ({} of them)", near.len());
        }
    }

    // Members are drawn straight in by their cables (see `web.rs`).
    let t = time.elapsed_secs();
    for (_, mut transform, mut s) in &mut swarm {
        let Mode::Gather(a) = s.mode else { continue };
        // (An assembly started this frame exists from the next.)
        let Ok((_, assembly)) = assemblies.get(a) else { continue };
        let to = assembly.centre - transform.translation;
        // (Reeled in: slowly, then faster, the web tightening over the whole
        // gathering.)
        let k = assembly.time / GATHER_TIME;
        let want = to * (0.3 + 2.5 * k * k) + Vec3::new((t * 7.0 + s.phase).sin(), 0.0, (t * 6.0 + s.phase).cos());
        let change = (want - s.velocity).clamp_length_max(40.0 * dt);
        s.velocity += change;
        transform.translation += s.velocity * dt;
        transform.rotate_local_y(dt * 9.0);
    }

    // Finished, or too many broken.
    for (entity, mut assembly) in &mut assemblies {
        assembly.time += dt;
        let members: Vec<Entity> = swarm.iter().filter(|(_, _, s)| s.mode == Mode::Gather(entity)).map(|(e, ..)| e).collect();
        if members.len() < GATHER_COUNT {
            for &e in &members {
                if let Ok((_, _, mut s)) = swarm.get_mut(e) {
                    s.mode = Mode::Free;
                }
            }
            info!("the assembly falls apart");
            commands.entity(entity).despawn();
            continue;
        }
        if assembly.time < GATHER_TIME {
            continue;
        }
        // The members are taken into the body.
        for &e in &members {
            commands.entity(e).despawn();
        }
        commands.entity(entity).despawn();
        spawn_hunter(&mut commands, &args, &world, &assets, &heads, &mut materials, entity.index_u32(), assembly.centre, player.translation);
    }
}

/// A hunter, built, standing at `centre` and facing the player.
#[allow(clippy::too_many_arguments)]
fn spawn_hunter(
    commands: &mut Commands,
    args: &Args,
    world: &WorldGen,
    assets: &Assets3,
    heads: &Heads,
    materials: &mut Assets<StandardMaterial>,
    seed: u32,
    centre: Vec3,
    player: Vec3,
) {
    // A biped, or a beast: a hound (lean, a narrow head, two rows of
    // needles) or an angler (a huge underbite jaw, uneven needles leaning
    // in). `--opt biped`, `hound`, `angler`, or `beast` (either beast).
    let kind = if args.opt("beast") || args.opt("hound") || args.opt("angler") {
        Kind::Beast
    } else if args.opt("biped") || hash01(seed as i32, 0, 0, 0x6b1) < 1.0 / 3.0 {
        Kind::Biped
    } else {
        Kind::Beast
    };
    let angler = kind == Kind::Beast && (args.opt("angler") || (!args.opt("hound") && hash01(seed as i32, 1, 0, 0x6b1) < 0.5));
    let feet = Vec3::new(centre.x, world.ground_height(centre.x, centre.z), centre.z);
    let to = player - feet;
    let ground = |x: f32, z: f32| world.ground_height(x, z);
    // (The angler's body is a size larger than the hound's; its head is the
    // same size, so smaller for the body.)
    let plan = if angler {
        let head = Plan::beast().head;
        Plan { head, ..Plan::beast().scaled(1.15) }
    } else if kind == Kind::Beast {
        Plan::beast()
    } else {
        Plan::biped()
    };
    // Bones: three of torso, the neck, the head, then two per leg and per
    // arm (upper, lower), then the tail.
    let limbs = 5..5 + 2 * (plan.legs.len() + plan.arms.len());
    let rig = Rig::new(plan, feet, to.x.atan2(to.z), seed, &ground);
    let bones = rig.bones.clone();
    let health = HUNTER_HEALTH * if args.opt("tough") { 10.0 } else { 1.0 };
    let hunter = commands
        .spawn((
            Hunter { kind, rig, last_bones: Vec::new(), stance: Stance::Stalk, timer: 1.0, health, stun: 0.0, hits: Vec::new(), struck: false, jaw: 0.0, gesture: Gesture::None, gesture_in: 2.0 },
            Transform::from_translation(feet + Vec3::Y * 1.5),
            Visibility::default(),
        ))
        .id();
    // Its voice, at its head: growls now and then (see `voice`).
    commands.spawn((Voice { hunter, next: 1.0, count: 0 }, Transform::from_translation(centre), Visibility::default()));
    info!("{} forms", if angler { "an angler" } else if kind == Kind::Beast { "a hound" } else { "a biped" });

    // Its body: shards along every bone, thicker bones more and bigger,
    // with pale markings.
    let seed = seed;
    let pale = |commands: &mut Commands, bone: usize, along: f32, offset: Vec3, dir: Vec3, length: f32, width: f32| {
        commands.spawn((
            Part { hunter, bone, along, offset, rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::Y)), velocity: Vec3::ZERO, stiffness: if bone == 4 { RIGID } else { STIFFNESS * 1.5 } },
            Mesh3d(assets.shard.clone()),
            MeshMaterial3d(assets.pale.clone()),
            Transform::from_scale(Vec3::new(width / 0.025, length / 0.75, width / 0.025)),
        ));
    };
    for (i, bone) in bones.iter().enumerate() {
        // (Woven cable instead; see `cables.rs`.)
        if cables::replaces(args, i) {
            continue;
        }
        let length = bone.a.distance(bone.b);
        if limbs.contains(&i) {
            // A limb: long blades laid along it, overlapping, so it reads
            // as one solid faceted limb; and a heavy paw at a foot.
            let lower = (i - limbs.start) % 2 == 1;
            let count = ((length / 0.18).round() as usize).clamp(3, 8);
            for k in 0..count {
                let r = |j: i32| hash01(seed as i32, (i * 97 + k) as i32, j, 0x6b4) - 0.5;
                let along = (k as f32 + 0.5) / count as f32 + r(0) * 0.1;
                let a = r(1) * std::f32::consts::TAU;
                let offset = Vec3::new(a.cos(), 0.0, a.sin()) * bone.radius * 0.3;
                let rotation = Quat::from_euler(EulerRot::XYZ, r(2) * 0.3, r(3) * 6.0, r(4) * 0.3);
                let width = bone.radius * (0.9 + 0.4 * (r(5) + 0.5)) / 0.025;
                let scale = Vec3::new(width, length * 0.6 / 0.75, width);
                commands.spawn((
                    Part { hunter, bone: i, along: along - 0.3 / count as f32, offset, rotation, velocity: Vec3::ZERO, stiffness: LIMB_STIFFNESS },
                    Mesh3d(assets.shard.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.a.lerp(bone.b, along)).with_scale(scale),
                ));
            }
            if lower {
                // A pale band above the paw.
                for k in 0..3 {
                    let a = k as f32 / 3.0 * std::f32::consts::TAU;
                    let (c, sn) = (a.cos(), a.sin());
                    pale(commands, i, 0.72, Vec3::new(c, 0.0, sn) * bone.radius * 1.1, Vec3::new(-sn, 0.0, c), bone.radius * 1.3, 0.02);
                }
                commands.spawn((
                    Part { hunter, bone: i, along: 1.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: LIMB_STIFFNESS },
                    Mesh3d(assets.swarmer.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.b).with_scale(Vec3::splat(bone.radius * 3.2)),
                ));
            }
            continue;
        }
        if i == 4 {
            // The head: one skull, a hinged jaw beneath, a thin pale brow.
            let (skull, jaw) = if kind == Kind::Beast { &heads.beast[angler as usize] } else { &heads.biped };
            let (across, up, _) = HEAD_EYES[angler as usize];
            let rr = bone.radius;
            commands.spawn((
                Part { hunter, bone: i, along: 0.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
                Mesh3d(skull.clone()),
                MeshMaterial3d(assets.dark.clone()),
                Transform::from_translation(bone.a).with_scale(Vec3::new(rr, length, rr)),
            ));
            commands.spawn((
                Jaw,
                Part { hunter, bone: i, along: 0.3, offset: Vec3::new(0.0, 0.0, rr * 0.3), rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: RIGID },
                Mesh3d(jaw.clone()),
                MeshMaterial3d(assets.dark.clone()),
                Transform::from_translation(bone.a).with_scale(Vec3::new(rr, length * 0.66, rr)),
            ));
            for side in [-1.0, 1.0] {
                if !angler {
                    pale(commands, i, 0.4, Vec3::new(side * rr * across, 0.0, -rr * (up + 0.34)), Vec3::new(-side * 0.35, 1.0, 0.15), length * 0.3, 0.012);
                }
            }
            if kind == Kind::Beast {
                // Teeth, bared when the jaw opens. Each: where along the
                // head, out from the middle (head radii), how long (head
                // radii), on the skull or the jaw, a lean.
                let mut teeth: Vec<(f32, f32, f32, bool, f32)> = Vec::new();
                if angler {
                    // Round the whole rim, of very uneven lengths, leaning
                    // in. Upper ones hang from the lip; lower ones stand on
                    // the jaw out to its upturned front (past the snout).
                    let teeth_rnd = |k: i32, j: i32| hash01(seed as i32, k, j, 0x7d1);
                    for k in 0..6 {
                        let t = 0.5 + 0.09 * k as f32 + 0.05 * (teeth_rnd(k, 2) - 0.5);
                        let l = 0.4 + 1.5 * teeth_rnd(k, 0).powi(2);
                        teeth.push((t, 0.95 - 0.35 * (k as f32 / 5.0) + 0.1 * (teeth_rnd(k, 3) - 0.5), l, true, -0.6 * teeth_rnd(k, 4)));
                    }
                    for k in 0..7 {
                        // (Along the jaw: past 1.0 it reaches beyond the skull.)
                        let t = 0.55 + 0.1 * k as f32 + 0.06 * (teeth_rnd(k, 5) - 0.5);
                        let l = 0.5 + 1.9 * teeth_rnd(k, 1).powi(2);
                        teeth.push((t, 0.95 - 0.5 * (k as f32 / 6.0) + 0.1 * (teeth_rnd(k, 6) - 0.5), l, false, -0.7 * teeth_rnd(k, 7)));
                    }
                } else {
                    // Two rows, the inner one shorter and set back between
                    // the outer teeth.
                    for k in 0..18 {
                        let inner = k >= 10;
                        let j = if inner { k - 10 } else { k };
                        let t = if inner { 0.46 + 0.065 * j as f32 } else { 0.42 + 0.056 * j as f32 };
                        let w = (0.42 - 0.25 * (t - 0.42)) * if inner { 0.62 } else { 1.0 };
                        let l = (0.6 - 0.3 * (t - 0.42)) * if inner { 0.7 } else { 1.0 };
                        teeth.push((t, w, l, true, 0.0));
                        teeth.push((t, w * 0.85, l * 0.8, false, 0.0));
                    }
                }
                for (k, &(t, w, l, upper, lean)) in teeth.iter().enumerate() {
                    for side in [-1.0, 1.0] {
                        // Crooked: each side leans its own way.
                        // Crooked: each side leans its own way; the angler's all
                        // lean in, towards the middle of the mouth.
                        let lean = if angler { lean * side * (0.6 + 0.4 * hash01(seed as i32, k as i32, 2, 0x7d2)) } else { lean * side * if k % 2 == 0 { 1.0 } else { -0.6 } };
                        // The angler's rake (forward or back) and thickness vary
                        // tooth by tooth.
                        let q = |j: i32| hash01(seed as i32, k as i32 * 2 + (side > 0.0) as i32, j, 0x7d3);
                        let (rake, thick) = if angler { (-0.15 + 0.7 * q(0), 0.45 + 0.7 * q(1)) } else { (0.25, 0.9) };
                        let (along, offset, dir) = if upper {
                            (t, Vec3::new(side * w * rr, 0.0, rr * 0.22), Vec3::new(lean, rake, 1.0))
                        } else {
                            (0.3, Vec3::new(side * w * rr, length * 0.66 * (t - 0.3) / 0.7, rr * 0.22), Vec3::new(lean, rake, -1.0))
                        };
                        let mut tooth = commands.spawn((
                            Part { hunter, bone: i, along, offset, rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize()), velocity: Vec3::ZERO, stiffness: RIGID },
                            Mesh3d(assets.shard.clone()),
                            MeshMaterial3d(assets.pale.clone()),
                            Transform::from_scale(Vec3::new(thick, l * rr / 0.75, thick)),
                        ));
                        if !upper {
                            tooth.insert(Jaw);
                        }
                    }
                }
            }
            continue;
        }
        if args.opt("wiry") {
            // Fibres: thin, long, bundled along the bone and splaying from
            // it, gaps between them; now and then a spine bristling out.
            let count = ((length / 0.06) * (bone.radius / 0.12).clamp(0.6, 2.0)).round().clamp(4.0, 40.0) as usize;
            for k in 0..count {
                let r = |j: i32| hash01(seed as i32, (i * 97 + k) as i32, j, 0x6b9) - 0.5;
                let along = k as f32 / count as f32 + r(0) / count as f32;
                let a = r(1) * std::f32::consts::TAU;
                let out = bone.radius * (0.2 + 0.85 * (r(2) + 0.5));
                let mut offset = Vec3::new(a.cos() * out, 0.0, a.sin() * out);
                if kind == Kind::Beast && i < 3 && offset.z > 0.0 {
                    offset.z *= 0.45;
                }
                let spine = r(3) > 0.4;
                let dir = if spine {
                    Vec3::new(a.cos() * 0.8, -0.6, a.sin() * 0.8)
                } else {
                    Vec3::new(r(4) * 0.5, 1.0, r(5) * 0.5)
                };
                let long = if spine { bone.radius * (1.0 + 1.2 * (r(6) + 0.5)) } else { length * (0.5 + 0.7 * (r(6) + 0.5)) };
                let thin = if spine { 0.6 } else { 0.35 + 0.35 * (r(7) + 0.5) };
                commands.spawn((
                    Part { hunter, bone: i, along, offset, rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize()), velocity: Vec3::ZERO, stiffness: STIFFNESS },
                    Mesh3d(assets.shard.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.a).with_scale(Vec3::new(thin, long / 0.75, thin)),
                ));
            }
            continue;
        }
        let count = ((length / 0.11) * (bone.radius / 0.12).clamp(0.6, 2.2)).round().clamp(3.0, 40.0) as usize;
        for k in 0..count {
            let r = |j: i32| hash01(seed as i32, (i * 97 + k) as i32, j, 0x6b2) - 0.5;
            let along = k as f32 / count as f32 + r(0) / count as f32;
            let a = r(1) * std::f32::consts::TAU;
            let out = bone.radius * (0.35 + 0.6 * (r(2) + 0.5));
            let mut offset = Vec3::new(a.cos() * out, 0.0, a.sin() * out);
            // A beast's belly is lean: what hangs under the torso (+Z in
            // its frame) sits close in.
            if kind == Kind::Beast && i < 3 && offset.z > 0.0 {
                offset.z *= 0.45;
            }
            // Knots, and now and then a long blade swept back.
            let blade = r(3) > 0.3;
            let rotation = if blade {
                Quat::from_rotation_arc(Vec3::Y, Vec3::new(a.cos() * 0.6, -0.8, a.sin() * 0.6).normalize())
            } else {
                Quat::from_euler(EulerRot::XYZ, r(4) * 6.0, r(5) * 6.0, r(6) * 6.0)
            };
            // The neck thick at its base, tapering to the head.
            let taper = if i == 3 { 1.25 - 0.6 * along } else { 1.0 };
            let size = bone.radius * (0.9 + 0.8 * (r(7) + 0.5)) * taper;
            let p = bone.a.lerp(bone.b, along) + bone.rotation() * offset;
            let (mesh, scale) = if blade { (assets.shard.clone(), Vec3::new(size * 3.0, size * 1.4, size * 3.0)) } else { (assets.swarmer.clone(), Vec3::splat(size * 1.6)) };
            commands.spawn((
                Part { hunter, bone: i, along, offset, rotation, velocity: Vec3::ZERO, stiffness: STIFFNESS },
                Mesh3d(mesh),
                MeshMaterial3d(assets.dark.clone()),
                Transform::from_translation(p).with_scale(scale),
            ));
        }
        // In a bone's frame Y runs along it, X to the body's right, and
        // -Z is the back (up on a beast's level torso).
        if kind == Kind::Beast && (i == 2 || i == 3) {
            // Muscle over the shoulders and up the back of the neck: a
            // heavy hump of mass on top, where its strength is.
            let (from, to) = if i == 2 { (0.45, 1.0) } else { (0.0, 0.45) };
            for k in 0..10 {
                let r = |j: i32| hash01(seed as i32, (i * 53 + k) as i32, j, 0x6b7) - 0.5;
                let along = from + (to - from) * (k as f32 + 0.5 + r(0) * 0.5) / 10.0;
                let offset = Vec3::new(r(1) * bone.radius * 0.9, 0.0, -bone.radius * (0.55 + 0.3 * (r(2) + 0.5)));
                let size = bone.radius * (1.1 + 0.5 * (r(3) + 0.5));
                commands.spawn((
                    Part { hunter, bone: i, along, offset, rotation: Quat::from_euler(EulerRot::XYZ, r(4) * 6.0, r(5) * 6.0, r(6) * 6.0), velocity: Vec3::ZERO, stiffness: STIFFNESS },
                    Mesh3d(assets.swarmer.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(bone.a).with_scale(Vec3::splat(size * 1.6)),
                ));
            }
        }
        if i < 3 {
            // Thin stripes lying across the back.
            for k in 0..2 {
                let r = |j: i32| hash01(seed as i32, (i * 31 + k) as i32, j, 0x6b6) - 0.5;
                let along = (k as f32 + 0.5 + r(0) * 0.4) / 2.0;
                for m in 0..3 {
                    let a = (m as f32 - 1.0) * 0.5 + r(1) * 0.3;
                    let (c, sn) = (a.cos(), a.sin());
                    let out = Vec3::new(sn, 0.0, -c) * bone.radius * 1.02;
                    pale(commands, i, along, out, Vec3::new(c, r(2) * 0.4, sn), bone.radius * 0.5, 0.015);
                }
            }
        }
    }
    // Pale pinprick eyes, flaring when it looks at you (see `flesh`).
    // (Not hazed, so eyeshine shows in the swarm's darkness.)
    let eyes = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.8),
        perceptual_roughness: 0.4,
        fog_enabled: false,
        ..default()
    });
    let (across, up, along) = HEAD_EYES[angler as usize];
    for side in [-1.0f32, 1.0] {
        commands.spawn((
            Eye { hunter, at: Vec3::new(side * across, up, along), material: eyes.clone() },
            Mesh3d(assets.core.clone()),
            MeshMaterial3d(eyes.clone()),
            Transform::from_translation(feet).with_scale(Vec3::splat(0.13)),
        ));
    }
}

/// Hunters stalk and attack; a broken one bursts.
#[allow(clippy::too_many_arguments)]
pub(super) fn hunt(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    world: Res<WorldGen>,
    assets: Res<Assets3>,
    mut director: ResMut<Director>,
    mut feedback: ResMut<Feedback>,
    ichor: Res<ichor::Ichor>,
    mut player: Single<(&Transform, &mut Player), Without<Hunter>>,
    mut hunters: Query<(Entity, &mut Hunter, &mut Transform), Without<Player>>,
    parts: Query<(Entity, &Part, &Transform), (Without<Hunter>, Without<Player>)>,
    eyes: Query<(Entity, &Eye)>,
) {
    let dt = time.delta_secs().min(0.05);
    let target = player.0.translation;
    let ground = |x: f32, z: f32| world.ground_height(x, z);
    for (entity, mut h, mut transform) in &mut hunters {
        if h.health <= 0.0 {
            // Broken: the whole body bursts at once.
            let at = h.rig.chest();
            commands.spawn((
                AudioPlayer::new(assets.death.clone()),
                PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(2.0)),
                Transform::from_translation(at),
            ));
            let knock = h.rig.knock;
            for (e, part, t) in &parts {
                if part.hunter != entity {
                    continue;
                }
                let out = (t.translation - at).normalize_or(Vec3::Y) * 9.0 + knock;
                if e.index_u32() % 3 == 0 {
                    shatter_quiet(&mut commands, &assets, t.translation, out, e.index_u32());
                    ichor::spray(&mut commands, &ichor, t.translation, out.normalize_or(Vec3::Y), 1.5, e.index_u32());
                }
                commands.entity(e).despawn();
            }
            for (e, eye) in &eyes {
                if eye.hunter == entity {
                    commands.entity(e).despawn();
                }
            }
            director.kills += 1;
            feedback.kill = 1.0;
            info!("a hunter is broken");
            commands.entity(entity).despawn();
            continue;
        }

        let root = h.rig.root;
        let flat = Vec3::new(target.x - root.x, 0.0, target.z - root.z);
        let distance = flat.length();
        let toward = flat.normalize_or(Vec3::Z);
        let beast = h.kind == Kind::Beast;
        h.timer -= dt;
        let stunned = h.stun > 0.0;
        h.stun -= dt;
        let around = Vec3::new(-toward.z, 0.0, toward.x);
        let r = |k: i32| hash01(entity.index_u32() as i32, (time.elapsed_secs() * 7.0) as i32, k, 0x6b5);
        let mut intent = Intent { velocity: Vec3::ZERO, look: target, crouch: 0.0, paw: None, tilt: 0.0, head_rate: 0.0, stretch: 0.0, still: false };
        if args.opt("specimen") {
            // Straight ahead from its body (not its head, or each glance
            // would turn it), and dead still.
            intent.look = h.rig.root + Vec3::new(h.rig.heading.sin(), 1.4, h.rig.heading.cos()) * 8.0;
            intent.stretch = args.num("stretch", 0.0);
            intent.still = true;
        } else if stunned {
        } else if args.opt("tame") {
            // (`--opt tame`, for captures: it circles at a distance, side on,
            // its pace changing every 8 s: prowl, trot, gallop.)
            let keep = args.num("tame_at", 14.0);
            let fixed = args.num("tame_pace", 0.0);
            let pace = if fixed > 0.0 { fixed } else { [2.5, 6.0, 12.0][(time.elapsed_secs() / 8.0) as usize % 3] };
            intent.velocity = (around + toward * ((distance - keep) / 3.0).clamp(-1.0, 1.0)).normalize_or(around) * pace;
            intent.crouch = if pace < 3.0 { 0.35 } else { 0.0 };
        } else if beast {
            match h.stance {
                // Prowling: low and slow, closing in; now and then it freezes,
                // or breaks into a charge.
                Stance::Stalk => {
                    intent.crouch = 0.35;
                    // Never quite straight, never quite steady: the path
                    // wanders either side, the pace comes and goes.
                    let t = time.elapsed_secs() + entity.index_u32() as f32 * 1.7;
                    let wander = (t * 0.55).sin() * 0.45 + (t * 1.3).sin() * 0.15;
                    let pace = 0.65 + 0.5 * (0.5 + 0.5 * (t * 0.37).sin());
                    let base = if distance > 18.0 { toward } else { (toward * 0.4 + around * 0.8).normalize_or(toward) };
                    let dir = Quat::from_rotation_y(wander) * base;
                    intent.velocity = dir * if distance > 18.0 { 3.2 } else { 2.0 } * pace;
                    // Now and then a gesture breaks the prowl.
                    h.gesture_in -= dt;
                    if h.gesture == Gesture::None && h.gesture_in <= 0.0 {
                        let pick = r(5);
                        let side = if r(7) < 0.5 { -1.0 } else { 1.0 };
                        h.gesture = if pick < 0.3 {
                            Gesture::Tilt(1.4 + 1.2 * r(6), side * (0.5 + 0.4 * r(9)))
                        } else if pick < 0.6 {
                            Gesture::Twitch(0.25, side)
                        } else if pick < 0.85 {
                            Gesture::Creep(1.5 + 1.5 * r(6))
                        } else {
                            Gesture::Burst(0.8 + 0.6 * r(6))
                        };
                        h.gesture_in = 1.5 + 3.0 * r(8);
                    }
                    let forward = Vec3::new(h.rig.heading.sin(), 0.0, h.rig.heading.cos());
                    h.gesture = match h.gesture {
                        Gesture::Tilt(t, angle) => {
                            // Stopped dead, staring, the head rolling over.
                            intent.velocity = Vec3::ZERO;
                            intent.crouch = 0.3;
                            intent.tilt = angle;
                            intent.still = true;
                            if t > dt { Gesture::Tilt(t - dt, angle) } else { Gesture::None }
                        }
                        Gesture::Twitch(t, side) => {
                            // A sudden snap of the head aside, and back.
                            let aside = Vec3::new(-forward.z, 0.0, forward.x) * side * 3.0 + forward;
                            intent.look = if t > 0.12 { root + aside + Vec3::Y * 1.0 } else { target };
                            intent.head_rate = 45.0;
                            intent.velocity *= 0.3;
                            if t > dt { Gesture::Twitch(t - dt, side) } else { Gesture::None }
                        }
                        Gesture::Creep(t) => {
                            intent.velocity *= 0.45;
                            intent.crouch = 0.65;
                            if t > dt { Gesture::Creep(t - dt) } else { Gesture::None }
                        }
                        Gesture::Burst(t) => {
                            intent.velocity = intent.velocity.normalize_or(toward) * 6.0;
                            intent.crouch = 0.1;
                            if t > dt { Gesture::Burst(t - dt) } else { Gesture::None }
                        }
                        Gesture::None => Gesture::None,
                    };
                    if h.timer <= 0.0 {
                        if distance < 28.0 && r(0) < 0.6 {
                            h.stance = Stance::Charge;
                            h.timer = 4.0;
                            h.gesture = Gesture::None;
                        } else {
                            h.stance = Stance::Freeze;
                            h.timer = 2.0 + 2.5 * r(1);
                        }
                    }
                }
                // Still, low, head locked on you.
                Stance::Freeze => {
                    intent.crouch = 0.5;
                    intent.still = true;
                    if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 1.2 + 2.5 * r(2);
                    }
                }
                // Building to a gallop, then a pounce out of the run, keeping
                // its momentum.
                Stance::Charge => {
                    intent.velocity = toward * 14.0;
                    intent.crouch = 0.1;
                    let speed = h.rig.speed();
                    if speed > 7.0 && distance < 5.0 + speed * 0.35 {
                        // A quick gather, still running.
                        h.stance = Stance::Windup;
                        h.timer = 0.13;
                    } else if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 2.0;
                    }
                }
                Stance::Windup => {
                    intent.velocity = toward * 14.0;
                    intent.crouch = 1.0;
                    intent.stretch = 1.0;
                    if h.timer <= 0.0 {
                        // Fast and flat, aimed to land on you: about a third
                        // of a second in the air.
                        let up = 4.5;
                        let flight = 2.0 * up / 25.0;
                        h.rig.leap(toward, (distance / flight).clamp(12.0, 28.0), up);
                        commands.spawn((
                            AudioPlayer::new(assets.pounce.clone()),
                            PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(2.0)),
                            Transform::from_translation(h.rig.head().0),
                        ));
                        h.stance = Stance::Attack(toward);
                        h.timer = 2.0;
                    }
                }
                Stance::Attack(_) => {
                    intent.stretch = 1.0;
                    if h.rig.airborne.is_none() && h.timer < 1.9 {
                        h.stance = Stance::Recover;
                        h.timer = RECOVER;
                        h.struck = false;
                    }
                }
                // Landed: it skids to a stop (its deceleration), low.
                Stance::Recover => {
                    intent.crouch = 0.4 * (h.timer / RECOVER).max(0.0);
                    if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 1.0 + 1.5 * r(3);
                    }
                }
            }
        } else {
            match h.stance {
                Stance::Stalk | Stance::Freeze | Stance::Charge => {
                    if distance > 2.5 {
                        intent.velocity = toward * 4.5 * ((distance - 2.5) / 3.0).min(1.0);
                    }
                    if distance < 9.0 && h.timer <= 0.0 {
                        h.stance = Stance::Windup;
                        h.timer = 0.7;
                    }
                }
                Stance::Windup => {
                    intent.crouch = 1.0;
                    if h.timer <= 0.0 {
                        h.stance = Stance::Attack(toward);
                        h.timer = 0.45;
                    }
                }
                // A dash, low and reaching.
                Stance::Attack(dir) => {
                    intent.velocity = dir * 20.0;
                    intent.crouch = 0.5;
                    h.rig.velocity = dir * 20.0;
                    if h.timer <= 0.0 {
                        h.stance = Stance::Recover;
                        h.timer = RECOVER;
                        h.struck = false;
                    }
                }
                Stance::Recover => {
                    intent.crouch = 0.3 * (h.timer / RECOVER).max(0.0);
                    if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 1.2;
                    }
                }
            }
        }
        // The jaw: nearly shut, open in a crouch, gaping in an attack.
        let open = match h.stance {
            Stance::Windup => if beast { 1.4 } else { 0.55 },
            Stance::Attack(_) => if beast { 1.9 } else { 0.8 },
            Stance::Charge => 0.3,
            Stance::Freeze => 0.15,
            _ => 0.06 + 0.04 * (time.elapsed_secs() * 1.3).sin(),
        };
        let open = if args.opt("specimen") { args.num("jaw", 0.1) } else { open };
        h.jaw += (open - h.jaw) * (1.0 - (-dt * 10.0).exp());
        // An attack that reaches the player strikes once.
        if matches!(h.stance, Stance::Attack(_)) && !h.struck && h.rig.chest().distance(target) < 2.2 {
            h.struck = true;
            player.1.health -= ATTACK;
            director.hurt = 1.0;
            commands.spawn((AudioPlayer::new(assets.bite.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
            if args.opt("hurtsound") {
                commands.spawn((AudioPlayer::new(assets.hurt.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
            }
        }
        h.rig.update(dt, intent, &ground);
        transform.translation = h.rig.chest();
    }
}

/// The body's pieces follow their bones by springs (lagging, swaying,
/// settling); those on a bone just struck are jolted along the shot. The
/// eyes sit on the head.
#[allow(clippy::type_complexity)]
pub(super) fn flesh(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Single<&Transform, (With<FlyCam>, Without<Part>, Without<Eye>)>,
    mut hunters: Query<&mut Hunter>,
    mut parts: Query<(&mut Part, &mut Transform, Has<Jaw>), (Without<Eye>, Without<FlyCam>)>,
    mut eyes: Query<(&Eye, &mut Transform), (Without<Part>, Without<FlyCam>)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut part, mut transform, jaw) in &mut parts {
        let Ok(h) = hunters.get(part.hunter) else { continue };
        let Some(bone) = h.rig.bones.get(part.bone) else { continue };
        // (A jaw turns about its hinge, opening downwards.)
        let rotation = bone.rotation() * if jaw { Quat::from_rotation_x(h.jaw) } else { Quat::IDENTITY };
        let target = bone.a.lerp(bone.b, part.along) + rotation * part.offset;
        // Carried along with its bone first (all but a little), so the
        // springs give jiggle and a little lag, not a body trailing a metre
        // behind its head at a run.
        if let Some(last) = h.last_bones.get(part.bone) {
            let last_rotation = last.rotation() * if jaw { Quat::from_rotation_x(h.jaw) } else { Quat::IDENTITY };
            let last_target = last.a.lerp(last.b, part.along) + last_rotation * part.offset;
            transform.translation += (target - last_target) * CARRY;
        }
        for &(b, dir) in &h.hits {
            if b == part.bone {
                part.velocity += dir * 7.0;
            }
        }
        let to = target - transform.translation;
        if part.stiffness == RIGID {
            // The skull and what is on it move as one with the head.
            transform.translation = target;
            transform.rotation = rotation * part.rotation;
            continue;
        }
        if to.length() > 4.0 {
            transform.translation = target;
            part.velocity = Vec3::ZERO;
        } else {
            let damping = DAMPING * (part.stiffness / STIFFNESS).sqrt();
            let accel = to * part.stiffness - part.velocity * damping;
            part.velocity += accel * dt;
            transform.translation += part.velocity * dt;
        }
        transform.rotation = transform.rotation.slerp(rotation * part.rotation, (dt * 12.0).min(1.0));
    }
    for (eye, mut transform) in &mut eyes {
        let Ok(h) = hunters.get(eye.hunter) else { continue };
        let (tip, dir) = h.rig.head();
        // In the skull's sides, under the brow.
        let head = h.rig.bones[4];
        let rotation = head.rotation();
        transform.translation = head.a.lerp(head.b, eye.at.z) + rotation * Vec3::new(eye.at.x * head.radius, 0.0, -eye.at.y * head.radius);
        transform.rotation = rotation;
        // Eyeshine: bright only when it looks straight at you.
        let facing = dir.dot((camera.translation - tip).normalize_or(Vec3::Y)).max(0.0).powi(8);
        if let Some(mut m) = materials.get_mut(&eye.material) {
            m.emissive = LinearRgba::rgb(1.0, 1.0, 1.0) * 6000.0 * facing;
        }
    }
    for mut h in &mut hunters {
        h.hits.clear();
        h.last_bones = h.rig.bones.clone();
    }
}

/// The voice follows the head and growls now and then: a phrase picked at
/// random from the recordings, a little higher or lower each time, with a
/// pause after it. Prowling, a growl every few seconds; creeping, quieter and
/// rarer; charging, one after another, louder and higher; stopped and
/// staring, or in the air, nothing (it lets the current phrase finish). A
/// voice whose hunter is gone goes with it.
pub(super) fn voice(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<Assets3>,
    hunters: Query<&Hunter>,
    mut voices: Query<(Entity, &mut Voice, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut voice, mut transform) in &mut voices {
        let Ok(h) = hunters.get(voice.hunter) else {
            commands.entity(e).despawn();
            continue;
        };
        transform.translation = h.rig.head().0;
        voice.next -= dt;
        // (volume, pitch, pause after a phrase), or silence.
        let style = match (h.stance, h.gesture) {
            (_, Gesture::Tilt(..)) | (Stance::Freeze | Stance::Attack(_), _) => None,
            (Stance::Charge | Stance::Windup, _) => Some((1.6, 1.08, 0.0..0.25)),
            (Stance::Recover, _) => Some((1.0, 0.92, 0.3..1.0)),
            (_, Gesture::Creep(_)) => Some((0.55, 0.9, 2.0..4.5)),
            _ => Some((1.0, 1.0, 1.0..3.5)),
        };
        let Some((volume, pitch, pause)) = style else { continue };
        if voice.next > 0.0 {
            continue;
        }
        voice.count += 1;
        let r = |k: i32| hash01(e.index_u32() as i32, voice.count as i32, k, 0x7c1);
        let (sound, length) = assets.growls[(r(0) * assets.growls.len() as f32) as usize % assets.growls.len()].clone();
        let speed = pitch * (0.9 + 0.18 * r(1));
        commands.entity(e).with_child((
            AudioPlayer::new(sound),
            PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(volume * 1.6)).with_speed(speed),
            Transform::default(),
        ));
        voice.next = length / speed + pause.start + (pause.end - pause.start) * r(2);
    }
}

/// `--opt watch` (for captures): the view turns to the nearest hunter.
pub(super) fn watch(
    args: Res<Args>,
    hunters: Query<&Transform, (With<Hunter>, Without<FlyCam>)>,
    mut camera: Single<(&Transform, &mut FlyCam)>,
) {
    if !args.opt("watch") {
        return;
    }
    let eye = camera.0.translation;
    let Some(h) = hunters.iter().min_by(|a, b| a.translation.distance(eye).total_cmp(&b.translation.distance(eye))) else { return };
    let to = h.translation - eye;
    camera.1.yaw = (-to.x).atan2(-to.z);
    camera.1.pitch = (to.y / Vec2::new(to.x, to.z).length().max(0.1)).atan().clamp(-0.6, 0.6);
}
