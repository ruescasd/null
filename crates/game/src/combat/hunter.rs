//! The swarm assembling into a hunter. When enough free swarmers bunch up
//! they fly together (with a grinding you can hear from afar) and, unless
//! enough of them are broken first, become a creature: a tall biped with
//! long arms, or a beast the size of a horse built like a big cat (`--opt
//! biped`, `--opt beast` to choose). Its body moves on a procedural rig (see
//! `rig.rs`): feet that plant and step, a spine that leads and follows, a
//! head that tracks you, a tail. What it is made of, dark shards and a few
//! glowing cores, hangs on the rig's bones by springs, so it lags, sways and
//! settles. It stalks and attacks after a crouch you can see coming: the
//! biped dashes, the beast pounces. It takes hits as a whole: each shard
//! jolts the part it strikes and staggers the body, and when its health is
//! gone the whole body bursts at once.

use super::*;
use crate::rig::{Intent, Plan, Rig};

/// How many free swarmers close together start an assembly, how close, how
/// long it takes, and how few members an assembly can be.
const GATHER_COUNT: usize = 7;
const GATHER_RADIUS: f32 = 9.0;
const GATHER_TIME: f32 = 3.5;
const MIN_MEMBERS: usize = 5;
/// Seconds between assemblies.
const GATHER_COOLDOWN: f32 = 18.0;

const RECOVER: f32 = 1.1;
const ATTACK: f32 = 30.0;
/// Shards it takes to break (about four good shots at close range).
const HUNTER_HEALTH: f32 = 45.0;
/// The body's pieces: how stiffly they follow their bones (limbs stiffer,
/// so they stay limbs), and how they settle.
const STIFFNESS: f32 = 160.0;
const LIMB_STIFFNESS: f32 = 600.0;
const DAMPING: f32 = 13.0;

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
    stance: Stance,
    timer: f32,
    health: f32,
    stun: f32,
    /// Hits this frame: the bone struck and the shot's direction (the pieces
    /// on that bone are jolted).
    hits: Vec<(usize, Vec3)>,
    struck: bool,
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

/// One of a hunter's eyes.
#[derive(Component)]
pub(super) struct Eye {
    hunter: Entity,
    side: f32,
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

    // Start one: a free swarmer with enough free ones close by.
    let allowed = 1 + (director.alive / 90.0) as usize;
    if assemblies.is_empty() && hunters.iter().count() < allowed && director.next_gather <= 0.0 {
        let free: Vec<(Entity, Vec3)> =
            swarm.iter().filter(|(_, _, s)| s.mode == Mode::Free).map(|(e, t, _)| (e, t.translation)).collect();
        let found = free.iter().find_map(|&(_, p)| {
            let near: Vec<Entity> = free.iter().filter(|(_, q)| q.distance(p) < GATHER_RADIUS).map(|&(e, _)| e).collect();
            (near.len() >= GATHER_COUNT).then_some(near)
        });
        if let Some(near) = found {
            let mut centre = near.iter().filter_map(|&e| swarm.get(e).ok()).map(|(_, t, _)| t.translation).sum::<Vec3>() / near.len() as f32;
            // Not on top of the player: a little way off.
            let away = Vec3::new(centre.x - player.translation.x, 0.0, centre.z - player.translation.z);
            if away.length() < 14.0 {
                centre = player.translation + away.normalize_or(Vec3::X) * 14.0;
            }
            centre.y = world.ground_height(centre.x, centre.z) + 2.0;
            let assembly = commands.spawn((Assembly { centre, time: 0.0 }, Transform::from_translation(centre))).id();
            commands.spawn((
                AudioPlayer::new(assets.assemble.clone()),
                PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.6)),
                Transform::from_translation(centre),
            ));
            for &e in near.iter().take(24) {
                if let Ok((_, _, mut s)) = swarm.get_mut(e) {
                    s.mode = Mode::Gather(assembly);
                }
            }
            director.next_gather = GATHER_COOLDOWN;
            info!("the swarm assembles ({} of them)", near.len());
        }
    }

    // Members spiral in to the centre.
    let t = time.elapsed_secs();
    for (_, mut transform, mut s) in &mut swarm {
        let Mode::Gather(a) = s.mode else { continue };
        // (An assembly started this frame exists from the next.)
        let Ok((_, assembly)) = assemblies.get(a) else { continue };
        let to = assembly.centre - transform.translation;
        let around = Vec3::Y.cross(to).normalize_or_zero() * 6.0;
        let want = to * 2.5 + around * (1.0 - assembly.time / GATHER_TIME) + Vec3::new((t * 7.0 + s.phase).sin(), 0.0, (t * 6.0 + s.phase).cos());
        let change = (want - s.velocity).clamp_length_max(40.0 * dt);
        s.velocity += change;
        transform.translation += s.velocity * dt;
        transform.rotate_local_y(dt * 9.0);
    }

    // Finished, or too many broken.
    for (entity, mut assembly) in &mut assemblies {
        assembly.time += dt;
        let members: Vec<Entity> = swarm.iter().filter(|(_, _, s)| s.mode == Mode::Gather(entity)).map(|(e, ..)| e).collect();
        if members.len() < MIN_MEMBERS {
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
        let kind = if args.opt("beast") {
            Kind::Beast
        } else if args.opt("biped") {
            Kind::Biped
        } else if hash01(entity.index_u32() as i32, 0, 0, 0x6b1) < 0.5 {
            Kind::Beast
        } else {
            Kind::Biped
        };
        let feet = Vec3::new(assembly.centre.x, world.ground_height(assembly.centre.x, assembly.centre.z), assembly.centre.z);
        let to = player.translation - feet;
        let ground = |x: f32, z: f32| world.ground_height(x, z);
        let plan = if kind == Kind::Beast { Plan::beast() } else { Plan::biped() };
        // Bones: three of torso, the neck, the head, then two per leg and per
        // arm (upper, lower), then the tail.
        let limbs = 5..5 + 2 * (plan.legs.len() + plan.arms.len());
        let rig = Rig::new(plan, feet, to.x.atan2(to.z), entity.index_u32(), &ground);
        let bones = rig.bones.clone();
        let health = HUNTER_HEALTH * if args.opt("tough") { 10.0 } else { 1.0 };
        let hunter = commands
            .spawn((
                Hunter { kind, rig, stance: Stance::Stalk, timer: 1.0, health, stun: 0.0, hits: Vec::new(), struck: false },
                Transform::from_translation(feet + Vec3::Y * 1.5),
                Visibility::default(),
            ))
            .with_child((
                // Its voice: the swarm's, slowed down.
                AudioPlayer::new(assets.swarm.clone()),
                PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(1.4), speed: 0.5, spatial: true, ..default() },
                Transform::default(),
            ))
            .id();
        info!("a {} forms", if kind == Kind::Beast { "beast" } else { "hunter" });

        // Its body: shards along every bone, thicker bones more and bigger;
        // the torso's carry some of the members' glowing cores.
        let seed = entity.index_u32();
        let mut cores = members.len().min(8);
        for (i, bone) in bones.iter().enumerate() {
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
                    commands.spawn((
                        Part { hunter, bone: i, along: 1.0, offset: Vec3::ZERO, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: LIMB_STIFFNESS },
                        Mesh3d(assets.swarmer.clone()),
                        MeshMaterial3d(assets.dark.clone()),
                        Transform::from_translation(bone.b).with_scale(Vec3::splat(bone.radius * 3.2)),
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
                let offset = Vec3::new(a.cos() * out, 0.0, a.sin() * out);
                // Knots, and now and then a long blade swept back.
                let blade = r(3) > 0.3;
                let rotation = if blade {
                    Quat::from_rotation_arc(Vec3::Y, Vec3::new(a.cos() * 0.6, -0.8, a.sin() * 0.6).normalize())
                } else {
                    Quat::from_euler(EulerRot::XYZ, r(4) * 6.0, r(5) * 6.0, r(6) * 6.0)
                };
                let size = bone.radius * (0.9 + 0.8 * (r(7) + 0.5));
                let p = bone.a.lerp(bone.b, along) + bone.rotation() * offset;
                let (mesh, scale) = if blade { (assets.shard.clone(), Vec3::new(size * 3.0, size * 1.4, size * 3.0)) } else { (assets.swarmer.clone(), Vec3::splat(size * 1.6)) };
                commands.spawn((
                    Part { hunter, bone: i, along, offset, rotation, velocity: Vec3::ZERO, stiffness: STIFFNESS },
                    Mesh3d(mesh),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(p).with_scale(scale),
                ));
            }
            if i < 3 {
                for k in 0..2 {
                    if cores == 0 {
                        break;
                    }
                    cores -= 1;
                    let r = |j: i32| hash01(seed as i32, (i * 13 + k) as i32, j, 0x6b3) - 0.5;
                    let offset = Vec3::new(r(0), 0.0, r(1)) * bone.radius * 0.6;
                    commands.spawn((
                        Part { hunter, bone: i, along: 0.5 + r(2) * 0.6, offset, rotation: Quat::IDENTITY, velocity: Vec3::ZERO, stiffness: STIFFNESS },
                        Mesh3d(assets.core.clone()),
                        MeshMaterial3d(assets.glow.clone()),
                        Transform::from_translation(bone.a).with_scale(Vec3::splat(1.3)),
                    ));
                }
            }
        }
        for side in [-1.0, 1.0] {
            commands.spawn((Eye { hunter, side }, Mesh3d(assets.core.clone()), MeshMaterial3d(assets.glow.clone()), Transform::from_translation(feet).with_scale(Vec3::splat(0.32))));
        }
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
        let mut intent = Intent { velocity: Vec3::ZERO, look: target, crouch: 0.0 };
        if stunned {
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
                Stance::Stalk | Stance::Windup => {
                    intent.crouch = 0.35;
                    intent.velocity = if distance > 18.0 { toward * 3.2 } else { (toward * 0.4 + around * 0.8).normalize_or(toward) * 2.0 };
                    if h.timer <= 0.0 {
                        if distance < 28.0 && r(0) < 0.6 {
                            h.stance = Stance::Charge;
                            h.timer = 4.0;
                        } else {
                            h.stance = Stance::Freeze;
                            h.timer = 0.6 + 1.2 * r(1);
                        }
                    }
                }
                // Still, low, head locked on you.
                Stance::Freeze => {
                    intent.crouch = 0.5;
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
                    if speed > 7.0 && distance < 6.0 + speed * 0.3 {
                        h.rig.leap(toward, (speed * 1.15).max(13.0), 6.5);
                        h.stance = Stance::Attack(toward);
                        h.timer = 2.0;
                    } else if h.timer <= 0.0 {
                        h.stance = Stance::Stalk;
                        h.timer = 2.0;
                    }
                }
                Stance::Attack(_) => {
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
        // An attack that reaches the player strikes once.
        if matches!(h.stance, Stance::Attack(_)) && !h.struck && h.rig.chest().distance(target) < 2.2 {
            h.struck = true;
            player.1.health -= ATTACK;
            director.hurt = 1.0;
            commands.spawn((AudioPlayer::new(assets.hurt.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
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
    mut hunters: Query<&mut Hunter>,
    mut parts: Query<(&mut Part, &mut Transform), Without<Eye>>,
    mut eyes: Query<(&Eye, &mut Transform), Without<Part>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut part, mut transform) in &mut parts {
        let Ok(h) = hunters.get(part.hunter) else { continue };
        let Some(bone) = h.rig.bones.get(part.bone) else { continue };
        let rotation = bone.rotation();
        let target = bone.a.lerp(bone.b, part.along) + rotation * part.offset;
        for &(b, dir) in &h.hits {
            if b == part.bone {
                part.velocity += dir * 7.0;
            }
        }
        let to = target - transform.translation;
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
        let side = dir.cross(Vec3::Y).normalize_or(Vec3::X);
        let r = if h.kind == Kind::Beast { 0.11 } else { 0.08 };
        transform.translation = tip - dir * 0.12 + side * eye.side * r + Vec3::Y * 0.06;
    }
    for mut h in &mut hunters {
        h.hits.clear();
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
