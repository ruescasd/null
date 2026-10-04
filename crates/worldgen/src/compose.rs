//! Composed complexes: architecture from a kit of walkable units repeated
//! at the same scale, not from fractals. The ground is a grid of bays
//! (`BAY` metres) and levels (`LEVEL` metres, ten walkable steps). From a
//! root platform the complex grows by operations off its edges:
//!
//! - a terrace: a platform a level up or down, with a stair at the
//!   junction;
//! - a raised platform two levels up on pillars, arches between them,
//!   reached by a two-flight stair;
//! - a bridge across a gap to a platform at the same level, an arch
//!   beneath.
//!
//! Operations repeat in runs (a flight of terraces climbing: rhythm), and
//! each copy may mutate (another operation, a turn, another size). The
//! result can be mirrored. Then the details that make it walkable and
//! legible: parapets along every drop (open where a stair arrives), and
//! porticos of columns on some terraces. Every stair joins two platforms,
//! so whatever grows can be walked.

use std::collections::{HashMap, HashSet};

use glam::{Quat, Vec2, Vec3};

use crate::noise::hash01;
use crate::structure::Solid;

/// A bay (metres).
pub const BAY: f32 = 6.0;
/// A level (metres): ten steps of 0.45.
pub const LEVEL: f32 = 4.5;
const STEPS: i32 = 10;
/// How far everything reaches into the ground.
const FOUNDATION: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// A solid platform.
    Ground,
    /// A deck on pillars.
    Raised,
    /// A flight of steps rising one level from `level`, towards `dir`.
    Stair { dir: (i32, i32) },
    /// A bridge deck (drawn when it is made).
    Bridge,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    level: i32,
    kind: Kind,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    level: i32,
    raised: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op {
    Up,
    Down,
    Raised,
    Bridge,
}

struct Composer {
    cells: HashMap<(i32, i32), Cell>,
    rects: Vec<Rect>,
    /// The grid's half extent, in bays.
    nx: i32,
    nz: i32,
    /// Solids made along the way (bridges), in grid metres.
    extra: Vec<Solid>,
    tone: f32,
}

/// The frame of a direction: along it, and across it.
fn to_frame(d: (i32, i32), x: i32, z: i32) -> (i32, i32) {
    (x * d.0 + z * d.1, -x * d.1 + z * d.0)
}

fn from_frame(d: (i32, i32), a: i32, c: i32) -> (i32, i32) {
    (a * d.0 - c * d.1, a * d.1 + c * d.0)
}

fn boxed(center: Vec3, half: Vec3, albedo: f32) -> Solid {
    Solid { wedge: false, round: false, center, rotation: Quat::IDENTITY, half, albedo }
}

/// The centre of a cell's floor at height `y` (grid metres).
fn cell_at(x: i32, z: i32, y: f32) -> Vec3 {
    Vec3::new((x as f32 + 0.5) * BAY, y, (z as f32 + 0.5) * BAY)
}

impl Composer {
    fn free(&self, x: i32, z: i32) -> bool {
        x.abs() < self.nx && z.abs() < self.nz && !self.cells.contains_key(&(x, z))
    }

    fn place(&mut self, r: Rect) -> bool {
        for x in r.x0..=r.x1 {
            for z in r.z0..=r.z1 {
                if !self.free(x, z) {
                    return false;
                }
            }
        }
        let kind = if r.raised { Kind::Raised } else { Kind::Ground };
        for x in r.x0..=r.x1 {
            for z in r.z0..=r.z1 {
                self.cells.insert((x, z), Cell { level: r.level, kind });
            }
        }
        self.rects.push(r);
        true
    }

    /// A rect given in a direction's frame: along `a0..=a1`, across
    /// `c0..=c1`.
    fn rect_in(d: (i32, i32), a0: i32, a1: i32, c0: i32, c1: i32, level: i32, raised: bool) -> Rect {
        let (p, q) = (from_frame(d, a0, c0), from_frame(d, a1, c1));
        Rect { x0: p.0.min(q.0), z0: p.1.min(q.1), x1: p.0.max(q.0), z1: p.1.max(q.1), level, raised }
    }

    /// A rect's extent in a direction's frame: (along min, along max,
    /// across min, across max).
    fn extent(d: (i32, i32), r: &Rect) -> (i32, i32, i32, i32) {
        let corners = [(r.x0, r.z0), (r.x1, r.z0), (r.x0, r.z1), (r.x1, r.z1)].map(|(x, z)| to_frame(d, x, z));
        let a = corners.iter().map(|c| c.0);
        let c = corners.iter().map(|c| c.1);
        (a.clone().min().unwrap(), a.max().unwrap(), c.clone().min().unwrap(), c.max().unwrap())
    }

    /// Whether a cell is a plain platform at `level` (a stair may go there).
    fn platform(&self, x: i32, z: i32, level: i32) -> bool {
        self.cells.get(&(x, z)).is_some_and(|c| c.level == level && matches!(c.kind, Kind::Ground | Kind::Raised))
    }

    /// Grows `op` off `r` towards `d`; the new rect, if it fits.
    fn grow(&mut self, r: Rect, op: Op, d: (i32, i32), r1: f32, r2: f32, r3: f32) -> Option<Rect> {
        let (amin, amax, cmin, cmax) = Self::extent(d, &r);
        let len = 2 + (r1 * 4.0) as i32;
        let width = 2 + (r2 * 4.0) as i32;
        let center = (cmin + cmax) / 2 + ((r3 - 0.5) * 3.0) as i32;
        let (c0, c1) = (center - width / 2, center - width / 2 + width - 1);
        // The stair's line, inside both rects across.
        let (o0, o1) = (c0.max(cmin), c1.min(cmax));
        if o0 > o1 {
            return None;
        }
        let cs = (o0 + o1) / 2;
        match op {
            Op::Up | Op::Down => {
                let level = r.level + if op == Op::Up { 1 } else { -1 };
                if level < 0 {
                    return None;
                }
                let new = Self::rect_in(d, amax + 1, amax + len, c0, c1, level, false);
                // The stair: in the lower rect, at the junction, rising
                // towards the higher.
                let (stair, dir, base) = if op == Op::Up {
                    (from_frame(d, amax, cs), d, r.level)
                } else {
                    (from_frame(d, amax + 1, cs), (-d.0, -d.1), level)
                };
                if op == Op::Up && (amax - amin < 1 || !self.platform(stair.0, stair.1, r.level)) {
                    return None;
                }
                if !self.place(new) {
                    return None;
                }
                if op == Op::Down && new.x1 - new.x0 + new.z1 - new.z0 < 2 {
                    return Some(new);
                }
                self.cells.insert(stair, Cell { level: base, kind: Kind::Stair { dir } });
                Some(new)
            }
            Op::Raised => {
                if amax - amin < 2 {
                    return None;
                }
                let (s1, s2) = (from_frame(d, amax - 1, cs), from_frame(d, amax, cs));
                if !self.platform(s1.0, s1.1, r.level) || !self.platform(s2.0, s2.1, r.level) {
                    return None;
                }
                let new = Self::rect_in(d, amax + 1, amax + len, c0, c1, r.level + 2, true);
                if !self.place(new) {
                    return None;
                }
                self.cells.insert(s1, Cell { level: r.level, kind: Kind::Stair { dir: d } });
                self.cells.insert(s2, Cell { level: r.level + 1, kind: Kind::Stair { dir: d } });
                Some(new)
            }
            Op::Bridge => {
                let gap = 2 + (r1 * 3.0) as i32;
                let new = Self::rect_in(d, amax + 1 + gap, amax + gap + len, c0, c1, r.level, false);
                let span: Vec<(i32, i32)> = (amax + 1..=amax + gap).map(|a| from_frame(d, a, cs)).collect();
                if span.iter().any(|&(x, z)| !self.free(x, z)) || !self.place(new) {
                    return None;
                }
                for &p in &span {
                    self.cells.insert(p, Cell { level: r.level, kind: Kind::Bridge });
                }
                self.bridge(span[0], *span.last().unwrap(), d, r.level);
                Some(new)
            }
        }
    }

    /// A bridge deck from cell `a` to cell `b` towards `d`, at `level`, with
    /// parapets and an arch beneath.
    fn bridge(&mut self, a: (i32, i32), b: (i32, i32), d: (i32, i32), level: i32) {
        let top = level as f32 * LEVEL;
        let (pa, pb) = (cell_at(a.0, a.1, 0.0) - Vec3::new(d.0 as f32, 0.0, d.1 as f32) * BAY * 0.5, cell_at(b.0, b.1, 0.0) + Vec3::new(d.0 as f32, 0.0, d.1 as f32) * BAY * 0.5);
        let along = Vec3::new(d.0 as f32, 0.0, d.1 as f32);
        let side = Vec3::new(-d.1 as f32, 0.0, d.0 as f32);
        let length = (pb - pa).length();
        let half = |l: f32, h: f32, w: f32| if d.0 != 0 { Vec3::new(l, h, w) } else { Vec3::new(w, h, l) };
        let mid = (pa + pb) * 0.5;
        let shade = self.tone + 0.03;
        self.extra.push(boxed(mid + Vec3::Y * (top - 0.6), half(length * 0.5, 0.6, BAY * 0.32), shade));
        for s in [-1.0, 1.0] {
            self.extra.push(boxed(mid + side * (s * BAY * 0.3) + Vec3::Y * (top + 0.55), half(length * 0.5, 0.55, 0.15), shade));
        }
        // The arch: strips from the deck down to a soffit falling to the
        // ends, which rest on the platforms.
        let strips = 16;
        let rise = (top - 1.2).min(length * 0.35).max(0.0);
        for k in 0..strips {
            let t = (k as f32 + 0.5) / strips as f32;
            let u = t * 2.0 - 1.0;
            let soffit = top - 1.2 - rise * (1.0 - u * u);
            let lo = soffit.min(top - 1.2);
            let c = pa + along * (length * t);
            self.extra.push(boxed(c + Vec3::Y * ((lo + top - 1.2) * 0.5), half(length / strips as f32 * 0.5 + 0.02, (top - 1.2 - lo) * 0.5 + 0.01, BAY * 0.3), shade));
        }
    }

    /// Draws the grid: platforms, decks on pillars with arches, stairs,
    /// parapets, porticos.
    fn draw(&self, porticos: &HashSet<usize>) -> Vec<Solid> {
        let mut out = self.extra.clone();
        let tone = self.tone;
        let top_of = |c: &Cell| match c.kind {
            Kind::Stair { .. } => (c.level + 1) as f32 * LEVEL,
            _ => c.level as f32 * LEVEL,
        };
        let mut pillars: HashSet<(i32, i32)> = HashSet::new();
        for (&(x, z), cell) in &self.cells {
            let top = cell.level as f32 * LEVEL;
            match cell.kind {
                Kind::Ground => {
                    out.push(boxed(cell_at(x, z, (top - FOUNDATION) * 0.5), Vec3::new(BAY * 0.5, (top + FOUNDATION) * 0.5, BAY * 0.5), tone));
                }
                Kind::Raised => {
                    out.push(boxed(cell_at(x, z, top - 0.7), Vec3::new(BAY * 0.5, 0.7, BAY * 0.5), tone + 0.02));
                    for (cx, cz) in [(x, z), (x + 1, z), (x, z + 1), (x + 1, z + 1)] {
                        pillars.insert((cx, cz));
                    }
                }
                Kind::Stair { dir } => {
                    // Solid up to the flight's base, then the steps.
                    if top > -FOUNDATION + 0.1 {
                        out.push(boxed(cell_at(x, z, (top - FOUNDATION) * 0.5), Vec3::new(BAY * 0.5, (top + FOUNDATION) * 0.5, BAY * 0.5), tone));
                    }
                    let along = Vec3::new(dir.0 as f32, 0.0, dir.1 as f32);
                    let run = BAY / STEPS as f32;
                    for k in 0..STEPS {
                        let h = LEVEL * (k + 1) as f32 / STEPS as f32;
                        let c = cell_at(x, z, top + h * 0.5) + along * (-BAY * 0.5 + run * (k as f32 + 0.5));
                        let half = if dir.0 != 0 { Vec3::new(run * 0.5 + 0.01, h * 0.5, BAY * 0.42) } else { Vec3::new(BAY * 0.42, h * 0.5, run * 0.5 + 0.01) };
                        out.push(boxed(c, half, tone + 0.04));
                    }
                }
                Kind::Bridge => {}
            }
            // Parapets along drops, open where a stair arrives or a bridge
            // or the same level continues.
            if matches!(cell.kind, Kind::Ground | Kind::Raised) {
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let open = match self.cells.get(&(x + dx, z + dz)) {
                        None => false,
                        Some(n) => match n.kind {
                            Kind::Stair { dir } => {
                                (dir == (-dx, -dz) && n.level + 1 == cell.level) || (dir == (dx, dz) && n.level == cell.level)
                            }
                            Kind::Bridge => n.level == cell.level,
                            _ => top_of(n) >= top - 0.01,
                        },
                    };
                    if open {
                        continue;
                    }
                    let c = cell_at(x, z, top + 0.55) + Vec3::new(dx as f32, 0.0, dz as f32) * (BAY * 0.5 - 0.2);
                    let half = if dx != 0 { Vec3::new(0.2, 0.55, BAY * 0.5) } else { Vec3::new(BAY * 0.5, 0.55, 0.2) };
                    out.push(boxed(c, half, tone + 0.05));
                }
            }
        }
        // Walls: every face of a platform over a drop is a wall of bays, one
        // per storey: pilasters at the bay's edges, an arch between them
        // under the storey line, a string course along each level.
        for (&(x, z), cell) in &self.cells {
            if cell.kind != Kind::Ground {
                continue;
            }
            let top = cell.level as f32 * LEVEL;
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let below = self.cells.get(&(x + dx, z + dz)).map_or(0.0, |n| top_of(n));
                let storeys = ((top - below) / LEVEL).round() as i32;
                if storeys < 1 {
                    continue;
                }
                let n = Vec3::new(dx as f32, 0.0, dz as f32);
                let along = Vec3::new(-dz as f32, 0.0, dx as f32);
                let face = cell_at(x, z, 0.0) + n * (BAY * 0.5);
                let sized = |a: f32, h: f32, d: f32| if dx != 0 { Vec3::new(d, h, a) } else { Vec3::new(a, h, d) };
                for k in 0..storeys {
                    let y0 = below + k as f32 * LEVEL;
                    let y1 = y0 + LEVEL;
                    // Pilasters at both edges of the bay.
                    for s in [-1.0, 1.0] {
                        let c = face + along * (s * (BAY * 0.5 - 0.35)) + n * 0.25 + Vec3::Y * ((y0 + y1) * 0.5);
                        out.push(boxed(c, sized(0.35, LEVEL * 0.5, 0.25), tone + 0.04));
                    }
                    // The arch between them, standing proud of the wall.
                    let rad = BAY * 0.5 - 0.7;
                    let spring = y1 - 0.5 - rad;
                    for j in 0..8 {
                        let u0 = -rad + 2.0 * rad * j as f32 / 8.0;
                        let u1 = u0 + 2.0 * rad / 8.0;
                        let ui = if u0.abs() < u1.abs() { u0 } else { u1 };
                        let arc = spring + (rad * rad - ui * ui).max(0.0).sqrt();
                        let c = face + along * ((u0 + u1) * 0.5) + n * 0.2 + Vec3::Y * ((arc + y1) * 0.5);
                        out.push(boxed(c, sized((u1 - u0) * 0.5 + 0.01, (y1 - arc) * 0.5 + 0.01, 0.2), tone + 0.04));
                    }
                    // The string course at the storey line.
                    let c = face + n * 0.3 + Vec3::Y * (y1 - 0.15);
                    out.push(boxed(c, sized(BAY * 0.5, 0.15, 0.3), tone + 0.06));
                }
            }
        }
        // Pillars under the decks, and arches between them along the
        // decks' outer edges.
        for &(px, pz) in &pillars {
            // The lowest deck on this corner.
            let decks: Vec<i32> = [(px, pz), (px - 1, pz), (px, pz - 1), (px - 1, pz - 1)]
                .iter()
                .filter_map(|k| self.cells.get(k).filter(|c| c.kind == Kind::Raised).map(|c| c.level))
                .collect();
            let Some(&deck) = decks.iter().min() else { continue };
            let top = deck as f32 * LEVEL - 1.4;
            let p = Vec3::new(px as f32 * BAY, top * 0.5 - FOUNDATION * 0.5, pz as f32 * BAY);
            out.push(boxed(p, Vec3::new(0.6, (top + FOUNDATION) * 0.5, 0.6), tone + 0.01));
        }
        for (&(x, z), cell) in &self.cells {
            if cell.kind != Kind::Raised {
                continue;
            }
            let deck = cell.level as f32 * LEVEL - 1.4;
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if self.cells.get(&(x + dx, z + dz)).is_some_and(|n| n.kind == Kind::Raised) {
                    continue;
                }
                // An arch head under the deck along this edge.
                let edge = cell_at(x, z, 0.0) + Vec3::new(dx as f32, 0.0, dz as f32) * BAY * 0.5;
                let along = Vec3::new(-dz as f32, 0.0, dx as f32);
                let rad = BAY * 0.5 - 0.6;
                let spring = deck - rad - 0.5;
                for k in 0..8 {
                    let u0 = -rad + 2.0 * rad * k as f32 / 8.0;
                    let u1 = u0 + 2.0 * rad / 8.0;
                    let ui = if u0.abs() < u1.abs() { u0 } else { u1 };
                    let arc = spring + (rad * rad - ui * ui).max(0.0).sqrt();
                    let c = edge + along * ((u0 + u1) * 0.5) + Vec3::Y * ((arc + deck) * 0.5);
                    let half = if dx != 0 { Vec3::new(0.35, (deck - arc) * 0.5 + 0.01, (u1 - u0) * 0.5 + 0.01) } else { Vec3::new((u1 - u0) * 0.5 + 0.01, (deck - arc) * 0.5 + 0.01, 0.35) };
                    out.push(boxed(c, half, tone + 0.01));
                }
            }
        }
        // Porticos: a column at every bay corner round a terrace, a lintel
        // over them, one level tall.
        for (i, r) in self.rects.iter().enumerate() {
            if !porticos.contains(&i) || r.raised {
                continue;
            }
            let base = r.level as f32 * LEVEL;
            let h = LEVEL;
            let corners = |x: i32, z: i32| Vec3::new(x as f32 * BAY, 0.0, z as f32 * BAY);
            let mut ring = Vec::new();
            for x in r.x0..=r.x1 + 1 {
                ring.push((x, r.z0));
                ring.push((x, r.z1 + 1));
            }
            for z in r.z0 + 1..=r.z1 {
                ring.push((r.x0, z));
                ring.push((r.x1 + 1, z));
            }
            for (x, z) in ring {
                let p = corners(x, z);
                // Inset a little from the parapet line.
                let inset = Vec3::new(
                    if x == r.x0 { 0.8 } else if x == r.x1 + 1 { -0.8 } else { 0.0 },
                    0.0,
                    if z == r.z0 { 0.8 } else if z == r.z1 + 1 { -0.8 } else { 0.0 },
                );
                out.push(boxed(p + inset + Vec3::Y * (base + h * 0.5), Vec3::new(0.35, h * 0.5, 0.35), tone + 0.06));
            }
            // The lintel round the top.
            let (x0, x1) = (r.x0 as f32 * BAY + 0.8, (r.x1 + 1) as f32 * BAY - 0.8);
            let (z0, z1) = (r.z0 as f32 * BAY + 0.8, (r.z1 + 1) as f32 * BAY - 0.8);
            let y = base + h + 0.3;
            for z in [z0, z1] {
                out.push(boxed(Vec3::new((x0 + x1) * 0.5, y, z), Vec3::new((x1 - x0) * 0.5 + 0.4, 0.3, 0.45), tone + 0.06));
            }
            for x in [x0, x1] {
                out.push(boxed(Vec3::new(x, y, (z0 + z1) * 0.5), Vec3::new(0.45, 0.3, (z1 - z0) * 0.5 + 0.4), tone + 0.06));
            }
        }
        out
    }
}

/// Composes a complex `half` metres across (local x, z) from `origin`
/// (world, its base at the ground) turned to `dir`, in about `steps`
/// growth operations.
pub fn compose(origin: Vec3, dir: Vec2, half: Vec2, steps: u32, symmetric: bool, tone: f32, seed: u32) -> Vec<Solid> {
    let r = |a: i32, b: i32| hash01(a, b, 0xc0b, seed);
    let mut c = Composer {
        cells: HashMap::new(),
        rects: Vec::new(),
        nx: (half.x / BAY) as i32,
        nz: (half.y / BAY) as i32,
        extra: Vec::new(),
        tone,
    };
    if c.nx < 3 || c.nz < 3 {
        return Vec::new();
    }
    // The root: a platform in the middle, a few levels up.
    let (w, d) = (2 + (r(0, 0) * 3.0) as i32, 2 + (r(0, 1) * 3.0) as i32);
    let root = Rect { x0: -w, z0: -d, x1: w - 1, z1: d - 1, level: 3 + (r(0, 2) * 3.0) as i32, raised: false };
    c.place(root);
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    // Weighted towards climbing: the complex grows up as well as out.
    let ops = [Op::Up, Op::Up, Op::Up, Op::Down, Op::Raised, Op::Raised, Op::Bridge];
    let mut k = 0;
    let mut attempts = 0;
    while k < steps && attempts < steps * 20 {
        attempts += 1;
        let q = |j: i32| r(attempts as i32, j);
        // A run: one operation repeated off the newest rect in one
        // direction, each copy perhaps mutated.
        let from = c.rects[(q(0) * c.rects.len() as f32) as usize % c.rects.len()];
        let mut dir = dirs[(q(1) * 4.0) as usize % 4];
        let mut op = ops[(q(2) * ops.len() as f32) as usize % ops.len()];
        let run = 1 + (q(3) * q(3) * 4.0) as u32;
        let mut cur = from;
        for n in 0..run {
            let m = |j: i32| r(attempts as i32 * 16 + n as i32, 10 + j);
            if n > 0 && m(0) < 0.2 {
                op = ops[(m(1) * ops.len() as f32) as usize % ops.len()];
            }
            if n > 0 && m(2) < 0.15 {
                dir = (-dir.1, dir.0);
            }
            match c.grow(cur, op, dir, m(3), m(4), m(5)) {
                Some(next) => {
                    cur = next;
                    k += 1;
                }
                None => break,
            }
        }
    }
    // Mirrored across x = -0.5 (the root is symmetric about it).
    if symmetric {
        let cells: Vec<((i32, i32), Cell)> = c.cells.iter().map(|(&k, &v)| (k, v)).collect();
        for ((x, z), cell) in cells {
            let mx = -1 - x;
            if c.cells.contains_key(&(mx, z)) {
                continue;
            }
            let kind = match cell.kind {
                Kind::Stair { dir } => Kind::Stair { dir: (-dir.0, dir.1) },
                other => other,
            };
            c.cells.insert((mx, z), Cell { level: cell.level, kind });
        }
        let extra: Vec<Solid> = c
            .extra
            .iter()
            .map(|s| Solid { center: Vec3::new(-s.center.x, s.center.y, s.center.z), ..*s })
            .collect();
        c.extra.extend(extra);
        let rects: Vec<Rect> = c
            .rects
            .iter()
            .map(|r| Rect { x0: -1 - r.x1, x1: -1 - r.x0, ..*r })
            .collect();
        c.rects.extend(rects);
    }
    let porticos: HashSet<usize> = (0..c.rects.len()).filter(|&i| r(i as i32 % (c.rects.len() / if symmetric { 2 } else { 1 }).max(1) as i32, 99) < 0.3).collect();
    let solids = c.draw(&porticos);
    let rot = Quat::from_rotation_y((-dir.y).atan2(dir.x));
    solids
        .into_iter()
        .map(|s| Solid { center: origin + rot * s.center, rotation: rot * s.rotation, ..s })
        .collect()
}
