//! Combat, a first prototype: the shard shotgun, a swarm that keeps coming,
//! the player's health, and their sounds.
//!
//! The shotgun (Mouse 1) throws a spray of shards: hitscan, each shard drawn
//! as a bright streak flying out to what it hit. The swarm is made of small
//! dark polygonal things with a lit core that hunt the player, darting in to
//! bite; a few shards break one. More keep coming, a little faster the longer
//! you last, so standing still is death. Left alone, enough of them close
//! together assemble into a hunter (see `hunter.rs`). `--opt peace` leaves
//! them out; `--opt fight` keeps them in a capture and fires the gun by
//! itself (`--opt holdfire` stops it).

use avian3d::prelude::*;
use bevy::{
    asset::embedded_asset,
    audio::{PlaybackMode, SpatialListener, Volume},
    prelude::*,
};
use worldgen::noise::hash01;

use crate::{
    Args,
    camera::FlyCam,
    player::{HEALTH_MAX, MoveInput, Player, tether::shard_mesh},
    terrain::{Streamer, WorldGen},
};

mod hunter;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "sounds/gun.wav");
        embedded_asset!(app, "sounds/shatter.wav");
        embedded_asset!(app, "sounds/swarm.wav");
        embedded_asset!(app, "sounds/drone.wav");
        embedded_asset!(app, "sounds/hurt.wav");
        embedded_asset!(app, "sounds/assemble.wav");
        embedded_asset!(app, "sounds/hit.wav");
        embedded_asset!(app, "sounds/death.wav");
        app.init_resource::<Gun>()
            .init_resource::<Director>()
            .init_resource::<Feedback>()
            .add_systems(PostStartup, setup)
            .add_systems(
                Update,
                (fire, fly_shards, swarm, hunter::gather, hunter::hunt, hunter::watch, bite, die, feedback, debris, swarm_sound, hud)
                    .chain()
                    .after(crate::player::walk),
            )
            .add_systems(PostUpdate, (kick, viewmodel).before(TransformSystems::Propagate));
    }
}

/// Seconds between shots.
const RELOAD: f32 = 0.85;
/// Shards per shot, the cone they spread in (radians, half-angle), how far
/// they reach and how fast they are drawn flying.
const SHARDS: usize = 16;
const SPREAD: f32 = 0.085;
const RANGE: f32 = 150.0;
const SHARD_SPEED: f32 = 260.0;

/// A swarmer: how many shards break it, its size, how fast it flies (a
/// little slower than a running player, much faster when it darts in) and
/// what a bite costs.
const SWARMER_HEALTH: f32 = 3.0;
const SWARMER_RADIUS: f32 = 0.55;
const SWARMER_SPEED: f32 = 8.0;
const DART_SPEED: f32 = 24.0;
const BITE: f32 = 8.0;

#[derive(Resource, Default)]
struct Gun {
    /// Time until it can fire again.
    cooldown: f32,
    /// The view's kick, radians, decaying.
    kick: f32,
    /// The viewmodel's recoil, metres back, decaying.
    recoil: f32,
    /// Shots fired, for the shards' pattern.
    shots: u32,
}

/// Sends the swarm: groups at a distance whenever there are too few, more
/// of them the longer the player lasts.
#[derive(Resource, Default)]
struct Director {
    /// Seconds survived in this life (from when the ground has loaded).
    alive: f32,
    next_wave: f32,
    /// Seconds until the swarm may assemble again.
    next_gather: f32,
    kills: u32,
    best: f32,
    /// Seconds left of the "you died" message, and what it says.
    notice: f32,
    last: String,
    /// The hurt flash, 0..1.
    hurt: f32,
}

#[derive(Resource)]
struct Assets3 {
    shard: Handle<Mesh>,
    swarmer: Handle<Mesh>,
    core: Handle<Mesh>,
    bright: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    gun: Handle<AudioSource>,
    shatter: Handle<AudioSource>,
    hurt: Handle<AudioSource>,
    assemble: Handle<AudioSource>,
    swarm: Handle<AudioSource>,
    hit: Handle<AudioSource>,
    death: Handle<AudioSource>,
}

/// What landed this frame, for the feedback: how many shards hit something
/// and where (summed), and the hit marker and impact light, decaying.
#[derive(Resource, Default)]
struct Feedback {
    hits: u32,
    at: Vec3,
    marker: f32,
    /// The marker is bigger after a kill.
    kill: f32,
}

/// A shard in flight from the muzzle to where it hit.
#[derive(Component)]
struct Streak {
    from: Vec3,
    to: Vec3,
    travelled: f32,
    /// Whether it hit the world (and sparks there).
    impact: bool,
}

/// What a swarmer is doing: hunting on its own, flying to an assembly, or
/// holding a slot in a hunter's body.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Free,
    Gather(Entity),
    Bound { hunter: Entity, slot: usize },
}

#[derive(Component)]
struct Swarmer {
    mode: Mode,
    velocity: Vec3,
    health: f32,
    /// Seconds until it may dart again, and whether it is darting.
    dart_in: f32,
    darting: f32,
    /// Seconds until it may bite again.
    bite_in: f32,
    phase: f32,
    /// A jolt from a hit, metres, decaying (a hunter's members).
    jolt: Vec3,
}

/// A piece flying off something broken, or a spark.
#[derive(Component)]
struct Debris {
    velocity: Vec3,
    spin: Vec3,
    life: f32,
    total: f32,
    size: Vec3,
}

#[derive(Component)]
struct Viewmodel;

#[derive(Component)]
struct MuzzleLight;

/// Where shards land on a body.
#[derive(Component)]
struct ImpactLight;

#[derive(Component)]
struct HitMarker;

#[derive(Component)]
struct SwarmVoice;

#[derive(Component)]
struct HurtFlash;

#[derive(Component)]
struct Score;

fn setup(
    mut commands: Commands,
    args: Res<Args>,
    server: Res<AssetServer>,
    camera: Single<Entity, With<FlyCam>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let load = |name: &str| server.load::<AudioSource>(format!("embedded://game/sounds/{name}.wav"));
    // A long thin shard, pointing along +Y.
    let shard = meshes.add(shard_mesh(&[(Vec3::Y, 0.7, 0.025), (Vec3::NEG_Y, 0.05, 0.025)]));
    // A swarmer: a knot of a few long shards round a small core.
    let mut spikes = Vec::new();
    for k in 0..7 {
        let d = Vec3::new(hash01(k, 0, 1, 0x5a) - 0.5, hash01(k, 1, 1, 0x5a) - 0.5, hash01(k, 2, 1, 0x5a) - 0.5).normalize();
        spikes.push((d, SWARMER_RADIUS * (0.7 + 0.6 * hash01(k, 3, 1, 0x5a)), 0.12));
    }
    let swarmer = meshes.add(shard_mesh(&spikes));
    let core = meshes.add(Sphere::new(0.13).mesh().ico(1).unwrap());
    let bright = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(60.0, 60.0, 60.0), ..default() });
    // Dark against the bright ground; in the dark, only their cores show.
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.03, 0.03),
        perceptual_roughness: 0.35,
        reflectance: 0.6,
        ..default()
    });
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(9000.0, 9000.0, 9000.0), ..default() });
    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.03, 0.03),
        perceptual_roughness: 0.3,
        reflectance: 0.6,
        ..default()
    });
    let assets = Assets3 {
        shard: shard.clone(),
        swarmer,
        core,
        bright: bright.clone(),
        dark: dark.clone(),
        glow: glow.clone(),
        gun: load("gun"),
        shatter: load("shatter"),
        hurt: load("hurt"),
        assemble: load("assemble"),
        swarm: load("swarm"),
        hit: load("hit"),
        death: load("death"),
    };

    // The ear is the camera.
    commands.entity(*camera).insert(SpatialListener::new(0.3));
    // The gun in view: a fan of dark shards round a glowing core, low right.
    commands.entity(*camera).with_children(|parent| {
        parent
            .spawn((Viewmodel, Transform::from_xyz(0.2, -0.17, -0.42), Visibility::default()))
            .with_children(|gun| {
                let mut fan = Vec::new();
                for k in 0..6 {
                    let a = k as f32 / 6.0 * std::f32::consts::TAU;
                    fan.push((Vec3::new(a.cos() * 0.22, a.sin() * 0.22, -1.0), 0.2, 0.012));
                }
                gun.spawn((
                    Mesh3d(meshes.add(shard_mesh(&fan))),
                    MeshMaterial3d(metal.clone()),
                    Transform::default(),
                    bevy::light::NotShadowCaster,
                ));
                gun.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.016).mesh().ico(1).unwrap())),
                    MeshMaterial3d(glow.clone()),
                    Transform::from_xyz(0.0, 0.0, -0.03),
                    bevy::light::NotShadowCaster,
                ));
            });
        parent.spawn((
            MuzzleLight,
            PointLight { intensity: 0.0, range: 25.0, shadow_maps_enabled: false, ..default() },
            Transform::from_xyz(0.2, -0.15, -1.0),
        ));
    });

    if !args.opt("peace") && args.shot.is_none() {
        // The ground's hum, and the swarm's voice (it follows the swarm).
        commands.spawn((
            AudioPlayer::new(load("drone")),
            PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.35), ..default() },
        ));
        commands.spawn((
            SwarmVoice,
            AudioPlayer::new(load("swarm")),
            PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.0), spatial: true, ..default() },
            Transform::default(),
        ));
    }
    commands.insert_resource(assets);
    commands.spawn((
        ImpactLight,
        PointLight { intensity: 0.0, range: 12.0, shadow_maps_enabled: false, ..default() },
        Transform::default(),
    ));
    // The hit marker: a small diamond round the crosshair.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_child((
            HitMarker,
            Node { width: px(18), height: px(18), border: UiRect::all(px(2)), ..default() },
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.0)),
            UiTransform::from_rotation(Rot2::degrees(45.0)),
        ));

    // A flash of black when bitten, and the score.
    commands.spawn((
        HurtFlash,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
    ));
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            top: px(20),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_child((
            Score,
            Text::new(""),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            TextColor(Color::srgb(0.85, 0.85, 0.85)),
            TextLayout::justify(Justify::Center),
        ));
}

/// Mouse 1: a spray of shards, hitscan, against the world and the swarm.
#[allow(clippy::too_many_arguments)]
fn fire(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    input: Res<MoveInput>,
    assets: Res<Assets3>,
    spatial: SpatialQuery,
    mut gun: ResMut<Gun>,
    mut feedback: ResMut<Feedback>,
    camera: Single<(&Transform, &FlyCam), With<Player>>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), Without<FlyCam>>,
    mut hunters: Query<&mut hunter::Hunter>,
    mut light: Single<&mut PointLight, With<MuzzleLight>>,
) {
    let dt = time.delta_secs();
    gun.cooldown -= dt;
    light.intensity *= (-dt * 40.0).exp();
    let (transform, fly) = *camera;
    // (For captures: `--opt fight` fires by itself, `huntfire` only at a hunter.)
    let auto = (args.opt("fight") && !args.opt("holdfire")) || (args.opt("huntfire") && !hunters.is_empty());
    if fly.noclip || !(input.fire || auto) || gun.cooldown > 0.0 {
        return;
    }
    gun.cooldown = RELOAD;
    gun.kick += 0.045;
    gun.recoil += 0.09;
    gun.shots += 1;
    light.intensity = 4.0e6;
    commands.spawn((AudioPlayer::new(assets.gun.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.8))));

    let eye = transform.translation;
    let (right, up, forward) = (transform.right(), transform.up(), transform.forward());
    let muzzle = eye + right * 0.2 - up * 0.15 + forward * 0.6;
    for k in 0..SHARDS {
        // A ring pattern with a little jitter, so a shot reads the same way
        // every time (as in Quake 3) but never quite repeats.
        let r = |j: i32| hash01(gun.shots as i32, k as i32, j, 0x5ad);
        let ring = if k == 0 { 0.0 } else if k < 7 { 0.5 } else { 1.0 };
        let a = k as f32 / if k < 7 { 6.0 } else { 9.0 } * std::f32::consts::TAU + r(0) * 0.6;
        let off = (ring + (r(1) - 0.5) * 0.25) * SPREAD;
        let dir = (*forward + *right * a.cos() * off + *up * a.sin() * off).normalize();
        let wall = spatial
            .cast_ray(eye, Dir3::new(dir).unwrap_or(Dir3::NEG_Z), RANGE, true, &default())
            .map_or(RANGE, |h| h.distance);
        // The nearest swarmer on the line, if before the wall.
        let mut best: Option<(f32, Entity)> = None;
        for (entity, t, _) in &swarm {
            let to = t.translation - eye;
            let along = to.dot(dir);
            if along <= 0.0 || along > wall {
                continue;
            }
            if (to - dir * along).length() < SWARMER_RADIUS * t.scale.x && best.is_none_or(|(d, _)| along < d) {
                best = Some((along, entity));
            }
        }
        let end = best.map_or(wall, |(d, _)| d);
        if let Some((along, entity)) = best
            && let Ok((_, mut t, mut s)) = swarm.get_mut(entity)
        {
            let point = eye + dir * along;
            if let Mode::Bound { hunter, .. } = s.mode {
                // A hunter takes the hit as a whole: the piece struck is
                // jolted, the body staggers.
                if let Ok(mut h) = hunters.get_mut(hunter) {
                    h.hurt(dir);
                }
                s.jolt += dir * 0.35;
            } else {
                s.health -= 1.0;
                s.velocity += dir * 4.0;
            }
            // The piece struck pops.
            t.scale *= 1.18;
            feedback.hits += 1;
            feedback.at += point;
            sparks(&mut commands, &assets, point, -dir, gun.shots * 31 + k as u32);
        }
        commands.spawn((
            Streak { from: muzzle, to: eye + dir * end, travelled: 0.0, impact: best.is_none() && end < RANGE },
            Mesh3d(assets.shard.clone()),
            MeshMaterial3d(assets.bright.clone()),
            // Stretched: seen nearly end-on, a short shard would be a dot.
            Transform::from_translation(muzzle).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)).with_scale(Vec3::new(1.5, 6.0, 1.5)),
        ));
    }
}

/// Shards fly out to where they hit; those that hit the world spark there.
fn fly_shards(mut commands: Commands, time: Res<Time>, assets: Res<Assets3>, mut streaks: Query<(Entity, &mut Streak, &mut Transform)>) {
    let dt = time.delta_secs();
    for (entity, mut streak, mut transform) in &mut streaks {
        let line = streak.to - streak.from;
        let length = line.length().max(0.01);
        streak.travelled += SHARD_SPEED * dt;
        if streak.travelled >= length {
            if streak.impact {
                let back = -line / length;
                for k in 0..3 {
                    let r = |j: i32| hash01(entity.index_u32() as i32, k, j, 0x5b7) - 0.5;
                    commands.spawn((
                        Debris { velocity: (back + Vec3::new(r(0), r(1) + 0.5, r(2))) * 6.0, spin: Vec3::new(r(3), r(4), r(5)) * 20.0, life: 0.35, total: 0.35, size: Vec3::splat(0.3) },
                        Mesh3d(assets.shard.clone()),
                        MeshMaterial3d(assets.bright.clone()),
                        Transform::from_translation(streak.to).with_scale(Vec3::splat(0.3)),
                    ));
                }
            }
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = streak.from + line / length * streak.travelled;
    }
}

/// The swarm: sent in groups from a distance; each one hunts the player,
/// keeping a little apart from the others, and now and then darts in.
#[allow(clippy::too_many_arguments)]
fn swarm(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    streamer: Res<Streamer>,
    world: Res<WorldGen>,
    assets: Res<Assets3>,
    mut director: ResMut<Director>,
    player: Single<(&Transform, &Player, &FlyCam)>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    let (ptransform, _, fly) = *player;
    let target = ptransform.translation - Vec3::Y * 0.3;
    // (`--opt fight` keeps it going in a capture, and fires the gun.)
    let capture = args.shot.is_some() && !args.opt("fight");
    if args.opt("peace") || capture || args.opt("bot") || !streamer.settled || fly.noclip {
        return;
    }
    director.alive += dt;
    director.next_wave -= dt;
    let count = swarm.iter().count();
    // More of them the longer you last.
    let wanted = (8.0 + director.alive / 10.0).min(40.0) as usize;
    if director.next_wave <= 0.0 && count < wanted {
        director.next_wave = 7.0;
        let t = time.elapsed_secs();
        let group = 4 + (director.alive / 40.0) as usize;
        let a = hash01(t as i32, 1, 2, 0x5a1) * std::f32::consts::TAU;
        let dist = 50.0 + 15.0 * hash01(t as i32, 3, 2, 0x5a1);
        let mut away = Vec3::new(a.cos(), 0.0, a.sin());
        if args.opt("fight") {
            // For captures: from straight ahead.
            let f = ptransform.forward();
            away = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::X);
        }
        let base = target + away * dist;
        for k in 0..group {
            let r = |j: i32| hash01(t as i32, k as i32, j, 0x5a2) - 0.5;
            let p = base + Vec3::new(r(0), 0.0, r(1)) * 8.0;
            let p = Vec3::new(p.x, world.ground_height(p.x, p.z) + 3.0 + r(2) * 2.0, p.z);
            commands
                .spawn((
                    Swarmer { mode: Mode::Free, velocity: Vec3::ZERO, health: SWARMER_HEALTH, dart_in: 2.0 + r(3) * 2.0, darting: 0.0, bite_in: 0.0, phase: r(4) * 50.0, jolt: Vec3::ZERO },
                    Mesh3d(assets.swarmer.clone()),
                    MeshMaterial3d(assets.dark.clone()),
                    Transform::from_translation(p),
                ))
                .with_child((Mesh3d(assets.core.clone()), MeshMaterial3d(assets.glow.clone()), Transform::default()));
        }
    }

    let positions: Vec<Vec3> = swarm.iter().map(|(_, t, _)| t.translation).collect();
    let t = time.elapsed_secs();
    for (_, mut transform, mut s) in &mut swarm {
        if s.mode != Mode::Free {
            continue;
        }
        transform.scale = transform.scale.lerp(Vec3::ONE, (dt * 4.0).min(1.0));
        let p = transform.translation;
        let to = target - p;
        let distance = to.length().max(0.01);
        // Hunt, keep apart, wobble.
        let mut want = to / distance * SWARMER_SPEED;
        // Hold the player's height (seen against the ground, not the sky).
        want.y += (target.y + 0.3 - p.y) * 2.0;
        for &q in &positions {
            let d = p - q;
            let l = d.length();
            if l > 0.01 && l < 1.6 {
                want += d / l * (1.6 - l) * 6.0;
            }
        }
        want += Vec3::new((t * 2.3 + s.phase).sin(), (t * 3.1 + s.phase).sin() * 0.6, (t * 1.9 + s.phase * 1.3).sin()) * 3.0;
        // Dart in when close enough.
        s.dart_in -= dt;
        s.darting -= dt;
        s.bite_in -= dt;
        if s.dart_in <= 0.0 && distance < 16.0 {
            s.darting = 0.45;
            s.dart_in = 2.0 + hash01(s.phase as i32, t as i32, 7, 0x5a3) * 2.0;
        }
        if s.darting > 0.0 {
            want = to / distance * DART_SPEED;
        }
        let accel = if s.darting > 0.0 { 60.0 } else { 14.0 };
        let change = (want - s.velocity).clamp_length_max(accel * dt);
        s.velocity += change;
        let mut next = p + s.velocity * dt;
        // Stay off the ground.
        let ground = world.ground_height(next.x, next.z) + 0.8;
        if next.y < ground {
            next.y = ground;
            s.velocity.y = s.velocity.y.max(0.0);
        }
        transform.translation = next;
        transform.rotate_local_y(dt * (3.0 + s.velocity.length() * 0.3));
        transform.rotate_local_x(dt * 1.7);
    }
}

/// A swarmer that reaches the player bites, and bounces off.
fn bite(
    mut commands: Commands,
    assets: Res<Assets3>,
    mut director: ResMut<Director>,
    mut player: Single<(&Transform, &mut Player)>,
    mut swarm: Query<(&Transform, &mut Swarmer), Without<Player>>,
) {
    let centre = player.0.translation - Vec3::Y * 0.7;
    for (transform, mut s) in &mut swarm {
        let d = transform.translation - centre;
        // The body is a capsule about 1.8 m tall.
        let flat = Vec2::new(d.x, d.z).length();
        if s.mode == Mode::Free && flat < 0.8 && d.y.abs() < 1.3 && s.bite_in <= 0.0 {
            s.bite_in = 1.0;
            s.darting = 0.0;
            s.velocity = (Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::X) + Vec3::Y * 0.15) * 12.0;
            player.1.health -= BITE;
            director.hurt = 1.0;
            commands.spawn((AudioPlayer::new(assets.hurt.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.7))));
        }
    }
}

/// Broken swarmers shatter; a dead player starts again.
#[allow(clippy::too_many_arguments)]
fn die(
    mut commands: Commands,
    args: Res<Args>,
    assets: Res<Assets3>,
    mut director: ResMut<Director>,
    mut feedback: ResMut<Feedback>,
    mut player: Single<&mut Player>,
    swarm: Query<(Entity, &Transform, &Swarmer)>,
    bodies: Query<Entity, Or<(With<hunter::Hunter>, With<hunter::Assembly>)>>,
) {
    for (entity, transform, s) in &swarm {
        if s.health > 0.0 {
            continue;
        }
        director.kills += 1;
        feedback.kill = feedback.kill.max(0.6);
        shatter(&mut commands, &assets, transform.translation, s.velocity, entity.index_u32());
        commands.entity(entity).despawn();
    }
    // (`--opt god`: for watching the swarm without dying.)
    if args.opt("god") {
        player.health = HEALTH_MAX;
    }
    if player.health <= 0.0 {
        director.best = director.best.max(director.alive);
        director.last = format!("broken after {:.0} s, {} destroyed", director.alive, director.kills);
        director.notice = 4.0;
        director.alive = 0.0;
        director.kills = 0;
        director.next_wave = 4.0;
        player.health = HEALTH_MAX;
        for (entity, ..) in &swarm {
            commands.entity(entity).despawn();
        }
        for entity in &bodies {
            commands.entity(entity).despawn();
        }
    }
}

/// Bright splinters bursting back from where a shard struck a body.
fn sparks(commands: &mut Commands, assets: &Assets3, at: Vec3, back: Vec3, seed: u32) {
    for k in 0..5 {
        let r = |j: i32| hash01(seed as i32, k, j, 0x5b9) - 0.5;
        let out = (back + Vec3::new(r(0), r(1), r(2)) * 1.6).normalize_or(back);
        commands.spawn((
            Debris { velocity: out * (9.0 + 9.0 * (r(3) + 0.5)), spin: Vec3::new(r(4), r(5), r(6)) * 30.0, life: 0.22, total: 0.22, size: Vec3::new(0.6, 0.9, 0.6) },
            Mesh3d(assets.shard.clone()),
            MeshMaterial3d(assets.bright.clone()),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::Y, out)),
        ));
    }
}

/// What landed this frame: one impact sound (louder and lower the more
/// shards struck), a flash where they struck, and the hit marker.
#[allow(clippy::type_complexity)]
fn feedback(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<Assets3>,
    mut feedback: ResMut<Feedback>,
    mut light: Single<(&mut Transform, &mut PointLight), With<ImpactLight>>,
    mut marker: Single<(&mut BorderColor, &mut Node), With<HitMarker>>,
) {
    let dt = time.delta_secs();
    let n = feedback.hits;
    if n > 0 {
        let k = n.min(12) as f32;
        commands.spawn((
            AudioPlayer::new(assets.hit.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.45 + 0.07 * k)).with_speed(1.15 - 0.025 * k),
        ));
        light.0.translation = feedback.at / n as f32;
        light.1.intensity = light.1.intensity.max(4.0e5 * k);
        feedback.marker = feedback.marker.max(0.5 + 0.05 * k);
        feedback.hits = 0;
        feedback.at = Vec3::ZERO;
    }
    light.1.intensity *= (-dt * 30.0).exp();
    feedback.marker = (feedback.marker - dt * 4.0).max(0.0);
    feedback.kill = (feedback.kill - dt * 2.0).max(0.0);
    let size = 18.0 + 14.0 * feedback.kill;
    *marker.0 = BorderColor::all(Color::srgba(1.0, 1.0, 1.0, feedback.marker.max(feedback.kill).min(1.0)));
    marker.1.width = px(size);
    marker.1.height = px(size);
}

fn shatter(commands: &mut Commands, assets: &Assets3, at: Vec3, velocity: Vec3, seed: u32) {
    shatter_quiet(commands, assets, at, velocity, seed);
    commands.spawn((
        AudioPlayer::new(assets.shatter.clone()),
        PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(1.0)),
        Transform::from_translation(at),
    ));
}

/// The pieces of something broken, without its sound.
fn shatter_quiet(commands: &mut Commands, assets: &Assets3, at: Vec3, velocity: Vec3, seed: u32) {
    for k in 0..9 {
        let r = |j: i32| hash01(seed as i32, k, j, 0x5b8) - 0.5;
        let out = Vec3::new(r(0), r(1) + 0.3, r(2)).normalize_or(Vec3::Y);
        commands.spawn((
            Debris { velocity: velocity * 0.4 + out * (5.0 + 6.0 * (r(3) + 0.5)), spin: Vec3::new(r(4), r(5), r(6)) * 14.0, life: 1.6, total: 1.6, size: Vec3::new(2.5, 0.5, 2.5) },
            Mesh3d(assets.shard.clone()),
            MeshMaterial3d(assets.dark.clone()),
            Transform::from_translation(at).with_scale(Vec3::new(2.5, 0.5, 2.5)),
        ));
    }
}

/// Pieces fall, bounce off the ground and shrink away.
fn debris(mut commands: Commands, time: Res<Time>, world: Res<WorldGen>, mut pieces: Query<(Entity, &mut Debris, &mut Transform)>) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut d, mut transform) in &mut pieces {
        d.life -= dt;
        if d.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        d.velocity.y -= 18.0 * dt;
        let mut p = transform.translation + d.velocity * dt;
        let ground = world.ground_height(p.x, p.z) + 0.05;
        if p.y < ground {
            p.y = ground;
            d.velocity = Vec3::new(d.velocity.x * 0.5, -d.velocity.y * 0.3, d.velocity.z * 0.5);
        }
        transform.translation = p;
        let spin = d.spin * dt;
        transform.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
        transform.scale = d.size * (d.life / d.total).sqrt().max(0.2);
    }
}

/// The swarm is heard from the middle of the nearest ones, louder the more
/// and the closer they are.
fn swarm_sound(
    player: Single<&Transform, (With<Player>, Without<SwarmVoice>)>,
    swarm: Query<&Transform, (With<Swarmer>, Without<SwarmVoice>, Without<Player>)>,
    mut voice: Query<(&mut Transform, &mut AudioSink), With<SwarmVoice>>,
) {
    let Ok((mut transform, mut sink)) = voice.single_mut() else { return };
    let ear = player.translation;
    let (mut sum, mut weight) = (Vec3::ZERO, 0.0);
    for t in &swarm {
        let w = 1.0 / (1.0 + (t.translation - ear).length_squared() * 0.01);
        sum += t.translation * w;
        weight += w;
    }
    if weight > 0.0 {
        transform.translation = sum / weight;
    }
    sink.set_volume(Volume::Linear((weight * 0.5).min(1.2)));
}

fn hud(
    time: Res<Time>,
    mut director: ResMut<Director>,
    mut flash: Single<&mut BackgroundColor, With<HurtFlash>>,
    mut score: Single<&mut Text, With<Score>>,
) {
    let dt = time.delta_secs();
    director.hurt = (director.hurt - dt * 2.5).max(0.0);
    flash.0 = Color::srgba(0.0, 0.0, 0.0, director.hurt * 0.55);
    director.notice -= dt;
    score.0 = if director.notice > 0.0 {
        director.last.clone()
    } else if director.alive > 0.0 {
        format!("{:.0} s   {} destroyed   best {:.0} s", director.alive, director.kills, director.best)
    } else {
        String::new()
    };
}

/// The view kicks up with each shot and settles back.
fn kick(time: Res<Time>, mut gun: ResMut<Gun>, mut camera: Single<&mut Transform, (With<FlyCam>, Without<Viewmodel>)>) {
    gun.kick *= (-time.delta_secs() * 9.0).exp();
    camera.rotate_local_x(gun.kick);
}

/// The gun recoils back with each shot.
fn viewmodel(time: Res<Time>, mut gun: ResMut<Gun>, mut model: Single<&mut Transform, With<Viewmodel>>) {
    gun.recoil *= (-time.delta_secs() * 10.0).exp();
    model.translation = Vec3::new(0.2, -0.17 - gun.recoil * 0.3, -0.42 + gun.recoil);
    model.rotation = Quat::from_rotation_x(gun.recoil * 2.0);
}
