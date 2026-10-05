//! The first version of the cells' units (floors on columns), kept for
//! comparison in the unit lab while the units are reworked.
//!
//! What stands on a lattice cell's deck (see `lattice.rs`), composed with
//! the cell as its frame: the deck's rim is the border, kept as a walk
//! round the edge, and buildings are open stacks of floors rather than
//! boxes. Floor slabs with a ledge all round, columns at every bay's
//! corners, some bays enclosed as rooms and the rest open as loggias,
//! balconies out past the edges, railings wherever a floor ends, and a
//! stair up through each building to its roof.
//!
//! Kinds of cell: an open plaza with a pavilion; a court (a ring of
//! building round an open court); terraces stepping back from the rim; a
//! gallery (a long building along one side, open colonnade below); a
//! tower of balconies in the middle.
//!
//! Everything is on the composer's grid: 6 m bays, 4.5 m storeys, 0.45 m
//! steps, in the cell's own frame: the deck's top at y = 0, x and z
//! centred, x along the cell.

use glam::{Quat, Vec3};

use crate::noise::hash01;
use crate::structure::Solid;

const BAY: f32 = 6.0;
const STOREY: f32 = 4.5;
const SLAB: f32 = 0.5;
/// How far rooms stand back from a floor's edge: the ledge.
const LEDGE: f32 = 1.2;
/// How far balconies reach out past a floor's edge.
const BALCONY: f32 = 2.4;
const RAIL: f32 = 1.05;
const RAIL_THICK: f32 = 0.15;
/// The walk round the rim, in bays.
const RIM: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Plaza,
    Court,
    Terraces,
    Gallery,
    Tower,
}

/// A building: bays `i0..=i1` by `j0..=j1`, storeys `0..storeys` up from
/// the deck; the share of bays enclosed (`fill`, the ground storey's
/// separately), of outer bays with balconies, which sides get railings
/// (+x, -x, +z, -z: not where another building's floor carries on), and
/// the bay its stair climbs in.
#[derive(Clone, Copy, Debug)]
struct Block {
    i0: i32,
    i1: i32,
    j0: i32,
    j1: i32,
    storeys: i32,
    fill: f32,
    ground_fill: f32,
    balconies: f32,
    rails: [bool; 4],
    stair: Option<(i32, i32)>,
}

impl Block {
    fn new(i0: i32, i1: i32, j0: i32, j1: i32, storeys: i32) -> Self {
        Block { i0, i1, j0, j1, storeys, fill: 0.55, ground_fill: 0.3, balconies: 0.25, rails: [true; 4], stair: Some((i0, j0)) }
    }
}

struct Drawer {
    out: Vec<Solid>,
    /// The bay grid's origin (the -x, -z corner of bay 0, 0).
    x0: f32,
    z0: f32,
    tone: f32,
    seed: u32,
}

impl Drawer {
    fn span(&mut self, lo: Vec3, hi: Vec3, shade: f32) {
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        if (hi - lo).min_element() < 0.01 {
            return;
        }
        self.out.push(Solid { glow: 0.0,
            detail: false,
            wedge: false,
            round: false,
            center: (lo + hi) * 0.5,
            rotation: Quat::IDENTITY,
            half: (hi - lo) * 0.5,
            albedo: self.tone + shade,
        });
    }

    /// A fine piece (railings, steps): left out from afar.
    fn fine(&mut self, lo: Vec3, hi: Vec3, shade: f32) {
        self.span(lo, hi, shade);
        if let Some(s) = self.out.last_mut() {
            s.detail = true;
        }
    }

    fn x(&self, i: i32) -> f32 {
        self.x0 + i as f32 * BAY
    }

    fn z(&self, j: i32) -> f32 {
        self.z0 + j as f32 * BAY
    }

    fn r(&self, a: i32, b: i32, c: i32) -> f32 {
        hash01(a * 131 + c, b, 0xce11, self.seed)
    }

    /// A slab over bays `i0..=i1`, `j0..=j1` with its top at `y`, leaving
    /// out the bay `hole`.
    fn slab(&mut self, (i0, i1, j0, j1): (i32, i32, i32, i32), y: f32, hole: Option<(i32, i32)>) {
        let parts = match hole {
            Some((hi, hj)) => vec![(i0, hi - 1, j0, j1), (hi + 1, i1, j0, j1), (hi, hi, j0, hj - 1), (hi, hi, hj + 1, j1)],
            None => vec![(i0, i1, j0, j1)],
        };
        for (a0, a1, b0, b1) in parts {
            if a0 > a1 || b0 > b1 {
                continue;
            }
            let lo = Vec3::new(self.x(a0), y - SLAB, self.z(b0));
            let hi = Vec3::new(self.x(a1 + 1), y, self.z(b1 + 1));
            self.span(lo, hi, 0.04);
        }
    }

    /// A railing along a floor's edge from `a` to `b` (on the floor, `y`).
    fn rail(&mut self, a: Vec3, b: Vec3, y: f32) {
        let t = RAIL_THICK * 0.5;
        let along_x = (b.x - a.x).abs() > (b.z - a.z).abs();
        let pad = if along_x { Vec3::new(0.0, 0.0, t) } else { Vec3::new(t, 0.0, 0.0) };
        self.fine(Vec3::new(a.x, y, a.z) - pad, Vec3::new(b.x, y + RAIL, b.z) + pad, 0.07);
    }

    fn block(&mut self, b: Block) {
        let stair = b.stair.filter(|&(i, j)| (b.i0..=b.i1).contains(&i) && (b.j0..=b.j1).contains(&j));
        let (x0, x1, z0, z1) = (self.x(b.i0), self.x(b.i1 + 1), self.z(b.j0), self.z(b.j1 + 1));
        for s in 0..b.storeys {
            let (y0, y1) = (s as f32 * STOREY, (s + 1) as f32 * STOREY - SLAB);
            // Columns at the bays' corners.
            let col = 0.3;
            for i in b.i0..=b.i1 + 1 {
                for j in b.j0..=b.j1 + 1 {
                    // Every corner round the edge, every other inside.
                    let edge = i == b.i0 || i == b.i1 + 1 || j == b.j0 || j == b.j1 + 1;
                    if !edge && ((i - b.i0) % 2 != 0 || (j - b.j0) % 2 != 0) {
                        continue;
                    }
                    let (x, z) = (self.x(i), self.z(j));
                    let (x, z) = (x.clamp(x0 + col, x1 - col), z.clamp(z0 + col, z1 - col));
                    self.span(Vec3::new(x - col, y0, z - col), Vec3::new(x + col, y1, z + col), 0.02);
                }
            }
            // Rooms in some bays, set back from the floor's edge.
            let fill = if s == 0 { b.ground_fill } else { b.fill };
            for i in b.i0..=b.i1 {
                for j in b.j0..=b.j1 {
                    if Some((i, j)) == stair || self.r(i * 7 + s, j, 1) > fill {
                        continue;
                    }
                    let lo = Vec3::new(self.x(i) + if i == b.i0 { LEDGE } else { 0.0 }, y0, self.z(j) + if j == b.j0 { LEDGE } else { 0.0 });
                    let hi = Vec3::new(self.x(i + 1) - if i == b.i1 { LEDGE } else { 0.0 }, y1, self.z(j + 1) - if j == b.j1 { LEDGE } else { 0.0 });
                    self.span(lo, hi, 0.0);
                }
            }
            // The stair up to the next floor, along x in its bay.
            if let Some((si, sj)) = stair {
                let steps = (STOREY / 0.45).round() as i32;
                let run = (BAY - 0.6) / steps as f32;
                let width = BAY * 0.45;
                for k in 0..steps {
                    let xa = self.x(si) + 0.3 + run * k as f32;
                    let za = self.z(sj) + 0.4;
                    self.fine(Vec3::new(xa, y0, za), Vec3::new(xa + run + 0.02, y0 + 0.45 * (k + 1) as f32, za + width), 0.05);
                }
            }
            // The floor above (the roof at the top), open over the stair.
            let top = (s + 1) as f32 * STOREY;
            self.slab((b.i0, b.i1, b.j0, b.j1), top, stair);
            // Balconies out past the edges, and railings round the floor
            // above (not on the ground: the deck carries on).
            for (side, (dx, dz)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
                if !b.rails[side] {
                    continue;
                }
                let bays: Vec<(i32, i32)> = match (dx, dz) {
                    (1, _) => (b.j0..=b.j1).map(|j| (b.i1, j)).collect(),
                    (-1, _) => (b.j0..=b.j1).map(|j| (b.i0, j)).collect(),
                    (_, 1) => (b.i0..=b.i1).map(|i| (i, b.j1)).collect(),
                    _ => (b.i0..=b.i1).map(|i| (i, b.j0)).collect(),
                };
                for (i, j) in bays {
                    // The bay's edge on this side, from one end to the other.
                    let (a, e) = match (dx, dz) {
                        (1, _) => (Vec3::new(x1, 0.0, self.z(j)), Vec3::new(x1, 0.0, self.z(j + 1))),
                        (-1, _) => (Vec3::new(x0, 0.0, self.z(j)), Vec3::new(x0, 0.0, self.z(j + 1))),
                        (_, 1) => (Vec3::new(self.x(i), 0.0, z1), Vec3::new(self.x(i + 1), 0.0, z1)),
                        _ => (Vec3::new(self.x(i), 0.0, z0), Vec3::new(self.x(i + 1), 0.0, z0)),
                    };
                    let n = Vec3::new(dx as f32, 0.0, dz as f32);
                    let inward = -n * (RAIL_THICK * 0.5);
                    let balcony = s + 1 < b.storeys && self.r(i * 3 + side as i32, j * 5 + s, 2) < b.balconies;
                    if balcony {
                        // A slab out past the edge, inset from the bay's
                        // corners, railed round its three open sides.
                        let along = (e - a).normalize();
                        let (p, q) = (a + along * 0.6, e - along * 0.6);
                        let out = n * BALCONY;
                        self.span(p.min(q + out) + Vec3::Y * (top - SLAB), p.max(q + out) + Vec3::Y * top, 0.04);
                        self.rail(p + out, q + out, top);
                        self.rail(p, p + out, top);
                        self.rail(q, q + out, top);
                        // The floor's own railing either side of it.
                        self.rail(a + inward, p + inward, top);
                        self.rail(q + inward, e + inward, top);
                    } else {
                        self.rail(a + inward, e + inward, top);
                    }
                }
            }
        }
    }
}

/// How many bays fit across `half` metres either side of the middle.
fn bays(half: f32) -> i32 {
    ((2.0 * half / BAY).floor() as i32).max(3)
}

/// The kinds a cell may be, by its length in slots.
pub fn pick(len: usize, h: f32) -> Kind {
    let kinds: &[(Kind, f32)] = if len >= 2 {
        &[(Kind::Gallery, 3.0), (Kind::Court, 2.0), (Kind::Terraces, 1.5), (Kind::Plaza, 0.6)]
    } else {
        &[(Kind::Court, 2.0), (Kind::Terraces, 2.0), (Kind::Tower, 1.5), (Kind::Plaza, 1.0), (Kind::Gallery, 1.0)]
    };
    let total: f32 = kinds.iter().map(|k| k.1).sum();
    let mut x = h * total;
    for &(kind, w) in kinds {
        if x < w {
            return kind;
        }
        x -= w;
    }
    kinds[0].0
}

/// What stands on a deck `half` metres across (x along the cell), up to
/// `storeys` high.
pub fn unit(kind: Kind, half: (f32, f32), storeys: i32, tone: f32, seed: u32) -> Vec<Solid> {
    let (nx, nz) = (bays(half.0), bays(half.1));
    let mut d = Drawer { out: Vec::new(), x0: -(nx as f32) * BAY * 0.5, z0: -(nz as f32) * BAY * 0.5, tone, seed };
    let storeys = storeys.max(1);
    let r = |a: i32| hash01(a, 17, 0xce12, seed);
    // The buildable area inside the walk round the rim.
    let (a0, a1, b0, b1) = (RIM, nx - 1 - RIM, RIM, nz - 1 - RIM);
    if a1 - a0 < 3 || b1 - b0 < 3 {
        return d.out;
    }
    match kind {
        Kind::Plaza => {
            // A pavilion: open columns and a roof.
            let (ci, cj) = ((a0 + a1) / 2, (b0 + b1) / 2);
            let mut b = Block::new(ci - 1, ci + 1, cj - 1, cj, 1);
            b.fill = 0.0;
            b.ground_fill = 0.0;
            b.stair = None;
            d.block(b);
        }
        Kind::Court => {
            // A ring two or three bays deep round an open court.
            let depth = 2 + (r(1) * 2.0) as i32;
            let high = storeys;
            let low = (storeys - 1).max(1);
            let rings = [
                (a0, a1, b0, b0 + depth - 1, high, [true, true, false, true]),
                (a0, a1, b1 - depth + 1, b1, if r(2) < 0.5 { high } else { low }, [true, true, true, false]),
                (a0, a0 + depth - 1, b0 + depth, b1 - depth, low, [true, true, false, false]),
                (a1 - depth + 1, a1, b0 + depth, b1 - depth, low, [true, true, false, false]),
            ];
            for (k, &(i0, i1, j0, j1, s, rails)) in rings.iter().enumerate() {
                if i0 > i1 || j0 > j1 {
                    continue;
                }
                let mut b = Block::new(i0, i1, j0, j1, s);
                b.rails = rails;
                b.balconies = 0.3;
                b.stair = (k == 0).then_some((i0, j0));
                d.block(b);
            }
        }
        Kind::Terraces => {
            // Steps back from the rim: each a storey taller than the one in
            // front, its roof a terrace onto the next one's upper floor.
            let steps = storeys.clamp(2, 4);
            let deep = ((b1 - b0 + 1) / steps).max(1);
            for k in 0..steps {
                let j0 = b0 + k * deep;
                let j1 = if k == steps - 1 { b1 } else { j0 + deep - 1 };
                let mut b = Block::new(a0, a1, j0, j1, k + 1 + (storeys - steps).max(0) * (k == steps - 1) as i32);
                b.rails = [true, true, k == steps - 1, true];
                b.balconies = 0.15;
                b.stair = Some((a0 + (r(10 + k) * (a1 - a0) as f32) as i32, j0));
                d.block(b);
            }
        }
        Kind::Gallery => {
            // A long building along the back, an open colonnade below it,
            // balconies on its front.
            let depth = 2 + (r(3) * 2.0) as i32;
            let mut b = Block::new(a0, a1, b1 - depth + 1, b1, storeys);
            b.ground_fill = 0.0;
            b.balconies = 0.45;
            b.stair = Some((a0, b1));
            d.block(b);
            // Sometimes a lower one along the front.
            if r(4) < 0.5 && b1 - depth - b0 >= 4 {
                let mut f = Block::new(a0 + 2, a1 - 2, b0, b0 + 1, (storeys / 2).max(1));
                f.balconies = 0.2;
                f.stair = Some((a0 + 2, b0));
                d.block(f);
            }
        }
        Kind::Tower => {
            // A tower of balconies in the middle.
            let (ci, cj) = ((a0 + a1) / 2, (b0 + b1) / 2);
            let w = 2 + (r(5) * 2.0) as i32;
            let mut b = Block::new(ci - w + 1, ci + w - 1, cj - w + 1, cj + w - 1, storeys);
            b.fill = 0.6;
            b.balconies = 0.55;
            b.stair = Some((ci - w + 1, cj - w + 1));
            d.block(b);
        }
    }
    d.out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_stay_on_the_deck() {
        for kind in [Kind::Plaza, Kind::Court, Kind::Terraces, Kind::Gallery, Kind::Tower] {
            for seed in 0..4 {
                let half = (42.0, 42.0);
                let solids = unit(kind, half, 5, 0.15, seed);
                assert!(!solids.is_empty(), "{kind:?}");
                for s in &solids {
                    let lo = s.center - s.half;
                    let hi = s.center + s.half;
                    assert!(lo.y >= -SLAB - 0.01 && hi.y <= 5.0 * STOREY + RAIL + 0.01, "{kind:?} {s:?}");
                    assert!(lo.x >= -half.0 - 0.01 && hi.x <= half.0 + 0.01 && lo.z >= -half.1 - 0.01 && hi.z <= half.1 + 0.01, "{kind:?} {s:?}");
                }
            }
        }
    }
}
