//! The swarm assembling into a hunter. When enough free swarmers bunch up
//! they fly together (with a grinding you can hear from afar) and, unless
//! enough of them are broken first, become one body: each takes a slot on a
//! tall, long-armed body plan. The hunter stalks at a walk and lunges, with
//! a crouch you can see coming. It takes hits as a whole: each shard jolts
//! the piece it strikes and staggers the body, and when its health is gone
//! the whole body bursts at once.

use super::*;

/// How many free swarmers close together start an assembly, how close, how
/// long it takes, and how few members a hunter (or an assembly) can be.
const GATHER_COUNT: usize = 7;
const GATHER_RADIUS: f32 = 9.0;
const GATHER_TIME: f32 = 3.5;
const MIN_MEMBERS: usize = 5;
/// Seconds between assemblies.
const GATHER_COOLDOWN: f32 = 18.0;

const HUNTER_SPEED: f32 = 5.0;
const LUNGE_RANGE: f32 = 10.0;
const WINDUP: f32 = 0.7;
const DASH_SPEED: f32 = 24.0;
const DASH_TIME: f32 = 0.45;
const RECOVER: f32 = 1.1;
const LUNGE: f32 = 30.0;
/// Members are drawn this much bigger than free swarmers.
const MEMBER_SCALE: f32 = 1.5;
/// Shards it takes to break (about four good shots at close range).
const HUNTER_HEALTH: f32 = 45.0;

#[derive(Component)]
pub(super) struct Assembly {
    centre: Vec3,
    time: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Stance {
    Stalk,
    Windup,
    Dash(Vec3),
    Recover,
}

#[derive(Component)]
pub(super) struct Hunter {
    heading: f32,
    stance: Stance,
    timer: f32,
    health: f32,
    /// Pushed back by hits (m/s, decaying), and frozen for a moment.
    knock: Vec3,
    stun: f32,
}

impl Hunter {
    /// A shard struck it, flying along `dir`.
    pub(super) fn hurt(&mut self, dir: Vec3) {
        self.health -= 1.0;
        self.knock = (self.knock + Vec3::new(dir.x, 0.0, dir.z) * 1.2).clamp_length_max(9.0);
        self.stun = self.stun.max(0.12);
    }
}

/// Slots on the body plan, feet at the origin, facing +Z, most important
/// first (a small hunter is a torso on legs; a full one has arms and spines).
fn slot(i: usize) -> Vec3 {
    const PLAN: [(f32, f32, f32); 20] = [
        // Torso and head.
        (0.0, 1.9, 0.0),
        (0.28, 2.2, 0.0),
        (-0.28, 2.2, 0.0),
        (0.0, 1.45, 0.05),
        (0.0, 2.75, 0.2),
        // Legs.
        (0.3, 1.0, 0.0),
        (-0.3, 1.0, 0.0),
        (0.33, 0.4, 0.08),
        (-0.33, 0.4, 0.08),
        // Arms, reaching forward and down.
        (0.62, 2.2, 0.1),
        (-0.62, 2.2, 0.1),
        (0.82, 1.7, 0.5),
        (-0.82, 1.7, 0.5),
        (0.88, 1.25, 0.9),
        (-0.88, 1.25, 0.9),
        // Spines along the back.
        (0.0, 2.35, -0.45),
        (0.22, 2.65, -0.35),
        (-0.22, 2.65, -0.35),
        (0.3, 1.7, -0.15),
        (-0.3, 1.7, -0.15),
    ];
    let (x, y, z) = PLAN[i.min(PLAN.len() - 1)];
    Vec3::new(x, y, z)
}
const SLOTS: usize = 20;

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
            if away.length() < 12.0 {
                centre = player.translation + away.normalize_or(Vec3::X) * 12.0;
            }
            centre.y = world.ground_height(centre.x, centre.z) + 2.0;
            let assembly = commands.spawn((Assembly { centre, time: 0.0 }, Transform::from_translation(centre))).id();
            commands.spawn((
                AudioPlayer::new(assets.assemble.clone()),
                PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.6)),
                Transform::from_translation(centre),
            ));
            for &e in near.iter().take(SLOTS + 4) {
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
        let feet = Vec3::new(assembly.centre.x, world.ground_height(assembly.centre.x, assembly.centre.z), assembly.centre.z);
        let to = player.translation - feet;
        let hunter = commands
            .spawn((
                Hunter { heading: to.x.atan2(to.z), stance: Stance::Stalk, timer: 0.0, health: HUNTER_HEALTH * if args.opt("tough") { 10.0 } else { 1.0 }, knock: Vec3::ZERO, stun: 0.0 },
                Transform::from_translation(feet),
                Visibility::default(),
            ))
            .with_child((
                // Its voice: the swarm's, slowed down.
                AudioPlayer::new(assets.swarm.clone()),
                PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(1.4), speed: 0.5, spatial: true, ..default() },
                Transform::from_xyz(0.0, 2.0, 0.0),
            ))
            .id();
        info!("a hunter forms ({} members)", members.len());
        for (i, &e) in members.iter().enumerate() {
            if let Ok((_, _, mut s)) = swarm.get_mut(e) {
                s.mode = if i < SLOTS { Mode::Bound { hunter, slot: i } } else { Mode::Free };
            }
        }
        commands.entity(entity).despawn();
    }
}

/// Hunters stalk and lunge; their members hold their slots; a hunter that
/// has lost too many falls apart.
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
    mut player: Single<(&Transform, &mut Player), Without<Swarmer>>,
    mut hunters: Query<(Entity, &mut Hunter, &mut Transform), (Without<Swarmer>, Without<Player>)>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), (Without<Hunter>, Without<Player>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let target = player.0.translation;
    for (entity, mut h, mut transform) in &mut hunters {
        let mine = |s: &Swarmer| matches!(s.mode, Mode::Bound { hunter, .. } if hunter == entity);
        if h.health <= 0.0 {
            // Broken: the whole body bursts at once.
            let at = transform.translation + Vec3::Y * 1.6;
            commands.spawn((
                AudioPlayer::new(assets.death.clone()),
                PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(2.0)),
                Transform::from_translation(at),
            ));
            for (e, t, s) in &swarm {
                if mine(s) {
                    let out = (t.translation - at).normalize_or(Vec3::Y) * 9.0 + h.knock;
                    shatter_quiet(&mut commands, &assets, t.translation, out, e.index_u32());
                    ichor::spray(&mut commands, &ichor, t.translation, out.normalize_or(Vec3::Y), 2.0, e.index_u32());
                    commands.entity(e).despawn();
                }
            }
            director.kills += 1;
            feedback.kill = 1.0;
            info!("a hunter is broken");
            commands.entity(entity).despawn();
            continue;
        }
        let count = swarm.iter().filter(|(_, _, s)| mine(s)).count();
        if count < MIN_MEMBERS {
            // Falls apart: everything left is a swarm again.
            for (_, t, mut s) in &mut swarm {
                if matches!(s.mode, Mode::Bound { hunter, .. } if hunter == entity) {
                    s.mode = Mode::Free;
                    s.velocity = (t.translation - transform.translation - Vec3::Y).normalize_or(Vec3::Y) * 10.0;
                }
            }
            info!("a hunter falls apart");
            commands.entity(entity).despawn();
            continue;
        }

        let to = target - transform.translation;
        let flat = Vec3::new(to.x, 0.0, to.z);
        let distance = flat.length();
        let facing = flat.x.atan2(flat.z);
        let turn = (facing - h.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        // Staggered by hits: pushed back, and frozen for an instant.
        let knock = h.knock;
        h.knock *= (-dt * 7.0).exp();
        let stunned = h.stun > 0.0;
        h.stun -= dt;
        h.timer -= dt;
        let mut step = Vec3::ZERO;
        match h.stance {
            _ if stunned => {}
            Stance::Stalk => {
                h.heading += turn.clamp(-2.5 * dt, 2.5 * dt);
                // (`--opt tame`, for captures: it keeps its distance.)
                let tame = args.opt("tame");
                if !tame || distance > 14.0 {
                    step = Vec3::new(h.heading.sin(), 0.0, h.heading.cos()) * HUNTER_SPEED * dt;
                }
                if distance < LUNGE_RANGE && h.timer <= 0.0 && !tame {
                    h.stance = Stance::Windup;
                    h.timer = WINDUP;
                }
            }
            Stance::Windup => {
                h.heading += turn.clamp(-4.0 * dt, 4.0 * dt);
                if h.timer <= 0.0 {
                    h.stance = Stance::Dash(flat.normalize_or(Vec3::Z));
                    h.timer = DASH_TIME;
                }
            }
            Stance::Dash(dir) => {
                step = dir * DASH_SPEED * dt;
                let chest = transform.translation + Vec3::Y * 1.6;
                if chest.distance(target) < 2.0 {
                    player.1.health -= LUNGE;
                    director.hurt = 1.0;
                    commands.spawn((AudioPlayer::new(assets.hurt.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
                    h.stance = Stance::Recover;
                    h.timer = RECOVER;
                } else if h.timer <= 0.0 {
                    h.stance = Stance::Recover;
                    h.timer = RECOVER;
                }
            }
            Stance::Recover => {
                if h.timer <= 0.0 {
                    h.stance = Stance::Stalk;
                    h.timer = 1.5;
                }
            }
        }
        let mut p = transform.translation + step + knock * dt;
        p.y = world.ground_height(p.x, p.z);
        transform.translation = p;
        transform.rotation = Quat::from_rotation_y(h.heading);

        // The body: members pulled to their slots. Crouched in the windup,
        // stretched forward in the dash; a slow sway as it walks.
        let t = time.elapsed_secs();
        let (squash, lean) = match h.stance {
            Stance::Windup => (0.7, 0.35),
            Stance::Dash(_) => (0.9, 0.6),
            _ => (1.0, 0.1 * (t * 3.0).sin()),
        };
        let pose = |s: Vec3| {
            let v = Vec3::new(s.x, s.y * squash, s.z + s.y * lean * 0.3);
            transform.translation + transform.rotation * v
        };
        let pull = 1.0 - (-dt * 10.0).exp();
        for (_, mut mt, mut s) in &mut swarm {
            let Mode::Bound { hunter, slot: i } = s.mode else { continue };
            if hunter != entity {
                continue;
            }
            s.jolt *= (-dt * 12.0).exp();
            let goal = pose(slot(i)) + s.jolt;
            let before = mt.translation;
            mt.translation = before.lerp(goal, pull);
            s.velocity = (mt.translation - before) / dt.max(1e-4);
            mt.scale = mt.scale.lerp(Vec3::splat(MEMBER_SCALE), pull);
            mt.rotate_local_y(dt * 2.0);
        }
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
    let to = h.translation + Vec3::Y * 1.6 - eye;
    camera.1.yaw = (-to.x).atan2(-to.z);
    camera.1.pitch = (to.y / Vec2::new(to.x, to.z).length().max(0.1)).atan().clamp(-0.6, 0.6);
}
