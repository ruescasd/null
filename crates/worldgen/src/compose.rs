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
/// The highest level anything reaches.
const MAX_LEVEL: i32 = 30;
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
    /// Kept empty: a court or a shaft, down to the ground.
    Void,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    level: i32,
    kind: Kind,
    /// The rect it belongs to (its walls share a style), or -1.
    rect: i32,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    level: i32,
    raised: bool,
    /// A tower: a stair winds up its faces from `base`.
    tower: bool,
    base: i32,
    /// 1: a court, 2: a shaft (a ring round a void).
    void: u8,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op {
    Up,
    Down,
    Raised,
    Bridge,
    /// A block standing several levels above its neighbour, a stair
    /// winding up its faces.
    Tower,
    /// A ring of terraces at the same level round an open court.
    Court,
    /// A narrow ring several levels up round a deep void.
    Shaft,
    /// From a high terrace, across a wide gap to a tower at its level.
    TowerBridge,
}

/// How a wall is built: all the bays of one face of a rect alike.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Wall {
    /// Arched openings a bay deep, a gallery behind them.
    Arcade,
    /// Two deep, tall, narrow openings per bay and storey.
    Slots,
    /// Pilasters the wall's full height, a cornice.
    Giant,
    /// Pilasters, an arch in relief and a string course per storey.
    Bays,
    /// Plain, a cornice.
    Blank,
}

/// How deep openings go into a wall (metres).
const DEPTH: f32 = 2.4;

struct Composer {
    cells: HashMap<(i32, i32), Cell>,
    rects: Vec<Rect>,
    /// The grid's half extent, in bays.
    nx: i32,
    nz: i32,
    /// Solids made along the way (bridges), in grid metres...
    extra: Vec<Solid>,
    /// ...and which cells and solids each bridge is.
    bridges: Vec<(Vec<(i32, i32)>, std::ops::Range<usize>)>,
    tone: f32,
    seed: u32,
    /// How central the rect being grown from is (1 at the centre, 0 at
    /// the edge).
    focus: f32,
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
    /// The style of a rect's wall facing `d`.
    fn wall(&self, rect: i32, d: (i32, i32)) -> Wall {
        let h = hash01(rect, d.0 * 3 + d.1 * 7 + 11, 0x3a1, self.seed);
        match h {
            h if h < 0.3 => Wall::Arcade,
            h if h < 0.5 => Wall::Slots,
            h if h < 0.7 => Wall::Giant,
            h if h < 0.85 => Wall::Bays,
            _ => Wall::Blank,
        }
    }

    /// How many storeys a platform's face drops to what is beyond it.
    fn drop(&self, x: i32, z: i32, d: (i32, i32)) -> i32 {
        let Some(cell) = self.cells.get(&(x, z)) else { return 0 };
        let top = cell.level as f32 * LEVEL;
        let below = self.cells.get(&(x + d.0, z + d.1)).map_or(0.0, |n| match n.kind {
            Kind::Stair { .. } => (n.level + 1) as f32 * LEVEL,
            Kind::Bridge => top,
            _ => n.level as f32 * LEVEL,
        });
        ((top - below) / LEVEL).round() as i32
    }

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
                self.cells.insert((x, z), Cell { level: r.level, kind, rect: self.rects.len() as i32 });
            }
        }
        self.rects.push(r);
        true
    }

    /// A rect given in a direction's frame: along `a0..=a1`, across
    /// `c0..=c1`.
    fn rect_in(d: (i32, i32), a0: i32, a1: i32, c0: i32, c1: i32, level: i32, raised: bool) -> Rect {
        let (p, q) = (from_frame(d, a0, c0), from_frame(d, a1, c1));
        Rect { x0: p.0.min(q.0), z0: p.1.min(q.1), x1: p.0.max(q.0), z1: p.1.max(q.1), level, raised, tower: false, base: level, void: 0 }
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
                self.cells.insert(stair, Cell { level: base, kind: Kind::Stair { dir }, rect: -1 });
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
                self.cells.insert(s1, Cell { level: r.level, kind: Kind::Stair { dir: d }, rect: -1 });
                self.cells.insert(s2, Cell { level: r.level + 1, kind: Kind::Stair { dir: d }, rect: -1 });
                Some(new)
            }
            Op::Tower => {
                let size = 2 + (r1 * 2.0) as i32;
                let level = (r.level + 2 + (r2 * 3.0 + self.focus * 6.0) as i32).min(MAX_LEVEL);
                let mut new = Self::rect_in(d, amax + 1, amax + size, cs - size / 2, cs - size / 2 + size - 1, level, false);
                new.tower = true;
                new.base = r.level;
                self.place(new).then_some(new)
            }
            Op::Court | Op::Shaft => {
                let inner = if op == Op::Shaft { 1 + (r1 * 2.0) as i32 } else { 2 + (r1 * 3.0) as i32 };
                let size = inner + 2;
                let level = if op == Op::Shaft { (r.level + 2 + (r2 * 4.0) as i32).min(MAX_LEVEL) } else { r.level };
                let mut new = Self::rect_in(d, amax + 1, amax + size, cs - size / 2, cs - size / 2 + size - 1, level, false);
                new.void = if op == Op::Shaft { 2 } else { 1 };
                for x in new.x0..=new.x1 {
                    for z in new.z0..=new.z1 {
                        if !self.free(x, z) {
                            return None;
                        }
                    }
                }
                let index = self.rects.len() as i32;
                for x in new.x0..=new.x1 {
                    for z in new.z0..=new.z1 {
                        let ring = x == new.x0 || x == new.x1 || z == new.z0 || z == new.z1;
                        let cell = if ring {
                            Cell { level, kind: Kind::Ground, rect: index }
                        } else {
                            Cell { level: 0, kind: Kind::Void, rect: -1 }
                        };
                        self.cells.insert((x, z), cell);
                    }
                }
                self.rects.push(new);
                Some(new)
            }
            Op::TowerBridge => {
                if r.level < 4 {
                    return None;
                }
                let gap = 3 + (r1 * 4.0) as i32;
                let size = 2 + (r2 * 2.0) as i32;
                let new = Self::rect_in(d, amax + 1 + gap, amax + gap + size, cs - size / 2, cs - size / 2 + size - 1, r.level, false);
                let span: Vec<(i32, i32)> = (amax + 1..=amax + gap).map(|a| from_frame(d, a, cs)).collect();
                if span.iter().any(|&(x, z)| !self.free(x, z)) || !self.place(new) {
                    return None;
                }
                for &p in &span {
                    self.cells.insert(p, Cell { level: r.level, kind: Kind::Bridge, rect: -1 });
                }
                let start = self.extra.len();
                self.bridge(span[0], *span.last().unwrap(), d, r.level);
                self.bridges.push((span, start..self.extra.len()));
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
                    self.cells.insert(p, Cell { level: r.level, kind: Kind::Bridge, rect: -1 });
                }
                let start = self.extra.len();
                self.bridge(span[0], *span.last().unwrap(), d, r.level);
                self.bridges.push((span, start..self.extra.len()));
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
    /// Covered passages: cells with an arched way through them at the
    /// bottom, along an axis, and the passage's floor.
    fn passages(&self) -> HashMap<(i32, i32), ((i32, i32), f32)> {
        let mut out = HashMap::new();
        for (i, r) in self.rects.iter().enumerate() {
            if r.raised || r.tower || r.void > 0 || hash01(i as i32, 5, 0x9a5, self.seed) > 0.45 {
                continue;
            }
            // Across the rect's shorter side, through its middle.
            let along_x = r.x1 - r.x0 <= r.z1 - r.z0;
            let axis = if along_x { (1, 0) } else { (0, 1) };
            let cells: Vec<(i32, i32)> = if along_x {
                let z = (r.z0 + r.z1) / 2;
                (r.x0..=r.x1).map(|x| (x, z)).collect()
            } else {
                let x = (r.x0 + r.x1) / 2;
                (r.z0..=r.z1).map(|z| (x, z)).collect()
            };
            let (first, last) = (cells[0], *cells.last().unwrap());
            let top = r.level as f32 * LEVEL;
            let ends = [self.drop(first.0, first.1, (-axis.0, -axis.1)), self.drop(last.0, last.1, axis)];
            if ends[0] < 2 || ends[0] != ends[1] {
                continue;
            }
            if !cells.iter().all(|c| self.cells.get(c).is_some_and(|c| c.rect == i as i32)) {
                continue;
            }
            let floor = top - ends[0] as f32 * LEVEL;
            for c in cells {
                out.insert(c, (axis, floor));
            }
        }
        out
    }

    fn draw(&self, porticos: &HashSet<usize>) -> Vec<Solid> {
        let mut out = self.extra.clone();
        let passages = self.passages();
        let tone = self.tone;
        let top_of = |c: &Cell| match c.kind {
            Kind::Stair { .. } => (c.level + 1) as f32 * LEVEL,
            _ => c.level as f32 * LEVEL,
        };
        let mut pillars: HashSet<(i32, i32)> = HashSet::new();
        for (&(x, z), cell) in &self.cells {
            let top = cell.level as f32 * LEVEL;
            match cell.kind {
                Kind::Ground if passages.contains_key(&(x, z)) => {
                    // Solid below the passage and above it; walls either
                    // side; an arched head.
                    let (axis, floor) = passages[&(x, z)];
                    let side = if axis.0 != 0 { Vec3::Z } else { Vec3::X };
                    let c = cell_at(x, z, 0.0);
                    out.push(boxed(c + Vec3::Y * ((floor - FOUNDATION) * 0.5), Vec3::new(BAY * 0.5, (floor + FOUNDATION) * 0.5, BAY * 0.5), tone));
                    let roof = floor + LEVEL;
                    out.push(boxed(c + Vec3::Y * ((roof + top) * 0.5), Vec3::new(BAY * 0.5, (top - roof) * 0.5, BAY * 0.5), tone));
                    let wall = 1.4;
                    for sgn in [-1.0, 1.0] {
                        let half = if axis.0 != 0 { Vec3::new(BAY * 0.5, LEVEL * 0.5, wall * 0.5) } else { Vec3::new(wall * 0.5, LEVEL * 0.5, BAY * 0.5) };
                        out.push(boxed(c + side * (sgn * (BAY * 0.5 - wall * 0.5)) + Vec3::Y * (floor + LEVEL * 0.5), half, tone));
                    }
                    let rad = BAY * 0.5 - wall;
                    for j in 0..8 {
                        let u0 = -rad + 2.0 * rad * j as f32 / 8.0;
                        let u1 = u0 + 2.0 * rad / 8.0;
                        let ui = if u0.abs() < u1.abs() { u0 } else { u1 };
                        let arc = roof - rad * 0.8 + 0.8 * (rad * rad - ui * ui).max(0.0).sqrt();
                        let half = if axis.0 != 0 { Vec3::new(BAY * 0.5, (roof - arc) * 0.5 + 0.01, (u1 - u0) * 0.5) } else { Vec3::new((u1 - u0) * 0.5, (roof - arc) * 0.5 + 0.01, BAY * 0.5) };
                        out.push(boxed(c + side * ((u0 + u1) * 0.5) + Vec3::Y * ((arc + roof) * 0.5), half, tone));
                    }
                }
                Kind::Ground => {
                    // Inset by the openings' depth on faces whose wall is
                    // open, so arcades and slots have real depth; the deck
                    // covers the gallery.
                    let mut lo = Vec2::splat(0.0);
                    let mut hi = Vec2::splat(BAY);
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if self.drop(x, z, (dx, dz)) >= 1 && matches!(self.wall(cell.rect, (dx, dz)), Wall::Arcade | Wall::Slots) {
                            match (dx, dz) {
                                (1, _) => hi.x -= DEPTH,
                                (-1, _) => lo.x += DEPTH,
                                (_, 1) => hi.y -= DEPTH,
                                _ => lo.y += DEPTH,
                            }
                        }
                    }
                    let (x0, z0) = (x as f32 * BAY, z as f32 * BAY);
                    let c = Vec3::new(x0 + (lo.x + hi.x) * 0.5, (top - FOUNDATION) * 0.5, z0 + (lo.y + hi.y) * 0.5);
                    out.push(boxed(c, Vec3::new((hi.x - lo.x) * 0.5, (top + FOUNDATION) * 0.5, (hi.y - lo.y) * 0.5), tone));
                    if lo != Vec2::ZERO || hi != Vec2::splat(BAY) {
                        out.push(boxed(cell_at(x, z, top - 0.5), Vec3::new(BAY * 0.5, 0.5, BAY * 0.5), tone));
                    }
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
                Kind::Bridge | Kind::Void => {}
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
        // Walls: every face of a platform over a drop is built in its
        // wall's style, bay by bay, storey by storey.
        for (&(x, z), cell) in &self.cells {
            if cell.kind != Kind::Ground {
                continue;
            }
            let top = cell.level as f32 * LEVEL;
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let mut storeys = self.drop(x, z, (dx, dz));
                if storeys < 1 {
                    continue;
                }
                let mut below = top - storeys as f32 * LEVEL;
                if let Some(&(axis, floor)) = passages.get(&(x, z))
                    && (axis == (dx, dz) || axis == (-dx, -dz))
                    && (below - floor).abs() < 0.1
                {
                    below += LEVEL;
                    storeys -= 1;
                    if storeys < 1 {
                        continue;
                    }
                }
                let n = Vec3::new(dx as f32, 0.0, dz as f32);
                let along = Vec3::new(-dz as f32, 0.0, dx as f32);
                let face = cell_at(x, z, 0.0) + n * (BAY * 0.5);
                let sized = |a: f32, h: f32, d: f32| if dx != 0 { Vec3::new(d, h, a) } else { Vec3::new(a, h, d) };
                // A piece of the face: `u0..u1` along it, `y0..y1`, `d0..d1`
                // in from the face (negative: proud of it).
                let mut piece = |u0: f32, u1: f32, y0: f32, y1: f32, d0: f32, d1: f32, shade: f32| {
                    if u1 - u0 < 0.01 || y1 - y0 < 0.01 {
                        return;
                    }
                    let c = face + along * ((u0 + u1) * 0.5) - n * ((d0 + d1) * 0.5) + Vec3::Y * ((y0 + y1) * 0.5);
                    out.push(boxed(c, sized((u1 - u0) * 0.5, (y1 - y0) * 0.5, (d1 - d0) * 0.5), shade));
                };
                let h = BAY * 0.5;
                match self.wall(cell.rect, (dx, dz)) {
                    Wall::Arcade => {
                        // Piers at the bay's edges, an arched head per storey,
                        // a floor per storey in the gallery behind.
                        let pier = 0.8;
                        for k in 0..storeys {
                            let (y0, y1) = (below + k as f32 * LEVEL, below + (k + 1) as f32 * LEVEL);
                            piece(-h, -h + pier, y0, y1, 0.0, DEPTH, tone + 0.03);
                            piece(h - pier, h, y0, y1, 0.0, DEPTH, tone + 0.03);
                            let rad = h - pier;
                            let spring = y1 - 0.6 - rad * 0.7;
                            for j in 0..10 {
                                let u0 = -rad + 2.0 * rad * j as f32 / 10.0;
                                let u1 = u0 + 2.0 * rad / 10.0;
                                let ui = if u0.abs() < u1.abs() { u0 } else { u1 };
                                let arc = (spring + 0.7 * (rad * rad - ui * ui).max(0.0).sqrt()).min(y1);
                                piece(u0 - 0.01, u1 + 0.01, arc, y1, 0.0, DEPTH, tone + 0.03);
                            }
                            if k > 0 {
                                piece(-h, h, y0 - 0.25, y0, 0.0, DEPTH, tone + 0.01);
                            }
                        }
                    }
                    Wall::Slots => {
                        for k in 0..storeys {
                            let (y0, y1) = (below + k as f32 * LEVEL, below + (k + 1) as f32 * LEVEL);
                            let (sill, head) = (y0 + 0.6, y1 - 0.9);
                            piece(-h, h, y0, sill, 0.0, DEPTH, tone + 0.02);
                            piece(-h, h, head, y1, 0.0, DEPTH, tone + 0.02);
                            for (u0, u1) in [(-h, -1.9), (-0.6, 0.6), (1.9, h)] {
                                piece(u0, u1, sill, head, 0.0, DEPTH, tone + 0.02);
                            }
                            if k > 0 {
                                piece(-h, h, y0 - 0.2, y0, 0.0, DEPTH, tone + 0.01);
                            }
                        }
                    }
                    Wall::Giant => {
                        for s in [-1.0, 1.0] {
                            let u = s * (h - 0.45);
                            piece(u - 0.45, u + 0.45, below, top - 0.8, -0.6, 0.0, tone + 0.05);
                        }
                        piece(-h, h, top - 0.8, top, -0.8, 0.0, tone + 0.06);
                    }
                    Wall::Bays => {
                        for k in 0..storeys {
                            let (y0, y1) = (below + k as f32 * LEVEL, below + (k + 1) as f32 * LEVEL);
                            piece(-h, -h + 0.7, y0, y1, -0.25, 0.0, tone + 0.04);
                            piece(h - 0.7, h, y0, y1, -0.25, 0.0, tone + 0.04);
                            let rad = h - 0.7;
                            let spring = y1 - 0.5 - rad;
                            for j in 0..8 {
                                let u0 = -rad + 2.0 * rad * j as f32 / 8.0;
                                let u1 = u0 + 2.0 * rad / 8.0;
                                let ui = if u0.abs() < u1.abs() { u0 } else { u1 };
                                let arc = spring + (rad * rad - ui * ui).max(0.0).sqrt();
                                piece(u0, u1, arc, y1, -0.2, 0.0, tone + 0.04);
                            }
                            piece(-h, h, y1 - 0.3, y1, -0.3, 0.0, tone + 0.06);
                        }
                    }
                    Wall::Blank => {
                        piece(-h, h, top - 0.6, top, -0.5, 0.0, tone + 0.05);
                    }
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
        // Courts: a cloister of columns along the ring's inner edge, a
        // lintel over them. Shafts: galleries cantilevered into the void
        // every other storey along the walls.
        for (i, r) in self.rects.iter().enumerate() {
            if r.void == 0 {
                continue;
            }
            let top = r.level as f32 * LEVEL;
            for x in r.x0..=r.x1 {
                for z in r.z0..=r.z1 {
                    if !self.cells.get(&(x, z)).is_some_and(|c| c.rect == i as i32 && c.kind == Kind::Ground) {
                        continue;
                    }
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if !self.cells.get(&(x + dx, z + dz)).is_some_and(|c| c.kind == Kind::Void) {
                            continue;
                        }
                        let n = Vec3::new(dx as f32, 0.0, dz as f32);
                        let along = Vec3::new(-dz as f32, 0.0, dx as f32);
                        let edge = cell_at(x, z, 0.0) + n * (BAY * 0.5);
                        let sized = |a: f32, h: f32, d: f32| if dx != 0 { Vec3::new(d, h, a) } else { Vec3::new(a, h, d) };
                        if r.void == 1 {
                            for s in [-0.5f32, 0.0] {
                                let c = edge - n * 0.7 + along * (s * BAY) + Vec3::Y * (top + LEVEL * 0.5);
                                out.push(boxed(c, Vec3::new(0.3, LEVEL * 0.5, 0.3), tone + 0.06));
                            }
                            let c = edge - n * 0.7 + Vec3::Y * (top + LEVEL + 0.3);
                            out.push(boxed(c, sized(BAY * 0.5 + 0.3, 0.3, 0.4), tone + 0.06));
                            // The roof over the walk.
                            let c = edge - n * (BAY * 0.5 - 0.35) + Vec3::Y * (top + LEVEL + 0.65);
                            out.push(boxed(c, sized(BAY * 0.5, 0.15, BAY * 0.5 - 0.3), tone + 0.04));
                        } else {
                            let mut k = 1;
                            while (k as f32) * LEVEL < top - 0.1 {
                                let y = k as f32 * LEVEL;
                                let c = edge + n * 0.9 + Vec3::Y * (y - 0.25);
                                out.push(boxed(c, sized(BAY * 0.5, 0.25, 0.9), tone + 0.05));
                                let c = edge + n * 1.7 + Vec3::Y * (y + 0.5);
                                out.push(boxed(c, sized(BAY * 0.5, 0.5, 0.1), tone + 0.05));
                                k += 2;
                            }
                        }
                    }
                }
            }
        }
        // Stairs winding up the towers' faces from their base to their top,
        // a landing at each corner.
        for (i, r) in self.rects.iter().enumerate() {
            if !r.tower || !(r.x0..=r.x1).all(|x| self.cells.get(&(x, r.z0)).is_some_and(|c| c.rect == i as i32)) {
                continue;
            }
            let (x0, x1) = (r.x0 as f32 * BAY, (r.x1 + 1) as f32 * BAY);
            let (z0, z1) = (r.z0 as f32 * BAY, (r.z1 + 1) as f32 * BAY);
            let w = 1.8;
            // The faces in turn, as (start, direction, length, outward).
            let faces = [
                (Vec3::new(x0, 0.0, z0 - w * 0.5), Vec3::X, x1 - x0, -Vec3::Z),
                (Vec3::new(x1 + w * 0.5, 0.0, z0), Vec3::Z, z1 - z0, Vec3::X),
                (Vec3::new(x1, 0.0, z1 + w * 0.5), -Vec3::X, x1 - x0, Vec3::Z),
                (Vec3::new(x0 - w * 0.5, 0.0, z1), -Vec3::Z, z1 - z0, -Vec3::X),
            ];
            let (mut y, end) = (r.base as f32 * LEVEL, r.level as f32 * LEVEL);
            let (rise, run) = (0.45, 0.5);
            let mut f = 0;
            while y < end - 0.01 && f < 64 {
                let (start, dir, len, _) = faces[f % 4];
                let steps = ((len - w) / run).floor().max(1.0) as i32;
                for k in 0..steps {
                    if y >= end - 0.01 {
                        break;
                    }
                    y = (y + rise).min(end);
                    let c = start + dir * (w * 0.5 + run * (k as f32 + 0.5)) + Vec3::Y * (y - 0.3);
                    let half = if dir.x != 0.0 { Vec3::new(run * 0.5 + 0.02, 0.3, w * 0.5) } else { Vec3::new(w * 0.5, 0.3, run * 0.5 + 0.02) };
                    out.push(boxed(c, half, tone + 0.05));
                }
                // A landing past the corner.
                let corner = start + dir * len + dir * (w * 0.5) + Vec3::Y * (y - 0.3);
                out.push(boxed(corner, Vec3::new(w * 0.5, 0.3, w * 0.5), tone + 0.05));
                f += 1;
            }
        }
        // Porticos: a column at every bay corner round a terrace, a lintel
        // over them, one level tall.
        for (i, r) in self.rects.iter().enumerate() {
            if !porticos.contains(&i) || r.raised {
                continue;
            }
            // Only on a terrace that was built whole (a mirrored one may have
            // found its cells taken).
            let whole = (r.x0..=r.x1).all(|x| {
                (r.z0..=r.z1).all(|z| {
                    self.cells.get(&(x, z)).is_some_and(|c| c.rect == i as i32 || matches!(c.kind, Kind::Stair { .. }))
                })
            });
            if !whole {
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
        bridges: Vec::new(),
        tone,
        seed,
        focus: 1.0,
    };
    if c.nx < 3 || c.nz < 3 {
        return Vec::new();
    }
    // The root: a platform in the middle, a few levels up.
    let (w, d) = (2 + (r(0, 0) * 3.0) as i32, 2 + (r(0, 1) * 3.0) as i32);
    let level = 3 + (r(0, 2) * 3.0) as i32;
    let root = Rect { x0: -w, z0: -d, x1: w - 1, z1: d - 1, level, raised: false, tower: false, base: level, void: 0 };
    c.place(root);
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    // Weighted towards climbing: the complex grows up as well as out.
    let mut k = 0;
    let mut attempts = 0;
    while k < steps && attempts < steps * 20 {
        attempts += 1;
        let q = |j: i32| r(attempts as i32, j);
        // A run: one operation repeated off the newest rect in one
        // direction, each copy perhaps mutated.
        let from = c.rects[(q(0) * c.rects.len() as f32) as usize % c.rects.len()];
        // Composition by position: towers, shafts and bridges at height
        // near the centre, courts between, terraces stepping down and
        // bridges out at the edges; growth mostly outwards.
        let mid = Vec2::new((from.x0 + from.x1 + 1) as f32, (from.z0 + from.z1 + 1) as f32) * 0.5;
        let t = (mid.x.abs() / c.nx as f32).max(mid.y.abs() / c.nz as f32).clamp(0.0, 1.0);
        c.focus = 1.0 - t;
        let mut dir = if q(1) < 0.6 && mid.length() > 0.5 {
            if mid.x.abs() > mid.y.abs() { (mid.x.signum() as i32, 0) } else { (0, mid.y.signum() as i32) }
        } else {
            dirs[(q(5) * 4.0) as usize % 4]
        };
        let pick = |h: f32| {
            let u = 1.0 - t;
            let weights = [
                (Op::Tower, 3.0 * u * u),
                (Op::Shaft, 1.5 * u),
                (Op::TowerBridge, 1.5 * u),
                (Op::Raised, 0.5 + u),
                (Op::Up, 0.4 + 1.5 * u),
                (Op::Court, 2.0 * (1.0 - (t - 0.5).abs() * 2.0).max(0.0) + 0.2),
                (Op::Down, 0.3 + 2.5 * t),
                (Op::Bridge, 0.2 + 1.5 * t),
            ];
            let total: f32 = weights.iter().map(|w| w.1).sum();
            let mut x = h * total;
            for (op, w) in weights {
                if x < w {
                    return op;
                }
                x -= w;
            }
            Op::Down
        };
        let mut op = pick(q(2));
        let run = 1 + (q(3) * q(3) * 4.0) as u32;
        let mut cur = from;
        for n in 0..run {
            let m = |j: i32| r(attempts as i32 * 16 + n as i32, 10 + j);
            if n > 0 && m(0) < 0.2 {
                op = pick(m(1));
            }
            if n > 0 && m(2) < 0.15 {
                dir = (-dir.1, dir.0);
            }
            // Towers are ends: only a bridge at height leaves one.
            if cur.tower && op != Op::TowerBridge {
                break;
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
        let rects_before = c.rects.len() as i32;
        let cells: Vec<((i32, i32), Cell)> = c.cells.iter().map(|(&k, &v)| (k, v)).collect();
        let mut mirrored: HashSet<(i32, i32)> = HashSet::new();
        for ((x, z), cell) in cells {
            let mx = -1 - x;
            if c.cells.contains_key(&(mx, z)) {
                continue;
            }
            mirrored.insert((mx, z));
            let kind = match cell.kind {
                Kind::Stair { dir } => Kind::Stair { dir: (-dir.0, dir.1) },
                other => other,
            };
            let rect = if cell.rect >= 0 { cell.rect + rects_before } else { -1 };
            c.cells.insert((mx, z), Cell { level: cell.level, kind, rect });
        }
        let mut extra: Vec<Solid> = Vec::new();
        for (span, range) in &c.bridges {
            if span.iter().all(|&(x, z)| mirrored.contains(&(-1 - x, z))) {
                extra.extend(c.extra[range.clone()].iter().map(|s| Solid { center: Vec3::new(-s.center.x, s.center.y, s.center.z), ..*s }));
            }
        }
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
