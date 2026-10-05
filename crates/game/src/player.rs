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

/// Ledges whose top is at most this far above the feet can be climbed onto
/// from the air: with a jump, ledges up to about 2.5 m.
const MANTLE_REACH: f32 = 1.1;
const MANTLE_TIME: f32 = 0.2;
/// Least horizontal speed when coming out of a mantle.
const MANTLE_EXIT_SPEED: f32 = 6.0;
/// Time constant of the view catching up with the body after stepping up or
/// down a step, so stairs read as walking rather than a run of hops.
const VIEW_SMOOTH: f32 = 0.05;
/// How far sideways the player may be nudged past an edge they clipped.
const SLIP: f32 = 0.24;

/// Inside a canal's pipe the player's feet ride the exact cylinder (this much
/// smaller than the pipe, for the hull's half width) instead of colliding
/// with its triangles, so speed is never lost at the facets.
const PIPE_RIDE_RADIUS: f32 = worldgen::canal::PIPE_RADIUS as f32 - HALF_WIDTH;

/// The tether lets go only if its line stays blocked this long, so grazing
/// an edge does not cut it.
const TETHER_SEVER_TIME: f32 = 0.15;

/// Half the player's width (Quake: 15 units).
pub const HALF_WIDTH: f32 = 0.45;
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
/// Holding jump while pulled lifts the player (m/s², upwards): not physical,
/// but it carries the body over a lip the eye could see past, where the
/// straight pull would hit it and sever the line. Against the pull it bends
/// the path up by about thirteen degrees.
const TETHER_LIFT: f32 = 13.6;

#[derive(Component)]
pub struct Player {
    pub velocity: Vec3,
    pub grounded: bool,
    ground_normal: Vec3,
    pub energy: f32,
    pub firing: bool,
    pub tether: Tether,
    pub mantle: Option<Mantle>,
    /// Whether the player is on a canal's slick surface.
    pub in_canal: bool,
    /// How long the tether's line has been blocked.
    tether_blocked: f32,
    /// The tether button must be released before the tether fires again.
    tether_held: bool,
    /// Movement waits until the terrain around the spawn point has loaded.
    pub ready: bool,
    /// How far the view lags behind the body after a step (negative after
    /// stepping up).
    view_offset: f32,
}

fn add_player(mut commands: Commands, camera: Single<Entity, With<FlyCam>>) {
    commands.entity(*camera).insert(Player {
        velocity: Vec3::ZERO,
        grounded: false,
        ground_normal: Vec3::Y,
        energy: ENERGY_MAX,
        firing: false,
        tether: Tether::Idle,
        mantle: None,
        in_canal: false,
        tether_blocked: 0.0,
        tether_held: false,
        view_offset: 0.0,
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

    /// Quake's PM_StepSlideMove, plus two forgiveness rules so that small
    /// things never stop you dead: stepping up also works in the air (while
    /// falling or near the top of a jump), and when still blocked the player
    /// may slip a hand's width sideways past an edge they only clipped.
    fn step_slide(&self, center: Vec3, velocity: Vec3, dt: f32, can_step: bool) -> (Vec3, Vec3) {
        let (moved, slid) = self.slide(center, velocity, dt);
        let flat = Vec3::new(velocity.x, 0.0, velocity.z);
        let wanted = flat.length() * dt;
        if wanted < 1e-4 {
            return (moved, slid);
        }
        let dir = flat / flat.length();
        let progress = |p: Vec3| (p - center).dot(dir);
        let mut best = (moved, slid, progress(moved));
        if best.2 >= wanted * 0.95 {
            return (moved, slid);
        }
        if can_step && let Some((p, v)) = self.step_up(center, velocity, dt) {
            if progress(p) > best.2 + 1e-3 {
                best = (p, v, progress(p));
            }
        }
        if best.2 < wanted * 0.5 && let Some((p, v)) = self.slip(center, velocity, dt, dir) {
            if progress(p) > best.2 + 1e-3 {
                best = (p, v, progress(p));
            }
        }
        (best.0, best.1)
    }

    /// The step part of PM_StepSlideMove: from `STEP_HEIGHT` higher, move,
    /// then settle down onto walkable ground.
    fn step_up(&self, center: Vec3, velocity: Vec3, dt: f32) -> Option<(Vec3, Vec3)> {
        let up = self.sweep(center, Vec3::Y * STEP_HEIGHT).map_or(STEP_HEIGHT, |(d, _)| d);
        let flat = Vec3::new(velocity.x, 0.0, velocity.z);
        let (stepped, step_vel) = self.slide(center + Vec3::Y * up, flat, dt);
        let (down, normal) = self.sweep(stepped, Vec3::NEG_Y * (up + 0.05))?;
        (normal.y >= MIN_WALK_NORMAL)
            .then(|| (stepped - Vec3::Y * down, Vec3::new(step_vel.x, 0.0, step_vel.z)))
    }

    /// The most level surface straight below the hull (its middle and
    /// corners). A box resting on a step's edge touches the edge itself,
    /// whose normal leans; what is right below says what the ground really
    /// is: on stairs a tread, on a ramp the ramp.
    fn normal_below(&self, center: Vec3) -> Option<Vec3> {
        let reach = HEIGHT * 0.5 + STEP_HEIGHT + 0.1;
        let c = HALF_WIDTH * 0.8;
        [(0.0, 0.0), (c, c), (-c, c), (c, -c), (-c, -c)]
            .into_iter()
            .filter_map(|(dx, dz)| {
                let from = center + Vec3::new(dx, 0.0, dz);
                self.query.spatial_query.cast_ray(from, Dir3::NEG_Y, reach, true, &self.filter).map(|hit| hit.normal)
            })
            .max_by(|a, b| a.y.total_cmp(&b.y))
    }

    /// Corner correction: try the move again from up to `SLIP` to either side.
    fn slip(&self, center: Vec3, velocity: Vec3, dt: f32, dir: Vec3) -> Option<(Vec3, Vec3)> {
        let side = Vec3::new(-dir.z, 0.0, dir.x);
        let mut best: Option<(Vec3, Vec3, f32)> = None;
        for offset in [SLIP * 0.5, -SLIP * 0.5, SLIP, -SLIP] {
            let shift = side * offset;
            if self.sweep(center, shift).is_some() {
                continue;
            }
            let (p, v) = self.slide(center + shift, velocity, dt);
            let progress = (p - center).dot(dir);
            if best.is_none_or(|(_, _, b)| progress > b) {
                best = Some((p, v, progress));
            }
        }
        best.map(|(p, v, _)| (p, v))
    }

    /// Where the hull would end up mantling onto a ledge ahead along `dir`,
    /// if there is a wall right in front whose top is within `MANTLE_REACH` of
    /// the feet and room to climb onto it.
    fn find_mantle(&self, center: Vec3, dir: Vec3) -> Option<Vec3> {
        let (to_wall, normal) = self.sweep(center, dir * 0.35)?;
        if normal.y.abs() > 0.3 || normal.dot(dir) > -0.5 {
            return None;
        }
        // Room to rise alongside the wall...
        let rise = MANTLE_REACH + 0.05;
        if self.sweep(center, Vec3::Y * rise).is_some() {
            return None;
        }
        let raised = center + Vec3::Y * rise;
        // ...to move over the lip...
        let over_by = to_wall + HALF_WIDTH * 1.6;
        if self.sweep(raised, dir * over_by).is_some() {
            return None;
        }
        let over = raised + dir * over_by;
        // ...and walkable ground to settle onto that is higher than a step.
        let (down, normal) = self.sweep(over, Vec3::NEG_Y * (rise + 0.1))?;
        let target = over - Vec3::Y * down;
        (normal.y >= MIN_WALK_NORMAL && target.y - center.y > STEP_HEIGHT * 0.5).then_some(target)
    }
}

/// A climb onto a ledge in progress: the hull moves from `from` to `to`
/// (rising first, then over the lip) and leaves with `exit` velocity.
#[derive(Clone, Copy, Debug)]
pub struct Mantle {
    from: Vec3,
    to: Vec3,
    t: f32,
    exit: Vec3,
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

/// The tether's pull: accelerate towards the anchor until the speed along
/// the line reaches the cap.
fn pull_towards(velocity: &mut Vec3, to_anchor: Vec3, dt: f32) {
    let dir = to_anchor.normalize_or_zero();
    let along = velocity.dot(dir);
    if along < TETHER_MAX_PULL_SPEED {
        *velocity += dir * (TETHER_PULL * dt).min(TETHER_MAX_PULL_SPEED - along);
    }
}

/// One movement step inside a canal's half-pipe. The surface is slick, as in
/// Quake 3 (Defrag's slick gliding): no friction and air-strength
/// acceleration, with gravity always acting. The feet ride the exact
/// cylinder: after moving, if they are outside it they are put back on it
/// and only the outward part of the velocity is removed, so speed turns into
/// height and back without loss: down one wall, up the other, and with
/// enough speed out above the lip. Returns the new hull centre.
fn ride_pipe(world: &WorldGen, player: &mut Player, center: Vec3, wishdir: Vec3, jump: bool, dt: f32) -> Vec3 {
    let radius = worldgen::canal::PIPE_RADIUS as f32;
    // Jump off the surface, away from it.
    if player.grounded && jump {
        player.velocity += player.ground_normal * JUMP_SPEED;
        player.grounded = false;
    }
    accelerate(&mut player.velocity, wishdir, MAX_SPEED, AIR_ACCEL, dt);
    player.velocity.y -= GRAVITY * dt;

    let mut moved = center + player.velocity * dt;
    player.grounded = false;
    let Some(pipe) = world.canal_at(moved.x, moved.z) else { return moved };
    let across = Vec3::new(pipe.across.x, 0.0, pipe.across.y);
    let axis_height = pipe.floor + radius;
    // The feet relative to the pipe's axis, in its cross-section.
    let (o, h) = (pipe.offset, moved.y - HEIGHT * 0.5 - axis_height);
    let r = (o * o + h * h).sqrt();
    if h < 0.0 && r > PIPE_RIDE_RADIUS {
        let k = PIPE_RIDE_RADIUS / r;
        moved += across * (o * k - o) + Vec3::Y * (h * k - h);
        let outward = (across * o + Vec3::Y * h) / r;
        let into = player.velocity.dot(outward);
        if into > 0.0 {
            player.velocity -= outward * into;
        }
        let normal = -outward;
        if normal.y >= MIN_WALK_NORMAL {
            player.grounded = true;
            player.ground_normal = normal;
        }
    }
    moved
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
        player.view_offset = 0.0;
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
        let blocked = Dir3::new(line).is_ok_and(|ray| {
            length > TETHER_SEVER_MARGIN
                && move_and_slide
                    .spatial_query
                    .cast_ray(eye, ray, length - TETHER_SEVER_MARGIN, true, &default())
                    .is_some()
        });
        player.tether_blocked = if blocked { player.tether_blocked + time.delta_secs() } else { 0.0 };
        if player.tether_blocked > TETHER_SEVER_TIME {
            player.tether = Tether::Missed;
        }
    } else {
        player.tether_blocked = 0.0;
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
    let mut center = transform.translation - Vec3::Y * (EYE - HEIGHT * 0.5 + player.view_offset);
    let total = time.delta_secs().min(0.1);
    let steps = (total / STEP_DT).ceil().max(1.0) as u32;
    let dt = total / steps as f32;
    for _ in 0..steps {
        if let Some(mantle) = &mut player.mantle {
            // Rise over the first two thirds, move over the lip in the last two.
            mantle.t = (mantle.t + dt / MANTLE_TIME).min(1.0);
            let ease = |x: f32| x * x * (3.0 - 2.0 * x);
            let rise = ease((mantle.t / 0.66).min(1.0));
            let over = ease(((mantle.t - 0.33) / 0.67).clamp(0.0, 1.0));
            let (from, to) = (mantle.from, mantle.to);
            center = Vec3::new(
                from.x + (to.x - from.x) * over,
                from.y + (to.y - from.y) * rise,
                from.z + (to.z - from.z) * over,
            );
            if mantle.t >= 1.0 {
                player.velocity = mantle.exit;
                player.mantle = None;
            }
            continue;
        }

        // On the ground unless moving away from it (as in Quake: a jump, a
        // pull, a push).
        let ground = mover
            .sweep(center, Vec3::NEG_Y * 0.08)
            .filter(|(_, n)| n.y >= MIN_WALK_NORMAL)
            .map(|(_, n)| mover.normal_below(center).filter(|n| n.y >= MIN_WALK_NORMAL).unwrap_or(n))
            .filter(|n| !(player.velocity.y > 0.0 && player.velocity.dot(*n) > 0.5));
        player.grounded = ground.is_some();
        if let Some(normal) = ground {
            player.ground_normal = normal;
        }

        // Jumping skips this step's friction, so holding jump bunny hops
        // without losing speed.
        if player.grounded && jump {
            player.velocity.y = JUMP_SPEED;
            player.grounded = false;
        }
        let was_grounded = player.grounded;
        let tethered = matches!(player.tether, Tether::Anchored { .. });
        // Inside a canal's pipe (up to just above its rim), movement is a
        // half-pipe: see `ride_pipe`.
        let feet = center.y - HEIGHT * 0.5;
        let pipe = world.canal_at(center.x, center.z).filter(|f| {
            f.offset.abs() <= PIPE_RIDE_RADIUS + 0.01
                && feet < f.floor + worldgen::canal::PIPE_RADIUS as f32 + 0.3
        });
        player.in_canal = pipe.is_some();
        if pipe.is_some() {
            if player.firing {
                player.velocity -= aim * THRUST * dt;
                player.energy = (player.energy - ENERGY_DRAIN * dt).max(0.0);
            }
            if let Tether::Anchored { point, .. } = player.tether {
                pull_towards(&mut player.velocity, point - center, dt);
            }
            center = ride_pipe(&world, &mut player, center, wishdir, jump, dt);
            continue;
        }
        if player.grounded {
            // The tether's pull drags you along the ground rather than
            // fighting friction.
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
            if (point - center).length() < TETHER_DETACH {
                player.tether = Tether::Missed;
            } else {
                pull_towards(&mut player.velocity, point - center, dt);
                if jump && !player.grounded {
                    player.velocity.y += TETHER_LIFT * dt;
                }
            }
        }

        if player.firing {
            player.velocity -= aim * THRUST * dt;
            player.energy = (player.energy - ENERGY_DRAIN * dt).max(0.0);
        } else if player.grounded {
            player.energy = (player.energy + ENERGY_RECHARGE * dt).min(ENERGY_MAX);
        }

        // Airborne and pushing into a wall: climb it if its top is in reach.
        if !player.grounded && !tethered && wishdir != Vec3::ZERO && player.velocity.y < 5.0
            && let Some(target) = mover.find_mantle(center, wishdir)
        {
            let speed = Vec2::new(player.velocity.x, player.velocity.z).length();
            let exit = wishdir * speed.max(MANTLE_EXIT_SPEED);
            player.mantle = Some(Mantle { from: center, to: target, t: 0.0, exit });
            continue;
        }

        let can_step = player.grounded || player.velocity.y < 2.0;
        let (moved, velocity) = mover.step_slide(center, player.velocity, dt, can_step);
        let rose = moved.y - center.y;
        center = moved;
        player.velocity = velocity;
        if was_grounded && !tethered && !player.firing {
            // Walking: rise no faster than the ground slopes. Catching the
            // corner of a step must not throw the player upwards.
            let n = player.ground_normal;
            let follow = -(player.velocity.x * n.x + player.velocity.z * n.z) / n.y.max(0.1);
            player.velocity.y = player.velocity.y.min(follow.max(0.0));
        }
        if was_grounded && player.grounded && rose > 0.05 {
            // Stepped up: the body is there, the view follows.
            player.view_offset -= rose;
        } else if was_grounded && player.velocity.y <= 0.1 {
            // Walked off a step: stay on the ground below it rather than
            // falling, as walking down stairs should.
            let on_ground = mover.sweep(center, Vec3::NEG_Y * 0.08).is_some_and(|(_, n)| n.y >= MIN_WALK_NORMAL);
            if !on_ground
                && let Some((down, n)) = mover.sweep(center, Vec3::NEG_Y * (STEP_HEIGHT + 0.05))
                && n.y >= MIN_WALK_NORMAL
            {
                center.y -= down;
                player.velocity.y = 0.0;
                player.view_offset += down;
            }
        }
    }
    player.view_offset = (player.view_offset * (-total / VIEW_SMOOTH).exp()).clamp(-1.0, 1.0);
    transform.translation = center + Vec3::Y * (EYE - HEIGHT * 0.5 + player.view_offset);

    // Never fall out of the world.
    if transform.translation.y < ground_height - 30.0 {
        player.view_offset = 0.0;
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
    if args.shot.is_some() || args.opt("labshots") {
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
