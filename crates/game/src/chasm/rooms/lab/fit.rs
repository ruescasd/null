//! Rooms built to fit their doors (the network's places, `plan/network.rs`):
//! given a footprint, a height and doors (which wall, where along it, at
//! what height), a room whose floors join every door. Its main floor at its
//! lowest door; each door higher up opens onto a landing, and from it
//! flights go round the walls down to the main floor, a quarter of a metre
//! every half metre (the stepwell's), level round the corners; each way
//! round tried, the one that blocks no other door and keeps headroom under
//! every other flight taken. No character yet: the circulation first.

use bevy::prelude::*;

use super::{B, Detail, Room, T, strength};

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
    let d = Detail { reveals: true, ..Default::default() };
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
    // Light: a lamp high in the middle, light back off the floor.
    let mid = Vec3::new(sx * 0.5, floor, sz * 0.5);
    let reach = sx.max(sz) + (ceiling - floor);
    b.lamp(mid + Vec3::Y * (ceiling - floor - 1.0), reach, strength(reach) * 2.0);
    b.bounce(mid + Vec3::Y, reach);
    // Views: from high in a corner, across the whole of it; from inside each
    // door, looking across the room.
    b.view("over", Vec3::new(T + 0.5, ceiling - 1.0, T + 0.5), Vec3::new(sx * 0.6, floor + 1.0, sz * 0.6));
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
