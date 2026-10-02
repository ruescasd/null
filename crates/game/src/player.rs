//! Quake-style movement and the thrust gun.
//!
//! Movement follows Quake 3's player physics: ground friction and
//! acceleration, weak air acceleration that only adds speed along the wish
//! direction (which is what makes strafe jumping gain speed), automatic
//! bunny hopping while jump is held, and stepping up small ledges.
//!
//! The thrust gun fires a beam whose recoil pushes the player directly away
//! from where it is aimed. Its thrust beats gravity, so aiming at your feet
//! lifts you, and angling it gives flight; it drains energy that only
//! recharges on the ground.
//!
//! The tether (see `tether.rs`) is a grappling hook: it flies out, roots
//! itself in whatever it hits, and while held pulls the player towards that
//! point by adding acceleration, so letting go keeps the momentum.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use worldgen::noise::hash01;

use crate::{Args, camera::FlyCam, terrain::Streamer, terrain::WorldGen};

pub use bot::drive as bot_drive;
use tether::Tether;

mod bot;
mod tether;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default())
            .init_resource::<MoveInput>()
            .add_systems(Startup, (setup_beam, setup_hud))
            .add_systems(Update, update_hud)
            .add_systems(PostStartup, add_player)
            .add_systems(Startup, tether::setup)
            .add_systems(
                PostUpdate,
                (draw_beam, tether::draw).before(TransformSystems::Propagate),
            );
    }
}

// Quake 3 constants in metres. A 56-unit Quake player is 1.8 m tall, so one
// unit is about 3.2 cm.
/// Top running speed (320 units/s).
const MAX_SPEED: f32 = 10.0;
const GROUND_ACCEL: f32 = 10.0;
/// Air acceleration. Small, so air strafing adds speed only when the wish
/// direction is nearly perpendicular to the velocity: strafe jumping.
const AIR_ACCEL: f32 = 1.0;
const FRICTION: f32 = 6.0;
/// Below this speed friction acts as if moving at this speed (100 units/s).
const STOP_SPEED: f32 = 3.2;
/// 800 units/s².
const GRAVITY: f32 = 25.6;
/// 270 units/s: about 1.45 m of jump height.
const JUMP_SPEED: f32 = 8.6;
/// Ledges up to this height are climbed without jumping (18 units).
const STEP_HEIGHT: f32 = 0.58;
/// Surfaces at most ~45 degrees from flat count as ground.
const MIN_WALK_NORMAL: f32 = 0.7;
/// Movement is integrated in steps of this size, like Quake at 125 fps, so
/// strafe jumping behaves the same at any frame rate.
const STEP_DT: f32 = 1.0 / 125.0;

/// Half the player's width (Quake: 15 units).
const HALF_WIDTH: f32 = 0.45;
const HEIGHT: f32 = 1.8;
pub const EYE: f32 = 1.6;

/// Beam recoil, in m/s². Greater than gravity, so aiming down lifts off.
const THRUST: f32 = 34.0;
const ENERGY_MAX: f32 = 100.0;
/// Energy per second while firing: a little under five seconds of thrust.
const ENERGY_DRAIN: f32 = 22.0;
/// Energy per second while standing on the ground and not firing.
const ENERGY_RECHARGE: f32 = 45.0;
const BEAM_RANGE: f32 = 30.0;

/// How far the tether reaches, and how fast its tip travels.
const TETHER_RANGE: f32 = 60.0;
const TETHER_SPEED: f32 = 160.0;
/// Pull towards the anchor, in m/s², applied until the speed along the line
/// reaches `TETHER_MAX_PULL_SPEED` (about 1000 units/s).
const TETHER_PULL: f32 = 55.0;
const TETHER_MAX_PULL_SPEED: f32 = 32.0;
/// The tether lets go by itself when the player's middle gets this close to
/// its anchor (an anchor underfoot is about 0.9 m away).
const TETHER_DETACH: f32 = 1.4;
/// When checking whether the line is blocked, this much of it next to the
/// anchor is ignored, so the surface the spike is rooted in does not count.
const TETHER_SEVER_MARGIN: f32 = 0.4;

#[derive(Component)]
pub struct Player {
    pub velocity: Vec3,
    pub grounded: bool,
    ground_normal: Vec3,
    pub energy: f32,
    pub firing: bool,
    pub tether: Tether,
    /// The tether button must be released before the tether fires again.
    tether_held: bool,
    /// Movement waits until the terrain around the spawn point has loaded.
    pub ready: bool,
}

fn add_player(mut commands: Commands, camera: Single<Entity, With<FlyCam>>) {
    commands.entity(*camera).insert(Player {
        velocity: Vec3::ZERO,
        grounded: false,
        ground_normal: Vec3::Y,
        energy: ENERGY_MAX,
        firing: false,
        tether: Tether::Idle,
        tether_held: false,
        ready: false,
    });
}

/// The player's collision shape: an upright box that never rotates, like
/// Quake's. Its flat bottom lands squarely on ledge edges, which a capsule's
/// rounded bottom would read as a steep slope.
fn hull() -> Collider {
    Collider::cuboid(HALF_WIDTH * 2.0, HEIGHT, HALF_WIDTH * 2.0)
}

/// Quake's PM_Friction, applied to horizontal speed.
fn friction(v: &mut Vec3, dt: f32) {
    let speed = Vec2::new(v.x, v.z).length();
    if speed < 0.03 {
        v.x = 0.0;
        v.z = 0.0;
        return;
    }
    let drop = speed.max(STOP_SPEED) * FRICTION * dt;
    let scale = (speed - drop).max(0.0) / speed;
    v.x *= scale;
    v.z *= scale;
}

/// Quake's PM_Accelerate: add speed along `wishdir`, but only up to
/// `wishspeed` measured along that direction.
fn accelerate(v: &mut Vec3, wishdir: Vec3, wishspeed: f32, accel: f32, dt: f32) {
    let add = wishspeed - v.dot(wishdir);
    if add <= 0.0 {
        return;
    }
    *v += wishdir * (accel * dt * wishspeed).min(add);
}

/// Removes the component of `v` going into a surface.
fn clip(v: Vec3, normal: Vec3) -> Vec3 {
    let into = v.dot(normal);
    let into = if into < 0.0 { into * 1.001 } else { into / 1.001 };
    v - normal * into
}

struct Mover<'a, 'w, 's> {
    query: &'a MoveAndSlide<'w, 's>,
    shape: Collider,
    config: MoveAndSlideConfig,
    filter: SpatialQueryFilter,
}

impl Mover<'_, '_, '_> {
    fn slide(&self, center: Vec3, velocity: Vec3, dt: f32) -> (Vec3, Vec3) {
        let out = self.query.move_and_slide(
            &self.shape,
            center,
            Quat::IDENTITY,
            velocity,
            Duration::from_secs_f32(dt),
            &self.config,
            &self.filter,
            |_| MoveAndSlideHitResponse::Accept,
        );
        (out.position, out.projected_velocity)
    }

    /// Distance the hull can move along `movement` before touching
    /// something, and the surface normal it would touch.
    fn sweep(&self, center: Vec3, movement: Vec3) -> Option<(f32, Vec3)> {
        self.query
            .cast_move(&self.shape, center, Quat::IDENTITY, movement, self.config.skin_width, &self.filter)
            .map(|hit| (hit.distance, hit.normal1))
    }

    /// Quake's PM_StepSlideMove: slide, and if a ledge blocked the way while
    /// on the ground, try again from `STEP_HEIGHT` higher and settle down.
    fn step_slide(&self, center: Vec3, velocity: Vec3, dt: f32, grounded: bool) -> (Vec3, Vec3) {
        let (moved, slid) = self.slide(center, velocity, dt);
        if !grounded {
            return (moved, slid);
        }
        let wanted = Vec2::new(velocity.x, velocity.z).length() * dt;
        let got = Vec2::new(moved.x - center.x, moved.z - center.z).length();
        if got >= wanted * 0.95 {
            return (moved, slid);
        }
        let up = self.sweep(center, Vec3::Y * STEP_HEIGHT).map_or(STEP_HEIGHT, |(d, _)| d);
        let flat = Vec3::new(velocity.x, 0.0, velocity.z);
        let (stepped, step_vel) = self.slide(center + Vec3::Y * up, flat, dt);
        let Some((down, normal)) = self.sweep(stepped, Vec3::NEG_Y * (up + 0.05)) else {
            return (moved, slid);
        };
        let landed = stepped - Vec3::Y * down;
        let further = Vec2::new(landed.x - center.x, landed.z - center.z).length() > got + 1e-3;
        if normal.y >= MIN_WALK_NORMAL && further {
            (landed, Vec3::new(step_vel.x, 0.0, step_vel.z))
        } else {
            (moved, slid)
        }
    }
}

/// What the player is asking for this frame, from the keyboard and mouse or
/// from the test bot.
#[derive(Resource, Default)]
pub struct MoveInput {
    /// x: strafe right, y: forward; each -1..1.
    pub wish: Vec2,
    pub jump: bool,
    pub fire: bool,
    pub tether: bool,
}

pub fn gather_input(
    args: Res<Args>,
    streamer: Res<Streamer>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    mut input: ResMut<MoveInput>,
) {
    let captured = cursor.grab_mode != CursorGrabMode::None;
    let pressed = |k: KeyCode| captured && keys.pressed(k);
    let axis = |pos: KeyCode, neg: KeyCode| pressed(pos) as i32 as f32 - pressed(neg) as i32 as f32;
    *input = MoveInput {
        wish: Vec2::new(axis(KeyCode::KeyD, KeyCode::KeyA), axis(KeyCode::KeyW, KeyCode::KeyS)),
        // Mouse 2 as well as Space, for easier testing until input is configurable.
        jump: pressed(KeyCode::Space) || (captured && mouse.pressed(MouseButton::Right)),
        fire: args.opt("beam") || (captured && mouse.pressed(MouseButton::Left)),
        // (Forced on for captures once there is terrain to anchor to.)
        tether: (args.opt("tether") && streamer.settled)
            || pressed(KeyCode::KeyE)
            || (captured
                && (mouse.pressed(MouseButton::Back) || mouse.pressed(MouseButton::Forward))),
    };
}

/// Runs between mouse look and the world wrap (see `camera.rs`).
#[allow(clippy::too_many_arguments)]
pub fn walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<MoveInput>,
    streamer: Res<Streamer>,
    world: Res<WorldGen>,
    move_and_slide: MoveAndSlide,
    camera: Single<(&mut Transform, &mut FlyCam, Option<&mut Player>)>,
) {
    let (mut transform, mut fly, player) = camera.into_inner();
    let Some(mut player) = player else { return };
    if keys.just_pressed(KeyCode::KeyV) {
        fly.noclip = !fly.noclip;
        player.velocity = Vec3::ZERO;
    }
    player.firing = input.fire && player.energy > 0.0;
    // Fire, fly and release the tether (once per frame; the pull itself is
    // integrated with the movement steps below).
    let aim = fly.rotation() * Vec3::NEG_Z;
    let eye = transform.translation;
    if !input.tether {
        player.tether = Tether::Idle;
    } else if !player.tether_held {
        player.tether = Tether::Flying { tip: eye, dir: aim, travelled: 0.0 };
    }
    player.tether_held = input.tether;
    if let Tether::Flying { tip, dir, travelled } = player.tether {
        let step = (TETHER_SPEED * time.delta_secs()).min(TETHER_RANGE - travelled);
        let ray = Dir3::new(dir).unwrap_or(Dir3::NEG_Z);
        player.tether = match move_and_slide.spatial_query.cast_ray(tip, ray, step, true, &default()) {
            Some(hit) => Tether::Anchored { point: tip + dir * hit.distance, normal: hit.normal },
            None if travelled + step >= TETHER_RANGE => Tether::Missed,
            None => Tether::Flying { tip: tip + dir * step, dir, travelled: travelled + step },
        };
    }
    // Anything coming between the player and the anchor severs the tether.
    // (The last bit of the line is ignored: that is the anchor's own surface.)
    if let Tether::Anchored { point, .. } = player.tether {
        let line = point - eye;
        let length = line.length();
        if let Ok(ray) = Dir3::new(line)
            && length > TETHER_SEVER_MARGIN
            && move_and_slide
                .spatial_query
                .cast_ray(eye, ray, length - TETHER_SEVER_MARGIN, true, &default())
                .is_some()
        {
            player.tether = Tether::Missed;
        }
    }

    if fly.noclip {
        return;
    }
    let p = transform.translation;
    let ground_height = world.ground_height(p.x, p.z);
    if !player.ready {
        if !streamer.settled {
            return;
        }
        player.ready = true;
        transform.translation.y = ground_height + EYE + 0.3;
    }

    let (sin, cos) = fly.yaw.sin_cos();
    let forward = Vec3::new(-sin, 0.0, -cos);
    let right = Vec3::new(cos, 0.0, -sin);
    let wishdir = (forward * input.wish.y + right * input.wish.x).normalize_or_zero();
    let jump = input.jump;
    let aim = fly.rotation() * Vec3::NEG_Z;

    let mover = Mover {
        query: &move_and_slide,
        shape: hull(),
        config: MoveAndSlideConfig::default(),
        filter: SpatialQueryFilter::default(),
    };
    let mut center = transform.translation - Vec3::Y * (EYE - HEIGHT * 0.5);
    let total = time.delta_secs().min(0.1);
    let steps = (total / STEP_DT).ceil().max(1.0) as u32;
    let dt = total / steps as f32;
    for _ in 0..steps {
        let ground = if player.velocity.y > 1.0 {
            None
        } else {
            mover.sweep(center, Vec3::NEG_Y * 0.08).filter(|(_, n)| n.y >= MIN_WALK_NORMAL)
        };
        player.grounded = ground.is_some();
        if let Some((_, normal)) = ground {
            player.ground_normal = normal;
        }

        // Jumping skips this step's friction, so holding jump bunny hops
        // without losing speed.
        if player.grounded && jump {
            player.velocity.y = JUMP_SPEED;
            player.grounded = false;
        }
        let tethered = matches!(player.tether, Tether::Anchored { .. });
        if player.grounded {
            // The pull drags you along the ground rather than fighting friction.
            if !tethered {
                friction(&mut player.velocity, dt);
            }
            accelerate(&mut player.velocity, wishdir, MAX_SPEED, GROUND_ACCEL, dt);
            player.velocity = clip(player.velocity, player.ground_normal);
        } else {
            accelerate(&mut player.velocity, wishdir, MAX_SPEED, AIR_ACCEL, dt);
            player.velocity.y -= GRAVITY * dt;
        }

        if let Tether::Anchored { point, .. } = player.tether {
            let to_anchor = point - center;
            let dist = to_anchor.length();
            if dist < TETHER_DETACH {
                player.tether = Tether::Missed;
            } else {
                let dir = to_anchor / dist;
                let along = player.velocity.dot(dir);
                if along < TETHER_MAX_PULL_SPEED {
                    player.velocity += dir * (TETHER_PULL * dt).min(TETHER_MAX_PULL_SPEED - along);
                }
            }
        }

        if player.firing {
            player.velocity -= aim * THRUST * dt;
            player.energy = (player.energy - ENERGY_DRAIN * dt).max(0.0);
        } else if player.grounded {
            player.energy = (player.energy + ENERGY_RECHARGE * dt).min(ENERGY_MAX);
        }

        let (moved, velocity) = mover.step_slide(center, player.velocity, dt, player.grounded);
        center = moved;
        player.velocity = velocity;
    }
    transform.translation = center + Vec3::Y * (EYE - HEIGHT * 0.5);

    // Never fall out of the world.
    if transform.translation.y < ground_height - 30.0 {
        transform.translation.y = ground_height + EYE + 0.5;
        player.velocity = Vec3::ZERO;
    }
}

#[derive(Component)]
struct BeamLight;

fn setup_beam(mut commands: Commands, mut gizmos: ResMut<GizmoConfigStore>) {
    gizmos.config_mut::<DefaultGizmoConfigGroup>().0.line.width = 3.0;
    commands.spawn((
        BeamLight,
        PointLight {
            intensity: 0.0,
            range: 30.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default(),
    ));
}

/// Draws the beam as a jittering bolt and flickers a light where it lands.
fn draw_beam(
    time: Res<Time>,
    spatial: SpatialQuery,
    camera: Single<(&Transform, &FlyCam, &Player), Without<BeamLight>>,
    mut light: Single<(&mut Transform, &mut PointLight), With<BeamLight>>,
    mut gizmos: Gizmos,
) {
    let (transform, fly, player) = *camera;
    let (light_transform, light) = &mut *light;
    if !player.firing {
        light.intensity = 0.0;
        return;
    }
    let rotation = fly.rotation();
    let (aim, right, up) = (rotation * Vec3::NEG_Z, rotation * Vec3::X, rotation * Vec3::Y);
    let eye = transform.translation;
    let hit = spatial.cast_ray(eye, Dir3::new(aim).unwrap_or(Dir3::NEG_Z), BEAM_RANGE, true, &default());
    let end = eye + aim * hit.map_or(BEAM_RANGE, |h| h.distance);
    let start = eye + aim * 0.6 + right * 0.25 - up * 0.22;

    // A fresh random bolt every 1/40 s, two strands.
    let frame = (time.elapsed_secs() * 40.0) as i32;
    const SEGMENTS: i32 = 16;
    for strand in 0..2 {
        let mut prev = start;
        for i in 1..=SEGMENTS {
            let t = i as f32 / SEGMENTS as f32;
            let r = |k: i32| hash01(frame, i * 4 + k, strand, 0xbea4) - 0.5;
            let taper = (t * (1.0 - t) * 4.0).min(1.0);
            let wobble = (right * r(0) + up * r(1)) * 0.5 * taper;
            let point = start.lerp(end, t) + if i == SEGMENTS { Vec3::ZERO } else { wobble };
            let brightness = if strand == 0 { 12.0 } else { 4.0 };
            gizmos.line(prev, point, LinearRgba::rgb(brightness, brightness, brightness));
            prev = point;
        }
    }

    // Light the surroundings from just short of the impact point.
    light_transform.translation = end - aim * 0.8;
    let flicker = 0.6 + 0.4 * hash01(frame, 99, 0, 0x11a7);
    light.intensity = 3.5e6 * flicker;
}

#[derive(Component)]
struct Readout;

/// The crosshair grows when the tether can reach what it points at.
#[derive(Component)]
struct Crosshair;

fn setup_hud(mut commands: Commands, args: Res<Args>) {
    if args.shot.is_some() {
        return;
    }
    // Crosshair: a small dot at the centre of the screen.
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
            Crosshair,
            Node { width: px(4), height: px(4), ..default() },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
        ));
    // Speed and energy, bottom centre.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            bottom: px(24),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_child((
            Readout,
            Text::new(""),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            TextColor(Color::srgb(0.85, 0.85, 0.85)),
            TextLayout::justify(Justify::Center),
        ));
}

fn update_hud(
    spatial: SpatialQuery,
    player: Single<(&Transform, &Player, &FlyCam)>,
    mut readout: Single<&mut Text, With<Readout>>,
    mut crosshair: Single<(&mut Node, &mut BackgroundColor), With<Crosshair>>,
) {
    let (transform, player, fly) = *player;
    let aim = Dir3::new(fly.rotation() * Vec3::NEG_Z).unwrap_or(Dir3::NEG_Z);
    let reachable = spatial.cast_ray(transform.translation, aim, TETHER_RANGE, true, &default()).is_some();
    let (node, color) = &mut *crosshair;
    let size = if reachable { 7.0 } else { 4.0 };
    (node.width, node.height) = (px(size), px(size));
    color.0 = Color::srgba(1.0, 1.0, 1.0, if reachable { 0.95 } else { 0.5 });
    if fly.noclip {
        readout.0 = "noclip".into();
        return;
    }
    let speed = Vec2::new(player.velocity.x, player.velocity.z).length();
    let bars = (player.energy / ENERGY_MAX * 20.0).round() as usize;
    readout.0 = format!(
        "{speed:4.1} m/s   {:4.0} ups
[{}{}]",
        speed / 0.032,
        "|".repeat(bars),
        " ".repeat(20 - bars),
    );
}
