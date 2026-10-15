//! Fabric: cloth done properly, the game way (XPBD, Macklin and Müller; in
//! small steps, Macklin et al. 2019), after `cloth.rs` clumped and crumpled
//! (the user: a solid attempt, or none). Each frame cut into many short
//! steps, each solving every constraint once: its edges kept at their
//! lengths (stiff against stretching, softer against squashing, so it folds
//! rather than shrinks); its folds kept by the angle across each edge
//! (dihedral bending: soft, so it hangs in folds, but no crumpling); its
//! points never nearer one another than its thickness (self-collision, by a
//! spatial hash: it folds over itself, never through); each point never
//! farther from the nearest held one than along the cloth (tethers: no
//! stretching under a jerk); out of the bodies it hangs on (capsules,
//! boxes, moving smoothly through the frame), with friction, so it rests
//! on them, not slides off. Held points (pinned to a body) move smoothly
//! through the frame too. Its points kept near an origin of its own, in
//! doubles (out in the world, at 1200 m, a float's step is 0.1 mm, ten
//! times what a point moves in one of its steps); its mesh drawn there.

use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    math::{DQuat, DVec3},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// How it behaves: steps a second (each the same length, however long the
/// frame: steps of a frame's length, never quite the same twice, moved where
/// it settled every frame, and it never came to rest); how much it gives to stretching and to
/// squashing (compliance, m/N: 0 rigid); how much to bending (larger:
/// limper); how thick (m: how near its points come, its layers kept apart);
/// how much it grips the bodies it rests on (0 slides, 1 sticks); how fast
/// it loses its speed (per second); how fast a point may go (m/s).
#[derive(Clone, Copy)]
pub struct Weave {
    pub rate: f32,
    pub stretch: f32,
    pub squash: f32,
    pub bend: f32,
    pub thickness: f32,
    pub friction: f32,
    pub damping: f32,
    pub fastest: f32,
    /// Whether it keeps clear of itself (the costliest part).
    pub self_collide: bool,
}

// (Never quite rigid: rigid, its edges were more than its points could keep,
// and what they could not was left as speed: a sheet hung by its top shook
// as if in a gale, stretched by a quarter. Given a millionth, it hangs
// still, stretched by 3%.)
impl Default for Weave {
    fn default() -> Self {
        Weave { rate: 900.0, stretch: 1e-6, squash: 1e-4, bend: 0.5, thickness: 0.02, friction: 0.4, damping: 1.0, fastest: 12.0, self_collide: true }
    }
}

/// How often (in steps) its folds are kept (soft: it needs it less often
/// than its lengths); the most steps it takes in a frame (a longer frame's
/// time is lost, not caught up).
const BEND_EVERY: u64 = 3;
const MOST: usize = 45;
/// How much further than they could go in a frame its points are looked
/// for round each body (m).
const MARGIN: f64 = 0.1;

/// What it hangs on: a capsule (two ends, a radius), or a box (centre,
/// turn, half its size).
#[derive(Clone, Copy, Debug)]
pub enum Body {
    Capsule(Vec3, Vec3, f32),
    Box(Vec3, Quat, Vec3),
}

impl Body {
    /// As a solid, from `origin`.
    fn solid(&self, origin: DVec3) -> Solid {
        match *self {
            Body::Capsule(a, b, r) => Solid::Capsule(a.as_dvec3() - origin, b.as_dvec3() - origin, r as f64),
            Body::Box(c, q, h) => Solid::Box(c.as_dvec3() - origin, q.as_dquat(), h.as_dvec3()),
        }
    }
}

/// A body, as the cloth meets it (from its origin, in doubles).
#[derive(Clone, Copy)]
enum Solid {
    Capsule(DVec3, DVec3, f64),
    Box(DVec3, DQuat, DVec3),
}

impl Solid {
    /// A sphere round it: its middle, its radius.
    fn bounds(&self) -> (DVec3, f64) {
        match *self {
            Solid::Capsule(a, b, r) => ((a + b) * 0.5, a.distance(b) * 0.5 + r),
            Solid::Box(c, _, h) => (c, h.length()),
        }
    }

    /// Partway (`t`) from `self` to `to` (the same kind of body).
    fn lerp(&self, to: &Solid, t: f64) -> Solid {
        match (*self, *to) {
            (Solid::Capsule(a0, b0, r0), Solid::Capsule(a1, b1, r1)) => Solid::Capsule(a0.lerp(a1, t), b0.lerp(b1, t), r0 + (r1 - r0) * t),
            (Solid::Box(c0, q0, h0), Solid::Box(c1, q1, h1)) => Solid::Box(c0.lerp(c1, t), q0.slerp(q1, t), h0.lerp(h1, t)),
            _ => *to,
        }
    }

    /// How far its surface at `p` has come since it was `was`.
    fn carried(&self, was: &Solid, p: DVec3) -> DVec3 {
        match (*was, *self) {
            (Solid::Capsule(a0, b0, _), Solid::Capsule(a1, b1, _)) => {
                let ab = b1 - a1;
                let u = ((p - a1).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                (a1 + ab * u) - a0.lerp(b0, u)
            }
            (Solid::Box(c0, q0, _), Solid::Box(c1, q1, _)) => p - (c0 + q0 * (q1.inverse() * (p - c1))),
            _ => DVec3::ZERO,
        }
    }

    /// Where `p`, if within `skin` of its surface or inside it, is put out
    /// to that distance from it (and the way out).
    fn push(&self, p: DVec3, skin: f64) -> Option<(DVec3, DVec3)> {
        match *self {
            Solid::Capsule(a, b, r) => {
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                let c = a + ab * t;
                let d = p - c;
                let reach = r + skin;
                let l2 = d.length_squared();
                if l2 >= reach * reach {
                    return None;
                }
                let n = if l2 > 1e-18 { d / l2.sqrt() } else { ab.any_orthonormal_vector() };
                Some((c + n * reach, n))
            }
            Solid::Box(centre, turn, half) => {
                let local = turn.inverse() * (p - centre);
                let out = half + DVec3::splat(skin);
                let gap = out - local.abs();
                if gap.min_element() <= 0.0 {
                    return None;
                }
                // (Out the nearest face.)
                let axis = if gap.x <= gap.y && gap.x <= gap.z {
                    0
                } else if gap.y <= gap.z {
                    1
                } else {
                    2
                };
                let mut q = local;
                let sign = if local[axis] < 0.0 { -1.0 } else { 1.0 };
                q[axis] = out[axis] * sign;
                let mut n = DVec3::ZERO;
                n[axis] = sign;
                Some((centre + turn * q, turn * n))
            }
        }
    }
}

/// A spatial hash of points (for self-collision): cells `size` wide, each
/// point's cell's start in `order`.
struct Hash {
    size: f64,
    starts: Vec<u32>,
    order: Vec<u32>,
}

impl Hash {
    fn cell(&self, p: DVec3) -> IVec3 {
        (p / self.size).floor().as_ivec3()
    }

    fn slot(&self, c: IVec3) -> usize {
        let h = (c.x.wrapping_mul(92837111)) ^ (c.y.wrapping_mul(689287499)) ^ (c.z.wrapping_mul(283923481));
        h as u32 as usize & (self.starts.len() - 2)
    }

    /// Filled with `points`.
    fn fill(&mut self, points: &[DVec3]) {
        let n = self.starts.len() - 1;
        self.starts.iter_mut().for_each(|s| *s = 0);
        for &p in points {
            let s = self.slot(self.cell(p));
            self.starts[s] += 1;
        }
        let mut sum = 0;
        for s in self.starts.iter_mut().take(n) {
            sum += *s;
            *s = sum;
        }
        self.starts[n] = sum;
        self.order.resize(points.len(), 0);
        for (i, &p) in points.iter().enumerate() {
            let s = self.slot(self.cell(p));
            self.starts[s] -= 1;
            self.order[self.starts[s] as usize] = i as u32;
        }
    }
}

pub struct Fabric {
    /// Its points, from its origin, in doubles (a step moves a point little
    /// more than a float's step a few metres out).
    points: Vec<DVec3>,
    origin: DVec3,
    /// Where each point was at the start of the step; how fast it goes;
    /// where it was made (its rest).
    before: Vec<DVec3>,
    velocity: Vec<DVec3>,
    rest: Vec<DVec3>,
    /// Its edges: two points and their length.
    edges: Vec<(u32, u32, f64)>,
    /// Its hinges: two edge points, the two points either side of it, and
    /// the angle it was made at.
    hinges: Vec<([u32; 4], f64)>,
    /// Each point's nearest held point and how far it is along the cloth
    /// (for points not held; see `tether`).
    tethers: Vec<(u32, u32, f64)>,
    triangles: Vec<u32>,
    /// Where its held points were held last frame, and the bodies it hung
    /// on (to move them smoothly through the frame).
    held_last: HashMap<u32, DVec3>,
    bodies_last: Vec<Body>,
    /// Neighbours near enough to touch this frame (self-collision), and
    /// how near each pair may come.
    near: Vec<(u32, u32, f64)>,
    hash: Hash,
    /// The time it has yet to step (s), and the steps it has taken; how
    /// far its points went in the last frame that stepped, on average (m/s).
    owed: f64,
    steps: u64,
    /// The bodies each point may meet this frame (see `step`), from where
    /// in `meets` each point's start.
    meets: Vec<u16>,
    meets_at: Vec<u32>,
    stir: f32,
    was: Vec<DVec3>,
    pub weave: Weave,
}

impl Fabric {
    /// Made from points (where it rests: its lengths and folds those it is
    /// made in) and triangles.
    pub fn new(points: Vec<Vec3>, triangles: Vec<u32>, weave: Weave) -> Fabric {
        let origin = points.first().copied().unwrap_or(Vec3::ZERO).as_dvec3();
        let points: Vec<DVec3> = points.into_iter().map(|p| p.as_dvec3() - origin).collect();
        // Each edge once, and the points either side of it.
        let mut sides: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
        for t in triangles.chunks(3) {
            for k in 0..3 {
                let (a, b, o) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                sides.entry((a.min(b), a.max(b))).or_default().push(o);
            }
        }
        let mut keys: Vec<_> = sides.keys().copied().collect();
        keys.sort();
        let edges = keys.iter().map(|&(a, b)| (a, b, points[a as usize].distance(points[b as usize]))).collect();
        let mut hinges = Vec::new();
        for &(a, b) in &keys {
            let o = &sides[&(a, b)];
            if o.len() == 2 {
                let q = [a, b, o[0], o[1]];
                hinges.push((q, hinge(&points, q).0));
            }
        }
        let n = points.len();
        Fabric {
            before: points.clone(),
            velocity: vec![DVec3::ZERO; n],
            rest: points.clone(),
            points,
            origin,
            edges,
            hinges,
            tethers: Vec::new(),
            triangles,
            held_last: HashMap::new(),
            bodies_last: Vec::new(),
            near: Vec::new(),
            hash: Hash { size: 1.0, starts: vec![0; (2 * n).next_power_of_two() + 1], order: Vec::new() },
            owed: 0.0,
            steps: 0,
            meets: Vec::new(),
            meets_at: Vec::new(),
            stir: 0.0,
            was: Vec::new(),
            weave,
        }
    }

    /// A sheet `cols` by `rows`, each point where `at` puts it (its
    /// diagonals alternating, so it folds alike every way).
    pub fn sheet(cols: usize, rows: usize, at: impl Fn(usize, usize) -> Vec3, weave: Weave) -> Fabric {
        let points = (0..rows).flat_map(|r| (0..cols).map(move |c| (c, r))).map(|(c, r)| at(c, r)).collect();
        let idx = |c: usize, r: usize| (r * cols + c) as u32;
        let mut triangles = Vec::new();
        for r in 0..rows - 1 {
            for c in 0..cols - 1 {
                let (a, b, d, e) = (idx(c, r), idx(c + 1, r), idx(c, r + 1), idx(c + 1, r + 1));
                if (c + r) % 2 == 0 {
                    triangles.extend([a, d, b, b, d, e]);
                } else {
                    triangles.extend([a, d, e, a, e, b]);
                }
            }
        }
        Fabric::new(points, triangles, weave)
    }

    /// Its folds kept flat, not as it was made: cloth as cut from a flat
    /// sheet, whatever shape it is made up in (made round, kept round, it
    /// stood as a tube).
    pub fn cut_flat(&mut self) {
        for hinge in &mut self.hinges {
            hinge.1 = 0.0;
        }
    }

    /// Ties each point to the nearest of `held` (by the way along the
    /// cloth): never farther from it than that, times `slack`.
    pub fn tether(&mut self, held: &[u32], slack: f32) {
        let n = self.points.len();
        let mut next: Vec<Vec<(u32, f64)>> = vec![Vec::new(); n];
        for &(a, b, l) in &self.edges {
            next[a as usize].push((b, l));
            next[b as usize].push((a, l));
        }
        let mut best = vec![(f64::MAX, u32::MAX); n];
        let mut queue = std::collections::BinaryHeap::new();
        for &h in held {
            best[h as usize] = (0.0, h);
            queue.push((std::cmp::Reverse(0u64), h));
        }
        // (Dijkstra, on distances as whole micrometres.)
        while let Some((std::cmp::Reverse(d), i)) = queue.pop() {
            let d = d as f64 * 1e-6;
            if d > best[i as usize].0 + 1e-6 {
                continue;
            }
            for &(j, l) in &next[i as usize] {
                let dj = d + l;
                if dj < best[j as usize].0 {
                    best[j as usize] = (dj, best[i as usize].1);
                    queue.push((std::cmp::Reverse((dj * 1e6) as u64), j));
                }
            }
        }
        let held: std::collections::HashSet<u32> = held.iter().copied().collect();
        self.tethers = (0..n as u32).filter(|i| !held.contains(i) && best[*i as usize].1 != u32::MAX).map(|i| (i, best[i as usize].1, best[i as usize].0 * slack as f64)).collect();
    }

    /// One frame (`dt`): under gravity; the `held` points moved smoothly to
    /// where they are put; on the `bodies` (as many, of the same kinds, as
    /// last frame, to be moved smoothly; else taken as they are); above
    /// `floor`. Held points far from where they were (put somewhere new)
    /// take the whole cloth with them, still.
    pub fn step(&mut self, held: &[(u32, Vec3)], bodies: &[Body], floor: f32, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let w = self.weave;
        // As many steps as the time owed holds (none, it waits).
        let h = 1.0 / w.rate as f64;
        self.owed += dt as f64;
        let count = ((self.owed / h) as usize).min(MOST);
        self.owed = if count == MOST { 0.0 } else { self.owed - count as f64 * h };
        if count == 0 {
            return;
        }
        // (Its origin moved to it, gone far from it: by exactly as much as
        // its points, its speeds as they were.)
        if let Some(&(_, at)) = held.first() {
            let at = at.as_dvec3();
            if at.distance(self.origin) > 20.0 {
                let by = at - self.origin;
                self.origin = at;
                for p in &mut self.points {
                    *p -= by;
                }
                for p in self.held_last.values_mut() {
                    *p -= by;
                }
            }
        }
        // Everything from its origin.
        let origin = self.origin;
        let held: Vec<(u32, DVec3)> = held.iter().map(|&(i, at)| (i, at.as_dvec3() - origin)).collect();
        let solids: Vec<Solid> = bodies.iter().map(|b| b.solid(origin)).collect();
        let floor = floor as f64 - origin.y;
        if let Some(&(i, at)) = held.first() {
            let jump = at - self.points[i as usize];
            if jump.length() > 3.0 {
                for p in &mut self.points {
                    *p += jump;
                }
                self.velocity.iter_mut().for_each(|v| *v = DVec3::ZERO);
                self.held_last.clear();
            }
        }
        let n = self.points.len();
        let mut inv = vec![1.0f64; n];
        let from: Vec<(u32, DVec3, DVec3)> = held.iter().map(|&(i, to)| (i, self.held_last.get(&i).copied().unwrap_or(self.points[i as usize]), to)).collect();
        for &(i, _) in &held {
            inv[i as usize] = 0.0;
        }
        let solids_from: Vec<Solid> = if self.bodies_last.len() == bodies.len() { self.bodies_last.iter().map(|b| b.solid(origin)).collect() } else { solids.clone() };
        // (Its neighbours found once a frame: near enough to touch in it.
        // Found every few steps, it cost more than all the rest.)
        if w.self_collide {
            self.find_near(count as f64 * h);
        }
        let gravity = DVec3::NEG_Y * 9.8 * h;
        let keep = (-w.damping as f64 * h).exp();
        let (friction, fastest) = (w.friction as f64, w.fastest as f64);
        let skin = w.thickness as f64 * 0.5;
        self.was.clone_from(&self.points);
        // Which bodies each point may meet this frame: those whose bounds
        // (over the frame, and as far again as it could go) it is in (each
        // point against every body every step cost more than its lengths).
        let quickest = self.velocity.iter().map(|v| v.length_squared()).fold(0.0, f64::max).sqrt();
        let reach = quickest * count as f64 * h + skin + MARGIN;
        let bounds: Vec<(DVec3, f64)> = solids_from.iter().zip(&solids).map(|(a, b)| {
            let (ca, ra) = a.bounds();
            let (cb, rb) = b.bounds();
            ((ca + cb) * 0.5, ra.max(rb) + ca.distance(cb) * 0.5 + reach)
        }).collect();
        self.meets.clear();
        self.meets_at.clear();
        for i in 0..n {
            self.meets_at.push(self.meets.len() as u32);
            if inv[i] > 0.0 {
                let p = self.points[i];
                self.meets.extend(bounds.iter().enumerate().filter(|(_, (c, r))| p.distance_squared(*c) < r * r).map(|(k, _)| k as u16));
            }
        }
        self.meets_at.push(self.meets.len() as u32);
        let mut now = solids_from.clone();
        let mut was = solids_from.clone();
        for s in 1..=count {
            let t = s as f64 / count as f64;
            self.steps += 1;
            for k in 0..now.len() {
                was[k] = now[k];
                now[k] = solids_from[k].lerp(&solids[k], t);
            }
            for i in 0..n {
                self.before[i] = self.points[i];
                if inv[i] > 0.0 {
                    self.velocity[i] = (self.velocity[i] + gravity).clamp_length_max(fastest);
                    self.points[i] += self.velocity[i] * h;
                }
            }
            for &(i, a, b) in &from {
                self.points[i as usize] = a.lerp(b, t);
            }
            self.solve(&inv, h, self.steps % BEND_EVERY == 0);
            // Out of the bodies, gripping them; above the floor.
            for i in 0..n {
                if inv[i] == 0.0 {
                    continue;
                }
                for &k in &self.meets[self.meets_at[i] as usize..self.meets_at[i + 1] as usize] {
                    let (k, solid) = (k as usize, &now[k as usize]);
                    if let Some((out, normal)) = solid.push(self.points[i], skin) {
                        // (Friction: its motion along the surface, relative
                        // to the body's there, reduced.)
                        let rel = (out - self.before[i]) - solid.carried(&was[k], out);
                        let along = rel - normal * rel.dot(normal);
                        self.points[i] = out - along * friction;
                    }
                }
                if self.points[i].y < floor + skin {
                    let p = &mut self.points[i];
                    p.y = floor + skin;
                    let along = DVec3::new(p.x - self.before[i].x, 0.0, p.z - self.before[i].z);
                    *p -= along * friction;
                }
            }
            for i in 0..n {
                self.velocity[i] = if inv[i] > 0.0 { (self.points[i] - self.before[i]) / h * keep } else { DVec3::ZERO };
            }
        }
        self.stir = (self.points.iter().zip(&self.was).map(|(a, b)| a.distance(*b)).sum::<f64>() / n.max(1) as f64 / (count as f64 * h)) as f32;
        self.held_last = held.into_iter().collect();
        self.bodies_last = bodies.to_vec();
    }

    /// Every constraint once (XPBD, one iteration a step: its λ starts at 0
    /// each time).
    fn solve(&mut self, inv: &[f64], h: f64, bend: bool) {
        let w = self.weave;
        let h2 = h * h;
        let p = &mut self.points;
        let (stretch, squash) = (w.stretch as f64 / h2, w.squash as f64 / h2);
        for &(a, b, rest) in &self.edges {
            let (a, b) = (a as usize, b as usize);
            let s = inv[a] + inv[b];
            if s == 0.0 {
                continue;
            }
            let d = p[b] - p[a];
            let l = d.length();
            if l < 1e-12 {
                continue;
            }
            let c = l - rest;
            let alpha = if c < 0.0 { squash } else { stretch };
            let lambda = -c / (s + alpha);
            let g = d / l;
            p[a] -= g * (lambda * inv[a]);
            p[b] += g * (lambda * inv[b]);
        }
        let alpha = w.bend as f64 / (h2 * (BEND_EVERY * BEND_EVERY) as f64);
        for &(q, rest) in if bend { &self.hinges[..] } else { &[] } {
            let (theta, grads) = hinge(p, q);
            let mut c = theta - rest;
            if c > std::f64::consts::PI {
                c -= std::f64::consts::TAU;
            } else if c < -std::f64::consts::PI {
                c += std::f64::consts::TAU;
            }
            let s: f64 = (0..4).map(|k| inv[q[k] as usize] * grads[k].length_squared()).sum();
            if s < 1e-12 {
                continue;
            }
            let lambda = -c / (s + alpha);
            for k in 0..4 {
                p[q[k] as usize] += grads[k] * (lambda * inv[q[k] as usize]);
            }
        }
        for &(i, at, max) in &self.tethers {
            let (i, at) = (i as usize, at as usize);
            let d = p[i] - p[at];
            let l = d.length();
            if l > max && inv[i] > 0.0 {
                p[i] -= d * ((l - max) / l);
            }
        }
        for &(a, b, min) in &self.near {
            let (a, b) = (a as usize, b as usize);
            let s = inv[a] + inv[b];
            if s == 0.0 {
                continue;
            }
            let d = p[b] - p[a];
            let l2 = d.length_squared();
            if l2 >= min * min || l2 < 1e-24 {
                continue;
            }
            let l = l2.sqrt();
            let fix = d * ((min - l) / l / s);
            p[a] -= fix * inv[a];
            p[b] += fix * inv[b];
        }
    }

    /// The pairs of points that may touch in the next `span` s (within its
    /// thickness, and as far again as the fastest could go in it, at its
    /// speed through the rest of it: falling, or carried, together, none
    /// nears another), each kept no nearer than its thickness. Not those
    /// made near one another (within twice its thickness: neighbours in
    /// the weave, kept apart by its lengths).
    fn find_near(&mut self, span: f64) {
        let thickness = self.weave.thickness as f64;
        let mean = self.velocity.iter().copied().sum::<DVec3>() / self.velocity.len().max(1) as f64;
        let fastest = self.velocity.iter().map(|v| (*v - mean).length_squared()).fold(0.0f64, f64::max).sqrt();
        let reach = thickness + (2.0 * fastest * span).min(thickness);
        let apart = (2.0 * thickness).powi(2);
        self.hash.size = reach;
        self.hash.fill(&self.points);
        self.near.clear();
        let mut seen = [usize::MAX; 8];
        for i in 0..self.points.len() {
            let p = self.points[i];
            // (The cells as wide as its reach: those it reaches into, two
            // each way; each slot once, though two cells share it.)
            let (lo, hi) = (self.hash.cell(p - DVec3::splat(reach)), self.hash.cell(p + DVec3::splat(reach)));
            let mut k = 0;
            for z in lo.z..=hi.z {
                for y in lo.y..=hi.y {
                    for x in lo.x..=hi.x {
                        let s = self.hash.slot(IVec3::new(x, y, z));
                        if seen[..k].contains(&s) {
                            continue;
                        }
                        if k < seen.len() {
                            seen[k] = s;
                            k += 1;
                        }
                        for &j in &self.hash.order[self.hash.starts[s] as usize..self.hash.starts[s + 1] as usize] {
                            let j = j as usize;
                            if j <= i || self.points[j].distance_squared(p) > reach * reach || self.rest[i].distance_squared(self.rest[j]) < apart {
                                continue;
                            }
                            self.near.push((i as u32, j as u32, thickness));
                        }
                    }
                }
            }
        }
    }

    /// Its points put where `at` says, still.
    pub fn set(&mut self, at: impl Fn(usize) -> Vec3) {
        let origin = self.origin;
        for (i, p) in self.points.iter_mut().enumerate() {
            *p = at(i).as_dvec3() - origin;
        }
        self.velocity.iter_mut().for_each(|v| *v = DVec3::ZERO);
        self.held_last.clear();
        self.bodies_last.clear();
    }

    /// Where a point is.
    pub fn point(&self, i: usize) -> Vec3 {
        (self.origin + self.points[i]).as_vec3()
    }

    /// How fast its points went in the last frame, on average (m/s):
    /// whether it has settled.
    pub fn stir(&self) -> f32 {
        self.stir
    }

    /// How many points it has.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Where its mesh is drawn from (its points are from here: its entity
    /// is to be put here).
    pub fn origin(&self) -> Vec3 {
        self.origin.as_vec3()
    }

    /// Its mesh (to be kept up with `draw`; drawn from its origin).
    pub fn mesh(&self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_indices(Indices::U32(self.triangles.clone()));
        self.draw(&mut mesh);
        mesh
    }

    /// Where its points are now, into its mesh, with smooth normals.
    pub fn draw(&self, mesh: &mut Mesh) {
        // (From its origin as a float: its entity is put there.)
        let off = self.origin - self.origin.as_vec3().as_dvec3();
        let points: Vec<Vec3> = self.points.iter().map(|p| (*p + off).as_vec3()).collect();
        let mut normals = vec![Vec3::ZERO; points.len()];
        for t in self.triangles.chunks(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (points[b] - points[a]).cross(points[c] - points[a]);
            normals[a] += n;
            normals[b] += n;
            normals[c] += n;
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, points.iter().map(|p| p.to_array()).collect::<Vec<_>>());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.iter().map(|n| n.normalize_or(Vec3::Y).to_array()).collect::<Vec<_>>());
    }
}

/// A hinge's angle (0 flat; signed), and how it changes as each of its
/// points moves (Bridson et al. 2003, "Simulation of clothing with folds and
/// wrinkles"): its edge `q[0]`-`q[1]`, its wings `q[2]` and `q[3]`.
fn hinge(p: &[DVec3], q: [u32; 4]) -> (f64, [DVec3; 4]) {
    let (x3, x4, x1, x2) = (p[q[0] as usize], p[q[1] as usize], p[q[2] as usize], p[q[3] as usize]);
    let e = x4 - x3;
    let el = e.length();
    let n1 = (x1 - x3).cross(x1 - x4);
    let n2 = (x2 - x4).cross(x2 - x3);
    let (l1, l2) = (n1.length_squared(), n2.length_squared());
    if el < 1e-12 || l1 < 1e-24 || l2 < 1e-24 {
        return (0.0, [DVec3::ZERO; 4]);
    }
    let (m1, m2) = (n1 / l1, n2 / l2);
    // (Its sign the gradients' own: see the test.)
    let theta = -(n1.cross(n2).dot(e) / el).atan2(n1.dot(n2));
    let ehat = e / el;
    let u1 = m1 * el;
    let u2 = m2 * el;
    let u3 = m1 * (x1 - x4).dot(ehat) + m2 * (x2 - x4).dot(ehat);
    let u4 = -(m1 * (x1 - x3).dot(ehat) + m2 * (x2 - x3).dot(ehat));
    // (In the order of `q`: edge, edge, wing, wing.)
    (theta, [u3, u4, u1, u2])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hinge's gradients are its angle's (by finite differences).
    #[test]
    fn hinge_gradients() {
        let p = vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 1.0, 0.1), DVec3::new(0.8, 0.4, 0.3), DVec3::new(-0.7, 0.5, 0.45)];
        let q = [0, 1, 2, 3];
        let (theta, grads) = hinge(&p, q);
        for k in 0..4 {
            for axis in 0..3 {
                let mut moved = p.clone();
                moved[k][axis] += 1e-6;
                let d = (hinge(&moved, q).0 - theta) / 1e-6;
                assert!((d - grads[k][axis]).abs() < 1e-4, "point {k} axis {axis}: {d} vs {}", grads[k][axis]);
            }
        }
    }

    /// A sheet held by two corners hangs, unstretched, and settles still.
    #[test]
    fn hangs_and_settles() {
        let mut f = Fabric::sheet(12, 12, |c, r| Vec3::new(c as f32 * 0.05, 1.0, r as f32 * 0.05), Weave::default());
        let held = [(0u32, f.point(0)), (11u32, f.point(11))];
        f.tether(&[0, 11], 1.0);
        for _ in 0..600 {
            f.step(&held, &[], -10.0, 1.0 / 60.0);
        }
        let worst = f.edges.iter().map(|&(a, b, l)| f.points[a as usize].distance(f.points[b as usize]) / l).fold(0.0f64, f64::max);
        assert!(worst < 1.1, "stretched to {worst}");
        let fastest = f.velocity.iter().map(|v| v.length()).fold(0.0f64, f64::max);
        assert!(fastest < 0.05, "still moving at {fastest} m/s");
    }

    /// A wide sheet hung by its top, out in the world (1200 m out), hangs
    /// still (rigid, it shook as if in a gale: see `Weave`).
    #[test]
    fn still_far_out() {
        let far = Vec3::new(1200.0, 30.0, 900.0);
        let mut f = Fabric::sheet(60, 40, |c, r| far + Vec3::new(c as f32 * 0.04, -(r as f32) * 0.04, 0.0), Weave { self_collide: false, ..Weave::default() });
        let held: Vec<(u32, Vec3)> = (0..60).map(|i| (i as u32, f.point(i))).collect();
        for _ in 0..600 {
            f.step(&held, &[], -100.0, 1.0 / 60.0);
        }
        let was = f.points.clone();
        f.step(&held, &[], -100.0, 1.0 / 60.0);
        let moved = f.points.iter().zip(&was).map(|(a, b)| a.distance(*b)).fold(0.0f64, f64::max) * 60.0;
        assert!(moved < 0.05, "moving at {moved} m/s");
    }
}


