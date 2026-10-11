//! Rooms built to fit their doors (the network's places, `plan/network.rs`):
//! given a footprint, a height and doors (which wall, where along it, at
//! what height), a room whose floors join every door. Its main floor at its
//! lowest door; each door higher up opens onto a landing, and from it
//! flights go round the walls down to the main floor, a quarter of a metre
//! every half metre (the stepwell's), level round the corners; each way
//! round tried, the one that blocks no other door and keeps headroom under
//! every other flight taken. No character yet: the circulation first.

use bevy::prelude::*;

use super::{B, Detail, Room, T, half, strength, thickness};

/// What a room must fit: its footprint (x, z, in metres, its walls
/// included), its height, and its doors: which wall (0 the one at x = 0, 1
/// at the far x, 2 at z = 0, 3 at the far z), where along it (the door's
/// middle, from the wall's start), how high its sill is.
#[derive(Clone, Debug)]
pub(in crate::chasm) struct Spec {
    pub size: Vec2,
    pub height: f32,
    pub doors: Vec<(usize, f32, f32)>,
    pub name: String,
}

/// The band along the walls the landings and flights take (m).
const BAND: f32 = 2.5;
/// A landing's or flight's slab; and the headroom over it.
const SLAB: f32 = 1.0;
const CLEAR: f32 = 3.0;

/// The walls' inner faces as one loop, from the corner at (T, T) along x,
/// then z, then back along x, then z: how long it is, and where along it a
/// door on wall `w` at `u` lies.
struct Loop {
    w: f32,
    d: f32,
    size: Vec2,
}

impl Loop {
    fn len(&self) -> f32 {
        2.0 * (self.w + self.d)
    }

    fn corners(&self) -> [f32; 5] {
        [0.0, self.w, self.w + self.d, 2.0 * self.w + self.d, self.len()]
    }

    fn of_door(&self, wall: usize, u: f32) -> f32 {
        match wall {
            2 => u - T,
            1 => self.w + (u - T),
            3 => self.w + self.d + (self.size.x - T - u),
            _ => 2.0 * self.w + self.d + (self.size.y - T - u),
        }
    }

    /// The band (x0, z0, x1, z1) along the walls from `p0` to `p1` (within
    /// one wall).
    fn band(&self, p0: f32, p1: f32) -> (Vec2, Vec2) {
        let (w, d, s) = (self.w, self.d, self.size);
        let mid = (p0 + p1) * 0.5;
        if mid < w {
            (Vec2::new(T + p0, T), Vec2::new(T + p1, T + BAND))
        } else if mid < w + d {
            (Vec2::new(s.x - T - BAND, T + p0 - w), Vec2::new(s.x - T, T + p1 - w))
        } else if mid < 2.0 * w + d {
            (Vec2::new(s.x - T - (p1 - w - d), s.y - T - BAND), Vec2::new(s.x - T - (p0 - w - d), s.y - T))
        } else {
            (Vec2::new(T, s.y - T - (p1 - 2.0 * w - d)), Vec2::new(T + BAND, s.y - T - (p0 - 2.0 * w - d)))
        }
    }

    /// Which wall `p` lies along (as in `band`).
    fn wall(&self, p: f32) -> usize {
        if p < self.w {
            0
        } else if p < self.w + self.d {
            1
        } else if p < 2.0 * self.w + self.d {
            2
        } else {
            3
        }
    }

    /// The band from `lo` to `hi` (as `band` gives it) drawn in toward its
    /// wall by `by`, along wall `k`.
    fn toward_wall(&self, k: usize, (mut lo, mut hi): (Vec2, Vec2), by: f32) -> (Vec2, Vec2) {
        match k {
            0 => hi.y -= by,
            1 => lo.x += by,
            2 => lo.y += by,
            _ => hi.x -= by,
        }
        (lo, hi)
    }

    /// The parapet along the open edge of the band from `p0` to `p1` (within
    /// one wall), clear of the corners (where the band turns, its edges
    /// are the ways on); none if nothing is left.
    fn parapet(&self, p0: f32, p1: f32) -> Option<(Vec2, Vec2)> {
        let c = self.corners();
        let k = self.wall((p0 + p1) * 0.5);
        let (a, b) = (p0.max(c[k] + BAND), p1.min(c[k + 1] - BAND));
        if b - a < 0.01 {
            return None;
        }
        let band = self.band(a, b);
        let edge = self.toward_wall(k, band, 0.0);
        // (The strip a quarter metre wide at the band's open edge.)
        let (lo, hi) = edge;
        Some(match k {
            0 => (Vec2::new(lo.x, hi.y - 0.25), hi),
            1 => (lo, Vec2::new(lo.x + 0.25, hi.y)),
            2 => (lo, Vec2::new(hi.x, lo.y + 0.25)),
            _ => (Vec2::new(hi.x - 0.25, lo.y), hi),
        })
    }

    /// Whether `p` lies within a band's width of a corner (where the way
    /// round is level).
    fn by_corner(&self, p: f32) -> bool {
        self.corners().iter().any(|c| (p - c).abs() < BAND)
    }
}

/// A piece of the way round: from `p0` to `p1` along the loop (`p0` <
/// `p1`, within the loop's length or wrapping past it), its top at `y`.
type Piece = (f32, f32, f32);

/// Whether two pieces overlap along the loop (each wrapped round it).
fn overlap(a: &Piece, b: &Piece, len: f32) -> bool {
    [-len, 0.0, len].iter().any(|o| a.0 < b.1 + o - 0.01 && a.1 > b.0 + o + 0.01)
}

/// The way down from a door at `p` (on the loop), `y` high, to `floor`,
/// going `dir` round: its landing at the door and its pieces after it, a
/// half metre each; and how many pieces already there it comes too near
/// (overlapping, less than a slab and headroom above or below).
fn way_down(l: &Loop, p: f32, y: f32, floor: f32, dir: f32, taken: &[Piece]) -> (Vec<Piece>, usize) {
    let len = l.len();
    let wrap = |p: f32| p.rem_euclid(len);
    let mut pieces = vec![(p - 1.5, p + 1.5, y)];
    let (mut at, mut top) = (p + dir * 1.5, y);
    let mut gone = 0.0;
    while top > floor + 0.01 && gone < len {
        let next = at + dir * 0.5;
        let (a, b) = (at.min(next), at.max(next));
        if !l.by_corner(wrap((a + b) * 0.5)) {
            top -= 0.25;
        }
        pieces.push((a, b, top.max(floor)));
        at = next;
        gone += 0.5;
    }
    let near = pieces.iter().filter(|q| taken.iter().any(|t| overlap(q, t, len) && (q.2 - t.2).abs() < SLAB + CLEAR)).count();
    (pieces, if gone >= len { usize::MAX } else { near })
}

/// The room for `spec`.
pub(in crate::chasm) fn fitted(spec: &Spec) -> Room {
    let d = Detail { reveals: true, coffers: true, bands: true, ..Default::default() };
    let (sx, sz) = (spec.size.x, spec.size.y);
    let mut b = B::new(Vec3::new(sx, spec.height, sz), d);
    let l = Loop { w: sx - 2.0 * T, d: sz - 2.0 * T, size: spec.size };
    let floor = spec.doors.iter().map(|d| d.2).fold(f32::MAX, f32::min);
    let ceiling = spec.height - SLAB;
    b.carve(Vec3::new(T, floor, T), Vec3::new(sx - T, ceiling, sz - T));
    b.note(format!("{}: {sx:.0} by {sz:.0} m, {:.0} m high, its floor {floor:.0} m up; {} doors", spec.name, spec.height, spec.doors.len()));
    // The doors, and their landings on the loop (those at the floor on it
    // already, to keep clear).
    let mut taken: Vec<Piece> = Vec::new();
    for &(wall, u, y) in &spec.doors {
        let (axis, n) = match wall {
            0 => (0, 0.0),
            1 => (0, sx - T),
            2 => (2, 0.0),
            _ => (2, sz - T),
        };
        b.door(axis, u - 1.0, 2.0, y, 2.5, n, T);
        if y <= floor + 0.01 {
            let p = l.of_door(wall, u);
            taken.push((p - 1.5, p + 1.5, y));
        }
    }
    // The higher doors, a level at a time, highest first: a gallery along
    // the walls joining those at the level (the shorter way round them all),
    // and one way down from an end of it, the end and way round that come
    // near the fewest pieces already there.
    let mut levels: Vec<f32> = spec.doors.iter().map(|d| d.2).filter(|&y| y > floor + 0.01).collect();
    levels.sort_by(|a, b| b.total_cmp(a));
    levels.dedup();
    let mut blocked = 0;
    for &y in &levels {
        let mut ps: Vec<f32> = spec.doors.iter().filter(|d| d.2 == y).map(|d| l.of_door(d.0, d.1)).collect();
        ps.sort_by(f32::total_cmp);
        // (The arc round them all: from after the widest gap between two
        // to before it.)
        let gaps: Vec<f32> = (0..ps.len()).map(|i| if i + 1 < ps.len() { ps[i + 1] - ps[i] } else { ps[0] + l.len() - ps[i] }).collect();
        let widest = (0..ps.len()).max_by(|&a, &b| gaps[a].total_cmp(&gaps[b])).unwrap();
        let start = ps[(widest + 1) % ps.len()];
        let end = start + (l.len() - gaps[widest]);
        let gallery: Piece = (start - 1.5, end + 1.5, y);
        let ((mut pieces, near), dir) = [(end, 1.0), (start, -1.0)]
            .map(|(p, dir)| {
                let (mut way, near) = way_down(&l, p, y, floor, dir, &taken);
                way[0] = gallery;
                let near = if taken.iter().any(|t| overlap(&gallery, t, l.len()) && (y - t.2).abs() < SLAB + CLEAR) { near.saturating_add(1) } else { near };
                ((way, near), dir)
            })
            .into_iter()
            .min_by_key(|((_, near), _)| *near)
            .unwrap();
        if near > 0 {
            blocked += 1;
        }
        // (A gallery round past the loop's end, in two.)
        if pieces[0].1 > l.len() {
            let g = pieces[0];
            pieces[0] = (g.0, l.len(), y);
            pieces.push((0.0, g.1 - l.len(), y));
        }
        let p = if dir > 0.0 { end } else { start };
        for &(a, c, top) in &pieces {
            for (a, c) in [(a, c), (a - l.len(), c - l.len()), (a + l.len(), c + l.len())] {
                let (a, c) = (a.max(0.0), c.min(l.len()));
                if c - a < 0.01 {
                    continue;
                }
                // (Split where a corner falls within it.)
                let mut cuts = vec![a];
                cuts.extend(l.corners().iter().copied().filter(|k| *k > a + 0.01 && *k < c - 0.01));
                cuts.push(c);
                for w in cuts.windows(2) {
                    let (lo, hi) = l.band(w[0], w[1]);
                    b.solid(Vec3::new(lo.x, (top - SLAB).max(floor), lo.y), Vec3::new(hi.x, top, hi.y));
                    // A parapet along its open edge (but where it is near
                    // the floor); under the level pieces, the slab stepped
                    // back twice into the wall (but low down).
                    if top > floor + 0.6
                        && let Some((p, q)) = l.parapet(w[0], w[1])
                    {
                        b.solid(Vec3::new(p.x, top, p.y), Vec3::new(q.x, top + 1.0, q.y));
                    }
                    let level = l.by_corner((w[0] + w[1]) * 0.5) || (top - y).abs() < 0.01;
                    if level && top - SLAB - 1.5 > floor + CLEAR {
                        let k = l.wall((w[0] + w[1]) * 0.5);
                        for (i, by) in [0.75, 1.5].into_iter().enumerate() {
                            let (p, q) = l.toward_wall(k, (lo, hi), by);
                            let y1 = top - SLAB - 0.75 * i as f32;
                            b.solid(Vec3::new(p.x, y1 - 0.75, p.y), Vec3::new(q.x, y1, q.y));
                        }
                    }
                }
            }
        }
        taken.extend(pieces.iter().copied());
        b.note(format!("{} at {y:.0} m up, a gallery {:.0} m along, the way down {} round{}", if ps.len() > 1 { format!("{} doors", ps.len()) } else { "a door".into() }, end - start + 3.0, if dir > 0.0 { "one way" } else { "the other way" }, if near > 0 { ", BLOCKING" } else { "" }));
        let (lo, hi) = l.band(p.clamp(0.0, l.len() - 0.5), (p + 0.5).min(l.len()));
        let c = (lo + hi) * 0.5;
        b.lamp(Vec3::new(c.x, y + 2.8, c.y), 14.0, strength(14.0) * 0.6);
    }
    if blocked > 0 {
        b.note(format!("{blocked} ways down BLOCK a door or another way"));
    }
    // Bands round the walls, coffers in the ceiling; in a hall wide and tall
    // enough, a giant order of columns in the middle, clear of the band the
    // ways take (by the orders: a tenth of its height thick, five
    // thicknesses apart).
    b.bands((T, sx - T), (T, sz - T), (floor, ceiling));
    b.coffers((T, sx - T), (T, sz - T), ceiling, half((sx.min(sz) - 2.0 * T) / 4.0).max(2.0));
    let (ox, oz) = ((T + BAND + 1.0, sx - T - BAND - 1.0), (T + BAND + 1.0, sz - T - BAND - 1.0));
    if !levels.is_empty() && ox.1 - ox.0 >= 10.0 && oz.1 - oz.0 >= 10.0 && ceiling - floor >= 10.0 {
        let dd = thickness(ceiling - floor, 10.0);
        let span = half(dd * 5.0).max(4.0);
        let (nx, nz) = (((ox.1 - ox.0) / span).floor() as i32, ((oz.1 - oz.0) / span).floor() as i32);
        for i in 0..=nx {
            for j in 0..=nz {
                let (x, z) = (half(ox.0 + (ox.1 - ox.0) * i as f32 / nx.max(1) as f32), half(oz.0 + (oz.1 - oz.0) * j as f32 / nz.max(1) as f32));
                // (None in the way in from a door on the floor: 3 m either
                // side of it, the band's width and 4 m more into the room.)
                let reach = BAND + 4.0 + dd * 0.5;
                let in_the_way = spec.doors.iter().filter(|d| d.2 <= floor + 0.01).any(|&(wall, u, _)| match wall {
                    0 => x < T + reach && (z - u).abs() < 3.0 + dd * 0.5,
                    1 => x > sx - T - reach && (z - u).abs() < 3.0 + dd * 0.5,
                    2 => z < T + reach && (x - u).abs() < 3.0 + dd * 0.5,
                    _ => z > sz - T - reach && (x - u).abs() < 3.0 + dd * 0.5,
                });
                if !in_the_way {
                    b.column(x, z, dd, (floor, ceiling));
                }
            }
        }
        b.note(format!("a giant order, {} by {} columns {dd:.1} m thick", nx + 1, nz + 1));
    }
    // Light: a lamp high in the middle, light back off the floor.
    let mid = Vec3::new(sx * 0.5, floor, sz * 0.5);
    let reach = sx.max(sz) + (ceiling - floor);
    b.lamp(mid + Vec3::Y * (ceiling - floor - 1.0), reach, strength(reach) * 2.0);
    b.bounce(mid + Vec3::Y, reach);
    // Views: from high in a corner, across the whole of it; from inside each
    // door, looking across the room.
    b.view("high", Vec3::new(sx * 0.5, ceiling - 1.5, T + BAND + 0.75), Vec3::new(sx * 0.5, floor + (ceiling - floor) * 0.3, sz - T - 2.0));
    // (From inside the first door on the floor, up at the far side.)
    if let Some(&(wall, u, _)) = spec.doors.iter().find(|d| d.2 <= floor + 0.01) {
        let (eye, far) = match wall {
            0 => (Vec3::new(T + 0.75, floor + 1.7, u), Vec3::new(sx - T - BAND, (floor + ceiling) * 0.55, sz - u)),
            1 => (Vec3::new(sx - T - 0.75, floor + 1.7, u), Vec3::new(T + BAND, (floor + ceiling) * 0.55, sz - u)),
            2 => (Vec3::new(u, floor + 1.7, T + 0.75), Vec3::new(sx - u, (floor + ceiling) * 0.55, sz - T - BAND)),
            _ => (Vec3::new(u, floor + 1.7, sz - T - 0.75), Vec3::new(sx - u, (floor + ceiling) * 0.55, T + BAND)),
        };
        b.view("floor", eye, far);
    }
    for (k, &(wall, u, y)) in spec.doors.iter().enumerate() {
        let eye = match wall {
            0 => Vec3::new(T + 1.0, y + 1.7, u),
            1 => Vec3::new(sx - T - 1.0, y + 1.7, u),
            2 => Vec3::new(u, y + 1.7, T + 1.0),
            _ => Vec3::new(u, y + 1.7, sz - T - 1.0),
        };
        b.view(["door1", "door2", "door3", "door4", "door5", "door6"][k.min(5)], eye, mid + Vec3::Y * ((y - floor) * 0.6 + 2.0));
    }
    b.finish(floor)
}
