//! The impaled brute (the user, 2026-11-09): too heavy to walk, held up by
//! thrust from under its feet (`field.rs`). It flies on `flight.rs`, its
//! weight in the physics. Flight is its only way to move, but it lands, and
//! it attacks landed (the user, 2026-11-10; step 4): it chooses a spot near
//! you, flies its way there (`way.rs`), hangs over it a moment and crashes
//! down onto it (letting go of its thrust), the ground struck round it;
//! stays, lashing at you with the cables wound round its waist (`attack.rs`),
//! rises and chooses again; it takes off sooner if you get away or out of
//! sight. `keep=N`: it hovers after you N m off
//! instead (step 2's way, to judge its field by).
//!
//! Shot (the agreed plan, 2026-11-09: its thrust stutters, its death a heavy
//! fall): each shard stuck in it a moment and pushing it a little (it is
//! heavy), its thrust cut a moment, so it dips, rocks back and labours up
//! again. Dead, its thrust is gone: it falls, lands hard and topples over,
//! away from the shot that killed it, and lies there.
//!
//!
//! `spawn impaled course`: it flies a set course across before you, side on,
//! to judge its flight: across low, up, back high (its thrust stuttering
//! midway), down, again. `log`: what it does, each second. Its weight's
//! numbers can be asked for (`lift=`, `spool=`, `tilt=`, `climb=`, `speed=`,
//! `hover=`), to judge by feel. What holds it up shows as one of three
//! fields (`field=1|2|3`, `fields`: one of each, side by side; see
//! `field.rs`).

use bevy::prelude::*;

pub(super) mod attack;
pub(super) mod field;
pub(super) mod plume;
mod way;

use crate::camera::FlyCam;
use crate::flight::{Craft, Flight, Goal, Pilot};

/// Its mesh, in `creatures/` (made by `tools/impaled_mesh.py`).
const FILE: &str = "impaled.glb";
/// How tall it stands (m); its mesh's height in its own units and where its
/// feet are below its centre (measured from the file).
const HEIGHT: f32 = 2.6;
const MESH_HEIGHT: f32 = 1.90;
const MESH_FEET: f32 = 0.952;
/// How far ahead of you it comes (m), and its course's middle; how high its
/// feet keep above the ground in flight (m).
const ARRIVE: f32 = 12.0;
const COURSE_AHEAD: f32 = 24.0;
const HOVER: f32 = 1.5;
/// Landing: how long it stays down (s; where its attack will be); how far
/// from where it stands you may get, and how long out of its sight, before
/// it takes off sooner (m, s); how far it rises before it chooses again
/// (m); how close it passes a point on its way before making for the next
/// (m); how long it may fly to a spot before choosing again (s); how long
/// before it tries again when it found no spot (s); how long beaten down
/// onto the ground before it stays there, landed (s).
const LANDED_FOR: f32 = 5.0;
const LEAVE: (f32, f32) = (16.0, 1.0);
const RISE: f32 = 3.0;
const PASS: f32 = 2.0;
const FLY_FOR: f32 = 25.0;
const AGAIN: f32 = 1.0;
const BEATEN: f32 = 0.6;
/// Its landing, a crash (the user, 2026-11-10: violently, not settling
/// gently): how high its feet are over the spot when it lets go (m), how
/// hard it shoves itself down then (m/s), how fast it rights itself as it
/// falls (1/s: it falls level), and the blow: how far off it shakes your
/// view (m); its body squashed down onto its feet, as much as this for
/// each m/s it hit at (of its height; at most `SQUASH.1`), squashing in
/// `SQUASH.2` s and back in about `SQUASH.3` s. (A rock of spin on impact
/// looked goofy, not reeling, the user: gone.)
const DROP: f32 = 4.0;
const SLAM: f32 = 5.0;
/// How long it hangs over its spot before it lets go (s: the crash's tell).
const HANG: f32 = 0.5;
const LEVEL: f32 = 14.0;
const JOLT: f32 = 30.0;
const SQUASH: (f32, f32, f32, f32) = (0.018, 0.18, 0.05, 0.3);
/// The course: how far either side of its middle (m), how much higher its
/// high legs (m), how long it holds at each corner (s), and how long its
/// thrust stutters on the way back (s).
const COURSE_HALF: f32 = 12.0;
const COURSE_RISE: f32 = 6.0;
const COURSE_HOLD: f32 = 3.0;
const STUTTER: f32 = 0.3;
/// Shot: how much it takes (the hunter's is 45, a close blast 29), how
/// wide its body (m, a capsule from its feet to its crown), how long its
/// thrust is cut by a hit (s), how much each shard pushes it (m/s) and
/// rocks it (rad/s).
const HEALTH: f32 = 80.0;
const RADIUS: f32 = 0.75;
const HIT_STUTTER: f32 = 0.12;
const HIT_PUSH: f32 = 0.08;
const HIT_ROCK: f32 = 0.05;
/// Dead, toppling: how far its middle is above the edge it tips on (m), how
/// fast it starts tipping (rad/s), and how far over it ends (rad: on its
/// front, propped on what is in it).
const TOPPLE_HIGH: f32 = 1.3;
const TOPPLE_START: f32 = 0.35;
const TOPPLE_END: f32 = 1.4;

fn scale() -> f32 {
    HEIGHT / MESH_HEIGHT
}

/// Its feet below its centre (m).
fn feet() -> f32 {
    MESH_FEET * scale()
}

/// A brute, flying.
#[derive(Component)]
pub(in crate::combat) struct Impaled {
    flight: Flight,
    craft: Craft,
    pilot: Pilot,
    hover: f32,
    keep: Option<f32>,
    course: Option<Course>,
    mind: Mind,
    log: bool,
    logged: f32,
    logged_inside: f32,
    health: f32,
    /// How long its thrust is still cut (s), and which way it was last shot
    /// (it falls away from that).
    stutter: f32,
    shot: Vec3,
    /// Dead and landed: toppling, then lying.
    down: Option<Down>,
    /// Crashed down: how long ago (s), and how deep its squash.
    crashed: Option<(f32, f32)>,
    /// Its lash, landed.
    lash: attack::Lash,
}

/// What it is doing.
enum Mind {
    /// Choosing where to land (again in `wait` s, none found).
    Choose { wait: f32 },
    /// On its way to land: its way, the point it makes for, how long it has
    /// flown, how long it has hung over its spot, how long it has been
    /// beaten down onto the ground.
    Fly { way: way::Way, leg: usize, flown: f32, hung: f32, grounded: f32 },
    /// Down on its feet at `spot`, how long, how long you have been out of
    /// its sight.
    Landed { spot: Vec3, time: f32, unseen: f32 },
    /// Rising off the ground to `to` (its middle's height) before choosing.
    Rise { from: Vec3, to: f32 },
}

impl Mind {
    /// Where it lands, or stands.
    fn spot(&self) -> Option<Vec3> {
        match self {
            Mind::Fly { way, .. } => Some(way.spot),
            Mind::Landed { spot, .. } => Some(*spot),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Mind::Choose { .. } => "choosing",
            Mind::Fly { .. } => "flying",
            Mind::Landed { .. } => "landed",
            Mind::Rise { .. } => "rising",
        }
    }
}

/// Dead, toppling over the edge of its feet: from where it landed, about
/// that edge, how far over and how fast.
struct Down {
    pos: Vec3,
    rot: Quat,
    edge: Vec3,
    axis: Vec3,
    angle: f32,
    speed: f32,
}

impl Impaled {
    /// Where a shot along `dir` from `eye` strikes its body, if before
    /// `limit` (the dead are not shot).
    pub(super) fn ray(&self, eye: Vec3, dir: Vec3, limit: f32) -> Option<f32> {
        if !self.alive() {
            return None;
        }
        let (pos, rot) = self.flight.drawn();
        let up = rot * Vec3::Y;
        let (a, b) = (pos - up * (feet() - RADIUS), pos + up * (HEIGHT - feet() - RADIUS));
        capsule(eye, dir, a, b, RADIUS).filter(|&d| d > 0.0 && d < limit)
    }

    /// A shard struck it, along `dir`: hurt, pushed and rocked a little, its
    /// thrust cut a moment.
    pub(super) fn hurt(&mut self, dir: Vec3, damage: f32) {
        self.health -= damage;
        let level = Vec3::new(dir.x, 0.0, dir.z);
        self.flight.vel += level * HIT_PUSH;
        self.flight.spin += Vec3::Y.cross(level).normalize_or_zero() * HIT_ROCK;
        self.stutter = HIT_STUTTER;
        self.shot = level;
    }

    /// Where it is drawn now (to stick a shard in it).
    pub(super) fn drawn(&self) -> Transform {
        let (pos, rot) = self.flight.drawn();
        Transform::from_translation(pos).with_rotation(rot)
    }

    pub(super) fn alive(&self) -> bool {
        self.health > 0.0
    }
}

/// Where a ray from `o` along `d` (unit) first meets a capsule from `a` to
/// `b`, `r` wide.
fn capsule(o: Vec3, d: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
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

/// A set course: its middle (on the ground) and across, the corner it is
/// making for, how long it has held there, and whether it has stuttered on
/// this leg.
struct Course {
    middle: Vec3,
    across: Vec3,
    facing: Vec3,
    leg: usize,
    held: f32,
    stuttered: bool,
}

impl Course {
    /// The corners: across low, then up, back high, down.
    fn corner(&self, leg: usize, hover: f32) -> Vec3 {
        let (side, high) = [(-1.0, false), (1.0, false), (1.0, true), (-1.0, true)][leg % 4];
        self.middle + self.across * (side * COURSE_HALF) + Vec3::Y * (feet() + hover + if high { COURSE_RISE } else { 0.0 })
    }
}

/// `spawn impaled [N] [course] [log]`: each where you look, ahead, on the
/// ground's height plus its hover.
#[allow(clippy::too_many_arguments)]
pub(super) fn arrive(mut commands: Commands, server: Res<AssetServer>, args: Res<crate::Args>, kit: Res<field::Kit>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut plumes: ResMut<Assets<plume::Plume>>, ground: crate::ground::Ground, camera: Single<&Transform, With<FlyCam>>, mut spawns: ResMut<crate::console::Spawns>) {
    let camera = *camera;
    let forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::Z);
    let across = Vec3::new(-forward.z, 0.0, forward.x);
    let (asked, others): (Vec<_>, Vec<_>) = std::mem::take(&mut spawns.creatures).into_iter().partition(|(kind, ..)| kind == "impaled");
    spawns.creatures = others;
    for (_, count, traits, places) in asked {
        for n in 0..count {
            let base = match places.len() {
                0 => camera.translation + forward * if traits.has("course") { COURSE_AHEAD } else { ARRIVE } + across * ((n as f32 - (count - 1) as f32 * 0.5) * 6.0),
                k => places[n as usize % k],
            };
            // (Its weight's numbers as asked: to judge by feel, two side by
            // side.)
            let craft = Craft { lift: traits.value("lift", Craft::BRUTE.lift), spool: traits.value("spool", Craft::BRUTE.spool), tilt: traits.value("tilt", Craft::BRUTE.tilt.to_degrees()).to_radians(), climb: traits.value("climb", Craft::BRUTE.climb), ..Craft::BRUTE };
            let pilot = Pilot { speed: traits.value("speed", Pilot::BRUTE.speed), ..Pilot::BRUTE };
            let hover = traits.value("hover", HOVER);
            let keep = traits.has("keep").then(|| traits.value("keep", 14.0));
            let ground_y = ground.at(base);
            let course = traits.has("course").then(|| Course { middle: Vec3::new(base.x, ground_y, base.z), across, facing: -forward, leg: 0, held: 0.0, stuttered: false });
            // (On its course, it starts at its first corner.)
            let at = course.as_ref().map_or(Vec3::new(base.x, ground_y + feet() + hover, base.z), |c| c.corner(0, hover));
            let flight = Flight::hovering(&craft, at, -forward);
            let (pos, rot) = flight.drawn();
            let brute = commands
                .spawn((Impaled { flight, craft, pilot, hover, keep, course, mind: Mind::Choose { wait: 0.0 }, log: traits.has("log"), logged: 0.0, logged_inside: 0.0, health: traits.value("health", HEALTH), stutter: 0.0, shot: Vec3::ZERO, down: None, crashed: None, lash: default() }, Transform::from_translation(pos).with_rotation(rot), Visibility::Visible))
                .with_child((WorldAssetRoot(server.load(GltfAssetLabel::Scene(0).from_asset(format!("creatures://{FILE}")))), Transform::from_scale(Vec3::splat(scale()))))
                .id();
            let shards = traits.value("plume", 2.0) as u8 == 2;
            field::make(&mut commands, &kit, &mut meshes, &mut materials, &mut plumes, brute, shards, args.shot.is_none());
            attack::make(&mut commands, &mut meshes, &mut materials, brute);
            info!("an impaled brute hovers at {at:.1}{}", if traits.has("course") { ", to fly its course" } else { "" });
        }
    }
}

/// Each brute flown a frame: towards its goal (after you, or along its
/// course), never below the ground, drawn between its steps; its thrust cut
/// while it stutters; dead, falling, landing, toppling.
#[allow(clippy::too_many_arguments)]
pub(super) fn fly(
    mut commands: Commands,
    time: Res<Time>,
    kit: Res<field::Kit>,
    assets: Res<super::Assets3>,
    mut gun: ResMut<super::Gun>,
    mut director: ResMut<super::Director>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ground: crate::ground::Ground,
    camera: Single<(&Transform, &mut crate::player::Player), With<FlyCam>>,
    mut pushes: ResMut<field::Pushes>,
    mut brutes: Query<(Entity, &mut Impaled, &mut Transform), Without<FlyCam>>,
) {
    let dt = time.delta_secs().min(0.1);
    let (camera, mut player) = camera.into_inner();
    let you = camera.translation;
    let right = *camera.right();
    pushes.0.clear();
    // (Where each lands, so that the others land apart.)
    let taken: Vec<(Entity, Vec3)> = brutes.iter().filter_map(|(e, b, _)| b.mind.spot().map(|s| (e, s))).collect();
    // (One choice a frame among them all: each costs up to 3 ms of sweeps.)
    let mut chose = false;
    for (entity, mut b, mut t) in &mut brutes {
        let b = &mut *b;
        // (Toppled: tipping over the edge of its feet, faster as it goes,
        // until it lies; then still.)
        if let Some(d) = &mut b.down {
            if d.angle < TOPPLE_END {
                d.speed += crate::flight::G / TOPPLE_HIGH * d.angle.sin().max(0.05) * dt;
                d.angle = (d.angle + d.speed * dt).min(TOPPLE_END);
                if d.angle >= TOPPLE_END {
                    commands.spawn((AudioPlayer::new(kit.thud.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(bevy::audio::Volume::Linear(0.5)), Transform::from_translation(d.edge)));
                }
            }
            let turn = Quat::from_axis_angle(d.axis, d.angle);
            t.translation = d.edge + turn * (d.pos - d.edge);
            t.rotation = turn * d.rot;
            t.scale = Vec3::ONE;
            let push = field::Push { feet: t.translation - Vec3::Y * feet(), strain: 0.0, at: t.translation, rot: t.rotation };
            pushes.0.insert(entity, (push, ground.at(d.edge)));
            continue;
        }
        let here = b.flight.pos;
        let goal = match &mut b.course {
            Some(c) => {
                let to = c.corner(c.leg, b.hover);
                if here.distance(to) < 0.4 && b.flight.vel.length() < 0.4 {
                    c.held += dt;
                    if c.held > COURSE_HOLD {
                        c.held = 0.0;
                        c.leg += 1;
                        c.stuttered = false;
                    }
                }
                // (On the way back high, passing its middle: a stutter.)
                if c.leg % 4 == 3 && !c.stuttered && (here - c.middle).dot(c.across).abs() < 1.0 {
                    c.stuttered = true;
                    b.stutter = STUTTER;
                }
                Goal { at: to, vel: Vec3::ZERO, facing: c.facing }
            }
            None if b.keep.is_some() => {
                // After you: at its distance on the line from you to it, its
                // feet clear of the ground there and under it, facing you.
                let away = Vec3::new(here.x - you.x, 0.0, here.z - you.z).normalize_or(Vec3::Z);
                let spot = Vec3::new(you.x, 0.0, you.z) + away * b.keep.unwrap_or_default();
                let floor = ground.at(spot.with_y(here.y)).max(ground.at(here));
                Goal { at: spot.with_y(floor + feet() + b.hover), vel: Vec3::ZERO, facing: -away }
            }
            None => {
                let others: Vec<Vec3> = taken.iter().filter(|(e, _)| *e != entity).map(|(_, s)| *s).collect();
                think(b, &ground, you, &others, entity.index_u32(), dt, &mut chose)
            }
        };
        // (Its thrust cut while it stutters, and landed; dead, for good.)
        b.stutter -= dt;
        let landed = matches!(b.mind, Mind::Landed { .. }) && b.course.is_none() && b.keep.is_none();
        // (Letting go over its spot, crashing down onto it.)
        let dropping = matches!(&b.mind, Mind::Fly { way, leg, .. } if *leg + 1 == way.points.len()) && b.course.is_none() && b.keep.is_none();
        b.flight.cut = if b.alive() && b.stutter <= 0.0 && !landed && !dropping { 1.0 } else { 0.0 };
        // (Falling level: it rights itself as it lets go.)
        if dropping && b.alive() {
            let facing = b.flight.rot * Vec3::Z;
            b.flight.rot = b.flight.rot.slerp(crate::flight::upright(Vec3::Y, facing), 1.0 - (-LEVEL * dt).exp());
            b.flight.spin = Vec3::ZERO;
        }
        let falling = b.flight.vel.y;
        b.flight.advance(&b.craft, &b.pilot, &goal, dt);
        let floor = ground.at(b.flight.pos) + feet();
        b.flight.rest_on(floor);
        // (Landed, its feet hold it where it stands.)
        if landed && b.alive() && b.flight.pos.y <= floor + 0.05 {
            b.flight.vel = Vec3::ZERO;
        }
        // (Its crash: the blow heard, felt in your view near it, rocking it.)
        if dropping && b.alive() && b.flight.pos.y <= floor + 1e-3 && falling < -1.0 {
            let speed = -falling;
            let at = b.flight.pos - Vec3::Y * feet();
            commands.spawn((AudioPlayer::new(kit.thud.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(bevy::audio::Volume::Linear((0.5 + 0.05 * speed).min(1.0))), Transform::from_translation(at)));
            gun.shake = gun.shake.max((1.0 - at.distance(you) / JOLT).clamp(0.0, 1.0));
            b.crashed = Some((0.0, (SQUASH.0 * speed).min(SQUASH.1)));
            if let Some((harm, push)) = attack::quake(&mut commands, kit.ring.clone(), &mut materials, at, you - Vec3::Y * crate::player::EYE, player.grounded) {
                blow(&mut commands, &assets, &mut player, &mut director, &mut gun, right, harm, push);
                if b.log {
                    info!("impaled: its crash struck you ({harm:.0})");
                }
            }
            b.flight.vel = Vec3::ZERO;
            if b.log {
                info!("impaled: crashed down at {:.1} m/s, {:.1}° off upright, squashed {:.0}%", speed, b.flight.up().angle_between(Vec3::Y).to_degrees(), (SQUASH.0 * speed).min(SQUASH.1) * 100.0);
            }
        }
        // (Dead, it lands: hard, the harder the faster it fell; then it
        // topples away from the shot that killed it.)
        if !b.alive() && b.flight.pos.y <= floor + 1e-3 {
            let (pos, rot) = b.flight.drawn();
            let back = rot * Vec3::NEG_Z;
            let away = if b.shot.length_squared() > 0.0 { b.shot.normalize() } else { Vec3::new(back.x, 0.0, back.z).normalize_or(Vec3::X) };
            let loud = (0.4 + 0.12 * (-falling).max(0.0)).min(1.0);
            commands.spawn((AudioPlayer::new(kit.thud.clone()), PlaybackSettings::DESPAWN.with_spatial(true).with_volume(bevy::audio::Volume::Linear(loud)), Transform::from_translation(pos - Vec3::Y * feet())));
            let edge = Vec3::new(pos.x, floor - feet(), pos.z) + away * 0.45;
            b.down = Some(Down { pos, rot, edge, axis: Vec3::Y.cross(away).normalize(), angle: 0.0, speed: TOPPLE_START });
            info!("an impaled brute is down at {pos:.1}");
        }
        let (pos, rot) = b.flight.drawn();
        t.translation = pos;
        t.rotation = rot;
        // (Landed, it lashes at you; else its lash is put away.)
        if landed && b.alive() {
            let seen = ground.clear(pos + Vec3::Y * (HEIGHT - feet() - 0.3), you);
            if let Some(along) = attack::lash(b, you, seen, dt) {
                let (harm, push) = attack::lash_blow(along);
                blow(&mut commands, &assets, &mut player, &mut director, &mut gun, right, harm, push);
                if b.log {
                    info!("impaled: its lash struck you");
                }
            }
        } else if b.lash.phase != attack::Phase::Rest {
            attack::stow(b);
        }
        // (Its crash's squash: down onto its feet, wider for it, quickly;
        // back slowly.)
        t.scale = Vec3::ONE;
        if let Some((since, deep)) = &mut b.crashed {
            *since += dt;
            let k = if *since < SQUASH.2 { *since / SQUASH.2 } else { (-(*since - SQUASH.2) / SQUASH.3).exp() };
            let squash = *deep * k;
            if *since > SQUASH.2 && squash < 0.002 {
                b.crashed = None;
            } else {
                let tall = 1.0 - squash;
                t.scale = Vec3::new(1.0 / tall.sqrt(), tall, 1.0 / tall.sqrt());
                t.translation -= rot * Vec3::Y * (feet() * squash);
            }
        }
        // (Its ways are to keep it clear of what is built: said if not.)
        if b.alive() && !landed && ground.inside(b.flight.pos, way::BODY * 0.7) && b.logged_inside <= 0.0 {
            warn!("impaled: inside something built at {:.1}, {}", b.flight.pos, b.mind.name());
            b.logged_inside = 1.0;
        }
        b.logged_inside -= dt;
        // (For its field: its feet, how hard it pushes, the ground there.)
        let feet_at = pos - Vec3::Y * feet();
        let push = field::Push { feet: feet_at, strain: b.flight.thrust / (b.craft.mass * crate::flight::G), at: pos, rot };
        pushes.0.insert(entity, (push, ground.at(feet_at)));
        if b.log {
            b.logged += dt;
            if b.logged >= 1.0 {
                b.logged = 0.0;
                let f = &b.flight;
                let leg = b.course.as_ref().map_or_else(|| format!(", {}", b.mind.name()), |c| format!(" leg {}", c.leg % 4));
                info!(
                    "impaled: at {:.1}, {:.1} m/s, tilt {:.0}°, feet {:.1} m up, thrust {:.2} of its weight, health {:.0}{leg}",
                    f.pos,
                    f.vel.length(),
                    f.up().angle_between(Vec3::Y).to_degrees(),
                    f.pos.y - feet() - ground.at(f.pos),
                    f.thrust / (b.craft.mass * crate::flight::G),
                    b.health
                );
            }
        }
    }
}

/// What a brute that lands makes for this frame, its mind moved on: choose
/// a spot, fly its way there, land, stay, rise, again (choosing only if no
/// other has this frame, `chose`).
fn think(b: &mut Impaled, ground: &crate::ground::Ground, you: Vec3, taken: &[Vec3], seed: u32, dt: f32, chose: &mut bool) -> Goal {
    let here = b.flight.pos;
    let facing = |at: Vec3| Vec3::new(you.x - at.x, 0.0, you.z - at.z).normalize_or(Vec3::Z);
    let floor = ground.at(here);
    let hover = b.hover;
    let log = b.log;
    let mut slam = false;
    // (Still falling: not landed till the crash has happened.)
    let falling = b.flight.vel.y < -0.5;
    let next = match &mut b.mind {
        Mind::Choose { wait } => {
            *wait -= dt;
            if *wait > 0.0 || *chose {
                None
            } else {
                *chose = true;
                let started = std::time::Instant::now();
                let chosen = way::choose(ground, here, you, hover.max(DROP), taken, seed.wrapping_add((here.x * 7.0) as u32));
                let took = started.elapsed().as_secs_f32() * 1000.0;
                match chosen {
                    Ok(way) => {
                        if log {
                            info!("impaled: to land at {:.1}, by {} points ({took:.2} ms)", way.spot, way.points.len());
                        }
                        Some(Mind::Fly { way, leg: 0, flown: 0.0, hung: 0.0, grounded: 0.0 })
                    }
                    Err(refused) => {
                        if log {
                            info!("impaled: no spot to land near you, {refused:?} ({took:.2} ms)");
                        }
                        Some(Mind::Choose { wait: AGAIN })
                    }
                }
            }
        }
        Mind::Fly { way, leg, flown, hung, grounded } => {
            *flown += dt;
            let last = way.points.len() - 1;
            let to = way.points[*leg];
            let speed = b.flight.vel.length();
            // (Through its way's points, slowing only over the spot; there,
            // it lets go and crashes down onto it.)
            if *leg + 1 == last && here.distance(to) < 1.0 && speed < 1.5 {
                *hung += dt;
            }
            if *leg + 1 < last && here.distance(to) < PASS || *leg + 1 == last && *hung > HANG {
                *leg += 1;
                slam |= *leg == last;
            }
            // (Beaten down onto the ground on its way: it stays there.)
            *grounded = if here.y - feet() - floor < 0.15 { *grounded + dt } else { 0.0 };
            if *leg == last && here.y - feet() - floor < 0.08 && !falling || *grounded > BEATEN {
                Some(Mind::Landed { spot: here.with_y(floor), time: 0.0, unseen: 0.0 })
            } else if way.spot.xz().distance(you.xz()) > LEAVE.0 || *flown > FLY_FOR {
                // (You got away from where it was landing, or it took too
                // long: choose again.)
                Some(Mind::Choose { wait: 0.0 })
            } else {
                None
            }
        }
        Mind::Landed { spot, time, unseen } => {
            *time += dt;
            *unseen = if ground.clear(*spot + Vec3::Y * (HEIGHT - 0.3), you) { 0.0 } else { *unseen + dt };
            (*time > LANDED_FOR || spot.xz().distance(you.xz()) > LEAVE.0 || *unseen > LEAVE.1).then(|| Mind::Rise { from: *spot, to: here.y + RISE })
        }
        Mind::Rise { to, .. } => (here.y > *to - 0.5).then_some(Mind::Choose { wait: 0.0 }),
    };
    if slam {
        b.flight.vel.y -= SLAM;
    }
    if let Some(mind) = next {
        if log {
            info!("impaled: {} at {:.1}", mind.name(), here);
        }
        b.mind = mind;
    }
    match &b.mind {
        Mind::Choose { .. } => Goal { at: here, vel: Vec3::ZERO, facing: facing(here) },
        Mind::Fly { way, leg, .. } => {
            let to = way.points[*leg];
            // (Facing where it flies on a long way, you near the end.)
            let ahead = Vec3::new(to.x - here.x, 0.0, to.z - here.z);
            let face = if ahead.length() > 6.0 { ahead.normalize() } else { facing(here) };
            Goal { at: to, vel: Vec3::ZERO, facing: face }
        }
        Mind::Landed { spot, .. } => Goal { at: spot.with_y(spot.y + feet()), vel: Vec3::ZERO, facing: facing(here) },
        Mind::Rise { from, to } => Goal { at: from.with_y(*to), vel: Vec3::ZERO, facing: facing(here) },
    }
}

/// A blow from a brute: you hurt, shoved, the view jolted from its side,
/// the hurt flash; you cry out.
#[allow(clippy::too_many_arguments)]
fn blow(commands: &mut Commands, assets: &super::Assets3, player: &mut crate::player::Player, director: &mut super::Director, gun: &mut super::Gun, right: Vec3, harm: f32, push: Vec3) {
    player.health -= harm;
    player.velocity += push;
    director.hurt = 1.0;
    gun.jolt += Vec2::new(0.06, -push.dot(right) * 0.02);
    commands.spawn((AudioPlayer::new(assets.player_pain.clone()), PlaybackSettings::DESPAWN));
}
