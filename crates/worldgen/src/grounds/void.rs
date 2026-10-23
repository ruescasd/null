//! Cylinders in the void (the user's, after the chasm, 2026-10-23: a huge
//! void, no floor in sight, and out of it cylinders on a grid, as of some
//! vast machine of unknown origin; what matters is how they are joined).
//! Unlike the chasm, no way down to follow, no rooms to line up: a field to
//! cross however one likes.
//!
//! Its joints are defined once, so nothing can misalign (the chasm's
//! lesson: free geometry meeting free geometry went makeshift, an exception
//! at every seed's angle): cylinders on a square grid, all alike; datum
//! levels shared by the whole field; at a level a cylinder may wear a
//! collar, a ring of floor round it; and a span always runs collar to
//! collar, radial to both, its ends landing on them. Variety only by
//! leaving things out and by discrete choices, never by nudging off the
//! grid. Two recipes for the spans: the machine (along the grid's axes, at
//! a level) and the web (also across the diagonals, and up or down a level
//! as ramps).
//!
//! It stands high (the lab's flat ground, its floor, lies far below, lost in
//! the haze, as the chasm's: see the game's `look.rs`).

use std::collections::HashSet;

use glam::{DVec2, DVec3};

use super::{Builder, SPAN, rand};

/// The grid (m), the cylinders' radius (m) and sides, and the chance one is
/// missing.
const GRID: f64 = 150.0;
const RADIUS: f64 = 22.0;
const SIDES: usize = 32;
const MISSING: f64 = 0.1;
/// The datum levels: the lowest (m), how far apart (m), how many.
pub const LEVEL: (f64, f64, usize) = (1200.0, 40.0, 8);
/// A cylinder's top: at one of the levels, from this one up.
const TOP_FROM: usize = 3;
/// Collars: the chance of one at a level under a cylinder's top, how far
/// out from the cylinder (m), how thick (m).
const COLLAR: (f64, f64, f64) = (0.5, 8.0, 3.0);
/// Spans: the chance of one where two collars face each other, how wide
/// (m), how thick (m).
const SPAN_CHANCE: f64 = 0.6;
const DECK: (f64, f64) = (7.0, 2.5);

/// Which spans.
pub enum Recipe {
    Machine,
    Web,
}

/// What collars and spans are made as (the user, 2026-10-23: solid, the
/// rings read as hoops and the spans as bars, a rack of pipes; more
/// engineered, thin members leaving gaps and showing the forces at work,
/// but not human: our own logics, not a bridge's trusses):
/// - `Solid`: slabs.
/// - `Lattice`: a deck over an open frame, frames within its frames (the
///   fractal rule's, frames within frames).
/// - `Tension`: decks hung by bundled stays from nodes on the cylinders
///   above.
/// - `Branch`: decks carried on struts branching up out of the cylinders
///   below.
/// - `Mixed`: lattice collars, branching spans (the user: the lattice the
///   best for collars, the branches for bridges).
/// - `Meander`: mixed, a path along each collar's members darker than the
///   rest, a motif repeated round the ring (the user's).
pub enum Style {
    Solid,
    Lattice,
    Tension,
    Branch,
    Mixed,
    Meander,
}

/// A lattice collar's members, by bay (round the ring from its first post):
/// the chord along its foot; the post at its left; a stay from its corner
/// (0 top left, 1 top right, 2 foot right, 3 foot left) to its inner
/// frame's; the inner frame's edge from that corner to the next.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Member {
    Chord(usize),
    Post(usize),
    Stay(usize, usize),
    Inner(usize, usize),
}

/// Decks over members: how thick (m); the lattice's depth under its deck
/// (m) and its bays (m, about); members' and decks' tones.
const THIN: f64 = 1.5;
const FRAME: (f64, f64) = (8.0, 9.0);
const MEMBER: f32 = 0.13;
/// A meander's members, and the rest of its collar's (lighter than members
/// are, or the path hardly showed against them).
const PATH: f32 = 0.025;
const AROUND: f32 = 0.4;
const PLATE: f32 = 0.21;

pub fn build(b: &mut Builder, seed: u32, recipe: Recipe, style: Style) {
    let half = (SPAN * 0.5 / GRID).floor() as i32;
    let middle = b.centre;
    let centre = |i: i32, j: i32| middle + DVec2::new(i as f64, j as f64) * GRID;
    let r = |i: i32, j: i32, k: i32| rand(seed, i, j, 900 + k);
    let level = |k: usize| LEVEL.0 + LEVEL.1 * k as f64;
    let present = |i: i32, j: i32| i.abs() <= half && j.abs() <= half && r(i, j, 0) >= MISSING;
    let top = |i: i32, j: i32| TOP_FROM + (r(i, j, 1) * (LEVEL.2 - TOP_FROM) as f64) as usize;
    let collar = |i: i32, j: i32, k: usize| present(i, j) && k < top(i, j) && r(i, j, 10 + k as i32) < COLLAR.0;
    // The spans first: from each cylinder to its neighbours ahead (each
    // pair once), where both could wear collars at the levels it joins.
    let ahead: &[(i32, i32)] = match recipe {
        Recipe::Machine => &[(1, 0), (0, 1)],
        Recipe::Web => &[(1, 0), (0, 1), (1, 1), (1, -1)],
    };
    let mut spans = Vec::new();
    for j in -half..=half {
        for i in -half..=half {
            for (n, &(di, dj)) in ahead.iter().enumerate() {
                let (ni, nj) = (i + di, j + dj);
                for k in 0..LEVEL.2 {
                    // (The web: also a ramp from this level to the one
                    // above or below at the other end.)
                    let ends: &[i32] = match recipe {
                        Recipe::Machine => &[0],
                        Recipe::Web => &[0, 1, -1],
                    };
                    for &dk in ends {
                        let k2 = k as i32 + dk;
                        if k2 < 0 || k2 >= LEVEL.2 as i32 || !collar(i, j, k) || !collar(ni, nj, k2 as usize) {
                            continue;
                        }
                        // (Fewer ramps and diagonals than level spans.)
                        let chance = SPAN_CHANCE * if dk != 0 { 0.25 } else { 1.0 } * if di != 0 && dj != 0 { 0.5 } else { 1.0 };
                        if r(i, j, 100 + n as i32 * 40 + k as i32 * 3 + dk + 1) >= chance {
                            continue;
                        }
                        spans.push(((i, j, k), (ni, nj, k2 as usize)));
                    }
                }
            }
        }
    }
    // Collars only where a span lands (the user: those nothing reached, a
    // ring round a cylinder for no reason, cluttered the field).
    let reached: std::collections::HashSet<(i32, i32, usize)> = spans.iter().flat_map(|&(a, z)| [a, z]).collect();
    // The cylinders, out of the depths; their collars.
    for j in -half..=half {
        for i in -half..=half {
            if !present(i, j) {
                continue;
            }
            let c = centre(i, j);
            b.shade(seed, (i, j), 0.12, 0.015);
            b.prism(&ring(c, RADIUS), level(top(i, j)), -1.0, false);
            for k in 0..LEVEL.2 {
                if reached.contains(&(i, j, k)) {
                    ring_collar(b, &style, c, level(k), r(i, j, 60 + k as i32), &|n| r(i, j, 200 + k as i32 * 20 + n));
                }
            }
        }
    }
    for ((i, j, k), (ni, nj, k2)) in spans {
        span(b, &style, centre(i, j), level(k), centre(ni, nj), level(k2));
    }
}

/// A cylinder's outline (`SIDES` sides) round `c`.
fn ring(c: DVec2, radius: f64) -> Vec<DVec2> {
    (0..SIDES).map(|s| c + DVec2::from_angle(s as f64 * std::f64::consts::TAU / SIDES as f64) * radius).collect()
}

/// A point at `p` (in plan) and height `y`.
fn at(p: DVec2, y: f64) -> DVec3 {
    DVec3::new(p.x, y, p.y)
}

/// A collar round the cylinder at `c`, its top at `y`, in `style`; `turn`
/// (0..1) turns its members a little, so collars differ; `pick` its own
/// random numbers (a meander's).
fn ring_collar(b: &mut Builder, style: &Style, c: DVec2, y: f64, turn: f64, pick: &dyn Fn(i32) -> f64) {
    let path = if matches!(style, Style::Meander) { walk(pick) } else { HashSet::new() };
    let rest = if matches!(style, Style::Meander) { AROUND } else { MEMBER };
    let shade = |m: Member| if path.contains(&m) { PATH } else { rest };
    let style = if matches!(style, Style::Mixed | Style::Meander) { &Style::Lattice } else { style };
    // (Its deck in segments, its inner edge half a metre into the cylinder:
    // overlapping, never just touching it.)
    let thick = if matches!(style, Style::Solid) { COLLAR.2 } else { THIN };
    let (inner, outer) = (ring(c, RADIUS - 0.5), ring(c, RADIUS + COLLAR.1));
    b.tone = PLATE;
    for s in 0..SIDES {
        let t = (s + 1) % SIDES;
        b.prism(&[inner[s], inner[t], outer[t], outer[s]], y, y - thick, true);
    }
    let under = y - thick;
    let dir = DVec2::from_angle;
    let offset = turn * std::f64::consts::TAU / 16.0;
    b.tone = MEMBER;
    match style {
        // (Mixed is made as lattice or branch, above.)
        Style::Solid | Style::Mixed | Style::Meander => {}
        Style::Lattice => {
            // Its chord a frame's depth down, posts to it every other side,
            // in each bay a frame within the frame, held at its corners;
            // spokes back into the cylinder at the chord.
            // (A meander's members darker.)
            let low = y - FRAME.0;
            let rim = ring(c, RADIUS + COLLAR.1 - 0.8);
            for t in 0..SIDES / 2 {
                let (p, q) = (rim[2 * t], rim[(2 * t + 2) % SIDES]);
                b.tone = shade(Member::Chord(t));
                b.beam(at(p, low), at(q, low), 0.6, 0.6, 6);
                b.tone = shade(Member::Post(t));
                b.beam(at(p, under), at(p, low), 0.7, 0.7, 6);
                b.tone = rest;
                b.beam(at(c + (p - c).normalize() * (RADIUS - 0.5), low), at(p, low), 0.6, 0.6, 6);
                inner_frame(b, [at(p, under), at(q, under), at(q, low), at(p, low)], |k, stay| shade(if stay { Member::Stay(t, k) } else { Member::Inner(t, k) }));
            }
        }
        Style::Tension => {
            // Hung from nodes on the cylinder over it, each stay a bundle.
            for k in 0..8 {
                let a = offset + k as f64 * std::f64::consts::TAU / 8.0;
                let node = at(c + dir(a) * (RADIUS + 0.8), y + 22.0);
                b.beam(at(c + dir(a) * (RADIUS - 0.5), y + 22.0), node, 1.4, 1.1, 6);
                bundle(b, node, at(c + dir(a) * (RADIUS + COLLAR.1 - 0.6), y - thick * 0.5));
            }
        }
        Style::Branch => {
            // Carried on trees out of the cylinder under it: a trunk to a
            // node, branches from it to the deck's rim.
            for k in 0..8 {
                let a = offset + k as f64 * std::f64::consts::TAU / 8.0;
                let root = at(c + dir(a) * (RADIUS - 0.5), y - 24.0);
                let node = at(c + dir(a) * (RADIUS + 3.5), y - 10.0);
                b.beam(root, node, 1.1, 0.85, 7);
                for da in [-0.2, 0.0, 0.2] {
                    b.beam(node, at(c + dir(a + da) * (RADIUS + COLLAR.1 - 1.0), under + 0.3), 0.6, 0.35, 6);
                }
                // (A knot where they part.)
                b.beam(node - DVec3::Y * 0.8, node + DVec3::Y * 0.8, 1.3, 1.3, 7);
            }
        }
    }
}

/// A span from the cylinder at `a` (its deck at `ya`) to the one at `z` (at
/// `yz`), collar to collar, in `style`.
fn span(b: &mut Builder, style: &Style, a: DVec2, ya: f64, z: DVec2, yz: f64) {
    let style = if matches!(style, Style::Mixed | Style::Meander) { &Style::Branch } else { style };
    let along = (z - a).normalize();
    let side = along.perp();
    // (Into each collar a metre: its ends landing on them.)
    let reach = RADIUS + COLLAR.1 - 1.0;
    let (p, q) = (a + along * reach, z - along * reach);
    let thick = if matches!(style, Style::Solid) { DECK.1 } else { THIN };
    b.tone = PLATE;
    b.deck(p, ya, q, yz, DECK.0, thick);
    b.tone = MEMBER;
    let w = DECK.0 * 0.5 - 0.6;
    // A point along the deck (0 at `p`, 1 at `q`), so far aside, so far
    // under its top.
    let on = |t: f64, aside: f64, down: f64| at(p + (q - p) * t + side * aside, ya + (yz - ya) * t - down);
    match style {
        // (Mixed is made as lattice or branch, above.)
        Style::Solid | Style::Mixed | Style::Meander => {}
        Style::Lattice => {
            // Chords a frame's depth down each side, posts at each bay, in
            // each bay a frame within the frame; ties across under it.
            let bays = ((p.distance(q) / FRAME.1).round() as usize).max(2);
            for e in [-w, w] {
                b.beam(on(0.0, e, FRAME.0), on(1.0, e, FRAME.0), 0.6, 0.6, 6);
                for n in 0..=bays {
                    let t = n as f64 / bays as f64;
                    b.beam(on(t, e, thick), on(t, e, FRAME.0), 0.6, 0.6, 6);
                    if n < bays {
                        let u = (n + 1) as f64 / bays as f64;
                        inner_frame(b, [on(t, e, thick), on(u, e, thick), on(u, e, FRAME.0), on(t, e, FRAME.0)], |_, _| MEMBER);
                    }
                }
            }
            for n in 0..=bays {
                let t = n as f64 / bays as f64;
                b.beam(on(t, -w, FRAME.0), on(t, w, FRAME.0), 0.5, 0.5, 6);
            }
        }
        Style::Tension => {
            // From a node on each cylinder over the span's end, a fan of
            // bundled stays to its edges.
            for (end, from, y) in [(a, 1.0, ya), (z, -1.0, yz)] {
                let node = at(end + along * from * (RADIUS + 0.8), y + 30.0);
                b.beam(at(end + along * from * (RADIUS - 0.5), y + 30.0), node, 1.6, 1.2, 6);
                for f in [0.12, 0.24, 0.36] {
                    let t = if from > 0.0 { f } else { 1.0 - f };
                    for e in [-w, w] {
                        bundle(b, node, on(t, e, thick * 0.5));
                    }
                }
            }
        }
        Style::Branch => {
            // From each cylinder, under the span's end, a trunk to a node
            // under the deck; from it, branches to its underside, the
            // middle one reaching the middle, where the two trees meet.
            for (end, from, y) in [(a, 1.0, ya), (z, -1.0, yz)] {
                let t0 = if from > 0.0 { 0.22 } else { 0.78 };
                let root = at(end + along * from * (RADIUS - 0.5), y - 28.0);
                let node = on(t0, 0.0, 10.0);
                b.beam(root, node, 1.3, 1.0, 7);
                b.beam(node - DVec3::Y * 0.9, node + DVec3::Y * 0.9, 1.5, 1.5, 7);
                for (f, e) in [(0.1, -w), (0.1, w), (0.34, -w), (0.34, w)] {
                    let t = if from > 0.0 { f } else { 1.0 - f };
                    b.beam(node, on(t, e, thick - 0.3), 0.6, 0.35, 6);
                }
                b.beam(node, on(0.5, 0.0, thick + 0.4), 0.8, 0.5, 6);
            }
        }
    }
}

/// A frame within the frame of corners `c` (a bay: two along its top, two
/// along its bottom): a smaller one, inset, held to its corners; `tone`
/// each member's (its corner, whether a stay or the edge from it).
fn inner_frame(b: &mut Builder, c: [DVec3; 4], tone: impl Fn(usize, bool) -> f32) {
    let mid = (c[0] + c[1] + c[2] + c[3]) * 0.25;
    let inner = c.map(|p| mid + (p - mid) * 0.5);
    for k in 0..4 {
        b.tone = tone(k, false);
        b.beam(inner[k], inner[(k + 1) % 4], 0.35, 0.35, 5);
        b.tone = tone(k, true);
        b.beam(c[k], inner[k], 0.3, 0.3, 5);
    }
    b.tone = MEMBER;
}

/// A meander round a lattice collar: a motif of one, two or four bays,
/// repeated round the ring; in each bay a way from its left side to its
/// right, along its chord or through its inner frame, either way round;
/// where one bay's way ends at the other height from where the next one's
/// starts, up or down the post between them. (`pick`: its random numbers.)
fn walk(pick: &dyn Fn(i32) -> f64) -> HashSet<Member> {
    let bays = SIDES / 2;
    let length = [1, 2, 4][(pick(0) * 3.0) as usize];
    // Each bay's way: the corner it comes in at (0 top, 3 foot), the one it
    // leaves by (1 top, 2 foot), and round its inner frame which way
    // (clockwise or not; none: along its chord).
    let motif: Vec<(usize, usize, Option<bool>)> = (0..length)
        .map(|n| {
            let r = |k: i32| pick(1 + n as i32 * 4 + k);
            let (from, to) = (if r(0) < 0.5 { 0 } else { 3 }, if r(1) < 0.5 { 1 } else { 2 });
            let chord = from == 3 && to == 2 && r(2) < 0.4;
            (from, to, if chord { None } else { Some(r(3) < 0.5) })
        })
        .collect();
    let mut path = HashSet::new();
    for t in 0..bays {
        let (from, to, way) = motif[t % length];
        match way {
            None => {
                path.insert(Member::Chord(t));
            }
            Some(clockwise) => {
                path.insert(Member::Stay(t, from));
                path.insert(Member::Stay(t, to));
                let mut k = from;
                while k != to {
                    if clockwise {
                        path.insert(Member::Inner(t, k));
                        k = (k + 1) % 4;
                    } else {
                        k = (k + 3) % 4;
                        path.insert(Member::Inner(t, k));
                    }
                }
            }
        }
        let (next, ..) = motif[(t + 1) % length];
        if (to == 1) != (next == 0) {
            path.insert(Member::Post((t + 1) % bays));
        }
    }
    path
}

/// A stay from `from` to `to`: three cables bundled, side by side.
fn bundle(b: &mut Builder, from: DVec3, to: DVec3) {
    let axis = (to - from).normalize_or_zero();
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    for k in 0..3 {
        let a = k as f64 * std::f64::consts::TAU / 3.0;
        let off = (u * a.cos() + v * a.sin()) * 0.32;
        b.beam(from + off, to + off, 0.22, 0.22, 5);
    }
}
