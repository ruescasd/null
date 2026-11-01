//! A body gone limp (a hunter's collapse, its death): the joints of its
//! bones as points falling under gravity (Verlet, in fixed steps), its bones
//! kept at their lengths, its trunk one rigid frame, each limb bending only
//! the way it bent, the tone in it ebbing away; on the ground, held there
//! by friction. What hangs on its bones follows them as in life. And what
//! it carried (the stalker's lance), let go: a rod falling on its own.

use crate::rig::{Bone, GunFrame, GUN_BORE};
use bevy::prelude::*;

/// Its fixed step (s), and how many passes over its links in each.
const STEP: f32 = 1.0 / 120.0;
const PASSES: usize = 10;
const GRAVITY: f32 = 9.8;
/// How much of its speed it keeps each step: falling; and fallen, from so
/// long after it fell to so long (s), settling (its neck swung on between
/// its chest and its head for seconds, lying).
const DRAG: (f32, f32, f32, f32) = (0.999, 0.97, 1.5, 3.0);
/// On the ground, how much of its slide a point keeps each step (friction).
const SLIDE: f32 = 0.3;
/// Sliding slower than this (m/s) on the ground, a point stops dead.
const STICK: f32 = 0.3;
/// Its tone: how strongly each limb keeps the bend it had (0..1 a step), at
/// first; ebbing by half every so long (s), down to the least it keeps
/// (any at all, it crept on along the ground for seconds, lying).
const TONE: (f32, f32, f32) = (0.2, 0.3, 0.0);
/// Folded, a limb's ends kept at least this much of its reach apart.
const FOLD: f32 = 0.3;
/// How long its hand holds on to what it carries, falling (s).
const GRIP: f32 = 0.12;
/// At rest: every point slower than this (m/s) for so long (s), it sleeps
/// (woken by a blow).
const REST: (f32, f32) = (0.04, 0.6);
/// The lance's ends, how thick on the ground (m), and how far back of its
/// grip its stock reaches.
const ROD: (f32, f32) = (0.07, 0.6);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// Kept at its length.
    Hard,
    /// Kept no nearer than its length.
    Least,
    /// Drawn to its length by its tone.
    Tone,
}

#[derive(Clone, Copy, Debug)]
struct Link {
    a: usize,
    b: usize,
    rest: f32,
    kind: Kind,
}

/// A limb's middle joint (a knee, an elbow, the neck's top): its ends, and
/// the axis it bends about, following the limb as it turns while bent, held
/// as it straightens: bent back past straight, it is pushed back. (About
/// its trunk's right, it was lost as the limb lay across the trunk, and
/// flipped the limb through as it came back, flinging it.)
#[derive(Clone, Copy, Debug)]
struct Hinge {
    a: usize,
    mid: usize,
    b: usize,
    axis: Vec3,
}

/// What it carried: its stock, middle and tip along its bore; which way is
/// up on it; and which point is its hand, holding on a moment (see `GRIP`).
#[derive(Clone, Copy, Debug)]
struct Rod {
    ends: [usize; 3],
    up: Vec3,
    hand: Option<(usize, f32)>,
}

pub struct Ragdoll {
    /// Where it fell: its points are kept from here. (Kept in the world's
    /// own, out at the arena 1200 m from its middle, a float's rounding,
    /// 0.1 mm there, moved them every step and it never lay still, twitching
    /// on the ground for ever: the gladiator's, 2026-10-31.)
    origin: Vec3,
    p: Vec<Vec3>,
    q: Vec<Vec3>,
    radius: Vec<f32>,
    links: Vec<Link>,
    hinges: Vec<Hinge>,
    /// Each bone: its joints, its thickness, and its side and its line as
    /// they were, in its trunk's frame (see `trunk`).
    bones: Vec<(usize, usize, f32, Vec3, Vec3)>,
    /// The trunk's frame: its rump and its chest (up along it), and two
    /// points across it (left to right).
    trunk: [usize; 4],
    rod: Option<Rod>,
    /// How long since it fell (s), the time not yet stepped, how long it has
    /// lain still.
    pub age: f32,
    clock: f32,
    still: f32,
}

impl Ragdoll {
    /// Limp from its pose (`bones`; `trunk`, how many of them, first, make up
    /// its trunk), each joint moving as it was (`before`, `dt` s earlier),
    /// pushed by `push` (m/s, all of it); and what it carried, let go from the
    /// hand at the end of bone `hand`: its frame and how long from its grip
    /// to its tip.
    pub fn new(bones: &[Bone], before: &[Bone], dt: f32, trunk: usize, push: Vec3, gun: Option<(GunFrame, f32, usize)>) -> Self {
        let mut p: Vec<Vec3> = Vec::new();
        let mut was: Vec<Vec3> = Vec::new();
        let mut radius: Vec<f32> = Vec::new();
        let mut joint = |at: Vec3, then: Vec3, r: f32, p: &mut Vec<Vec3>| -> usize {
            if let Some(i) = p.iter().position(|x| x.distance(at) < 0.01) {
                radius[i] = radius[i].max(r);
                return i;
            }
            p.push(at);
            was.push(then);
            radius.push(r);
            p.len() - 1
        };
        let then = |k: usize, end: usize| before.get(k).map_or(if end == 0 { bones[k].a } else { bones[k].b }, |b| if end == 0 { b.a } else { b.b });
        let mut ends = Vec::new();
        for (k, b) in bones.iter().enumerate() {
            let a = joint(b.a, then(k, 0), b.radius, &mut p);
            let e = joint(b.b, then(k, 1), b.radius, &mut p);
            ends.push((a, e));
        }
        let mut links: Vec<Link> = ends.iter().map(|&(a, b)| Link { a, b, rest: p[a].distance(p[b]), kind: Kind::Hard }).collect();
        // Its trunk rigid: its bones' joints, and where its limbs hang from
        // it (a joint only ever a bone's start: a hip, a shoulder).
        let ended: Vec<usize> = ends.iter().map(|e| e.1).collect();
        let mut frame: Vec<usize> = ends[..trunk].iter().flat_map(|&(a, b)| [a, b]).collect();
        for &(a, _) in &ends[trunk..] {
            if !ended.contains(&a) && !frame.contains(&a) {
                frame.push(a);
            }
        }
        frame.dedup();
        for (i, &a) in frame.iter().enumerate() {
            for &b in &frame[i + 1..] {
                if a != b && !links.iter().any(|l| (l.a, l.b) == (a, b) || (l.a, l.b) == (b, a)) {
                    links.push(Link { a, b, rest: p[a].distance(p[b]), kind: Kind::Hard });
                }
            }
        }
        // Its frame: rump to chest; across it, the two of its limbs' roots
        // farthest apart along its first bone's side.
        let (rump, chest) = (ends[0].0, ends[trunk - 1].1);
        let side = bones[0].side;
        let roots: Vec<usize> = frame.iter().copied().filter(|i| ![rump, chest].contains(i)).collect();
        let mut across = (rump, chest);
        let mut widest = 0.0;
        for &i in &roots {
            for &j in &roots {
                let w = (p[j] - p[i]).dot(side);
                if w > widest {
                    (widest, across) = (w, (i, j));
                }
            }
        }
        let trunk_ix = [rump, chest, across.0, across.1];
        let mut doll = Ragdoll { origin: Vec3::ZERO, p, q: was, radius, links, hinges: Vec::new(), bones: Vec::new(), trunk: trunk_ix, rod: None, age: 0.0, clock: 0.0, still: 0.0 };
        let r0 = doll.frame(bones[0].side);
        doll.bones = bones.iter().zip(&ends).map(|(b, &(a, e))| (a, e, b.radius, r0.inverse() * b.side, r0.inverse() * (b.b - b.a).normalize_or(Vec3::Y))).collect();
        // Its limbs' middle joints, each bending the way it bent, its tone
        // holding it a while, folding no further than `FOLD`.
        let right = r0 * Vec3::X;
        for (k, &(a, mid)) in ends.iter().enumerate() {
            if frame.contains(&mid) {
                continue;
            }
            let next: Vec<usize> = ends.iter().enumerate().filter(|&(j, e)| j != k && e.0 == mid).map(|(_, e)| e.1).collect();
            if let [b] = next[..] {
                let pa = doll.p[a];
                let bend = (doll.p[mid] - pa).cross(doll.p[b] - pa).dot(right);
                let reach = doll.p[mid].distance(pa) + doll.p[b].distance(doll.p[mid]);
                doll.links.push(Link { a, b, rest: doll.p[a].distance(doll.p[b]), kind: Kind::Tone });
                doll.links.push(Link { a, b, rest: reach * FOLD, kind: Kind::Least });
                doll.hinges.push(Hinge { a, mid, b, axis: right * if bend >= 0.0 { 1.0 } else { -1.0 } });
            }
        }
        // Moving as it was, and pushed.
        let dt = dt.max(1e-3);
        for i in 0..doll.p.len() {
            let v = ((doll.p[i] - doll.q[i]) / dt).clamp_length_max(12.0) + push;
            doll.q[i] = doll.p[i] - v * STEP;
        }
        // What it carried: along its bore, from its stock to its tip.
        if let Some((f, length, hand)) = gun {
            let bore = f.grip + f.up * GUN_BORE;
            let hand = ends.get(hand).map(|e| e.1);
            let n = doll.p.len();
            let v = hand.map_or(push, |h| (doll.p[h] - doll.q[h]) / STEP);
            for at in [bore - f.axis * ROD.1, bore + f.axis * (length - ROD.1) * 0.5, bore + f.axis * length] {
                doll.p.push(at);
                doll.q.push(at - v * STEP);
                doll.radius.push(ROD.0);
            }
            for (a, b) in [(n, n + 1), (n + 1, n + 2), (n, n + 2)] {
                doll.links.push(Link { a, b, rest: doll.p[a].distance(doll.p[b]), kind: Kind::Hard });
            }
            doll.rod = Some(Rod { ends: [n, n + 1, n + 2], up: f.up, hand: hand.map(|h| (h, doll.p[h].distance(bore))) });
        }
        let origin = doll.p[rump];
        for i in 0..doll.p.len() {
            doll.p[i] -= origin;
            doll.q[i] -= origin;
        }
        doll.origin = origin;
        doll
    }

    /// Its trunk's frame now: x across it, y up along it.
    fn frame(&self, fallback: Vec3) -> Quat {
        let [rump, chest, l, r] = self.trunk;
        let y = (self.p[chest] - self.p[rump]).normalize_or(Vec3::Y);
        let across = if l == r { fallback } else { self.p[r] - self.p[l] };
        let x = (across - y * across.dot(y)).normalize_or(y.any_orthonormal_vector());
        Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
    }

    /// Whether it lies still (asleep: see `REST`).
    pub fn at_rest(&self) -> bool {
        self.still >= REST.1
    }

    /// A blow on bone `bone` (m/s, to its joints): wakes it.
    pub fn strike(&mut self, bone: usize, v: Vec3) {
        if let Some(&(a, b, ..)) = self.bones.get(bone) {
            for i in [a, b] {
                self.q[i] -= v * STEP;
            }
            self.still = 0.0;
        }
    }

    /// On `dt` s: falling, lying; `floor` the ground's height under a point,
    /// `wall` where a point moving from one place to another is stopped.
    pub fn step(&mut self, dt: f32, floor: &dyn Fn(Vec3) -> f32, wall: &dyn Fn(Vec3, Vec3) -> Vec3) {
        let o = self.origin;
        let floor = &|p: Vec3| floor(p + o) - o.y;
        let wall = &|a: Vec3, b: Vec3| wall(a + o, b + o) - o;
        self.age += dt;
        if self.at_rest() {
            return;
        }
        let from = self.p.clone();
        self.clock = (self.clock + dt).min(STEP * 6.0);
        while self.clock >= STEP {
            self.clock -= STEP;
            self.tick(floor);
        }
        // (Not through what is built: each point, the way it went this
        // frame.)
        for i in 0..self.p.len() {
            if from[i].distance_squared(self.p[i]) > 1e-8 {
                let to = wall(from[i], self.p[i]);
                if to.distance_squared(self.p[i]) > 1e-8 {
                    self.p[i] = to;
                    self.q[i] = to;
                }
            }
        }
        let fastest = (0..self.p.len()).map(|i| (self.p[i] - from[i]).length()).fold(0.0, f32::max) / dt.max(1e-4);
        self.still = if fastest < REST.0 { self.still + dt } else { 0.0 };
    }

    fn tick(&mut self, floor: &dyn Fn(Vec3) -> f32) {
        let n = self.p.len();
        let g = Vec3::NEG_Y * GRAVITY * STEP * STEP;
        let settled = ((self.age - DRAG.2) / (DRAG.3 - DRAG.2)).clamp(0.0, 1.0);
        let drag = DRAG.0 + (DRAG.1 - DRAG.0) * settled;
        for i in 0..n {
            let v = (self.p[i] - self.q[i]) * drag;
            self.q[i] = self.p[i];
            self.p[i] += v + g;
        }
        let tone = TONE.2 + (TONE.0 - TONE.2) * 0.5f32.powf(self.age / TONE.1);
        if let Some(rod) = self.rod.as_mut()
            && rod.hand.is_some_and(|_| self.age > GRIP)
        {
            rod.hand = None;
        }
        let rod = self.rod;
        // (The ground under each point, felt once a step for all its passes,
        // not in each: ten times over, it was a thousand casts a frame for a
        // body still moving.)
        let floors: Vec<f32> = (0..n).map(|i| floor(Vec3::new(self.p[i].x, self.p[i].y.max(self.q[i].y) + 0.3, self.p[i].z))).collect();
        for pass in 0..PASSES {
            for l in &self.links {
                let d = self.p[l.b] - self.p[l.a];
                let len = d.length().max(1e-5);
                let k = match l.kind {
                    Kind::Hard => 1.0,
                    Kind::Least if len < l.rest => 1.0,
                    Kind::Least => continue,
                    Kind::Tone if pass == 0 => tone,
                    Kind::Tone => continue,
                };
                let fix = d * ((len - l.rest) / len) * 0.5 * k;
                self.p[l.a] += fix;
                self.p[l.b] -= fix;
            }
            // (Its hand on its grip, a moment.)
            if let Some(Rod { ends: [s, m, _], hand: Some((h, rest)), .. }) = rod {
                let grip = self.p[s] + (self.p[m] - self.p[s]).normalize_or(Vec3::Z) * ROD.1;
                let d = grip - self.p[h];
                let len = d.length().max(1e-5);
                let fix = d * ((len - rest) / len) * 0.5;
                self.p[h] += fix;
                self.p[s] -= fix;
                self.p[m] -= fix;
            }
            self.hinge();
            self.ground(&floors);
        }
        // (Its long bones by their middles too: across a hump, not through
        // it. Its rod's middle is a point of its own.)
        for k in 0..self.bones.len() {
            let (a, b, r, ..) = self.bones[k];
            self.lift(a, b, r, floor);
        }
        // Its rod's up, turned with it.
        if let Some(rod) = self.rod.as_mut() {
            let [s, _, t] = rod.ends;
            let axis = (self.p[t] - self.p[s]).normalize_or(Vec3::Z);
            rod.up = (rod.up - axis * rod.up.dot(axis)).normalize_or(axis.any_orthonormal_vector());
        }
    }

    /// Each limb bent only the way it bent.
    fn hinge(&mut self) {
        for k in 0..self.hinges.len() {
            let h = self.hinges[k];
            let (a, mid, b) = (self.p[h.a], self.p[h.mid], self.p[h.b]);
            let line = b - a;
            // (Bent, its axis turned with it.)
            let n = (mid - a).cross(b - mid);
            let bent = n.length() / ((mid - a).length() * (b - mid).length()).max(1e-6);
            if bent > 0.15 && n.dot(h.axis) > 0.0 {
                self.hinges[k].axis = h.axis.lerp(n.normalize(), 0.2).normalize();
            }
            let out = line.cross(h.axis);
            let held = out.length() / line.length().max(1e-6);
            let out = out.normalize_or(Vec3::ZERO);
            let by = (mid - a).dot(out);
            let least = line.length() * 0.02;
            if by < least {
                let fix = out * (least - by) * held;
                self.p[h.mid] += fix * 0.7;
                self.p[h.a] -= fix * 0.15;
                self.p[h.b] -= fix * 0.15;
            }
        }
    }

    /// Off the ground (its height under each point), held by it.
    fn ground(&mut self, floors: &[f32]) {
        for i in 0..self.p.len() {
            let p = self.p[i];
            let top = floors[i] + self.radius[i];
            if p.y < top {
                self.p[i].y = top;
                let q = self.q[i];
                // (Slowly, not at all: it stops dead.)
                let slide = Vec3::new(p.x - q.x, 0.0, p.z - q.z);
                let slide = if slide.length() < STICK * STEP { Vec3::ZERO } else { slide * SLIDE };
                self.q[i] = Vec3::new(p.x - slide.x, q.y.max(top), p.z - slide.z);
            }
        }
    }

    /// A long thing's middle kept off the ground: both ends raised by as
    /// much as it is under (not thrown up by it).
    fn lift(&mut self, a: usize, b: usize, r: f32, floor: &dyn Fn(Vec3) -> f32) {
        let mid = (self.p[a] + self.p[b]) * 0.5;
        if self.p[a].distance(self.p[b]) < 0.6 {
            return;
        }
        let under = floor(mid + Vec3::Y * 0.3) + r - mid.y;
        if under > 0.0 {
            for i in [a, b] {
                self.p[i].y += under;
                self.q[i].y += under;
            }
        }
    }

    /// Its bones as they lie.
    pub fn bones(&self) -> Vec<Bone> {
        let r = self.frame(Vec3::X);
        self.bones
            .iter()
            .map(|&(a, b, radius, side, line)| {
                let dir = (self.p[b] - self.p[a]).normalize_or(Vec3::Y);
                let side = Quat::from_rotation_arc((r * line).normalize_or(dir), dir) * (r * side);
                Bone { a: self.p[a] + self.origin, b: self.p[b] + self.origin, radius, side }
            })
            .collect()
    }

    /// What it carried, as it lies (its aim 0).
    pub fn gun(&self) -> Option<GunFrame> {
        self.rod.map(|rod| {
            let [s, _, t] = rod.ends;
            let axis = (self.p[t] - self.p[s]).normalize_or(Vec3::Z);
            let grip = self.p[s] + self.origin + axis * ROD.1 - rod.up * GUN_BORE;
            GunFrame { grip, axis, up: rod.up, fore: grip + axis * crate::rig::GUN_FORE + rod.up * (GUN_BORE - crate::rig::GUN_HOLD), aim: 0.0 }
        })
    }

    /// Every point, and its thickness (for tests).
    #[cfg(test)]
    pub fn points(&self) -> impl Iterator<Item = (Vec3, f32)> + '_ {
        self.p.iter().map(|&p| p + self.origin).zip(self.radius.iter().copied())
    }
}

