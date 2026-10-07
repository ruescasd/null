//! Combat, a prototype: the shard shotgun, the swarm, the player's health,
//! and their sounds.
//!
//! The shotgun (Mouse 1) throws a spray of shards: hitscan, each shard drawn
//! as a bright streak flying out to what it hit. The swarm comes three at a
//! time, buds (see `forms.rs`), the next three once these are gone. They do
//! not bite: they bring darkness, the nearer and the more of them the darker
//! (`gloom`), and they stalk you, moving only where you are not looking, to
//! places behind you, where they wait (`stalk`). Settled there together they
//! weave themselves into a hunter with cables (see `hunter.rs`, `web.rs`),
//! unless you turn and break one first. A few shards break one. `--opt peace`
//! leaves them out; `--opt fight` keeps them in a capture and fires the gun
//! by itself (`--opt holdfire` stops it).

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
mod forms;
mod grove;
mod hair;
mod ichor;
mod weapon;
mod web;

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
        embedded_asset!(app, "sounds/growl0.wav");
        embedded_asset!(app, "sounds/growl1.wav");
        embedded_asset!(app, "sounds/growl2.wav");
        embedded_asset!(app, "sounds/growl3.wav");
        embedded_asset!(app, "sounds/growl4.wav");
        embedded_asset!(app, "sounds/growl5.wav");
        embedded_asset!(app, "sounds/growl6.wav");
        embedded_asset!(app, "sounds/pounce.wav");
        embedded_asset!(app, "sounds/shot.wav");
        embedded_asset!(app, "sounds/reload.wav");
        embedded_asset!(app, "sounds/bite.wav");
        // Spatial sounds fade with the square of the distance; at this scale
        // something 10 m away is heard at about two thirds, 25 m at a tenth.
        app.insert_resource(bevy::audio::DefaultSpatialScale(bevy::audio::SpatialScale::new(0.12)));
        app.init_resource::<Gun>()
            .init_resource::<Director>()
            .init_resource::<Feedback>()
            .add_systems(PostStartup, setup)
            .add_systems(Startup, (ichor::setup, hunter::setup, forms::setup))
            .add_systems(
                Update,
                (fire, fly_shards, ichor::fly, ichor::burst, swarm, grove::grove, gloom, hunter::gather, web::web, hunter::hunt, hunter::voice, hunter::flesh, hunter::cables, hunter::watch, die, feedback, debris, swarm_sound, hud)
                    .chain()
                    .after(crate::player::walk),
            )
            .add_systems(Update, (forms::lab, forms::gaze, hair::hair).chain().after(swarm))
            .add_systems(PostUpdate, (kick, weapon::animate, weapon::effects).before(TransformSystems::Propagate));
    }
}

/// Seconds between shots.
const RELOAD: f32 = 1.5;
/// Shards per shot, the cone they spread in (radians, half-angle), how far
/// they reach and how fast they are drawn flying.
const SHARDS: usize = 16;
const SPREAD: f32 = 0.085;
const RANGE: f32 = 150.0;
const SHARD_SPEED: f32 = 260.0;

/// A swarmer: how many shards break it, its size, and how fast it flies
/// (faster than a running player).
const SWARMER_HEALTH: f32 = 3.0;
/// Shut, a bud is armoured: shards glance off it (knocking it back) until it
/// is this far open (`--opt noarmour`: never).
const ARMOUR: f32 = 0.5;
const SWARMER_RADIUS: f32 = 0.55;
const SWARMER_SPEED: f32 = 18.0;
/// How far off a swarmer starts to darken the world, and how near it does
/// so fully.
const GLOOM_REACH: f32 = 20.0;
const GLOOM_FULL: f32 = 3.0;
/// Within this, a swarmer you look at stops dead.
const STILL_WITHIN: f32 = 12.0;

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
    /// The view's shake, 0..1, decaying.
    shake: f32,
    /// Seconds until the reload is heard.
    reload_in: Option<f32>,
}

/// Sends the swarm: a group at a distance once the last one is gone (and
/// whatever it made).
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
    /// Pale and matte: the markings on creatures that do not glow.
    pale: Handle<StandardMaterial>,
    /// The shot (a recording; `--opt oldgun` for the synthesised one), and
    /// the reload as the pump racks.
    gun: Handle<AudioSource>,
    reload: Handle<AudioSource>,
    shatter: Handle<AudioSource>,
    hurt: Handle<AudioSource>,
    assemble: Handle<AudioSource>,
    hit: Handle<AudioSource>,
    death: Handle<AudioSource>,
    /// A hunter's growls: phrases cut from recordings, and their lengths.
    growls: Vec<(Handle<AudioSource>, f32)>,
    pounce: Handle<AudioSource>,
    bite: Handle<AudioSource>,
}

/// What landed this frame, for the feedback: how many shards hit something
/// and where (summed), and the hit marker and impact light, decaying.
#[derive(Resource, Default)]
struct Feedback {
    hits: u32,
    /// Shards that glanced off armour.
    glances: u32,
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

/// What a swarmer is doing: asleep in a grove, stalking on its own, or drawn
/// into an assembly.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Dormant,
    Free,
    Gather(Entity),
}

#[derive(Component)]
struct Swarmer {
    mode: Mode,
    velocity: Vec3,
    health: f32,
    phase: f32,
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
    mut images: ResMut<Assets<Image>>,
) {
    let load = |name: &str| server.load::<AudioSource>(format!("embedded://game/sounds/{name}.wav"));
    // A long thin shard, pointing along +Y.
    let shard = meshes.add(shard_mesh(&[(Vec3::Y, 0.7, 0.025), (Vec3::NEG_Y, 0.05, 0.025)]));
    // A knot of a few long shards (pieces of the hunters).
    let mut spikes = Vec::new();
    for k in 0..7 {
        let d = Vec3::new(hash01(k, 0, 1, 0x5a) - 0.5, hash01(k, 1, 1, 0x5a) - 0.5, hash01(k, 2, 1, 0x5a) - 0.5).normalize();
        spikes.push((d, SWARMER_RADIUS * (0.7 + 0.6 * hash01(k, 3, 1, 0x5a)), 0.12));
    }
    let swarmer = meshes.add(shard_mesh(&spikes));
    let core = meshes.add(Sphere::new(0.13).mesh().ico(1).unwrap());
    let bright = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(60.0, 60.0, 60.0), ..default() });
    // Dark against the bright ground.
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.03, 0.03),
        perceptual_roughness: 0.35,
        reflectance: 0.6,
        ..default()
    });
    let assets = Assets3 {
        shard: shard.clone(),
        swarmer,
        core,
        bright: bright.clone(),
        dark: dark.clone(),
        pale: materials.add(StandardMaterial { base_color: Color::srgb(0.72, 0.72, 0.72), perceptual_roughness: 0.7, ..default() }),
        gun: load(if args.opt("oldgun") { "gun" } else { "shot" }),
        reload: load("reload"),
        shatter: load("shatter"),
        hurt: load("hurt"),
        assemble: load("assemble"),
        hit: load("hit"),
        death: load("death"),
        growls: [1.40, 2.10, 1.55, 1.10, 1.10, 1.40, 1.15].iter().enumerate().map(|(k, &l)| (load(&format!("growl{k}")), l)).collect(),
        pounce: load("pounce"),
        bite: load("bite"),
    };

    // The ear is the camera.
    commands.entity(*camera).insert(SpatialListener::new(2.0));
    // The gun in view (see `weapon.rs`), and the flash at its muzzle.
    let gun = weapon::spawn(&mut commands, &mut meshes, &mut materials, &mut images, assets.pale.clone());
    commands.entity(*camera).add_child(gun);
    commands.entity(*camera).with_children(|parent| {
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
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer, &forms::Gazed), Without<FlyCam>>,
    mut hunters: Query<(Entity, &mut hunter::Hunter)>,
    ichor: Res<ichor::Ichor>,
    fx: Res<weapon::Fx>,
    viewmodel: Single<Entity, With<weapon::Viewmodel>>,
    mut light: Single<&mut PointLight, With<MuzzleLight>>,
) {
    let dt = time.delta_secs();
    gun.cooldown -= dt;
    if let Some(t) = gun.reload_in.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            gun.reload_in = None;
            commands.spawn((AudioPlayer::new(assets.reload.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.6))));
        }
    }
    light.intensity *= (-dt * 40.0).exp();
    let (transform, fly) = *camera;
    // (For captures: `--opt fight` fires by itself, `huntfire` only at a hunter.)
    let auto = (args.opt("fight") && !args.opt("holdfire")) || (args.opt("huntfire") && !hunters.is_empty());
    if fly.noclip || !(input.fire || auto) || gun.cooldown > 0.0 {
        return;
    }
    gun.cooldown = RELOAD;
    gun.kick += 0.07;
    gun.shake = 1.0;
    gun.recoil += 0.11;
    gun.shots += 1;
    light.intensity = 1.2e7;
    commands.spawn((AudioPlayer::new(assets.gun.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.96))));
    gun.reload_in = Some(0.12);

    let eye = transform.translation;
    let (right, up, forward) = (transform.right(), transform.up(), transform.forward());
    let muzzle = eye + right * weapon::MUZZLE_VIEW.x + up * weapon::MUZZLE_VIEW.y - forward * weapon::MUZZLE_VIEW.z;
    weapon::fire(&mut commands, &fx, *viewmodel, muzzle, *forward, *right, *up, gun.shots);
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
        for (entity, t, ..) in &swarm {
            let to = t.translation - eye;
            let along = to.dot(dir);
            if along <= 0.0 || along > wall {
                continue;
            }
            if (to - dir * along).length() < SWARMER_RADIUS * t.scale.x && best.is_none_or(|(d, _)| along < d) {
                best = Some((along, entity));
            }
        }
        // Or a hunter's body, nearer still: it takes the hit as a whole.
        let mut body: Option<(f32, Entity, usize)> = None;
        for (entity, h) in &hunters {
            let limit = best.map_or(wall, |(d, _)| d);
            if let Some((d, bone)) = h.ray(eye, dir, limit)
                && body.is_none_or(|(b, ..)| d < b)
            {
                body = Some((d, entity, bone));
            }
        }
        let mut end = best.map_or(wall, |(d, _)| d);
        let mut struck = None;
        if let Some((d, entity, bone)) = body
            && let Ok((_, mut h)) = hunters.get_mut(entity)
        {
            h.hurt(bone, dir);
            end = d;
            struck = Some(eye + dir * d);
        } else if let Some((along, entity)) = best
            && let Ok((_, mut t, mut s, gazed)) = swarm.get_mut(entity)
        {
            if gazed.openness() < ARMOUR && !args.opt("noarmour") {
                // Shut: it glances off the petals, knocking the bud back, and
                // sparks; no harm done.
                t.translation += dir * 0.3;
                let point = eye + dir * along;
                feedback.glances += 1;
                sparks(&mut commands, &assets, point, -dir, gun.shots * 41 + k as u32);
                end = along;
                commands.spawn((
                    Streak { from: muzzle, to: point, travelled: 0.0, impact: false },
                    Mesh3d(assets.shard.clone()),
                    MeshMaterial3d(assets.bright.clone()),
                    Transform::from_translation(muzzle).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)).with_scale(Vec3::new(1.5, 6.0, 1.5)),
                ));
                continue;
            }
            s.health -= 1.0;
            s.velocity += dir * 4.0;
            // It pops.
            t.scale *= 1.18;
            struck = Some(eye + dir * along);
        }
        if let Some(point) = struck {
            feedback.hits += 1;
            feedback.at += point;
            sparks(&mut commands, &assets, point, -dir, gun.shots * 31 + k as u32);
            ichor::spray(&mut commands, &ichor, point, dir, 1.0, gun.shots * 37 + k as u32);
        }
        commands.spawn((
            Streak { from: muzzle, to: eye + dir * end, travelled: 0.0, impact: struck.is_none() && end < RANGE },
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

/// The swarm: sent a group at a time from a distance, the next once the last
/// is gone; each one stalks the player (see `stalk`).
#[allow(clippy::too_many_arguments)]
fn swarm(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    streamer: Res<Streamer>,
    world: Res<WorldGen>,
    kit: Res<forms::Kit>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut director: ResMut<Director>,
    player: Single<(&Transform, &Player, &FlyCam)>,
    hunters: Query<(), Or<(With<hunter::Hunter>, With<hunter::Assembly>)>>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    let (ptransform, _, fly) = *player;
    let target = ptransform.translation - Vec3::Y * 0.3;
    // (`--opt fight` keeps it going in a capture, and fires the gun.)
    let capture = args.shot.is_some() && !args.opt("fight");
    // (Playing, it starts once the ground round you is in; measuring or
    // capturing, once everything is, so it happens in view.)
    let ready = if args.unattended() { streamer.settled } else { streamer.near };
    if args.opt("peace") || capture || args.opt("bot") || !ready || (fly.noclip && !args.opt("specimen")) {
        return;
    }
    director.alive += dt;
    // The next group a while after the last is gone, and what it made.
    // (`--set wave=N`, `wave_every=S`, `wave_at=M`: N at a time, S seconds
    // after, M metres off.)
    if !swarm.is_empty() || !hunters.is_empty() {
        director.next_wave = director.next_wave.max(args.num("wave_every", 6.0));
    } else {
        director.next_wave -= dt;
    }
    if director.next_wave <= 0.0 && swarm.is_empty() && !args.opt("hunters") && !args.opt("specimen") {
        let t = time.elapsed_secs();
        // (`--opt grove`: a grove of them asleep on the ground, nearer; see
        // `grove.rs`.)
        let grove = args.opt("grove");
        let group = args.num("wave", if grove { 7.0 } else { 3.0 }) as usize;
        let a = hash01(t as i32, 1, 2, 0x5a1) * std::f32::consts::TAU;
        let dist = args.num("wave_at", if grove { 30.0 } else { 50.0 }) + 15.0 * hash01(t as i32, 3, 2, 0x5a1);
        let mut away = Vec3::new(a.cos(), 0.0, a.sin());
        if args.opt("fight") {
            // For captures: from straight ahead.
            let f = ptransform.forward();
            away = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::X);
        }
        let base = target + away * dist;
        for k in 0..group {
            let r = |j: i32| hash01(t as i32, k as i32, j, 0x5a2) - 0.5;
            let p = base + Vec3::new(r(0), 0.0, r(1)) * if grove { 14.0 } else { 8.0 };
            let mode = if grove { Mode::Dormant } else { Mode::Free };
            let swarmer = Swarmer { mode, velocity: Vec3::ZERO, health: SWARMER_HEALTH, phase: r(4) * 50.0 };
            let root = if grove {
                // Low on its roots, opening to the sky, leaning a little.
                let p = Vec3::new(p.x, world.ground_height(p.x, p.z) + 0.55, p.z);
                let lean = Quat::from_euler(EulerRot::XYZ, r(5) * 0.6, 0.0, r(6) * 0.6);
                let transform = Transform::from_translation(p).looking_to(lean * Vec3::Y, Vec3::Z);
                commands.spawn((swarmer, forms::Gazed::default(), forms::Asleep, transform, Visibility::default())).id()
            } else {
                let p = Vec3::new(p.x, world.ground_height(p.x, p.z) + 3.0 + r(2) * 2.0, p.z);
                commands.spawn((swarmer, forms::Gazed::default(), Transform::from_translation(p), Visibility::default())).id()
            };
            forms::build(&mut commands, &kit, &mut materials, root);
        }
    }

    let positions: Vec<Vec3> = swarm.iter().map(|(_, t, _)| t.translation).collect();
    for (_, mut transform, mut s) in &mut swarm {
        if s.mode != Mode::Free {
            continue;
        }
        transform.scale = transform.scale.lerp(Vec3::ONE, (dt * 4.0).min(1.0));
        stalk(&world, ptransform, &positions, &mut transform, &mut s, dt);
    }
}

/// How a swarmer moves. Close by it moves only where you are not looking:
/// each has a place of its own a few metres behind you, and goes there and
/// waits, dead still; looked at, it stops dead where it is. Turn round and
/// they are all still; turn back and they have moved behind you again.
/// Arriving in front of you, they swing wide round your side.
fn stalk(world: &WorldGen, player: &Transform, positions: &[Vec3], transform: &mut Transform, s: &mut Swarmer, dt: f32) {
    let p = transform.translation;
    let eye = player.translation;
    // (It turns to face you, even while it stands still.)
    face(transform, eye, dt);
    let seen = (p - eye).normalize_or(Vec3::Y).dot(*player.forward()) > 0.55;
    if seen && p.distance(eye) < STILL_WITHIN {
        s.velocity = Vec3::ZERO;
        return;
    }
    // Its place: within about 60 degrees of straight behind you, 4 to 8 m
    // off, about head height.
    let h = |j: i32| hash01((s.phase * 1000.0) as i32, j, 0, 0x5c1);
    let back = Vec3::new(-player.forward().x, 0.0, -player.forward().z).normalize_or(Vec3::X);
    let around = Quat::from_rotation_y((h(0) - 0.5) * 2.1) * back;
    let mut place = eye + around * (4.0 + 4.0 * h(1)) + Vec3::Y * (h(2) * 1.5 - 0.6);
    // Not yet behind you: out to your side first, wide.
    let right = Vec3::new(-back.z, 0.0, back.x);
    let rel = Vec3::new(p.x - eye.x, 0.0, p.z - eye.z);
    if rel.dot(back) < 2.0 {
        let side = if rel.dot(right) < 0.0 { -1.0 } else { 1.0 };
        place = eye + right * side * 14.0 + back * 6.0 + Vec3::Y * (place.y - eye.y);
    }
    let to = place - p;
    let distance = to.length();
    let mut want = to.normalize_or(Vec3::ZERO) * SWARMER_SPEED * (distance / 3.0).min(1.0);
    for &q in positions {
        let d = p - q;
        let l = d.length();
        if l > 0.01 && l < 1.6 {
            want += d / l * (1.6 - l) * 6.0;
        }
    }
    s.velocity += (want - s.velocity).clamp_length_max(20.0 * dt);
    let mut next = p + s.velocity * dt;
    let ground = world.ground_height(next.x, next.z) + 0.8;
    if next.y < ground {
        next.y = ground;
        s.velocity.y = s.velocity.y.max(0.0);
    }
    transform.translation = next;
}

/// A swarmer turns, slowly, to face you.
fn face(transform: &mut Transform, eye: Vec3, dt: f32) {
    let to = eye - transform.translation;
    if to.length_squared() < 1e-4 {
        return;
    }
    let want = Transform::IDENTITY.looking_to(to, Vec3::Y).rotation;
    transform.rotation = transform.rotation.slerp(want, (dt * 3.0).min(1.0));
}

/// The swarm does not bite; it brings darkness. Each swarmer within reach
/// darkens the world, more the nearer it is, and together they all but blind
/// you (`--set gloom_each`: how much one close by does); it closes in fast
/// and lifts slowly. (Those gathering into a hunter count too: it forms in
/// the dark, and the dark lifts off it.)
fn gloom(
    time: Res<Time>,
    args: Res<Args>,
    player: Single<&Transform, With<Player>>,
    swarm: Query<(&Transform, &Swarmer), Without<Player>>,
    mut gloom: ResMut<crate::look::Gloom>,
) {
    let dt = time.delta_secs().min(0.05);
    // (Asleep, they do nothing.)
    let sum: f32 = swarm
        .iter()
        .filter(|(_, s)| s.mode != Mode::Dormant)
        .map(|(t, _)| ((GLOOM_REACH - t.translation.distance(player.translation)) / (GLOOM_REACH - GLOOM_FULL)).clamp(0.0, 1.0).powi(2))
        .sum();
    let target = 1.0 - (-args.num("gloom_each", 0.3) * sum).exp();
    let rate = if target > gloom.0 { 2.0 } else { 0.7 };
    gloom.0 += (target - gloom.0) * (rate * dt).min(1.0);
}

/// Broken swarmers shatter; a dead player starts again.
#[allow(clippy::too_many_arguments)]
fn die(
    mut commands: Commands,
    args: Res<Args>,
    assets: Res<Assets3>,
    mut director: ResMut<Director>,
    mut feedback: ResMut<Feedback>,
    ichor: Res<ichor::Ichor>,
    mut player: Single<&mut Player>,
    swarm: Query<(Entity, &Transform, &Swarmer)>,
    bodies: Query<Entity, Or<(With<hunter::Hunter>, With<hunter::Assembly>, With<hunter::Part>, With<hunter::Eye>)>>,
) {
    for (entity, transform, s) in &swarm {
        if s.health > 0.0 {
            continue;
        }
        director.kills += 1;
        feedback.kill = feedback.kill.max(0.6);
        ichor::spray(&mut commands, &ichor, transform.translation, s.velocity.normalize_or(Vec3::Y), 2.5, entity.index_u32());
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
    if feedback.glances > 0 {
        commands.spawn((
            AudioPlayer::new(assets.hit.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.35 + 0.04 * feedback.glances.min(10) as f32)).with_speed(2.3),
        ));
        feedback.glances = 0;
    }
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
    mut voice: Query<(&mut Transform, &mut SpatialAudioSink), With<SwarmVoice>>,
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
/// The view kicks up with each shot and settles back; it shakes for an
/// instant, and its field of view punches out and back.
fn kick(time: Res<Time>, mut gun: ResMut<Gun>, mut camera: Single<(&mut Transform, &mut Projection), (With<FlyCam>, Without<weapon::Viewmodel>)>) {
    let dt = time.delta_secs();
    gun.kick *= (-dt * 9.0).exp();
    gun.shake *= (-dt * 18.0).exp();
    let (transform, projection) = &mut *camera;
    transform.rotate_local_x(gun.kick);
    let t = time.elapsed_secs();
    let s = gun.shake * 0.012;
    transform.rotate_local_y(s * (t * 83.0).sin());
    transform.rotate_local_z(s * 1.5 * (t * 71.0).cos());
    if let Projection::Perspective(p) = &mut **projection {
        p.fov = (65.0 + 5.0 * gun.shake).to_radians();
    }
}
