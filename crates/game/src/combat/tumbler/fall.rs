//! A tumbler killed (the user, 2026-11-13: physical by default, its simple
//! geometry suits a collapse): its three pieces (its two blades and its
//! core) falling under their weight, each rigid, onto the ground and lying
//! there. `death=1`: its hinge gone limp, the X falling as one, its blades
//! swinging free about where they cross; `death=2`: it comes apart, each
//! piece falling on its own; (`death=3`, bursting as the others do, is not
//! here: see `tumbler::tumble`). Coming apart is the default (the user,
//! 2026-11-13: it reads better).
//!
//! Each piece is a few points (along its blade, about its core), moved
//! freely (Verlet, in fixed steps) and then put back into its shape: the
//! rigid turn and place that best fits them (shape matching), so it keeps
//! its momentum and its spin. Limp, its core goes with its first blade (the
//! blades pass through it; a piece of its own on the hinge, it swung there
//! for ever, never still), and its hinge is two points on its axis
//! (through where they cross, square to its plane) shared by both blades,
//! so they turn only about it. On the ground, held by friction. Each moves
//! on as it was moving, thrown by the shards that killed it: along them,
//! spun about where they struck.

use super::{MESH_BODY, MESH_BODY_SIZE, MESH_CENTRE, MESH_HEAD, MESH_TIPS};
use bevy::prelude::*;

/// Its fixed step (s), and how many passes over its shapes in each.
const STEP: f32 = 1.0 / 120.0;
const PASSES: usize = 8;
/// Its gravity (m/s²: more than earth's, to read heavy at its size), how
/// much of its speed it keeps each step, its friction on the ground (as
/// Coulomb's: a point slides once pushed along the ground more than this
/// times as hard as it is pressed into it; holding most of each slide
/// every step, it stood on its two tips like a stepladder, dead).
const GRAVITY: f32 = 14.0;
const DRAG: f32 = 0.998;
const FRICTION: f32 = 0.4;
/// The shards that killed it throwing it (the user, 2026-11-13: pushed
/// only near where they struck, it fell where it stood, not taking the
/// shot; then, thrown at most 9 m/s, too little: point blank it should
/// hurl its pieces back): how fast each throws it, whole, along the shot
/// (m/s, `shove=` times), how fast at most (m/s), how much of that up
/// besides (off its feet, through the air), how much of each shard's push
/// its struck piece takes alone (flung harder), and how fast it spins at
/// most (rad/s).
const THROW: f32 = 1.3;
const THROW_MOST: f32 = 16.0;
const THROW_LIFT: f32 = 0.35;
const THROW_STRUCK: f32 = 0.4;
const THROW_SPIN: f32 = 12.0;
/// Coming apart: how fast its pieces part (m/s).
const PARTING: f32 = 1.2;
/// Its hinge's two points, either side of its plane (its units: within its
/// blades, and never on the ground: further out, they were feet, and it
/// stood dead on its edge on them).
const HINGE: f32 = 0.04;
/// It lies still (asleep, as physics engines put their bodies) once none
/// of its pieces has moved more than so far (m) and turned more than so
/// much (rad) in so long (s), twice running (its pieces as wholes, its
/// points jostled a millimetre each step by its hinge, unseen; and so
/// slowly, too slow to read as moving: propped near balance, it crept a
/// degree a second, held back by its friction); and it stops, still or
/// not, after so long (s).
const STILL: (f32, f32, f32) = (0.01, 0.0175, 0.25);
const LONGEST: f32 = 10.0;

/// How it dies (`death=`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Death {
    Limp,
    Apart,
    Burst,
}

impl Death {
    pub(super) fn from(n: u8) -> Death {
        match n {
            1 => Death::Limp,
            3 => Death::Burst,
            _ => Death::Apart,
        }
    }
}

/// A piece: its points (`first..first + rest.len()`), each where it is from
/// the piece's middle in its first pose; that middle; its turn and its
/// middle now.
struct Piece {
    first: usize,
    rest: Vec<Vec3>,
    middle: Vec3,
    turn: Quat,
    at: Vec3,
}

/// Its fall: where it fell (its points kept from there: see
/// `ragdoll::Ragdoll`'s origin), its points now and a step before, how far
/// each stands off the ground; its pieces; its hinge's points (shared, each
/// one point of each blade); each part (its two blades, its body, its head)
/// with its piece and where it was in its first pose (from `origin`).
pub(super) struct Fall {
    origin: Vec3,
    p: Vec<Vec3>,
    q: Vec<Vec3>,
    radius: Vec<f32>,
    pieces: Vec<Piece>,
    pins: Vec<[usize; 2]>,
    parts: [(usize, Transform); 4],
    clock: f32,
    /// Its pieces where they were when it last looked to see if it was
    /// still, and how many times running it was.
    still: (Vec<(Vec3, Quat)>, u32),
    age: f32,
    asleep: bool,
}

/// Its blade `k`'s points (its units, from where they cross, in its part's
/// frame) and how thick it is at each (half, through its plane).
/// (And either side of its line, in its plane, where it is widest: wider
/// across than through its plane, on its edge it tips over onto its face;
/// a line of points alone, it rolled on its edge, all but balanced, crept
/// along by its core's weight, never still.)
fn blade(k: usize) -> Vec<(Vec3, f32)> {
    let (a, b) = (MESH_TIPS[k] - MESH_CENTRE, MESH_TIPS[k + 2] - MESH_CENTRE);
    let across = a.normalize().perp() * 0.06;
    let mut points: Vec<(Vec3, f32)> = [1.0f32, 2.0 / 3.0, 1.0 / 3.0].iter().flat_map(|&f| [(a * f).extend(0.0), (b * f).extend(0.0)].map(|p| (p, 0.03 + 0.07 * (1.0 - f)))).collect();
    for end in [a, b] {
        for side in [-1.0, 1.0] {
            points.push(((end * 0.4 + across * side).extend(0.0), 0.08));
        }
    }
    points
}

/// Its core's points (its units, in its body's frame), its body `tall` times
/// as tall (see `MESH_BODY`).
fn core(tall: f32) -> Vec<(Vec3, f32)> {
    let (low, top) = MESH_BODY;
    let high = low + (top - low) * tall;
    let head = MESH_HEAD.0 + (top - low) * (tall - 1.0);
    let middle = (low + high) / 2.0;
    let (w, d) = (MESH_BODY_SIZE.0 / 2.0, MESH_BODY_SIZE.1 / 2.0);
    [Vec3::Y * low, Vec3::Y * head, Vec3::new(w, middle, 0.0), Vec3::new(-w, middle, 0.0), Vec3::new(0.0, middle, d), Vec3::new(0.0, middle, -d)].map(|p| (p, 0.04)).to_vec()
}

/// The rigid turn nearest `a` (Σ (point − middle) ⊗ rest), from `q`
/// (Müller et al. 2016, "A robust method to extract the rotational part of
/// deformations").
fn nearest_turn(a: Mat3, mut q: Quat) -> Quat {
    for _ in 0..8 {
        let r = Mat3::from_quat(q);
        let w = (r.x_axis.cross(a.x_axis) + r.y_axis.cross(a.y_axis) + r.z_axis.cross(a.z_axis)) / ((r.x_axis.dot(a.x_axis) + r.y_axis.dot(a.y_axis) + r.z_axis.dot(a.z_axis)).abs() + 1e-9);
        let angle = w.length();
        if angle < 1e-7 {
            break;
        }
        q = (Quat::from_axis_angle(w / angle, angle) * q).normalize();
    }
    q
}

/// How fast the shards that killed it (`hits`) throw it, whole (m/s, before
/// its lift), `shove` times as hard.
pub(super) fn thrown(hits: &[(usize, Vec3, Vec3)], shove: f32) -> Vec3 {
    (hits.iter().map(|h| h.2).sum::<Vec3>() * THROW * shove).clamp_length_max(THROW_MOST)
}

/// The speed each of points `of` of `p` (as many, alike; held as one rigid
/// body) takes from pushes (where, and how hard: a speed times their
/// number), its spin at most `THROW_SPIN`.
fn impulse(p: &[Vec3], of: &[usize], pushes: &[(Vec3, Vec3)]) -> Vec<Vec3> {
    let n = of.len() as f32;
    let middle = of.iter().map(|&i| p[i]).sum::<Vec3>() / n;
    let mut inertia = Mat3::ZERO;
    for &i in of {
        let r = p[i] - middle;
        inertia += Mat3::from_diagonal(Vec3::splat(r.length_squared())) - Mat3::from_cols(r * r.x, r * r.y, r * r.z);
    }
    let push = pushes.iter().map(|x| x.1).sum::<Vec3>();
    let torque = pushes.iter().map(|(at, x)| (*at - middle).cross(*x)).sum::<Vec3>();
    let spin = if inertia.determinant().abs() > 1e-9 { (inertia.inverse() * torque).clamp_length_max(THROW_SPIN) } else { Vec3::ZERO };
    of.iter().map(|&i| push / n + spin.cross(p[i] - middle)).collect()
}

impl Fall {
    /// Falling from its parts where they are (`now`: its two blades, its
    /// body, its head, in the world) and where they were `dt` s before
    /// (`then`); its body `tall` times as tall; dying `how` (limp or coming
    /// apart); thrown by the shards that killed it (`hits`: which part, where,
    /// which way), `shove` times as hard.
    pub(super) fn new(now: [Transform; 4], then: [Transform; 4], dt: f32, tall: f32, how: Death, hits: &[(usize, Vec3, Vec3)], shove: f32) -> Fall {
        let scale = now[0].scale.x;
        let origin = (now[0].translation + now[1].translation) / 2.0;
        let axis = |t: &[Transform; 4], side: f32| (t[0].translation + t[1].translation) / 2.0 + t[0].rotation * Vec3::Z * side * HINGE * scale;
        let (mut p, mut q, mut radius) = (Vec::new(), Vec::new(), Vec::new());
        // (Its pieces, each of its parts: limp, its blades, its core with
        // its first; apart, its blades and its core.)
        let limp = how != Death::Apart;
        let piece_of_part = |k: usize| if k < 2 { k } else if limp { 0 } else { 2 };
        let mut pins = [[0; 2]; 2];
        let pieces: Vec<Piece> = (0..if limp { 2 } else { 3 })
            .map(|b| {
                let first = p.len();
                for k in (0..3).filter(|&k| piece_of_part(k) == b) {
                    // (Its blades' points in one plane, through where they
                    // cross, not set apart through it as they are drawn:
                    // lying, its upper blade hung on its hinge and turned it
                    // on the ground, never still.)
                    let plane = |t: &[Transform; 4]| Transform { translation: (t[0].translation + t[1].translation) / 2.0, ..t[k] };
                    let (here, there) = if k < 2 { (plane(&now), plane(&then)) } else { (now[k], then[k]) };
                    for (local, r) in if k < 2 { blade(k) } else { core(tall) } {
                        p.push(here.transform_point(local) - origin);
                        q.push(there.transform_point(local) - origin);
                        radius.push(r * scale);
                    }
                }
                if limp {
                    for (s, side) in [-1.0, 1.0].into_iter().enumerate() {
                        pins[s][b] = p.len();
                        p.push(axis(&now, side) - origin);
                        q.push(axis(&then, side) - origin);
                        radius.push(0.0);
                    }
                }
                let n = p.len() - first;
                let middle = p[first..].iter().sum::<Vec3>() / n as f32;
                Piece { first, rest: p[first..].iter().map(|x| *x - middle).collect(), middle, turn: Quat::IDENTITY, at: middle }
            })
            .collect();
        // (Moving on as it was: a step back along its way.)
        for (x, was) in p.iter().zip(q.iter_mut()) {
            let v = if dt > 1e-4 { ((*x - *was) / dt).clamp_length_max(40.0) } else { Vec3::ZERO };
            *was = *x - v * STEP;
        }
        let piece_of = |i: usize| pieces.iter().position(|c| i >= c.first && i < c.first + c.rest.len()).expect("in a piece");
        // Thrown by the shards that killed it: whole (it was one as they
        // struck), along them and spun about where they struck it, and
        // each piece struck flung on its own besides; a little up, off its
        // feet.
        let total = hits.iter().map(|h| h.2).sum::<Vec3>() * THROW * shove;
        let most = if total.length() > THROW_MOST { THROW_MOST / total.length() } else { 1.0 };
        let all: Vec<usize> = (0..p.len()).collect();
        let pushes = |of: &[usize], share: f32, piece: Option<usize>| -> Vec<(Vec3, Vec3)> {
            hits.iter().filter(|h| piece.is_none_or(|b| piece_of_part(h.0) == b)).map(|&(_, at, way)| (at - origin, way * THROW * shove * most * share * of.len() as f32)).collect()
        };
        let mut thrown = impulse(&p, &all, &pushes(&all, 1.0 - THROW_STRUCK, None));
        for (b, c) in pieces.iter().enumerate() {
            let of: Vec<usize> = (c.first..c.first + c.rest.len()).collect();
            for (&i, v) in of.iter().zip(impulse(&p, &of, &pushes(&of, THROW_STRUCK, Some(b)))) {
                thrown[i] += v;
            }
        }
        let lift = Vec3::Y * total.with_y(0.0).length() * most * THROW_LIFT;
        for (was, v) in q.iter_mut().zip(&thrown) {
            *was -= (*v + lift) * STEP;
        }
        // Coming apart: its blades parting through its plane, its core up,
        // tipping (on its point, untouched, it stood balanced).
        let pins = if !limp {
            let out = now[0].rotation * Vec3::Z;
            let tip = now[0].rotation * Vec3::new(0.6, 0.0, 0.8) * PARTING / (0.3 * scale);
            for i in 0..p.len() {
                q[i] -= match piece_of(i) {
                    0 => out * PARTING,
                    1 => -out * PARTING,
                    _ => Vec3::Y * PARTING + tip.cross(p[i] - pieces[2].middle),
                } * STEP;
            }
            Vec::new()
        } else {
            pins.to_vec()
        };
        let parts = std::array::from_fn(|k| {
            let mut t = now[k];
            t.translation -= origin;
            (piece_of_part(k), t)
        });
        Fall { origin, p, q, radius, pieces, pins, parts, clock: 0.0, still: (Vec::new(), 0), age: 0.0, asleep: false }
    }

    /// Each piece put back into its shape: its points where its nearest
    /// rigid turn and middle put them.
    fn shape(&mut self) {
        for c in &mut self.pieces {
            let points = &mut self.p[c.first..c.first + c.rest.len()];
            let at = points.iter().sum::<Vec3>() / points.len() as f32;
            let mut a = Mat3::ZERO;
            for (x, r) in points.iter().zip(&c.rest) {
                let d = *x - at;
                a += Mat3::from_cols(d * r.x, d * r.y, d * r.z);
            }
            c.turn = nearest_turn(a, c.turn);
            c.at = at;
            for (x, r) in points.iter_mut().zip(&c.rest) {
                *x = at + c.turn * *r;
            }
        }
    }

    /// Each piece, whole, lifted out of the ground (`floor`: the least
    /// height of each point), if put back into its shape sank it (or, at
    /// first, it stood in it): moved, not flung, and its fall into the
    /// ground stopped (lifted alone, its lift was its speed upward and it
    /// rose into the air, in the game, 2026-11-13; lifted with where it was
    /// a step before, its fall went on and it crept, never still).
    fn lift(&mut self, floor: &[f32]) {
        for c in &mut self.pieces {
            let range = c.first..c.first + c.rest.len();
            let up = range.clone().map(|i| floor[i] - self.p[i].y).fold(0.0f32, f32::max);
            if up > 0.0 {
                for i in range {
                    self.p[i].y += up;
                    self.q[i].y = (self.q[i].y + up).max(self.p[i].y);
                }
                c.at.y += up;
            }
        }
    }

    /// The least height of each point (from `origin`): the ground under it
    /// and its thickness.
    fn floor(&self, ground: &dyn Fn(Vec3) -> f32) -> Vec<f32> {
        self.p.iter().zip(&self.radius).map(|(x, &r)| if r > 0.0 { ground(*x + self.origin) - self.origin.y + r } else { f32::MIN }).collect()
    }

    /// Fallen on `dt` s further, on the ground (`ground`: its height under a
    /// point).
    pub(super) fn step(&mut self, dt: f32, ground: &dyn Fn(Vec3) -> f32) {
        if self.asleep {
            return;
        }
        if self.age == 0.0 {
            let floor = self.floor(ground);
            self.lift(&floor);
        }
        self.clock += dt.min(0.1);
        while self.clock >= STEP {
            self.clock -= STEP;
            self.age += STEP;
            for (x, was) in self.p.iter_mut().zip(self.q.iter_mut()) {
                let v = (*x - *was) * DRAG;
                *was = *x;
                *x += v + Vec3::NEG_Y * GRAVITY * STEP * STEP;
            }
            // (The ground under each, once a step; how far into it each
            // went: how hard it is pressed, for its friction.)
            let floor = self.floor(ground);
            let pressed: Vec<f32> = self.p.iter().zip(&floor).map(|(x, h)| (h - x.y).max(0.0)).collect();
            for _ in 0..PASSES {
                self.shape();
                for pin in &self.pins {
                    let at = pin.iter().map(|&i| self.p[i]).sum::<Vec3>() / pin.len() as f32;
                    for &i in pin {
                        self.p[i] = at;
                    }
                }
                for (x, h) in self.p.iter_mut().zip(&floor) {
                    x.y = x.y.max(*h);
                }
            }
            self.shape();
            self.lift(&floor);
            // (On the ground, its slide held back by friction: all of it,
            // if slow enough.)
            for (((x, was), h), d) in self.p.iter_mut().zip(&self.q).zip(&floor).zip(&pressed) {
                if x.y <= h + 0.01 {
                    let slid = (*x - *was).with_y(0.0);
                    let held = FRICTION * d;
                    *x -= if slid.length() <= held { slid } else { slid.normalize() * held };
                }
            }
            let looked = (self.age / STILL.2).floor() != ((self.age - STEP) / STILL.2).floor();
            if looked {
                let now: Vec<(Vec3, Quat)> = self.pieces.iter().map(|c| (c.at, c.turn)).collect();
                let still = self.still.0.len() == now.len() && self.still.0.iter().zip(&now).all(|(a, b)| a.0.distance(b.0) < STILL.0 && a.1.angle_between(b.1) < STILL.1);
                self.still = (now, if still { self.still.1 + 1 } else { 0 });
            }
            if self.still.1 >= 2 || self.age > LONGEST {
                self.shape();
                self.lift(&floor);
                self.asleep = true;
                return;
            }
        }
    }

    /// Where each of its parts is now (in the world).
    pub(super) fn parts(&self) -> [Transform; 4] {
        self.parts.map(|(b, t)| {
            let c = &self.pieces[b];
            Transform { translation: self.origin + c.at + c.turn * (t.translation - c.middle), rotation: c.turn * t.rotation, scale: t.scale }
        })
    }

    pub(super) fn asleep(&self) -> bool {
        self.asleep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Its parts standing upright, `scale` times its model, its crossing at
    /// `at`, its face along z.
    fn standing(at: Vec3, scale: f32) -> [Transform; 4] {
        let apart = 0.05 * scale;
        [
            Transform::from_translation(at + Vec3::Z * apart).with_scale(Vec3::splat(scale)),
            Transform::from_translation(at - Vec3::Z * apart).with_scale(Vec3::splat(scale)),
            Transform::from_translation(at).with_scale(Vec3::splat(scale)),
            Transform::from_translation(at).with_scale(Vec3::splat(scale)),
        ]
    }

    fn tips(f: &Fall) -> [Vec3; 4] {
        let t = f.parts();
        std::array::from_fn(|j| t[j % 2].transform_point((MESH_TIPS[j] - MESH_CENTRE).extend(0.0)))
    }

    /// Standing, then dead: it falls, lies on the ground (nothing in it),
    /// still within a few seconds, each blade rigid, its hinge holding
    /// (limp) or its pieces apart.
    #[test]
    fn falls_and_lies_still() {
        let scale = 4.0 / 1.901;
        let floor = |_: Vec3| 0.0;
        for how in [Death::Limp, Death::Apart] {
            let start = standing(Vec3::Y * 0.95 * scale, scale);
            // (Struck high on one blade, from the front.)
            let struck = start[0].transform_point(((MESH_TIPS[0] - MESH_CENTRE) * 0.7).extend(0.0));
            let mut f = Fall::new(start, start, 1.0 / 60.0, 1.4, how, &[(0, struck, Vec3::NEG_Z), (0, struck, Vec3::NEG_Z)], 1.0);
            let length = |t: &[Vec3; 4], k: usize| t[k].distance(t[k + 2]);
            let first = tips(&f);
            // (Lifted out of the ground it stood in first: its tips' thickness.)
            f.step(0.0, &floor);
            let top = f.p.iter().map(|x| x.y).fold(f32::MIN, f32::max);
            let mut highest = f32::MIN;
            let mut lowest = f32::MAX;
            let mut took = None;
            for frame in 0..60 * 10 {
                f.step(1.0 / 60.0, &floor);
                highest = highest.max(f.p.iter().map(|x| x.y).fold(f32::MIN, f32::max));
                lowest = lowest.min(f.p.iter().zip(&f.radius).map(|(x, r)| x.y + f.origin.y - r).fold(f32::MAX, f32::min));
                if f.asleep() && took.is_none() {
                    took = Some(frame as f32 / 60.0);
                }
            }
            let t = tips(&f);
            let high = t.iter().map(|p| p.y).fold(f32::MIN, f32::max);
            let gap = f.pieces[0].at.distance(f.pieces[1].at);
            println!("{how:?}: still after {took:?} s, its tips at most {high:.2} m up, deepest {lowest:.3}, blades' middles {gap:.2} m apart");
            assert!(took.is_some_and(|s| s < 8.0), "{how:?}: never still");
            println!("{how:?}: rose {:.3} m at most", highest - top);
            assert!(highest < top + 0.05, "{how:?}: rose {:.2} m over where it fell from", highest - top);
            assert!(lowest > -0.02, "{how:?}: {lowest:.3} m into the ground");
            assert!(high < 0.8, "{how:?}: not lying down: a tip {high:.2} m up");
            for k in 0..2 {
                assert!((length(&t, k) - length(&first, k)).abs() < 0.01, "{how:?}: blade {k} bent");
            }
            if how == Death::Limp {
                for pin in &f.pins {
                    assert!(f.p[pin[0]].distance(f.p[pin[1]]) < 0.02, "its hinge parted");
                }
            } else {
                assert!(gap > 0.3, "it did not come apart: {gap:.2} m");
                let up = (f.parts()[2].rotation * Vec3::Y).angle_between(Vec3::Y).to_degrees();
                assert!(up > 45.0, "its core still stands on its point ({up:.0} degrees over)");
            }
        }
    }

    /// Shot dead from the front by one blast, it is thrown back along the
    /// shot, limp or apart (the user, 2026-11-13: not falling where it
    /// stood), its pieces each well back.
    #[test]
    fn thrown_along_the_shot() {
        let scale = 4.0 / 1.901;
        for how in [Death::Limp, Death::Apart] {
            let start = standing(Vec3::Y * 0.95 * scale, scale);
            let struck: Vec<(usize, Vec3, Vec3)> = (0..6).map(|k| (k % 3, start[k % 3].transform_point(Vec3::new(0.0, 0.3, 0.0)), Vec3::NEG_Z)).collect();
            let mut f = Fall::new(start, start, 1.0 / 60.0, 1.4, how, &struck, 1.0);
            for _ in 0..60 * 4 {
                f.step(1.0 / 60.0, &|_| 0.0);
            }
            let went: Vec<f32> = f.parts().iter().take(3).map(|t| t.translation.z).collect();
            println!("{how:?}: its blades and core went {went:.2?} m along the shot");
            assert!(went.iter().all(|z| *z < -1.5), "{how:?}: not thrown along the shot: {went:?}");
        }
    }

    /// Killed point blank (a whole blast, 16 shards, square on): its
    /// pieces hurled back, through the air, far (the user, 2026-11-13).
    #[test]
    fn hurled_point_blank() {
        let scale = 4.0 / 1.901;
        let start = standing(Vec3::Y * 0.95 * scale, scale);
        let struck: Vec<(usize, Vec3, Vec3)> = (0..16).map(|k| (k % 3, start[k % 3].transform_point(Vec3::new(0.0, 0.2, 0.0)), Vec3::NEG_Z)).collect();
        let mut f = Fall::new(start, start, 1.0 / 60.0, 1.4, Death::Apart, &struck, 1.0);
        let mut highest = 0.0f32;
        for _ in 0..60 * 6 {
            f.step(1.0 / 60.0, &|_| 0.0);
            highest = highest.max(f.pieces.iter().map(|c| c.at.y).fold(f32::MIN, f32::max));
        }
        let went: Vec<f32> = f.parts().iter().take(3).map(|t| t.translation.z).collect();
        println!("point blank: its blades and core went {went:.1?} m, their middles rose {highest:.2} m at most");
        assert!(went.iter().all(|z| *z < -8.0), "not hurled: {went:?}");
    }

    /// Falling as it was moving: it carries on that way.
    #[test]
    fn keeps_its_way() {
        let scale = 4.0 / 1.901;
        let now = standing(Vec3::Y * 0.95 * scale, scale);
        let then = standing(Vec3::Y * 0.95 * scale - Vec3::X * 0.1, scale);
        let mut f = Fall::new(now, then, 1.0 / 60.0, 1.4, Death::Limp, &[], 1.0);
        for _ in 0..60 * 4 {
            f.step(1.0 / 60.0, &|_| 0.0);
        }
        let middle = (f.parts()[0].translation + f.parts()[1].translation) / 2.0;
        assert!(middle.x > 1.0, "did not carry on: {middle}");
    }
}
