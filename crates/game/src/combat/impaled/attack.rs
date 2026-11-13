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
//! Its grasp: flying or landed, you within its (long) range and in its
//! sight, each arm on its own, again and again (rested a while between,
//! not to be foreseen): its shoulder draws back (the tell), the line
//! between them bowing open; it flings a cable, rolling its shoulder
//! forward, and it comes at you slowly through the air, turning after you
//! (`cables=2`, harpoons: fast, aimed where you will be, turning only a
//! little), its way behind it sagging under its weight, fed from its waist
//! band, dragged along by its body as it flies, the nearer its body the
//! more (a cable flung on the move carries its speed); one that
//! touches you takes hold, hurts you as it holds and reels you in (as
//! `latch=N` has it); hurt it enough, or hold out, and it lets go. Let go, or past its range, a cable is dropped, not drawn back:
//! it falls to the ground, lies there a while and thins away.
//!
//! (The user, 2026-11-11: its slow cables, landed only, were trivially
//! escaped, slower than you run, and it no threat till landed; harpoons
//! were tried (`cables=2`) and had no personality; so its own slow ones,
//! its flight made far more agile, and flung flying too, reaching far.)
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
/// short fast attack it cannot use, so long, slow, homing, holding you;
/// 2026-11-11: relentless, its arms each on their own, many cables out, none
/// coming back): how far it reaches (m); how long an arm winds up (s), and
/// rests after flinging (s, from-to: not to be foreseen); how long after it
/// lands its first arm starts (s); how fast its heads are flung and how
/// fast they home (m/s: you run at 10), and how long to get to speed (s);
/// how near one must come to take hold (m); how much it hurts taking
/// hold, and holding (a second); how long it holds at most (s), and how
/// much hurting it makes it let go; how far apart its cables' way is kept
/// (m), and how fast and how far it sags under its weight (m/s, m). How
/// they fly: see `Throw`.
pub(super) const RANGE: f32 = 60.0;
const WIND: f32 = 0.5;
const REST: (f32, f32) = (0.6, 1.4);
const FIRST: f32 = 0.4;
const LATCH: f32 = 0.6;
const BITE: f32 = 8.0;
const GNAW: f32 = 6.0;
const HOLD_FOR: f32 = 4.0;
const RELEASE: f32 = 20.0;
const STEP: f32 = 0.3;
const DROP: (f32, f32) = (0.5, 1.2);
/// Its cables flung on the move: its head carries its body's velocity as
/// it was flung, losing it at this (1/s); the cable behind dragged by its
/// body as it flies, by all its velocity at its elbow, falling off along
/// the cable over about this (m); how many of a
/// cable's points have the ground under them looked at again each frame
/// as they are dragged (to sag down to it, not through it).
const CARRY: f32 = 0.1;
const DRAG: f32 = 10.0;
const ROOM_EACH: usize = 16;
/// Held (`latch=N`): slowed to this (m/s); reeled in at this (m/s), till
/// this near it (m).
const SLOW: f32 = 4.0;
const REEL: (f32, f32) = (4.0, 5.0);

/// How its cables fly: how fast flung and how fast at most (m/s; you run at
/// 10), how long to get there (s); how fast they turn after you (rad/s);
/// how much they lead you (aimed where you will be as they come: 1, all
/// the way); how far up and out its own side they are flung first (of
/// their way at you).
#[derive(Clone, Copy)]
pub(super) struct Throw {
    speed: (f32, f32),
    ramp: f32,
    turn: f32,
    lead: f32,
    out: f32,
}

impl Throw {
    /// Harpoons (`cables=2`; the user, 2026-11-11, tried and found
    /// without personality): faster than you, aimed ahead of you, turning
    /// little.
    pub const HARPOON: Throw = Throw { speed: (32.0, 32.0), ramp: 0.0, turn: 0.2, lead: 1.0, out: 0.0 };
    /// Its own (`cables=1`): slower than you, homing; flung on the move,
    /// faster by its own speed.
    pub const SLOW: Throw = Throw { speed: (3.0, 7.0), ramp: 0.8, turn: 1.4, lead: 0.0, out: 1.0 };

    /// How fast a head goes `age` s after it was flung.
    fn speed(&self, age: f32) -> f32 {
        let k = if self.ramp > 0.0 { (age / self.ramp).min(1.0) } else { 1.0 };
        self.speed.0 + (self.speed.1 - self.speed.0) * k
    }

    /// Where you (your chest, moving at `vel`) will be as a head from
    /// `from` going at `speed` comes to you.
    fn ahead(&self, from: Vec3, chest: Vec3, vel: Vec3, speed: f32) -> Vec3 {
        let mut at = chest;
        for _ in 0..2 {
            at = chest + vel * (from.distance(at) / speed * self.lead);
        }
        at
    }

    /// A head at `at` going along `vel`, `age` s after it was flung, turned
    /// after you as it can for `dt` s: its new velocity.
    fn steer(&self, at: Vec3, vel: Vec3, age: f32, chest: Vec3, your_vel: Vec3, dt: f32) -> Vec3 {
        let speed = self.speed(age);
        let now = vel.normalize_or(Vec3::Z);
        let want = (self.ahead(at, chest, your_vel, speed) - at).normalize_or(now);
        let axis = now.cross(want).normalize_or_zero();
        let turned = if axis == Vec3::ZERO { now } else { Quat::from_axis_angle(axis, now.angle_between(want).min(self.turn * dt)) * now };
        turned * speed
    }
}

impl Default for Throw {
    fn default() -> Self {
        Throw::SLOW
    }
}
/// A cable let go of, or that missed (the user, 2026-11-11: not drawn back,
/// dropped): how long it lies on the ground (s), how long it takes to thin
/// away (s), and how many lie about each brute at most (the oldest go).
const LIE: f32 = 6.0;
const THIN: f32 = 1.0;
const LITTER: usize = 24;
/// Each shoulder in its grasp (see `arms::bent`; mesh units, forward):
/// winding up, drawn back, the line between them bowing open (the force
/// drawn from the shoulder's arch, not from leaning back, the user);
/// flinging, rolled forward as its cable flies (no lean either, the user);
/// how long it takes to roll it forward (s), how long it takes to settle
/// after (s), and how quickly a shoulder follows all that (1/s: no jumps,
/// as when one of its cables takes hold).
const ARCH: f32 = -0.10;
const THROW: f32 = 0.11;
const FLING: f32 = 0.3;
const SETTLE: f32 = 0.5;
const FOLLOW: f32 = 14.0;
/// You, struck: from below your eye to above it (m).
const BODY: (f32, f32) = (1.3, 0.15);

/// Its arms, left (+x) then right.
const SIDES: [f32; 2] = [1.0, -1.0];

/// Its grasp: its arms; its cables' heads out (after you, or holding you);
/// the cables it has dropped; how long it has held you, its health when it
/// took hold, and how far from it you were then (m); how many cables it has
/// flung (what it does next is drawn from that); whether it was flinging
/// them; how they fly.
#[derive(Default)]
pub(super) struct Grasp {
    arms: [Arm; 2],
    pub heads: Vec<Head>,
    pub litter: Vec<Litter>,
    held: f32,
    health_at: Option<f32>,
    reach: f32,
    thrown: u32,
    armed: bool,
    throw: Throw,
}

impl Grasp {
    pub fn new(throw: Throw) -> Self {
        Grasp { throw, ..default() }
    }
}

/// One arm in its grasp: how long it has been winding up, how long since
/// it flung a cable, how long before it may wind up again (s), and how far
/// forward its shoulder is now (mesh units).
#[derive(Default)]
struct Arm {
    wind: Option<f32>,
    flung: Option<f32>,
    wait: f32,
    shoulder: f32,
}

/// One of its cables flung out: which cable, its head (where, how fast), the
/// way it has come (the points its cable lies along, each with how far it
/// has sagged since, and how far it can before it lies on the ground, and
/// which point's ground is to be looked at next), whether it holds you, how
/// far it has gone and how long; and what it carries of its body's
/// velocity (m/s).
pub(super) struct Head {
    pub cable: usize,
    pub at: Vec3,
    vel: Vec3,
    carry: Vec3,
    pub trail: Vec<(Vec3, f32)>,
    room: Vec<f32>,
    next: usize,
    pub holds: bool,
    gone: f32,
    age: f32,
}

impl Head {
    /// How fast it goes (m/s, the world's): flung, its own and what it
    /// carries of its body's; holding you, none.
    pub fn velocity(&self) -> Vec3 {
        if self.holds { Vec3::ZERO } else { self.vel + self.carry }
    }
}

/// A cable it dropped: its points (m, the world's), which of them is its
/// head, how fast each falls (m/s), the ground under each (m), how thick it
/// is (m), how long since.
pub(super) struct Litter {
    pub points: Vec<Vec3>,
    pub tip: usize,
    fall: Vec<f32>,
    floor: Vec<f32>,
    radius: f32,
    age: f32,
}

impl Litter {
    /// How thick it is now (m): thinning away at the end.
    pub fn thick(&self) -> f32 {
        self.radius * (1.0 - ((self.age - LIE) / THIN).clamp(0.0, 1.0))
    }

    /// How thick it was (m).
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Whether it lies still: dropped a while ago, all of it on the ground,
    /// not thinning yet.
    pub fn still(&self) -> bool {
        self.age > 0.1 && self.age < LIE && self.points.iter().zip(&self.floor).all(|(p, f)| p.y <= *f)
    }
}

/// What its grasp does to you this frame: how much it hurts, whether it
/// just took hold, and where it holds you from (it).
pub(super) struct Grip {
    pub harm: f32,
    pub bitten: bool,
    pub from: Vec3,
    pub reach: f32,
}

/// What a brute's grasp does this frame. Alive, each arm on its own:
/// rested, you in range and sight, not holding you, it winds up and flings
/// its outermost cable not out, which flies at you (where you will be, as
/// its `Throw` leads you, moving at `your_vel`), turning after you as it
/// can, dragged by its body as it flies; one that touches you takes hold.
/// Past its range, or let go of (it held long enough, you hurt it enough,
/// you got far enough away), or it dies: dropped, falling to the ground
/// (`floor`, the ground under a point), lying, thinning away. Some(what it
/// does to you) while it holds you.
pub(super) fn grasp(b: &mut Impaled, you: Vec3, your_vel: Vec3, seen: bool, armed: bool, floor: &dyn Fn(Vec3) -> f32, dt: f32) -> Option<Grip> {
    let (pos, rot) = b.flight.drawn();
    let body = b.flight.vel;
    let scale = super::scale();
    let health = b.health;
    let log = b.log;
    let g = &mut b.grasp;
    let throw = g.throw;
    let chest = you - Vec3::Y * 0.5;
    // (Armed again: its arms start one after the other.)
    if armed && !g.armed {
        g.arms[0].wait = FIRST;
        g.arms[1].wait = FIRST + (REST.0 + REST.1) * 0.25 + WIND * 0.5;
    }
    g.armed = armed;
    let mut drop = vec![!armed; g.heads.len()];
    let in_range = seen && you.distance(pos) < RANGE * 0.9;
    for (i, side) in SIDES.into_iter().enumerate() {
        let holding = g.heads.iter().any(|h| h.holds && super::arms::cables()[h.cable].side == side);
        let out: Vec<usize> = g.heads.iter().map(|h| h.cable).collect();
        let arm = &mut g.arms[i];
        arm.wait -= dt;
        if let Some(f) = &mut arm.flung {
            *f += dt;
        }
        if !armed {
            arm.wind = None;
        } else if arm.wind.is_none() && arm.wait <= 0.0 && in_range && !holding && super::arms::lasher(side, &out).is_some() {
            arm.wind = Some(0.0);
        }
        if let Some(w) = &mut arm.wind {
            *w += dt;
            if *w > WIND {
                arm.wind = None;
                arm.flung = Some(0.0);
                g.thrown += 1;
                let k = worldgen::noise::hash01(g.thrown as i32, i as i32, 0, 0x6a5);
                arm.wait = REST.0 + (REST.1 - REST.0) * k;
                if let Some(cable) = super::arms::lasher(side, &out) {
                    let start = pos + rot * super::arms::elbow(cable, scale, shoulders(side, ARCH));
                    // (At where you will be; the slow ones up and out its
                    // own side first, then turning to you.)
                    let at = throw.ahead(start, chest, your_vel, throw.speed.1);
                    let fling = ((at - start).normalize_or(rot * Vec3::Z) + (Vec3::Y * 0.6 + rot * Vec3::X * (side * 0.5)) * throw.out).normalize();
                    g.heads.push(Head { cable, at: start, vel: fling * throw.speed.0, carry: body, trail: vec![(start, 0.0)], room: vec![0.0], next: 0, holds: false, gone: 0.0, age: 0.0 });
                    drop.push(false);
                }
            }
        }
        // (Its shoulder: drawn back winding up, rolled forward flinging,
        // held forward holding you, else settling; following smoothly.)
        let ease = |t: f32| t * t * (3.0 - 2.0 * t);
        let want = match (arm.wind, arm.flung) {
            _ if !armed => 0.0,
            (Some(w), _) => ARCH * ease((w / WIND).min(1.0)),
            (None, Some(f)) if f < FLING => ARCH + (THROW - ARCH) * ease(f / FLING),
            _ if holding => THROW,
            (None, Some(f)) => THROW * (-(f - FLING) / SETTLE).exp(),
            (None, None) => 0.0,
        };
        arm.shoulder += (want - arm.shoulder) * (1.0 - (-FOLLOW * dt).exp());
    }
    let (feet, top) = (you - Vec3::Y * BODY.0, you + Vec3::Y * BODY.1);
    let mut bitten = false;
    for (h, drop) in g.heads.iter_mut().zip(&mut drop) {
        h.age += dt;
        if h.holds {
            h.at = chest;
        } else {
            let speed = throw.speed(h.age);
            h.vel = throw.steer(h.at, h.vel, h.age, chest, your_vel, dt);
            let before = h.at;
            h.at += (h.vel + h.carry) * dt;
            h.carry *= (-CARRY * dt).exp();
            h.gone += speed * dt;
            if *drop {
            } else if crate::combat::bishop::between(before, h.at, feet, top) < LATCH {
                h.holds = true;
                bitten = true;
            } else if h.gone > RANGE * 1.2 {
                *drop = true;
            }
        }
        // (Dragged by its body as it flies, the nearer it along the cable
        // the more; the ground under the dragged points looked at again, a
        // few a frame.)
        if body.length_squared() > 0.01 {
            let mut along = 0.0;
            let mut last = h.trail.first().map_or(h.at, |(p, _)| *p);
            for (p, _) in &mut h.trail {
                along += p.distance(last);
                last = *p;
                *p += body * ((-along / DRAG).exp() * dt);
            }
            let radius = super::arms::cables()[h.cable].radius * scale;
            for _ in 0..ROOM_EACH.min(h.room.len()) {
                let k = h.next % h.room.len();
                let p = h.trail[k].0;
                h.room[k] = (p.y - floor(p) - radius).max(0.0);
                h.next += 1;
            }
        }
        // (Its way laid as its head goes, and sagging under its weight, as
        // far as the ground.)
        if h.trail.last().is_none_or(|(p, _)| p.distance(h.at) > STEP) {
            h.trail.push((h.at, 0.0));
            h.room.push((h.at.y - floor(h.at) - super::arms::cables()[h.cable].radius * scale).max(0.0));
        }
        for ((_, sag), room) in h.trail.iter_mut().zip(&h.room).skip(1) {
            *sag = (*sag + DROP.0 * dt).min(DROP.1).min(*room);
        }
    }
    let mut grip = None;
    if g.heads.iter().zip(&drop).any(|(h, d)| h.holds && !d) {
        // (Its hold counted from the first cable to take hold.)
        if g.health_at.is_none() {
            g.health_at = Some(health);
            g.reach = you.distance(pos);
        }
        g.held += dt;
        let at = g.health_at.unwrap_or(health);
        // (Let go: held long enough, hurt enough, or you got far away.)
        if g.held > HOLD_FOR || at - health >= RELEASE || you.distance(pos) > RANGE * 1.3 {
            for (h, d) in g.heads.iter().zip(&mut drop) {
                *d |= h.holds;
            }
        } else {
            grip = Some(Grip { harm: GNAW * dt + if bitten { BITE } else { 0.0 }, bitten, from: pos, reach: g.reach });
        }
    }
    if !g.heads.iter().zip(&drop).any(|(h, d)| h.holds && !d) {
        g.held = 0.0;
        g.health_at = None;
    }
    // (Those dropped fall where they were, its own no more.)
    let mut kept = Vec::new();
    for (h, d) in std::mem::take(&mut g.heads).into_iter().zip(drop) {
        if d {
            let (points, tip) = super::arms::dropped(&h, rot);
            let radius = super::arms::cables()[h.cable].radius * scale;
            let under: Vec<f32> = points.iter().map(|p| floor(*p) + radius).collect();
            // (None of it in the ground: its head may have gone under.)
            let points: Vec<Vec3> = points.iter().zip(&under).map(|(p, f)| p.with_y(p.y.max(*f))).collect();
            if log {
                let low = points.iter().zip(&under).map(|(p, f)| p.y - f).fold(f32::MAX, f32::min);
                info!("impaled: dropped a cable, {} points, {:.1} m to {:.1} m over the ground", points.len(), low, points.iter().zip(&under).map(|(p, f)| p.y - f).fold(f32::MIN, f32::max));
            }
            g.litter.push(Litter { fall: vec![0.0; points.len()], points, tip, floor: under, radius, age: 0.0 });
        } else {
            kept.push(h);
        }
    }
    g.heads = kept;
    for l in &mut g.litter {
        l.age += dt;
        for ((p, v), f) in l.points.iter_mut().zip(&mut l.fall).zip(&l.floor) {
            if p.y > *f {
                *v -= crate::flight::G * dt;
                p.y = (p.y + *v * dt).max(*f);
            }
        }
    }
    g.litter.retain(|l| l.age < LIE + THIN);
    let over = g.litter.len().saturating_sub(LITTER);
    g.litter.drain(..over);
    grip
}

/// Both shoulders' pose with the one on `side` at `forward`, the other at
/// rest.
fn shoulders(side: f32, forward: f32) -> Vec2 {
    if side > 0.0 { Vec2::new(forward, 0.0) } else { Vec2::new(0.0, forward) }
}

/// You held, as `latch` has it (1 hurt only, 2 slowed, 3 held where you
/// were, 4 reeled in, as far as near it; the user, 2026-11-10: which is a
/// menace and not frustrating, to try; 2026-11-11, reeled in, so that
/// getting away is not free): your velocity (after you moved).
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
            if away.length() > REEL.1 && outward > -REEL.0 {
                *velocity -= dir * (outward + REEL.0);
            }
        }
        _ => {}
    }
}

/// Its shoulders now (mesh units forward, its left then its right; back,
/// less than none; see `arms::bent`).
pub(super) fn pose(g: &Grasp) -> Vec2 {
    Vec2::new(g.arms[0].shoulder, g.arms[1].shoulder)
}

/// Whether its arms move (to be drawn again): winding up, cables out, a
/// shoulder not yet at rest.
pub(super) fn moving(g: &Grasp) -> bool {
    !g.heads.is_empty() || g.arms.iter().any(|a| a.wind.is_some() || a.shoulder.abs() > 1e-4)
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


#[cfg(test)]
mod tests {
    use super::*;

    /// You, moving at `from`, wanting `to` from `when` s (before the
    /// release, less than none: in its wind-up), your speed changing as
    /// running does (m/s²).
    struct You {
        from: Vec3,
        to: Vec3,
        when: f32,
    }
    const CHANGE: f32 = 15.0;

    /// A head flung from 20 m off at you, the way the game flies it:
    /// whether it took hold, and how near it came (m).
    fn shot(throw: Throw, y: You) -> (bool, f32) {
        let dt = 1.0 / 120.0;
        let (mut you, mut v) = (Vec3::new(0.0, 1.7, 0.0), y.from);
        let mut t = y.when.min(0.0);
        let step = |t: f32, you: &mut Vec3, v: &mut Vec3| {
            let want = if t >= y.when { y.to } else { y.from };
            *v += (want - *v).clamp_length_max(CHANGE * dt);
            *you += *v * dt;
        };
        while t < 0.0 {
            step(t, &mut you, &mut v);
            t += dt;
        }
        let start = Vec3::new(0.0, 2.0, 20.0);
        let chest = |you: Vec3| you - Vec3::Y * 0.5;
        let at = throw.ahead(start, chest(you), v, throw.speed.1);
        let fling = ((at - start).normalize() + (Vec3::Y * 0.6 + Vec3::X * 0.5) * throw.out).normalize();
        let (mut h, mut hv, mut age, mut gone) = (start, fling * throw.speed.0, 0.0, 0.0);
        let mut nearest = f32::MAX;
        while gone < RANGE * 1.2 {
            step(age, &mut you, &mut v);
            age += dt;
            hv = throw.steer(h, hv, age, chest(you), v, dt);
            let before = h;
            h += hv * dt;
            gone += hv.length() * dt;
            let d = crate::combat::bishop::between(before, h, you - Vec3::Y * BODY.0, you + Vec3::Y * BODY.1);
            nearest = nearest.min(d);
            if d < LATCH {
                return (true, d);
            }
        }
        (false, nearest)
    }

    /// The user, 2026-11-11: its cables were slower than you, and running
    /// away beat them. A harpoon hits you going on as you were when it was
    /// flung, standing, running across or away (it leads you), or stepping
    /// aside in its wind-up (you are going on as you were by then), or too
    /// late; it misses you changing course as it is flung (stepping aside,
    /// turning back, stopping); the slow ones never caught you running.
    #[test]
    fn harpoons_hit_you_running_on_miss_you_stepping_aside() {
        let h = Throw::HARPOON;
        let run = 10.24;
        let (across, away, still) = (Vec3::X * run, -Vec3::Z * run, Vec3::ZERO);
        let on = |v: Vec3| You { from: v, to: v, when: 0.0 };
        for (what, y, hits) in [
            ("standing", on(still), true),
            ("running across", on(across), true),
            ("running away", on(away), true),
            ("stepping aside in its wind-up", You { from: still, to: across, when: -WIND }, true),
            ("stepping aside as it is flung", You { from: still, to: across, when: 0.0 }, false),
            ("turning back as it is flung", You { from: across, to: -across, when: 0.0 }, false),
            ("stopping as it is flung", You { from: across, to: still, when: 0.0 }, false),
            ("stepping aside 0.1 s late", You { from: still, to: across, when: 0.1 }, false),
            ("stepping aside 0.3 s late", You { from: still, to: across, when: 0.3 }, true),
        ] {
            let (hit, near) = shot(h, y);
            eprintln!("{what}: {} ({near:.2} m)", if hit { "hit" } else { "missed" });
            assert_eq!(hit, hits, "{what}: came within {near:.2} m");
        }
        let s = Throw::SLOW;
        assert!(!shot(s, on(away)).0, "the slow ones caught you running away");
        assert!(!shot(s, on(across)).0, "the slow ones caught you running across");
    }
}
