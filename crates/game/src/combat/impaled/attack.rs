//! The brute's attacks (the user, 2026-11-10): its crash down onto its spot
//! a blow to the ground round it, and, landed, its grasp: cables of its arms
//! (`arms.rs`: its arms are cables, wound round its waist, the original
//! concept) flung out at you, homing slowly, taking hold.
//!
//! The crash: it hangs a moment over its spot (the tell), lets go, and
//! where it lands the ground is struck: a ring sweeps out, and if you
//! stand within `QUAKE` you are hurt, more the nearer, and thrown off your
//! feet; in the air (a jump), you are spared.
//!
//! Its grasp: landed, you within its range and in its sight, its shoulders
//! draw back, the line between them bowing open (the tell); it flings a
//! cable from each arm, rolling its shoulders forward, and they come at you
//! slowly through the air, turning after you, their way behind them
//! sagging under its weight, fed from its waist band; one that touches you
//! takes hold, and hurts you as it holds (and as `latch=N` has it, slows
//! you, holds you, or reels you in); hurt it enough, or get far enough
//! away, and it lets go; out of range they come back.
//!
//! (The user, 2026-11-10: first a lash, short and fast; this creature is
//! too slow to reposition to use one; so this. What the lash taught: the
//! body must move the cables, as a hand a whip, its shoulders drawn back
//! and rolled forward, no lean; the arms' own cables, not a bundle of its
//! own; both arms.)

use bevy::prelude::*;

use super::Impaled;

/// The crash: how far round it the ground is struck (m), how much it hurts
/// at its middle (falling to none at its edge), how hard it throws you
/// (m/s, out and up), how long its ring takes to sweep out (s).
pub(super) const QUAKE: f32 = 6.0;
const QUAKE_HARM: f32 = 25.0;
const QUAKE_THROW: (f32, f32) = (6.0, 3.0);
const RING_TIME: f32 = 0.35;
/// Its grasp (the user, 2026-11-10: not a lash: too slow to come at you, a
/// short fast attack it cannot use, so long, slow, homing, holding you):
/// how far it reaches (m); how long it rests between and winds up (s); how
/// fast its heads are flung and how fast they home (m/s: you run at 10),
/// and how long to get to speed (s); how fast they turn towards you
/// (rad/s); how near one must come to take hold (m); how much it hurts
/// taking hold, and holding (a second); how long it holds at most (s), and
/// how much hurting it makes it let go; how fast its heads come back (m/s);
/// how far apart its cables' way is kept (m), and how fast and how far it
/// sags under its weight (m/s, m).
pub(super) const RANGE: f32 = 25.0;
const COOL: f32 = 1.5;
const WIND: f32 = 0.6;
const SPEED: (f32, f32) = (3.0, 7.0);
const RAMP: f32 = 0.8;
const TURN: f32 = 1.4;
const LATCH: f32 = 0.6;
const BITE: f32 = 8.0;
const GNAW: f32 = 6.0;
const HOLD_FOR: f32 = 6.0;
const RELEASE: f32 = 20.0;
const BACK: f32 = 9.0;
const STEP: f32 = 0.3;
const DROP: (f32, f32) = (0.5, 1.2);
/// Held (`latch=N`): slowed to this (m/s), reeled in at this (m/s).
const SLOW: f32 = 4.0;
const REEL: f32 = 3.0;
/// Its body in its grasp (see `arms::bent`): winding up, its shoulders drawn
/// back (mesh units), the line between them bowing open, its body not
/// leaning (rad about its waist band: none; the force is drawn from the
/// shoulders' arch, not from leaning back, the user); flinging, its
/// shoulders rolled forward, its body not leaning either (the user), as its
/// cables fly; how long it takes to roll them forward (s).
const ARCH: Vec2 = Vec2::new(0.0, -0.10);
const THROW: Vec2 = Vec2::new(0.0, 0.11);
const FLING: f32 = 0.3;
/// You, struck: from below your eye to above it (m).
const BODY: (f32, f32) = (1.3, 0.15);

/// Its grasp: what it is doing (winding up; its cables out; at rest), how
/// long it has been winding up, how long since it last flung its cables,
/// how long since none of them was after you or holding you, its cables'
/// heads out (coming back too), how long it has held you, its health when
/// it took hold, and how far from it you were then (m).
#[derive(Default)]
pub(super) struct Grasp {
    pub phase: Phase,
    wind: Option<f32>,
    flung: f32,
    idle: f32,
    pub heads: Vec<Head>,
    held: f32,
    health_at: Option<f32>,
    pub reach: f32,
    busy: bool,
    /// Whether one of its grasps ended (missed, or let go) this frame.
    pub ended: bool,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub(super) enum Phase {
    #[default]
    Rest,
    Wind,
    Out,
}

/// One of its cables flung out: which cable, its head (where, how fast), the
/// way it has come (the points its cable lies along, each with how far it
/// has sagged since), what it is doing, how far it has gone and how long.
pub(super) struct Head {
    pub cable: usize,
    pub at: Vec3,
    vel: Vec3,
    pub trail: Vec<(Vec3, f32)>,
    pub state: State,
    gone: f32,
    age: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum State {
    Fly,
    Held,
    Back,
    Home,
}

/// What its grasp does to you this frame: how much it hurts, whether it
/// just took hold, and where it holds you from (it).
pub(super) struct Grip {
    pub harm: f32,
    pub bitten: bool,
    pub from: Vec3,
    pub reach: f32,
}

/// What a landed brute's grasp does this frame: winds up when it can (you
/// in range and sight, rested), flings a cable from each arm, which home on
/// you slowly, turning; one that touches you takes hold; let go of (it held
/// long enough, you hurt it enough, you got far enough away) or out of
/// range, they come back along the way they came. Some(what it does to
/// you) while it holds you.
pub(super) fn grasp(b: &mut Impaled, you: Vec3, seen: bool, landed: bool, dt: f32) -> Option<Grip> {
    let (pos, rot) = b.flight.drawn();
    let scale = super::scale();
    let health = b.health;
    let g = &mut b.grasp;
    let chest = you - Vec3::Y * 0.5;
    g.flung += dt;
    // (Free again once none of its cables is after you or holding you: its
    // rest counts from then, not from their coming back, the user,
    // 2026-11-10: a failed grasp must not leave it a sitting duck.)
    // (Aloft: no grasp; its cables come back to it, wherever it goes: their
    // coming back holds nothing up, neither its next grasp nor its flight,
    // the user, 2026-11-10.)
    if !landed {
        g.wind = None;
        for h in &mut g.heads {
            if matches!(h.state, State::Fly | State::Held) {
                h.state = State::Back;
            }
        }
    }
    let busy = g.heads.iter().any(|h| matches!(h.state, State::Fly | State::Held));
    g.ended = g.busy && !busy;
    g.busy = busy;
    g.idle = if busy || g.wind.is_some() { 0.0 } else { g.idle + dt };
    if landed && !busy && g.wind.is_none() && g.idle > COOL && seen && you.distance(pos) < RANGE * 0.9 {
        g.wind = Some(0.0);
    }
    if let Some(w) = &mut g.wind {
        *w += dt;
        if *w > WIND {
            g.wind = None;
            g.flung = 0.0;
            // (Each arm its outermost cable not out already, coming back.)
            let out: Vec<usize> = g.heads.iter().map(|h| h.cable).collect();
            for side in [1.0f32, -1.0] {
                let Some(cable) = super::arms::lasher(side, &out) else { continue };
                let start = pos + rot * super::arms::elbow(cable, scale, ARCH);
                // (Flung up and out its own side, then turning to you.)
                let fling = ((chest - start).normalize_or(rot * Vec3::Z) + Vec3::Y * 0.6 + rot * Vec3::X * (side * 0.5)).normalize();
                g.heads.push(Head { cable, at: start, vel: fling * SPEED.0, trail: vec![(start, 0.0)], state: State::Fly, gone: 0.0, age: 0.0 });
            }
        }
    }
    g.phase = if g.wind.is_some() {
        Phase::Wind
    } else if g.heads.is_empty() {
        Phase::Rest
    } else {
        Phase::Out
    };
    if g.heads.is_empty() {
        return None;
    }
    let (feet, top) = (you - Vec3::Y * BODY.0, you + Vec3::Y * BODY.1);
    let mut bitten = false;
    for h in &mut g.heads {
        h.age += dt;
        match h.state {
            State::Fly => {
                let speed = SPEED.0 + (SPEED.1 - SPEED.0) * (h.age / RAMP).min(1.0);
                let now = h.vel.normalize_or(Vec3::Z);
                let want = (chest - h.at).normalize_or(now);
                let axis = now.cross(want).normalize_or_zero();
                let turned = if axis == Vec3::ZERO { now } else { Quat::from_axis_angle(axis, now.angle_between(want).min(TURN * dt)) * now };
                h.vel = turned * speed;
                let before = h.at;
                h.at += h.vel * dt;
                h.gone += speed * dt;
                if crate::combat::bishop::between(before, h.at, feet, top) < LATCH {
                    h.state = State::Held;
                    bitten = true;
                } else if h.gone > RANGE {
                    h.state = State::Back;
                }
            }
            State::Held => h.at = chest,
            State::Back => {
                // (Aloft, straight back to its arm, which goes with it.)
                if !landed {
                    let elbow = pos + rot * super::arms::elbow(h.cable, scale, Vec2::ZERO);
                    h.trail = vec![(elbow, 0.0)];
                }
                let mut left = BACK * dt;
                while left > 0.0 {
                    let Some(&(to, sag)) = h.trail.last() else { break };
                    let to = to - Vec3::Y * sag;
                    let d = h.at.distance(to);
                    if d > left {
                        h.at += (to - h.at) / d * left;
                        break;
                    }
                    h.at = to;
                    left -= d;
                    h.trail.pop();
                }
                if h.trail.is_empty() {
                    h.state = State::Home;
                }
            }
            State::Home => {}
        }
        // (Its way laid as its head goes, and sagging under its weight.)
        if h.state != State::Back && h.trail.last().is_none_or(|(p, _)| p.distance(h.at) > STEP) {
            h.trail.push((h.at, 0.0));
        }
        for (_, sag) in h.trail.iter_mut().skip(1) {
            *sag = (*sag + DROP.0 * dt).min(DROP.1);
        }
    }
    let holding = g.heads.iter().any(|h| h.state == State::Held);
    let mut grip = None;
    if holding {
        g.held += dt;
        let at = *g.health_at.get_or_insert(health);
        if bitten {
            g.reach = you.distance(pos);
        }
        // (Let go: held long enough, hurt enough, or you got far away.)
        if g.held > HOLD_FOR || at - health >= RELEASE || you.distance(pos) > RANGE * 1.3 {
            for h in &mut g.heads {
                if h.state == State::Held {
                    h.state = State::Back;
                }
            }
            g.health_at = None;
            g.held = 0.0;
        } else {
            grip = Some(Grip { harm: GNAW * dt + if bitten { BITE } else { 0.0 }, bitten, from: pos, reach: g.reach });
        }
    }
    // (Those come back, gone; all of them, at rest, unless winding up.)
    g.heads.retain(|h| h.state != State::Home);
    if g.heads.is_empty() && g.wind.is_none() {
        g.phase = Phase::Rest;
    }
    grip
}

/// You held, as `latch` has it (1 hurt only, 2 slowed, 3 held where you
/// were, 4 reeled in; the user, 2026-11-10: which is a menace and not
/// frustrating, to try): your velocity (after you moved).
pub(super) fn held(latch: u8, grip: &Grip, you: Vec3, velocity: &mut Vec3) {
    let away = Vec3::new(you.x - grip.from.x, 0.0, you.z - grip.from.z);
    let dir = away.normalize_or(Vec3::X);
    let outward = velocity.dot(dir);
    match latch {
        2 => {
            let flat = Vec2::new(velocity.x, velocity.z).clamp_length_max(SLOW);
            *velocity = Vec3::new(flat.x, velocity.y, flat.y);
        }
        3 => {
            if outward > 0.0 {
                *velocity -= dir * outward;
            }
            let over = away.length() - grip.reach;
            if over > 0.0 {
                *velocity -= dir * (over * 4.0);
            }
        }
        4 => {
            if outward > -REEL {
                *velocity -= dir * (outward + REEL);
            }
        }
        _ => {}
    }
}

/// Its body's pose now (x: leaning forward about its waist band, rad; y:
/// its shoulders forward, mesh units; back, less than none): drawn back
/// winding up, rolled forward as it flings its cables and while they are
/// out, settling as they come back.
pub(super) fn pose(g: &Grasp) -> Vec2 {
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    if let Some(w) = g.wind {
        return ARCH * ease((w / WIND).min(1.0));
    }
    if g.heads.is_empty() {
        return Vec2::ZERO;
    }
    let thrown = ARCH.lerp(THROW, ease((g.flung / FLING).min(1.0)));
    if g.heads.iter().any(|h| matches!(h.state, State::Fly | State::Held)) { thrown } else { thrown * 0.4 }
}

/// Its grasp put away (it took off, or died).
pub(super) fn stow(b: &mut Impaled) {
    b.grasp = Grasp::default();
}

/// The crash's ring sweeping out over the ground, and fading.
#[derive(Component)]
pub(in crate::combat) struct Ring {
    age: f32,
}

/// The crash strikes the ground at `at`: its ring; and you, standing within
/// it on the ground, hurt by how near, thrown; Some(how much) if so.
pub(super) fn quake(commands: &mut Commands, ring: Handle<Mesh>, materials: &mut Assets<StandardMaterial>, at: Vec3, you: Vec3, grounded: bool) -> Option<(f32, Vec3)> {
    let look = materials.add(StandardMaterial { base_color: Color::srgba(0.85, 0.88, 0.95, 0.8), unlit: true, alpha_mode: AlphaMode::Add, depth_bias: 2.0, ..default() });
    commands.spawn((Ring { age: 0.0 }, Mesh3d(ring), MeshMaterial3d(look), bevy::light::NotShadowCaster, Transform::from_translation(at + Vec3::Y * 0.04).with_scale(Vec3::splat(0.5))));
    let off = Vec2::new(you.x - at.x, you.z - at.z);
    let near = 1.0 - off.length() / QUAKE;
    if !grounded || near <= 0.0 {
        return None;
    }
    let out = Vec3::new(off.x, 0.0, off.y).normalize_or(Vec3::X);
    Some((QUAKE_HARM * near.min(1.0), out * (QUAKE_THROW.0 * near) + Vec3::Y * QUAKE_THROW.1))
}

/// Rings swept out and faded.
pub(in crate::combat) fn rings(mut commands: Commands, time: Res<Time>, mut rings: Query<(Entity, &mut Ring, &mut Transform, &MeshMaterial3d<StandardMaterial>)>, mut materials: ResMut<Assets<StandardMaterial>>) {
    for (entity, mut r, mut t, m) in &mut rings {
        r.age += time.delta_secs();
        let k = r.age / RING_TIME;
        if k >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // (Its mesh one metre across: out to the crash's edge.)
        let wide = 0.5 + (2.0 * QUAKE - 0.5) * (1.0 - (1.0 - k) * (1.0 - k));
        t.scale = Vec3::new(wide, 1.0, wide);
        if let Some(mut m) = materials.get_mut(&m.0) {
            m.base_color = Color::srgba(0.85, 0.88, 0.95, 0.8 * (1.0 - k));
        }
    }
}

