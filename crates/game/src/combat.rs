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
    terrain::Streamer,
};

mod hunter;
pub(crate) use hunter::{clip::folder as creatures_folder, clip::names as creatures, trace, trace_file};
pub(crate) use hunter::STEP_UP as CREATURE_STEP;
mod cape;
mod slab;
mod lance;
mod halo;
mod beam;
mod envelop;
mod strike;
mod stalker;
mod skull;
mod pieces;
mod chain;
/// A hunter, for what counts them from outside (the benches).
pub(crate) use hunter::Hunter as HunterMarker;
mod bishop;
mod impaled;
mod tumbler;
mod forms;
mod grove;
mod hair;
mod ichor;
mod leech;
mod weapon;
mod web;
mod round;
mod skeleton;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "sounds/shard_break.wav");
        embedded_asset!(app, "sounds/swarm_voice.wav");
        embedded_asset!(app, "sounds/ground_hum.wav");
        embedded_asset!(app, "sounds/impaled_hum.wav");
        embedded_asset!(app, "sounds/swarm_assemble.wav");
        embedded_asset!(app, "sounds/shard_hit_synth.wav");
        embedded_asset!(app, "sounds/shard_hit_stab.wav");
        embedded_asset!(app, "sounds/hunter_death.wav");
        embedded_asset!(app, "sounds/hunter_growl0.wav");
        embedded_asset!(app, "sounds/hunter_growl1.wav");
        embedded_asset!(app, "sounds/hunter_growl2.wav");
        embedded_asset!(app, "sounds/hunter_growl3.wav");
        embedded_asset!(app, "sounds/hunter_growl4.wav");
        embedded_asset!(app, "sounds/hunter_growl5.wav");
        embedded_asset!(app, "sounds/hunter_growl6.wav");
        embedded_asset!(app, "sounds/stalker_step.wav");
        embedded_asset!(app, "sounds/stalker_pain.wav");
        embedded_asset!(app, "sounds/stalker_death.wav");
        embedded_asset!(app, "sounds/stalker_fall_cry.wav");
        embedded_asset!(app, "sounds/stalker_shot.wav");
        embedded_asset!(app, "sounds/stalker_impact.wav");
        embedded_asset!(app, "sounds/hunter_pounce.wav");
        embedded_asset!(app, "sounds/gun_shot_rolling.wav");
        embedded_asset!(app, "sounds/gun_reload.wav");
        embedded_asset!(app, "sounds/hunter_bite.wav");
        embedded_asset!(app, "sounds/bishop_cast.wav");
        embedded_asset!(app, "sounds/bishop_death.wav");
        embedded_asset!(app, "sounds/player_pain.wav");
        embedded_asset!(app, "sounds/bishop_call.wav");
        embedded_asset!(app, "sounds/bishop_pain.wav");
        embedded_asset!(app, "sounds/bishop_blink.wav");
        embedded_asset!(app, "sounds/bishop_mote_burst.wav");
        embedded_asset!(app, "sounds/player_step0.wav");
        embedded_asset!(app, "sounds/player_step1.wav");
        embedded_asset!(app, "sounds/player_step2.wav");
        embedded_asset!(app, "sounds/player_step3.wav");
        embedded_asset!(app, "sounds/player_step4.wav");
        embedded_asset!(app, "sounds/player_step5.wav");
        // Spatial sounds fade with the square of the distance; at this scale
        // something 10 m away is heard at about two thirds, 25 m at a tenth.
        embedded_asset!(app, "combat/beam.wgsl");
        embedded_asset!(app, "combat/impaled/plume.wgsl");
        embedded_asset!(app, "combat/impaled/haze.wgsl");
        embedded_asset!(app, "combat/impaled/tube.wgsl");
        embedded_asset!(app, "combat/tumbler/fold.wgsl");
        embedded_asset!(app, "combat/tumbler/vortex.wgsl");
        embedded_asset!(app, "combat/tumbler/scars.wgsl");
        embedded_asset!(app, "combat/envelop.wgsl");
        app.add_plugins((MaterialPlugin::<beam::Ray>::default(), MaterialPlugin::<envelop::Sky>::default(), MaterialPlugin::<envelop::World>::default(), MaterialPlugin::<impaled::plume::Plume>::default(), MaterialPlugin::<impaled::haze::Haze>::default(), MaterialPlugin::<impaled::tube::TubeMaterial>::default(), MaterialPlugin::<tumbler::FoldMaterial>::default(), MaterialPlugin::<tumbler::Vortex>::default(), MaterialPlugin::<tumbler::scars::ScarMaterial>::default()));
        app.insert_resource(bevy::audio::DefaultSpatialScale(bevy::audio::SpatialScale::new(0.12)));
        app.init_resource::<Gun>()
            .init_resource::<Director>()
            .init_resource::<Feedback>()
            .init_resource::<round::Round>()
            .init_resource::<round::Gates>()
            .add_plugins(hunter::clip::plugin)
            .add_plugins(pieces::plugin)
            .add_plugins(hunter::mace::plugin)
            .add_systems(PostStartup, setup)
            .add_systems(Startup, (round::setup, ichor::setup, hunter::setup, beam::setup, envelop::setup, strike::setup, forms::setup, bishop::settle, cape::hang))
            .add_systems(
                Update,
                (fire, fly_shards, ichor::fly, ichor::burst, swarm, grove::grove, leech::leech, gloom, hunter::gather, web::web, hunter::hunt, hunter::voice, hunter::flesh, hunter::cables, hunter::watch, (round::track, die, round::gates).chain(), feedback, debris, swarm_sound, hud)
                    .chain()
                    .after(crate::player::walk)
                    .run_if(crate::loading::playing),
            )
            .add_systems(Update, choose.before(fire).run_if(crate::loading::playing))
            .add_systems(Update, (hunter::apart.after(hunter::hunt), hunter::keep_on_nodes.after(hunter::apart)).before(hunter::flesh).run_if(crate::loading::playing))
            // (Not till the game is on: see `loading.rs`.)
            .add_systems(Update, (forms::gaze, hair::hair).chain().after(swarm).run_if(crate::loading::playing))
            .add_systems(Update, footsteps.after(crate::player::walk).run_if(crate::loading::playing)).add_systems(Update, hunter::bodies.before(crate::player::walk).run_if(crate::loading::playing))
            .init_resource::<impaled::field::Pushes>()
            .add_systems(Startup, impaled::field::setup)
            .add_systems(Update, (impaled::arrive, impaled::fly, impaled::arms::adopt, impaled::arms::bend, impaled::arms::draw,impaled::attack::rings, impaled::field::show, impaled::field::beams).chain().after(crate::player::walk).before(hunter::spawn).run_if(crate::loading::playing))
            .init_resource::<tumbler::Parts>()
            .add_systems(Update, (tumbler::arrive, tumbler::cut, tumbler::tumble, tumbler::splinters, tumbler::bolts, tumbler::crack::cracks).chain().after(crate::player::walk).before(hunter::spawn).run_if(crate::loading::playing))
            .add_systems(Update, (hunter::skeleton.after(hunter::flesh), hunter::loads.after(hunter::flesh), lance::place.after(hunter::flesh), halo::animate, hunter::footfalls.after(hunter::hunt), hunter::later, beam::shots, envelop::envelop, strike::strikes, strike::scars, strike::hold, strike::floaters, cape::cloaks.after(hunter::flesh), slab::pulse, chain::chains.after(hunter::flesh), needles, (bishop::arrive, bishop::float, bishop::carry, bishop::gleam, bishop::ghosts, bishop::bolts, bishop::flashes).chain().before(hunter::spawn), hunter::spawn).run_if(crate::loading::playing))
            .add_systems(PostUpdate, (kick, weapon::animate, weapon::effects).before(TransformSystems::Propagate));
    }
}

/// Shard `k` of shot `shot`, from a view looking along `forward`: a ring
/// pattern with a little jitter, so a shot reads the same way every time (as
/// in Quake 3) but never quite repeats.
fn shard_dir(shot: u32, k: usize, forward: Vec3, right: Vec3, up: Vec3) -> Vec3 {
    let r = |j: i32| hash01(shot as i32, k as i32, j, 0x5ad);
    let ring = if k == 0 { 0.0 } else if k < 7 { 0.5 } else { 1.0 };
    let a = k as f32 / if k < 7 { 6.0 } else { 9.0 } * std::f32::consts::TAU + r(0) * 0.6;
    let off = (ring + (r(1) - 0.5) * 0.25) * SPREAD;
    (forward + right * a.cos() * off + up * a.sin() * off).normalize()
}

/// Seconds between shots.
const RELOAD: f32 = 1.5;
/// How far into the reload's sound its last clack has died away (s): it
/// is started so as to end as you may fire again, telling you so (the
/// user); `--set reload_at`, how long after the shot it starts instead.
/// (Started 0.12 s after the shot, it was done halfway through the wait.)
const RELOAD_SOUND: f32 = 0.8;
/// Shards per shot, the cone they spread in (radians, half-angle), how far
/// they reach and how fast they are drawn flying.
const SHARDS: usize = 16;
const SPREAD: f32 = 0.085;
const RANGE: f32 = 150.0;
const SHARD_SPEED: f32 = 260.0;
/// Your footsteps: how many recorded, and how far a stride (m).
const STEPS: usize = 6;
const STRIDE: f32 = 2.4;
/// How loud a step (30% up from 0.9, the user).
const STEP_VOLUME: f32 = 1.17;
/// What a shard does to what it strikes.
const DAMAGE: f32 = 1.8;
/// How hard a shot pushes you back (m/s).
const KICKBACK: f32 = 3.0;

/// A swarmer: how many shards break it, its size, and how fast it flies
/// (faster than a running player).
const SWARMER_HEALTH: f32 = 3.0;
const SWARMER_RADIUS: f32 = 0.55;
const SWARMER_SPEED: f32 = 18.0;
/// How often a bud's way along a built place's floors is found again (s),
/// sooner if its place has moved this far from the way's end (m).
const ROUTE: f32 = 3.0;
const ROUTE_MOVED: f32 = 4.0;
/// How far off a swarmer starts to darken the world, and how near it does
/// so fully.
const GLOOM_REACH: f32 = 20.0;
const GLOOM_FULL: f32 = 3.0;
/// Within this, a swarmer you look at stops dead.
const STILL_WITHIN: f32 = 12.0;
/// A swarmer slower than this is still, and casts its darkness, which
/// creeps in over `DARK_IN` seconds and fades over `DARK_OUT` once it moves.
const STILL: f32 = 1.5;
const DARK_IN: f32 = 2.0;
const DARK_OUT: f32 = 0.8;

#[derive(Resource, Default)]
struct Gun {
    /// Time until it can fire again.
    cooldown: f32,
    /// The view's kick, radians, decaying.
    kick: f32,
    /// The view jolted by a blow (pitch, roll; radians, decaying).
    jolt: Vec2,
    /// The viewmodel's recoil, metres back, decaying.
    recoil: f32,
    /// Shots fired, for the shards' pattern.
    shots: u32,
    /// The view's shake, 0..1, decaying.
    shake: f32,
    /// Seconds until the reload is heard, and how long after the shot it is
    /// (the pump racks with it: see `weapon::animate`).
    reload_in: Option<f32>,
    reload_at: f32,
    /// Nothing in your hands (the user: to judge what you see with nothing
    /// in the way): key 1; the shotgun, key 3 (2 kept for the next weapon).
    /// Nothing in clean captures (`--opt clean`) and the room lab's.
    empty: bool,
}

/// Keys 1 (nothing) and 3 (the shotgun) choose what is in your hands; the
/// gun shows only if it is (and not while a screenshot is taken, see
/// `capture::Hiding`).
fn choose(keys: Res<ButtonInput<KeyCode>>, mut gun: ResMut<Gun>, mut model: Single<&mut Visibility, With<weapon::Viewmodel>>, hiding: Res<crate::capture::Hiding>) {
    if keys.just_pressed(KeyCode::Digit1) {
        gun.empty = true;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        gun.empty = false;
    }
    let shown = if gun.empty || hiding.0 { Visibility::Hidden } else { Visibility::Inherited };
    if **model != shown {
        **model = shown;
    }
}

/// When the swarm may next assemble, and the flashes. (No score, and no
/// clock of how long you have lasted, which let more hunters come the
/// longer it ran: there is no game yet; creatures come from `spawn` at the
/// console, the user.)
#[derive(Resource, Default)]
struct Director {
    /// Seconds until the swarm may assemble again.
    next_gather: f32,
    /// The hurt flash, 0..1; and a white one (a stalker's shot striking
    /// near, see `strike.rs`).
    hurt: f32,
    white: f32,
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
    /// Black, unlit: a shadow (a tumbler travelling as its shadow).
    shade: Handle<StandardMaterial>,
    /// The shot, and the reload as the pump racks.
    gun: Handle<AudioSource>,
    reload: Handle<AudioSource>,
    shatter: Handle<AudioSource>,
    assemble: Handle<AudioSource>,
    hit: Handle<AudioSource>,
    death: Handle<AudioSource>,
    /// A hunter's growls: phrases cut from recordings, and their lengths.
    growls: Vec<(Handle<AudioSource>, f32)>,
    /// A stalker's stilt coming down; its pain.
    stalker_step: Handle<AudioSource>,
    stalker_pain: Handle<AudioSource>,
    stalker_death: Handle<AudioSource>,
    stalker_fall_cry: Handle<AudioSource>,
    pounce: Handle<AudioSource>,
    bite: Handle<AudioSource>,
    /// The bishop's cast (a swell, its peak as its motes are loosed) and its
    /// death; yours, in pain.
    bishop_cast: Handle<AudioSource>,
    bishop_death: Handle<AudioSource>,
    player_pain: Handle<AudioSource>,
    /// The bishop's call now and then, its pain, its blink.
    bishop_call: Handle<AudioSource>,
    bishop_pain: Handle<AudioSource>,
    bishop_blink: Handle<AudioSource>,
    /// A bishop's mote bursting (on you, on what is built, shot down).
    mote_burst: Handle<AudioSource>,
    /// Your footsteps (one of these, a step).
    steps: Vec<Handle<AudioSource>>,
}

/// What landed this frame, for the feedback: how many shards hit something
/// and where (summed). (No hit marker on the screen: the needles show what
/// struck, in the world, the user.)
#[derive(Resource, Default)]
struct Feedback {
    hits: u32,
    at: Vec3,
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
    /// How much of its darkness it casts, 0..1: it grows while it is still
    /// (waiting, or frozen under your gaze), and fades while it moves.
    dark: f32,
    /// Seconds left of its leap on waking, flung up and away before it
    /// stalks (or freezes under your gaze).
    leap: f32,
    /// In a built place, its way to its place along the floors (see
    /// `stalk`), and how long till it is found again.
    route: (Vec<Vec3>, f32),
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

/// A shard stuck in a creature where it struck, white hot a moment, then
/// gone (seen from the moment its streak gets there): each hit seen, a full
/// blast's sixteen counted at a glance, on a black body, in the dark (the
/// user: the black ichor is not). It is carried with what it is in.
#[derive(Component)]
struct Needle {
    delay: f32,
    life: f32,
    size: Vec3,
}

/// How long a needle glows, how far it stands out of what it is in (its
/// tip a little in), and its size (the shard mesh, 0.7 m: 0.42 m, 7 cm; at
/// 4 cm a speck at 10 m).
const NEEDLE_LIFE: f32 = 0.9;
const NEEDLE_OUT: f32 = 0.25;
const NEEDLE_SIZE: Vec3 = Vec3::new(2.8, 0.6, 2.8);

/// Needles show once their streak gets there, glow, and shrink away.
fn needles(mut commands: Commands, time: Res<Time>, mut needles: Query<(Entity, &mut Needle, &mut Transform)>) {
    let dt = time.delta_secs();
    for (entity, mut n, mut transform) in &mut needles {
        if n.delay > 0.0 {
            n.delay -= dt;
            transform.scale = Vec3::ZERO;
            continue;
        }
        n.life -= dt;
        if n.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = n.size * (n.life / 0.25).min(1.0);
    }
}

/// Where shards land on a body.
#[derive(Component)]
struct ImpactLight;

#[derive(Component)]
struct SwarmVoice;

#[derive(Component)]
struct HurtFlash;

fn setup(
    mut commands: Commands,
    args: Res<Args>,
    server: Res<AssetServer>,
    camera: Single<Entity, With<FlyCam>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut held: ResMut<Gun>,
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
        shade: materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, ..default() }),
        // (A blast rolling away; a pistol's first, then a dry blast: see
        // docs/removed.md.)
        gun: load("gun_shot_rolling"),
        reload: load("gun_reload"),
        shatter: load("shard_break"),
        assemble: load("swarm_assemble"),
        // (`--set hit`: 0 the synthesised, 1 a stab cut from a recording;
        // to compare, the user: a very important part of the gun's feel.)
        hit: load(if args.num("hit", 0.0) > 0.5 { "shard_hit_stab" } else { "shard_hit_synth" }),
        death: load("hunter_death"),
        growls: [1.40, 2.10, 1.55, 1.10, 1.10, 1.40, 1.15].iter().enumerate().map(|(k, &l)| (load(&format!("hunter_growl{k}")), l)).collect(),
        stalker_step: load("stalker_step"),
        // (`--set pain_sound 0`: the ship's creak, to compare.)
        stalker_pain: load("stalker_pain"),
        stalker_death: load("stalker_death"),
        // (Its cry falling: a screech.)
        stalker_fall_cry: load("stalker_fall_cry"),
        pounce: load("hunter_pounce"),
        bite: load("hunter_bite"),
        bishop_cast: load("bishop_cast"),
        bishop_death: load("bishop_death"),
        player_pain: load("player_pain"),
        bishop_call: load("bishop_call"),
        bishop_pain: load("bishop_pain"),
        bishop_blink: load("bishop_blink"),
        mote_burst: load("bishop_mote_burst"),
        steps: (0..STEPS).map(|k| load(&format!("player_step{k}"))).collect(),
    };

    // The ear is the camera.
    commands.entity(*camera).insert(SpatialListener::new(2.0));
    // The gun in view (see `weapon.rs`), and the flash at its muzzle.
    let gun = weapon::spawn(&mut commands, &mut meshes, &mut materials, &mut images, assets.pale.clone());
    commands.entity(*camera).add_child(gun);
    // (Nothing in your hands in the room lab's captures, the rooms alone,
    // nor in clean ones, `--opt clean`: the world alone.)
    if args.opt("roomshots") || args.opt("clean") {
        held.empty = true;
        commands.entity(gun).insert(Visibility::Hidden);
    }
    commands.entity(*camera).with_children(|parent| {
        parent.spawn((
            MuzzleLight,
            PointLight { intensity: 0.0, range: 25.0, shadow_maps_enabled: false, ..default() },
            Transform::from_xyz(0.2, -0.15, -1.0),
        ));
    });

    if args.shot.is_none() {
        // The ground's hum, and the swarm's voice (it follows the swarm).
        commands.spawn((
            AudioPlayer::new(load("ground_hum")),
            PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.35), ..default() },
        ));
        commands.spawn((
            SwarmVoice,
            AudioPlayer::new(load("swarm_voice")),
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
    // A flash of black when bitten.
    commands.spawn((
        HurtFlash,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
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
    camera: Single<(&Transform, &FlyCam, &mut Player)>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), Without<FlyCam>>,
    mut hunters: Query<(Entity, &mut hunter::Hunter)>,
    // (Floaters: bishops and brutes, and tumblers and their parts, as one
    // parameter.)
    (mut bishops, mut brutes, mut tumblers, tumbler_parts): (Query<(Entity, &mut bishop::Bishop)>, Query<(Entity, &mut impaled::Impaled)>, Query<(Entity, &mut tumbler::Tumbler)>, Query<&GlobalTransform, With<tumbler::Part>>),
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
    let (transform, fly, mut player) = camera.into_inner();
    // (For captures: `--opt fight` fires by itself, `huntfire` only at a hunter.)
    let auto = (args.opt("fight") && !args.opt("holdfire")) || (args.opt("huntfire") && (!hunters.is_empty() || bishops.iter().any(|(_, b)| b.health > 0.0) || brutes.iter().any(|(_, b)| b.alive()) || tumblers.iter().any(|(_, t)| t.alive())));
    if fly.noclip || !(input.fire || auto) || gun.cooldown > 0.0 || gun.empty {
        return;
    }
    gun.cooldown = RELOAD;
    gun.kick += 0.07;
    gun.shake = 1.0;
    gun.recoil += 0.11;
    gun.shots += 1;
    light.intensity = 1.2e7;
    commands.spawn((AudioPlayer::new(assets.gun.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.9))));
    gun.reload_at = args.num("reload_at", RELOAD - RELOAD_SOUND);
    gun.reload_in = Some(gun.reload_at);

    let eye = transform.translation;
    let (mut right, mut up, mut forward) = (transform.right(), transform.up(), transform.forward());
    // (For captures: `--opt aim`, its shots at the nearest tumbler's body
    // standing, wherever you look.)
    if args.opt("aim")
        && let Some(at) = tumblers.iter().filter_map(|(_, t)| t.aim(&tumbler_parts)).min_by(|a, b| a.distance(eye).total_cmp(&b.distance(eye)))
    {
        let t = Transform::from_translation(eye).looking_at(at, Vec3::Y);
        (right, up, forward) = (t.right(), t.up(), t.forward());
    }
    // The gun pushes you back a little (in the air, it carries).
    player.velocity -= *forward * KICKBACK;
    let muzzle = eye + right * weapon::MUZZLE_VIEW.x + up * weapon::MUZZLE_VIEW.y - forward * weapon::MUZZLE_VIEW.z;
    weapon::fire(&mut commands, &fx, *viewmodel, muzzle, *forward, *right, *up, gun.shots);
    for k in 0..SHARDS {
        let dir = shard_dir(gun.shots, k, *forward, *right, *up);
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
        // Or a brute's, nearer than any (taking the shot from them).
        let limit = body.map_or(end, |(b, ..)| b);
        let mut brute = brutes.iter().filter_map(|(e, b)| b.ray(eye, dir, limit).map(|d| (d, e))).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, _)) = brute {
            body = None;
            best = None;
            end = d;
        }
        // Or a tumbler's (standing: its shadow is not shot), nearer still.
        let limit = body.map_or(end, |(b, ..)| b);
        let tumbled = tumblers.iter().filter_map(|(e, t)| t.ray(&tumbler_parts, eye, dir, limit).map(|(d, k)| (d, e, k))).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, ..)) = tumbled {
            body = None;
            best = None;
            brute = None;
            end = d;
        }
        // Or a bishop's, nearer than either.
        let limit = body.map_or(end, |(b, ..)| b);
        let robed = bishops.iter_mut().filter_map(|(e, b)| b.ray(eye, dir, limit).map(|d| (d, e, b))).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, owner, mut b)) = robed {
            b.hurt(dir, DAMAGE);
            b.embed(&mut commands, &assets, owner, eye + dir * d, dir, d / SHARD_SPEED);
            end = d;
            struck = Some(eye + dir * d);
        } else if let Some((d, entity, k)) = tumbled
            && let Ok((_, mut t)) = tumblers.get_mut(entity)
        {
            let at = eye + dir * d;
            t.hurt(k, at, dir, DAMAGE);
            // Its needle, in the part it struck, carried with it.
            let part = t.part(k);
            if let Ok(g) = tumbler_parts.get(part) {
                let base = eye + dir * (d - NEEDLE_OUT);
                commands.spawn((
                    Needle { delay: d / SHARD_SPEED, life: NEEDLE_LIFE, size: NEEDLE_SIZE / g.scale() },
                    Mesh3d(assets.shard.clone()),
                    MeshMaterial3d(assets.bright.clone()),
                    Transform::from_translation(g.affine().inverse().transform_point3(base)).with_rotation(g.rotation().inverse() * Quat::from_rotation_arc(Vec3::Y, dir)).with_scale(Vec3::ZERO),
                    ChildOf(part),
                ));
            }
            end = d;
            struck = Some(at);
        } else if let Some((d, entity)) = brute
            && let Ok((_, mut b)) = brutes.get_mut(entity)
        {
            b.hurt(dir, DAMAGE);
            // Its needle, in it where it struck, carried with it.
            let t = b.drawn();
            let base = eye + dir * (d - NEEDLE_OUT);
            commands.spawn((
                Needle { delay: d / SHARD_SPEED, life: NEEDLE_LIFE, size: NEEDLE_SIZE },
                Mesh3d(assets.shard.clone()),
                MeshMaterial3d(assets.bright.clone()),
                Transform::from_translation(t.rotation.inverse() * (base - t.translation)).with_rotation(t.rotation.inverse() * Quat::from_rotation_arc(Vec3::Y, dir)).with_scale(Vec3::ZERO),
                ChildOf(entity),
            ));
            end = d;
            struck = Some(eye + dir * d);
        } else if let Some((d, entity, bone)) = body
            && let Ok((_, mut h)) = hunters.get_mut(entity)
        {
            h.hurt(bone, dir, DAMAGE);
            end = d;
            struck = Some(eye + dir * d);
            hunter::embed(&mut commands, &assets, entity, &h, bone, eye + dir * d, dir, d / SHARD_SPEED);
        } else if let Some((along, entity)) = best
            && let Ok((_, mut t, mut s)) = swarm.get_mut(entity)
        {
            s.health -= DAMAGE;
            s.velocity += dir * 4.0;
            // Its needle, in its side towards you, carried with it.
            let base = eye + dir * (along - SWARMER_RADIUS * t.scale.x * 0.8 - NEEDLE_OUT);
            let inverse = t.compute_affine().inverse();
            commands.spawn((
                Needle { delay: along / SHARD_SPEED, life: NEEDLE_LIFE, size: NEEDLE_SIZE / t.scale },
                Mesh3d(assets.shard.clone()),
                MeshMaterial3d(assets.bright.clone()),
                Transform::from_translation(inverse.transform_point3(base)).with_rotation(t.rotation.inverse() * Quat::from_rotation_arc(Vec3::Y, dir)).with_scale(Vec3::ZERO),
                ChildOf(entity),
            ));
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

/// The swarm: each bud stalks the player (see `stalk`). None come by
/// themselves (the user: waves of them were for testing them; creatures are
/// placed by `spawn` at the console until they are placed properly).
#[allow(clippy::too_many_arguments)]
fn swarm(
    time: Res<Time>,
    args: Res<Args>,
    streamer: Res<Streamer>,
    ground: crate::ground::Ground,
    navs: Option<Res<crate::navmesh::Navmeshes>>,
    player: Single<(&Transform, &Player, &FlyCam)>,
    mut swarm: Query<(Entity, &mut Transform, &mut Swarmer), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    let (ptransform, _, fly) = *player;
    // (`--opt fight` keeps it going in a capture, and fires the gun.)
    let capture = args.shot.is_some() && !args.opt("fight");
    // (Playing, it starts once the ground round you is in; measuring or
    // capturing, once everything is, so it happens in view.)
    // (A region of the plan stands high over the terrain, which is never
    // in round it: ready at once.)
    let region = args.opt("chasm") && (args.opt("region") || args.opt("arena"));
    let ready = region || if args.unattended() { streamer.settled } else { streamer.near };
    // (The labs, the rooms' and the plan's, in peace: they are for looking
    // at; a hound forming in one, the user. A region is fought in.)
    let lab = args.opt("chasm") && (args.opt("rooms") || args.opt("plan"));
    if lab || capture || args.opt("bot") || !ready || fly.noclip {
        return;
    }

    let positions: Vec<Vec3> = swarm.iter().map(|(_, t, _)| t.translation).collect();
    for (_, mut transform, mut s) in &mut swarm {
        if s.mode != Mode::Free {
            continue;
        }
        transform.scale = transform.scale.lerp(Vec3::ONE, (dt * 4.0).min(1.0));
        if s.leap > 0.0 {
            // Leaping free: flung, slowing, then it is its own.
            s.leap -= dt;
            s.velocity *= (-dt * 2.5).exp();
            transform.translation = ground.sweep(&Collider::sphere(0.35), transform.translation, s.velocity * dt);
            face(&mut transform, ptransform.translation, dt);
            continue;
        }
        stalk(&ground, navs.as_deref().and_then(|n| n.floors(crate::navmesh::Body::Biped)), ptransform, &positions, &mut transform, &mut s, dt);
        // Still, its darkness creeps in; moving, it fades.
        s.dark = if s.velocity.length() < STILL { (s.dark + dt / DARK_IN).min(1.0) } else { (s.dark - dt / DARK_OUT).max(0.0) };
    }
}

/// How a swarmer moves. Close by it moves only where you are not looking:
/// each has a place of its own a few metres behind you, and goes there and
/// waits, dead still; looked at, it stops dead where it is. Turn round and
/// they are all still; turn back and they have moved behind you again.
/// Arriving in front of you, they swing wide round your side.
fn stalk(ground: &crate::ground::Ground, floors: Option<&crate::navmesh::Floors>, player: &Transform, positions: &[Vec3], transform: &mut Transform, s: &mut Swarmer, dt: f32) {
    let p = transform.translation;
    let eye = player.translation;
    // (It turns to face you, even while it stands still.)
    face(transform, eye, dt);
    // (Seen: not through a wall or a floor.)
    let seen = p.distance(eye) < STILL_WITHIN && (p - eye).normalize_or(Vec3::Y).dot(*player.forward()) > 0.55 && ground.clear(eye, p);
    if seen {
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
    // In a built place: its place on the floors nearest it, at about head
    // height over them, and there by the way along them, flying at about
    // head height (not through the walls).
    // (Its way found again every `ROUTE` s, or once its place has moved;
    // the corner on along it it has not come within a metre of, at head
    // height over it.)
    let mut aim = place;
    let mut left = None;
    if let Some(floors) = floors {
        if let Some(f) = floors.on(place, 3.0) {
            place = Vec3::new(f.x, f.y + 1.0 + h(2) * 1.5, f.z);
            aim = place;
        }
        // (Kept to, not found afresh every moment: hanging over a rim, the
        // floor under it was found now on the rim, now on the flight below,
        // each a way of its own, and it swung back and forth between them,
        // never coming down.)
        s.route.1 -= dt;
        if s.route.1 <= 0.0 || s.route.0.len() < 2 || s.route.0.last().is_none_or(|end| end.distance(place) > ROUTE_MOVED) {
            s.route = (floors.way(p, place).unwrap_or_default(), ROUTE);
        }
        let way = &mut s.route.0;
        while way.len() > 1 && Vec2::new(way[0].x - p.x, way[0].z - p.z).length() < 1.0 {
            way.remove(0);
        }
        // (The furthest of the next few corners in plain sight, not the
        // next: at its speed it could not turn in the metre between two at
        // the head of a flight, and overshot them, back and forth.)
        if let Some(&first) = way.first() {
            let l = std::iter::once(p).chain(way.iter().copied()).collect::<Vec<_>>().windows(2).map(|w| w[0].distance(w[1])).sum::<f32>();
            left = Some(l);
            if l > 3.5 {
                let ahead = way.iter().take(8).rev().copied().find(|&q| ground.clear(p, q + Vec3::Y * 1.6)).unwrap_or(first);
                aim = ahead + Vec3::Y * 1.6;
            }
        }
    }
    let to = aim - p;
    let distance = left.unwrap_or(to.length());
    let mut want = to.normalize_or(Vec3::ZERO) * SWARMER_SPEED * (distance / 3.0).min(1.0);
    for &q in positions {
        let d = p - q;
        let l = d.length();
        if l > 0.01 && l < 1.6 {
            want += d / l * (1.6 - l) * 6.0;
        }
    }
    s.velocity += (want - s.velocity).clamp_length_max(20.0 * dt);
    // (Flying, a small round body: what is built stops it, it slides on.)
    // (Kept over the floor under it, looked for from where it is: from a
    // walker's foot height over it, as `Ground::at` looks, under the low
    // ceiling of a flight going down in the rock, the ceiling was taken for
    // its floor, and it was held up against it, never going down.)
    let mut next = ground.sweep(&Collider::sphere(0.35), p, s.velocity * dt);
    let ground = ground.hit(next, Vec3::NEG_Y, 40.0).map_or(ground.at(next), |d| next.y - d) + 0.8;
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
/// darkens the world once it is still (waiting, or frozen under your gaze;
/// see `Swarmer::dark`), more the nearer it is, and together they all but
/// blind you (`--set gloom_each`: how much one close by does); it closes in fast
/// and lifts slowly. (Those gathering into a hunter count too: it forms in
/// the dark, and the dark lifts off it.)
fn gloom(
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    player: Single<&Transform, With<Player>>,
    swarm: Query<(&Transform, &Swarmer), Without<Player>>,
    mut gloom: ResMut<crate::look::Gloom>,
) {
    let dt = time.delta_secs().min(0.05);
    // (Asleep, they do nothing; flying, nothing yet.)
    let sum: f32 = swarm
        .iter()
        .filter(|(_, s)| s.mode != Mode::Dormant)
        // (Through a wall or a floor, a quarter of it: something unseen
        // close by, not the whole room's dark from the storey below.)
        .map(|(t, s)| {
            let near = ((GLOOM_REACH - t.translation.distance(player.translation)) / (GLOOM_REACH - GLOOM_FULL)).clamp(0.0, 1.0).powi(2);
            let seen = if near > 0.0 && !ground.clear(player.translation, t.translation) { 0.25 } else { 1.0 };
            s.dark * near * seen
        })
        .sum();
    let target = 1.0 - (-args.num("gloom_each", 0.4) * sum).exp();
    let rate = if target > gloom.0 { 2.0 } else { 0.7 };
    gloom.0 += (target - gloom.0) * (rate * dt).min(1.0);
    // (`--set gloom=0..1`: the darkness held there, to judge it.)
    let held = args.num("gloom", -1.0);
    if held >= 0.0 {
        gloom.0 = held;
    }
}

/// A grove's bud at `at` (on the ground), asleep on its roots, opening to
/// the sky, leaning a little (see `grove.rs`).
fn spawn_sleeper(commands: &mut Commands, kit: &forms::Kit, materials: &mut Assets<StandardMaterial>, at: Vec3, seed: u32, traits: &crate::traits::Traits) {
    let r = |j: i32| hash01(seed as i32, j, 0, 0x5a2) - 0.5;
    let swarmer = Swarmer { mode: Mode::Dormant, velocity: Vec3::ZERO, health: SWARMER_HEALTH, phase: r(4) * 50.0, dark: 0.0, leap: 0.0, route: (Vec::new(), 0.0) };
    let lean = Quat::from_euler(EulerRot::XYZ, r(5) * 0.6, 0.0, r(6) * 0.6);
    let transform = Transform::from_translation(at + Vec3::Y * 0.55).looking_to(lean * Vec3::Y, Vec3::Z);
    let root = commands.spawn((swarmer, forms::Gazed::default(), forms::Asleep, traits.clone(), transform, Visibility::default())).id();
    forms::build(commands, kit, materials, root);
}

/// A free bud at `at`, stalking (`spawn bud` at the console).
fn spawn_bud(commands: &mut Commands, kit: &forms::Kit, materials: &mut Assets<StandardMaterial>, at: Vec3, seed: u32, traits: &crate::traits::Traits) {
    let phase = hash01(seed as i32, 4, 0, 0x5a2) * 50.0;
    let swarmer = Swarmer { mode: Mode::Free, velocity: Vec3::ZERO, health: SWARMER_HEALTH, phase, dark: 0.0, leap: 0.0, route: (Vec::new(), 0.0) };
    let root = commands.spawn((swarmer, forms::Gazed::default(), traits.clone(), Transform::from_translation(at), Visibility::default())).id();
    forms::build(commands, kit, materials, root);
}

/// Your footsteps: on the ground and moving, a step every stride (one of
/// them, a little higher or lower each time, never the same twice running),
/// and one as you land from a jump or a fall. None on a flight (its slope,
/// 37 degrees, steeper than any ground walked: a step every tread's bump, the
/// user), and none landing from a bump.
fn footsteps(time: Res<Time>, mut commands: Commands, assets: Res<Assets3>, player: Single<(&Player, &FlyCam)>, mut walked: Local<(f32, f32, u32, usize)>) {
    let (player, fly) = *player;
    let (walked_far, airborne, count, last) = &mut *walked;
    let speed = Vec2::new(player.velocity.x, player.velocity.z).length();
    let landed = player.grounded && *airborne >= 0.3;
    *airborne = if player.grounded { 0.0 } else { *airborne + time.delta_secs() };
    let flight = player.ground_normal.y < 0.88;
    if fly.noclip || !player.grounded || (flight && !landed) {
        return;
    }
    *walked_far += speed * time.delta_secs();
    if landed || (speed > 1.0 && *walked_far >= STRIDE) {
        *walked_far = 0.0;
        *count += 1;
        let r = |j: i32| hash01(*count as i32, j, 0, 0x5c1);
        let mut k = (r(0) * STEPS as f32) as usize % STEPS;
        if k == *last {
            k = (k + 1) % STEPS;
        }
        *last = k;
        commands.spawn((AudioPlayer::new(assets.steps[k].clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(STEP_VOLUME * if landed { 1.4 } else { 1.0 })).with_speed(0.92 + 0.16 * r(1))));
    }
}

/// Broken swarmers shatter; a dead player starts again.
#[allow(clippy::too_many_arguments)]
fn die(
    mut commands: Commands,
    args: Res<Args>,
    assets: Res<Assets3>,
    ichor: Res<ichor::Ichor>,
    mut player: Single<&mut Player>,
    mut spawns: ResMut<crate::console::Spawns>,
    swarm: Query<(Entity, &Transform, &Swarmer)>,
    bodies: Query<Entity, Or<(With<hunter::Hunter>, With<hunter::Assembly>, With<hunter::Part>, With<hunter::Eye>, With<hunter::clip::Body>)>>,
    bishops: Query<Entity, With<bishop::OfBishop>>,
    bolts: Query<(Entity, &bishop::Bolt)>,
    brutes: Query<Entity, With<impaled::Impaled>>,
    tumblers: Query<Entity, With<tumbler::Tumbler>>,
) {
    for (entity, transform, s) in &swarm {
        if s.health > 0.0 {
            continue;
        }
        ichor::spray(&mut commands, &ichor, transform.translation, s.velocity.normalize_or(Vec3::Y), 2.5, entity.index_u32());
        shatter(&mut commands, &assets, transform.translation, s.velocity, entity.index_u32());
        commands.entity(entity).despawn();
    }
    // (`--opt god`, or `god on` at the console: for watching the swarm,
    // or testing a fight, without dying.)
    if args.opt("god") {
        player.health = HEALTH_MAX;
    }
    // (Fallen, or `reset` at the console: you start again.)
    if player.health <= 0.0 || std::mem::take(&mut spawns.reset) {
        player.health = HEALTH_MAX;
        for (entity, ..) in &swarm {
            commands.entity(entity).despawn();
        }
        for entity in &bodies {
            commands.entity(entity).despawn();
        }
        // (The bishops too, their cloth and lights, and their lightning
        // in flight: left standing, a round after `reset` began with them.)
        // (Tried: one dying this frame may be despawning itself already.)
        for entity in &bishops {
            commands.entity(entity).try_despawn();
        }
        for entity in brutes.iter().chain(&tumblers) {
            commands.entity(entity).try_despawn();
        }
        for (entity, bolt) in &bolts {
            commands.entity(entity).try_despawn();
            for part in bolt.parts() {
                commands.entity(part).try_despawn();
            }
        }
    }
}

/// Where a ray from `o` along `d` (unit) first meets a capsule from `a` to
/// `b`, `r` wide.
pub(super) fn capsule(o: Vec3, d: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
    let ba = b - a;
    let oa = o - a;
    let (baba, bard, baoa) = (ba.dot(ba), ba.dot(d), ba.dot(oa));
    let (rdoa, oaoa) = (d.dot(oa), oa.dot(oa));
    let qa = baba - bard * bard;
    let qb = baba * rdoa - baoa * bard;
    let qc = baba * oaoa - baoa * baoa - r * r * baba;
    let h = qb * qb - qa * qc;
    if h < 0.0 {
        return None;
    }
    let t = (-qb - h.sqrt()) / qa;
    let y = baoa + t * bard;
    if y > 0.0 && y < baba {
        return Some(t);
    }
    // (Else one of its round ends.)
    let oc = if y <= 0.0 { oa } else { o - b };
    let (cb, cc) = (d.dot(oc), oc.dot(oc) - r * r);
    let h = cb * cb - cc;
    (h > 0.0).then(|| -cb - h.sqrt())
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

/// The shards' impact sound, played again in quick succession the more
/// struck (the user's experiment: many shards hitting): so many times for
/// every so many, at most so many lots, so far apart (s), each a little
/// higher or lower.
const HITS: (u32, u32, u32, f32) = (2, 4, 3, 0.035);
/// How much louder the repeats than the single hit was (halfway between
/// it and twice it, overdone to hear them at all, the user).
const HITS_LOUD: f32 = 1.05;

/// What landed this frame: its impact sound (louder and lower the more
/// shards struck, again the more, see `HITS`), and a flash where they
/// struck.
#[allow(clippy::type_complexity)]
fn feedback(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<Assets3>,
    mut feedback: ResMut<Feedback>,
    mut light: Single<(&mut Transform, &mut PointLight), With<ImpactLight>>,
    mut queued: Local<Vec<(f32, f32, f32)>>,
) {
    let dt = time.delta_secs();
    // Those still to sound: (when, how loud, how fast).
    queued.retain_mut(|(at, volume, speed)| {
        *at -= dt;
        if *at > 0.0 {
            return true;
        }
        commands.spawn((AudioPlayer::new(assets.hit.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(*volume)).with_speed(*speed)));
        false
    });
    let n = feedback.hits;
    if n > 0 {
        let k = n.min(12) as f32;
        let times = n.div_ceil(HITS.1).min(HITS.2) * HITS.0;
        for t in 0..times {
            let r = hash01(n as i32, t as i32, 0, 0x5d1) - 0.5;
            queued.push((HITS.3 * t as f32 + 0.012 * r, (0.45 + 0.07 * k) * HITS_LOUD, (1.15 - 0.025 * k) * (1.0 + 0.08 * r)));
        }
        light.0.translation = feedback.at / n as f32;
        light.1.intensity = light.1.intensity.max(4.0e5 * k);
        feedback.hits = 0;
        feedback.at = Vec3::ZERO;
    }
    light.1.intensity *= (-dt * 30.0).exp();
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

/// Chips of the ground kicked up where something heavy lands at `at`, going
/// on with it (`velocity`): low, pale, quick.
fn kicked_up(commands: &mut Commands, assets: &Assets3, at: Vec3, velocity: Vec3, seed: u32) {
    for k in 0..8 {
        let r = |j: i32| hash01(seed as i32, k, j, 0x5c3) - 0.5;
        let out = Vec3::new(r(0), 0.0, r(1)).normalize_or(Vec3::X);
        let size = Vec3::new(1.2, 0.3, 1.2) * (0.6 + 0.8 * (r(2) + 0.5));
        commands.spawn((
            Debris { velocity: velocity + out * (2.0 + 3.0 * (r(3) + 0.5)) + Vec3::Y * (2.5 + 2.5 * (r(4) + 0.5)), spin: Vec3::new(r(5), r(6), r(7)) * 16.0, life: 0.7, total: 0.7, size },
            Mesh3d(assets.shard.clone()),
            MeshMaterial3d(assets.pale.clone()),
            Transform::from_translation(at + out * 0.3 + Vec3::Y * 0.05).with_scale(size),
        ));
    }
}

/// Pieces fall, bounce off the ground and shrink away.
fn debris(mut commands: Commands, time: Res<Time>, ground: crate::ground::Ground, mut pieces: Query<(Entity, &mut Debris, &mut Transform)>) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut d, mut transform) in &mut pieces {
        d.life -= dt;
        if d.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        d.velocity.y -= 18.0 * dt;
        let mut p = transform.translation + d.velocity * dt;
        let ground = ground.at(p) + 0.05;
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
    global: Res<bevy::audio::GlobalVolume>,
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
    // (Times the volume of everything: set outright every frame, it ignored
    // it, heard at full in a muted test and with `volume 0.1`, the user.)
    sink.set_volume(Volume::Linear((weight * 0.5).min(1.2) * global.volume.to_linear()));
}

fn hud(
    time: Res<Time>,
    mut director: ResMut<Director>,
    mut flash: Single<&mut BackgroundColor, With<HurtFlash>>,
) {
    let dt = time.delta_secs();
    director.hurt = (director.hurt - dt * 2.5).max(0.0);
    director.white *= (-dt * 9.0).exp();
    flash.0 = if director.white > 0.01 { Color::srgba(1.0, 1.0, 1.0, director.white) } else { Color::srgba(0.0, 0.0, 0.0, director.hurt * 0.55) };
}

/// The view kicks up with each shot and settles back; it shakes for an
/// instant, and its field of view punches out and back.
fn kick(time: Res<Time>, mut gun: ResMut<Gun>, mut camera: Single<(&mut Transform, &mut Projection), (With<FlyCam>, Without<weapon::Viewmodel>)>) {
    let dt = time.delta_secs();
    gun.kick *= (-dt * 9.0).exp();
    gun.shake *= (-dt * 18.0).exp();
    gun.jolt *= (-dt * 8.0).exp();
    let (transform, projection) = &mut *camera;
    transform.rotate_local_x(gun.kick + gun.jolt.x);
    transform.rotate_local_z(gun.jolt.y);
    let t = time.elapsed_secs();
    let s = gun.shake * 0.012;
    transform.rotate_local_y(s * (t * 83.0).sin());
    transform.rotate_local_z(s * 1.5 * (t * 71.0).cos());
    if let Projection::Perspective(p) = &mut **projection {
        p.fov = (65.0 + 5.0 * gun.shake).to_radians();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shards_on_a_stalker() {
        // How many of a shot's shards strike a stalker standing square to
        // you, aimed at its chest, from near to far: what "point-blank"
        // means for its death (see `hunter::OVERKILL`).
        let rig = crate::rig::Rig::new(crate::rig::Plan::stalker(), Vec3::ZERO, 0.0, 5, &|_, _| 0.0);
        let chest = rig.chest();
        for d in [2.0f32, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 15.0, 20.0] {
            let eye = chest + rig.facing() * d;
            let forward = (chest - eye).normalize();
            let right = forward.cross(Vec3::Y).normalize();
            let up = right.cross(forward);
            let hits: Vec<usize> = (0..20u32)
                .map(|shot| (0..SHARDS).filter(|&k| rig.bones.iter().any(|b| b.ray(eye, shard_dir(shot, k, forward, right, up), RANGE).is_some())).count())
                .collect();
            let mean = hits.iter().sum::<usize>() as f32 / hits.len() as f32;
            println!("{d:4.0} m: {mean:4.1} of {SHARDS} shards on average, {}..{}", hits.iter().min().unwrap(), hits.iter().max().unwrap());
            let (least, most) = (*hits.iter().min().unwrap(), *hits.iter().max().unwrap());
            if d <= 3.0 {
                assert!(least >= hunter::OVERKILL, "at {d} m as few as {least} shards: not always overkill");
            }
            if d >= 5.0 {
                assert!(most < hunter::OVERKILL, "at {d} m as many as {most} shards: overkill");
            }
        }
    }
}
