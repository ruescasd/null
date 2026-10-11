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

/// How a fitted hall's columns go (`--set order=N`): 0 a giant order (a
/// tenth of its height thick, five thicknesses apart), 1 the same in the
/// larger halls only, 2 slimmer (a twelfth), 3 in tiers a storey high, each
/// carrying an entablature joining it to its neighbours, the next tier
/// standing on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Giant,
    Large,
    Slim,
    Tiers,
}

impl Order {
    pub fn of(n: u8) -> Order {
        [Order::Giant, Order::Large, Order::Slim, Order::Tiers][(n as usize).min(3)]
    }
}

/// What a fitted room is (`--set kind=N`): 0 Gallery I, a galleried hall
/// (its columns as `Order` says); 1 a cascade where its doors allow (see
/// `cascade`), else Gallery I; 2 Gallery II, the same galleries and ways
/// round its walls about an open well instead of columns, lamps hanging
/// down it from an oculus, a basin at its foot. (Named as one kind and its
/// variant, so they read as close: the user, review 87.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Gallery,
    Cascade,
    GalleryWell,
}

impl Kind {
    pub fn of(n: u8) -> Kind {
        [Kind::Gallery, Kind::Cascade, Kind::GalleryWell][(n as usize).min(2)]
    }
}

/// How far a door's landing reaches either side of its middle (past its
/// reveals, 2 m: shorter, they overhung the landing's end).
const LANDING: f32 = 2.5;

/// A storey (m), as the network's.
const STOREY: f32 = 6.0;

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
        Some(self.edge(k, self.band(a, b)))
    }

    /// The strip a quarter metre wide along the side of a box (in the
    /// band) away from wall `k`.
    fn edge(&self, k: usize, (lo, hi): (Vec2, Vec2)) -> (Vec2, Vec2) {
        match k {
            0 => (Vec2::new(lo.x, hi.y - 0.25), hi),
            1 => (lo, Vec2::new(lo.x + 0.25, hi.y)),
            2 => (lo, Vec2::new(hi.x, lo.y + 0.25)),
            _ => (Vec2::new(hi.x - 0.25, lo.y), hi),
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
    let mut pieces = vec![(p - LANDING, p + LANDING, y)];
    let (mut at, mut top) = (p + dir * LANDING, y);
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
pub(in crate::chasm) fn fitted(spec: &Spec, order: Order, kind: Kind) -> Room {
    if kind == Kind::Cascade
        && let Some(room) = cascade(spec)
    {
        return room;
    }
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
            taken.push((p - LANDING, p + LANDING, y));
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
        let ((mut pieces, near), dir) = [(end, 1.0), (start, -1.0)]
            .map(|(p, dir)| {
                // (Its free end, away from the way down, part way into a
                // corner's square: on to fill the square, which is then
                // drawn as one; ended within it, the square's stepped slab
                // and parapet stood where the gallery was not.)
                let (mut g0, mut g1) = (start - LANDING, end + LANDING);
                for c in l.corners() {
                    for o in [-l.len(), 0.0, l.len()] {
                        let c = c + o;
                        if dir > 0.0 && g0 > c + 0.01 && g0 < c + BAND {
                            g0 = c;
                        }
                        if dir < 0.0 && g1 < c - 0.01 && g1 > c - BAND {
                            g1 = c;
                        }
                    }
                }
                let gallery: Piece = (g0, g1, y);
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
        let free_end = if dir > 0.0 { pieces[0].0 } else { pieces[0].1 };
        // (A gallery round past the loop's end, in two.)
        if pieces[0].1 > l.len() {
            let g = pieces[0];
            pieces[0] = (g.0, l.len(), y);
            pieces.push((0.0, g.1 - l.len(), y));
        }
        let p = if dir > 0.0 { end } else { start };
        // (The gallery's free end, away from the way down: under it the
        // slab steps back from the end too, and a parapet runs across it;
        // in a corner, the square sees to it.)
        let free = free_end.rem_euclid(l.len());
        let clear_of_end = |(a, c): (f32, f32), by: f32| -> (f32, f32) {
            let (f0, f1) = if dir > 0.0 { (free, free + by) } else { (free - by, free) };
            let (mut a, mut c) = (a, c);
            for o in [-l.len(), 0.0, l.len()] {
                let (f0, f1) = (f0 + o, f1 + o);
                if a < f1 && c > f0 {
                    if dir > 0.0 { a = a.max(f1) } else { c = c.min(f0) }
                }
            }
            (a, c)
        };
        if !l.by_corner(free) && y > floor + 0.6 {
            let (a, c) = if dir > 0.0 { (free, free + 0.25) } else { (free - 0.25, free) };
            let (lo, hi) = l.band(a.rem_euclid(l.len()), a.rem_euclid(l.len()) + (c - a));
            b.solid(Vec3::new(lo.x, y, lo.y), Vec3::new(hi.x, y + 1.0, hi.y));
        }
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
                    // Along the gallery, a parapet on its open edge (but
                    // where it is near the floor), and under it the slab
                    // stepped back twice into the wall (but low down).
                    // (The gallery's only: on a flight, a solid wall
                    // following its steps up read as the flight made
                    // thicker, not a parapet; and under a flight's corner
                    // landing the stepped slab hung like a stray box.)
                    let level = (top - y).abs() < 0.01;
                    if level
                        && top > floor + 0.6
                        && let Some((p, q)) = l.parapet(w[0], w[1])
                    {
                        b.solid(Vec3::new(p.x, top, p.y), Vec3::new(q.x, top + 1.0, q.y));
                    }
                    // (In the corners, the square drawn once, below.)
                    let mid = (w[0] + w[1]) * 0.5;
                    if level && !l.by_corner(mid) && top - SLAB - 1.5 > floor + CLEAR {
                        let k = l.wall(mid);
                        for (i, by) in [0.75, 1.5].into_iter().enumerate() {
                            let (a, c) = clear_of_end((w[0], w[1]), by);
                            if c - a < 0.01 {
                                continue;
                            }
                            let (p, q) = l.toward_wall(k, l.band(a, c), by);
                            let y1 = top - SLAB - 0.75 * i as f32;
                            b.solid(Vec3::new(p.x, y1 - 0.75, p.y), Vec3::new(q.x, y1, q.y));
                        }
                    }
                }
            }
        }
        // The corner squares the gallery takes: under each, the slab stepped
        // back from the sides the gallery does not go on across (a turn, an
        // L; an end, straight on to the wall; a landing alone, from both); a
        // parapet along each side nothing goes on across, gallery or flight.
        // (Each piece stepped back towards its own wall, the steps ran on
        // past the turn under the flight going on, or cut a notch where the
        // gallery ended; and a corner had no parapet.)
        let len = l.len();
        let covers = |q: &Piece, p: f32| [-len, 0.0, len].iter().any(|o| q.0 + o <= p && p <= q.1 + o);
        let at_level = |p: f32| pieces.iter().any(|q| covers(q, p) && (q.2 - y).abs() < 0.01);
        let goes_on = |p: f32| pieces.iter().any(|q| covers(q, p) && q.2 > y - 0.6);
        let corners = l.corners();
        for i in 1..=4 {
            let c = corners[i];
            if !(at_level(c - 0.25) || at_level((c + 0.25).rem_euclid(len))) {
                continue;
            }
            let (wa, wb) = (i - 1, i % 4);
            let square = l.band(c - BAND, c);
            let (on_a, on_b) = (at_level(c - BAND - 0.25), at_level((c + BAND + 0.25).rem_euclid(len)));
            if y - SLAB - 1.5 > floor + CLEAR {
                for (k, by) in [0.75, 1.5].into_iter().enumerate() {
                    let y1 = y - SLAB - 0.75 * k as f32;
                    let boxes = match (on_a, on_b) {
                        (true, true) => vec![l.toward_wall(wa, square, by), l.toward_wall(wb, square, by)],
                        (true, false) => vec![l.toward_wall(wa, square, by)],
                        (false, true) => vec![l.toward_wall(wb, square, by)],
                        (false, false) => vec![l.toward_wall(wb, l.toward_wall(wa, square, by), by)],
                    };
                    for (p, q) in boxes {
                        b.solid(Vec3::new(p.x, y1 - 0.75, p.y), Vec3::new(q.x, y1, q.y));
                    }
                }
            }
            if y > floor + 0.6 {
                // (The side across which wall A's band goes on is the one
                // away from wall B, and the other way round.)
                for (open, from) in [(!goes_on(c - BAND - 0.25), wb), (!goes_on((c + BAND + 0.25).rem_euclid(len)), wa)] {
                    if open {
                        let (p, q) = l.edge(from, square);
                        b.solid(Vec3::new(p.x, y, p.y), Vec3::new(q.x, y + 1.0, q.y));
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
    // enough, columns in the middle, clear of the band the ways take and of
    // the way in from any door on the floor, as `order` says (see `Order`).
    b.bands((T, sx - T), (T, sz - T), (floor, ceiling));
    b.coffers((T, sx - T), (T, sz - T), ceiling, half((sx.min(sz) - 2.0 * T) / 4.0).max(2.0));
    let (ox, oz) = ((T + BAND + 1.0, sx - T - BAND - 1.0), (T + BAND + 1.0, sz - T - BAND - 1.0));
    let least = if order == Order::Large { 16.0 } else { 10.0 };
    if kind == Kind::GalleryWell {
        tower(&mut b, spec, floor, ceiling);
    } else if !levels.is_empty() && ox.1 - ox.0 >= least && oz.1 - oz.0 >= least && ceiling - floor >= 10.0 {
        // (Where they stand: the giant order's grid, a tenth of the height
        // thick, five thicknesses apart; or the tiers'.)
        let giant = thickness(ceiling - floor, 10.0);
        let tiers = if order == Order::Tiers { ((ceiling - floor) / STOREY).round().max(1.0) as i32 } else { 1 };
        let tier = (ceiling - floor) / tiers as f32;
        // (In tiers, closer: three quarters of a tier apart.)
        let span = if tiers > 1 { half(tier * 0.75).max(3.0) } else { half(giant * 5.0).max(4.0) };
        let (nx, nz) = (((ox.1 - ox.0) / span).floor() as i32, ((oz.1 - oz.0) / span).floor() as i32);
        let dd = match order {
            Order::Slim => thickness(ceiling - floor, 12.0),
            Order::Tiers => thickness(tier, 9.0),
            _ => giant,
        };
        let mut posts = Vec::new();
        for i in 0..=nx {
            for j in 0..=nz {
                let (x, z) = (half(ox.0 + (ox.1 - ox.0) * i as f32 / nx.max(1) as f32), half(oz.0 + (oz.1 - oz.0) * j as f32 / nz.max(1) as f32));
                // (None in the way in from a door on the floor: 3 m either
                // side of it, the band's width and 4 m more into the room.)
                let reach = BAND + 4.0 + giant * 0.5;
                let in_the_way = spec.doors.iter().filter(|d| d.2 <= floor + 0.01).any(|&(wall, u, _)| match wall {
                    0 => x < T + reach && (z - u).abs() < 3.0 + giant * 0.5,
                    1 => x > sx - T - reach && (z - u).abs() < 3.0 + giant * 0.5,
                    2 => z < T + reach && (x - u).abs() < 3.0 + giant * 0.5,
                    _ => z > sz - T - reach && (x - u).abs() < 3.0 + giant * 0.5,
                });
                if !in_the_way {
                    posts.push((i, j, x, z));
                }
            }
        }
        // (Two tiers or more: each its own columns, an entablature on them
        // joining each to the next in its row and its file, the next tier
        // standing on it.)
        let beam = half(tier / 10.0).max(0.5);
        for t in 0..tiers {
            let (y0, y1) = (floor + tier * t as f32, floor + tier * (t + 1) as f32 - if tiers > 1 { beam } else { 0.0 });
            for &(_, _, x, z) in &posts {
                b.column(x, z, dd, (y0, y1));
            }
            if tiers > 1 && t + 1 < tiers {
                let w = dd * 0.5 + 0.25;
                for &(i, j, x, z) in &posts {
                    for &(i2, j2, x2, z2) in &posts {
                        if (i2, j2) == (i + 1, j) || (i2, j2) == (i, j + 1) {
                            b.solid(Vec3::new(x.min(x2) - w, y1, z.min(z2) - w), Vec3::new(x.max(x2) + w, y1 + beam, z.max(z2) + w));
                        }
                    }
                }
            }
        }
        b.note(format!("{}, {} columns {dd:.1} m thick", match order {
            Order::Giant => "a giant order".to_string(),
            Order::Large => "a giant order (the larger halls only)".to_string(),
            Order::Slim => "a slim giant order (a twelfth)".to_string(),
            Order::Tiers => format!("an order in {tiers} tiers, an entablature on each"),
        }, posts.len()));
    }
    let mid = Vec3::new(sx * 0.5, floor, sz * 0.5);
    light(&mut b, spec.size, floor, ceiling);
    // Views: from high in a corner, across the whole of it; from inside each
    // door, looking across the room.
    // (From inside the first door on the floor, up at the far side: where
    // you start.)
    if let Some(&(wall, u, _)) = spec.doors.iter().find(|d| d.2 <= floor + 0.01) {
        let (eye, far) = match wall {
            0 => (Vec3::new(T + 0.75, floor + 1.7, u), Vec3::new(sx - T - BAND, (floor + ceiling) * 0.55, sz - u)),
            1 => (Vec3::new(sx - T - 0.75, floor + 1.7, u), Vec3::new(T + BAND, (floor + ceiling) * 0.55, sz - u)),
            2 => (Vec3::new(u, floor + 1.7, T + 0.75), Vec3::new(sx - u, (floor + ceiling) * 0.55, sz - T - BAND)),
            _ => (Vec3::new(u, floor + 1.7, sz - T - 0.75), Vec3::new(sx - u, (floor + ceiling) * 0.55, T + BAND)),
        };
        b.view("floor", eye, far);
    }
    b.view("high", Vec3::new(sx * 0.5, ceiling - 1.5, T + BAND + 0.75), Vec3::new(sx * 0.5, floor + (ceiling - floor) * 0.3, sz - T - 2.0));
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

/// Gallery II's well: an oculus in the roof over its middle, a chain of
/// lamps hanging down it, a basin with a fountain at its foot.
fn tower(b: &mut B, spec: &Spec, floor: f32, ceiling: f32) {
    let (sx, sz) = (spec.size.x, spec.size.y);
    let (cx, cz) = (sx * 0.5, sz * 0.5);
    let side = sx.min(sz) - 2.0 * T;
    b.carve(Vec3::new(cx - 2.0, ceiling - 0.5, cz - 2.0), Vec3::new(cx + 2.0, spec.height, cz + 2.0));
    b.sky(Vec3::new(cx, spec.height, cz), 2.0, ceiling - floor + 10.0, strength(ceiling - floor + 10.0) * 4.0);
    let (top, low) = (ceiling - 2.0, floor + 4.0);
    let n = ((top - low) / 3.0).floor().max(1.0) as i32;
    b.solid(Vec3::new(cx - 0.05, low, cz - 0.05), Vec3::new(cx + 0.05, ceiling, cz + 0.05));
    for k in 0..=n {
        let y = low + (top - low) * k as f32 / n as f32;
        if k % 2 == 0 {
            b.lamp(Vec3::new(cx, y, cz), side * 0.6 + 6.0, strength(side * 0.6 + 6.0) * 0.5);
        } else {
            b.glow.push((Vec3::new(cx - 0.15, y - 0.15, cz - 0.15), Vec3::new(cx + 0.15, y + 0.15, cz + 0.15)));
        }
    }
    super::basin(b, (cx, cz), side, BAND, floor);
    b.note(format!("Gallery II: an open well, an oculus, {} lamps hanging down it, a basin at its foot", n + 1));
}

/// A level run or a flight of a cascade: from where to where along it
/// (from its high end), and how high its top is (a flight's at its start).
enum Run {
    Level(f32, f32, f32),
    Flight(f32, f32, f32),
}

/// A cascade fitted to its doors, if they allow one: along x or z, one way
/// or the other, the doors' sills only falling (doors at the high end, at
/// the low end, or along the sides at the height of the terrace there).
/// Its floor steps down in level terraces and full-width flights between
/// them (a quarter metre every half metre, at most two metres a flight,
/// terraces between 1.5 m deep at least), each door on a terrace 2.5 m deep
/// either side of it at least;
/// a flat coffered ceiling over it all, so it grows taller as it goes
/// down; pilasters along its walls where they are clear. None if the doors
/// rise somewhere along every way, there is no room for the flights, or
/// they are all on one level.
pub(in crate::chasm) fn cascade(spec: &Spec) -> Option<Room> {
    let (sx, sz) = (spec.size.x, spec.size.y);
    let ceiling = spec.height - SLAB;
    // Each way: along x or z (`axis`), from which end; where each door is
    // along it from the high end (end walls at 0 or the far end). The
    // longest way the doors allow.
    let mut best: Option<(usize, f32, f32, Vec<(f32, f32)>)> = None;
    for axis in [0usize, 2] {
        let len = if axis == 0 { sx - 2.0 * T } else { sz - 2.0 * T };
        let (lo_wall, hi_wall) = if axis == 0 { (0, 1) } else { (2, 3) };
        for dir in [1.0f32, -1.0] {
            let along = |wall: usize, u: f32| -> f32 {
                let t = if wall == lo_wall { 0.0 } else if wall == hi_wall { len } else { u - T };
                if dir > 0.0 { t } else { len - t }
            };
            let mut anchors: Vec<(f32, f32)> = spec.doors.iter().map(|&(w, u, y)| (along(w, u), y)).collect();
            anchors.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.total_cmp(&a.1)));
            let falls = anchors.windows(2).all(|p| p[1].1 <= p[0].1 + 0.01 && (p[1].0 - p[0].0 > 0.01 || (p[1].1 - p[0].1).abs() < 0.01));
            if falls && best.as_ref().is_none_or(|b| len > b.2) {
                best = Some((axis, dir, len, anchors));
            }
        }
    }
    let (axis, dir, len, anchors) = best?;
    // (Doors all on one level: a hall, not a cascade.)
    if anchors.iter().all(|a| (a.1 - anchors[0].1).abs() < 0.01) {
        return None;
    }
    // The profile: level runs at each door (2.5 m either side), flights
    // between, spread evenly with level terraces between them.
    let mut runs = Vec::new();
    let mut at = 0.0;
    let mut y = anchors[0].1;
    for (i, &(t, ya)) in anchors.iter().enumerate() {
        if ya < y - 0.01 {
            let room = t - 2.5 - at;
            let drop = y - ya;
            let k = (drop / 2.0).ceil().max(1.0);
            let each = (drop / k / 0.25).round() * 0.25;
            let run = each * 2.0;
            let spare = room - k * run;
            if spare < (k - 1.0) * 1.5 {
                return None;
            }
            let gap = spare / (k + 1.0);
            let mut p = at;
            let mut top = y;
            for _ in 0..k as i32 {
                runs.push(Run::Level(p, p + gap, top));
                p += gap;
                runs.push(Run::Flight(p, p + run, top));
                p += run;
                top = (top - each).max(ya);
            }
            at = p;
            y = ya;
        }
        let end = if i + 1 == anchors.len() { len } else { (t + 2.5).min(len) };
        runs.push(Run::Level(at, end.max(at), y));
        at = end.max(at);
    }
    let (top, bottom) = (anchors[0].1, anchors.last().unwrap().1);
    let d = Detail { reveals: true, coffers: true, bands: true, pilasters: true, ..Default::default() };
    let mut b = B::new(Vec3::new(sx, spec.height, sz), d);
    b.carve(Vec3::new(T, bottom, T), Vec3::new(sx - T, ceiling, sz - T));
    // (A box across the room from `t0` to `t1` along the way, up to `y`.)
    let across = |b: &mut B, t0: f32, t1: f32, y: f32| {
        if y <= bottom + 0.01 || t1 - t0 < 0.01 {
            return;
        }
        let (a, c) = if dir > 0.0 { (T + t0, T + t1) } else { (T + len - t1, T + len - t0) };
        if axis == 0 {
            b.solid(Vec3::new(a, bottom, T), Vec3::new(c, y, sz - T));
        } else {
            b.solid(Vec3::new(T, bottom, a), Vec3::new(sx - T, y, c));
        }
    };
    let mut flights = 0;
    for r in &runs {
        match *r {
            Run::Level(t0, t1, y) => across(&mut b, t0, t1, y),
            Run::Flight(t0, t1, y) => {
                flights += 1;
                let n = ((t1 - t0) / 0.5).round() as i32;
                for i in 0..n {
                    across(&mut b, t0 + i as f32 * 0.5, t0 + (i + 1) as f32 * 0.5, y - 0.25 * (i + 1) as f32);
                }
            }
        }
    }
    for &(wall, u, y) in &spec.doors {
        let (ax, n) = match wall {
            0 => (0, 0.0),
            1 => (0, sx - T),
            2 => (2, 0.0),
            _ => (2, sz - T),
        };
        b.door(ax, u - 1.0, 2.0, y, 2.5, n, T);
    }
    b.note(format!("{}: a cascade {sx:.0} by {sz:.0} m along {}, from {top:.0} m down to {bottom:.0} m in {flights} flights, full width; {} doors", spec.name, if axis == 0 { "x" } else { "z" }, spec.doors.len()));
    b.bands((T, sx - T), (T, sz - T), (bottom, ceiling));
    b.coffers((T, sx - T), (T, sz - T), ceiling, half((sx.min(sz) - 2.0 * T) / 4.0).max(2.0));
    b.rhythm((T, sx - T), (T, sz - T), (bottom, ceiling), half((ceiling - bottom) / 3.0).clamp(4.0, 8.0), None, false);
    let point = |t: f32, y: f32| {
        let p = if dir > 0.0 { T + t } else { T + len - t };
        if axis == 0 { Vec3::new(p, y, sz * 0.5) } else { Vec3::new(sx * 0.5, y, p) }
    };
    light(&mut b, spec.size, bottom, ceiling);
    b.view("floor", point(1.0, top + 1.7), point(len * 0.8, bottom + 1.0));
    b.view("high", point(len - 1.0, bottom + 1.7), point(len * 0.2, top + 3.0));
    Some(b.finish(bottom))
}

/// A fitted room's light (its own alone: the lab is at night): a lamp high
/// in the middle, a lamp on each wall at its middle two thirds of the way
/// up, and light thrown back off the floor, strong enough to fill it.
/// (Until review 87 the rooms were lit mostly by a sun 3 degrees up leaking
/// in through their walls; by their own lamps alone they were near black.)
fn light(b: &mut B, size: Vec2, floor: f32, ceiling: f32) {
    let (sx, sz) = (size.x, size.y);
    let mid = Vec3::new(sx * 0.5, floor, sz * 0.5);
    let high = ceiling - floor;
    let reach = sx.max(sz) + high;
    b.lamp(mid + Vec3::Y * (high - 1.0), reach, strength(reach) * 2.0);
    let y = floor + high * 0.66;
    let wall = sx.max(sz) * 0.9 + 4.0;
    for at in [Vec3::new(T + 0.4, y, sz * 0.5), Vec3::new(sx - T - 0.4, y, sz * 0.5), Vec3::new(sx * 0.5, y, T + 0.4), Vec3::new(sx * 0.5, y, sz - T - 0.4)] {
        b.lamp(at, wall, strength(wall) * 1.5);
    }
    b.fill.push((mid + Vec3::Y, reach, strength(reach) * 3.0));
}
