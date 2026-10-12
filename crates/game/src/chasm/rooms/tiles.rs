//! Tiles of the ways through the rock (step 3b, see
//! `plan::network::region`): each a plan cell (8 m across, a storey or more
//! high) of solid rock on the room grid, a way carved through it from each
//! side it opens by to its middle: a passage (3 m wide, arched), a door's
//! narrower mouth where it meets a room (as the rooms' doors: 2 m, springing
//! at 2.5 m), or a portal out onto the chasm. Its sides on opposite sides at
//! different storeys, a flight between them, the length of the cell (a
//! storey up a cell: as steep as a steep stair), walked on a hidden slope.
//! The entrance's tile climbs to the rim, open to the sky.

use bevy::prelude::*;

use super::super::Parts;
use super::{Cache, Filler, Grid};

/// A plan cell's side (m).
const SIDE: f32 = 8.0;
/// A passage: its width, where its arch springs (from its floor).
const WIDTH: f32 = 3.0;
const SPRING: f32 = 3.0;
/// A door's mouth: as the rooms' doors, as deep as their walls are thick.
const DOOR: f32 = 2.0;
const DOOR_SPRING: f32 = 2.5;
const MOUTH: f32 = 1.5;
/// Tiles to a mesh (in the order they are given: along their paths).
const RUN: usize = 6;
/// A flight's risers, and how high its slope stands over its steps.
const RISER: f32 = 0.25;
const LIFT: f32 = 0.05;

/// How a tile opens by a side: a passage on into the next, a door's mouth
/// into a room, a portal onto the chasm.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::chasm) enum Opening {
    Pass,
    Door,
    Portal,
}

/// A side a tile opens by (0 at x = 0, 1 at the far x, 2 at z = 0, 3 at the
/// far z, as a room's walls), the floor's height there (from the tile's
/// foot), and how.
#[derive(Clone, Copy, Debug)]
pub(in crate::chasm) struct Arm {
    pub side: usize,
    pub floor: f32,
    pub kind: Opening,
}

/// A tile: its corner (in the world), its height, the sides it opens by,
/// whether it has a lamp, and whether it climbs to its top, open (the
/// entrance's).
pub(in crate::chasm) struct Tile {
    pub origin: Vec3,
    pub height: f32,
    pub arms: Vec<Arm>,
    pub lamp: bool,
    pub up: bool,
}

/// A frame for a side: where along its way in a point `t` in from the side
/// is, across the way at `a` (its middle at 0); and the axis the way runs
/// along.
fn frame(side: usize) -> (usize, Box<dyn Fn(f32, f32, f32) -> Vec3>) {
    let c = SIDE * 0.5;
    match side {
        0 => (0, Box::new(move |t, a, y| Vec3::new(t, y, c + a))),
        1 => (0, Box::new(move |t, a, y| Vec3::new(SIDE - t, y, c + a))),
        2 => (2, Box::new(move |t, a, y| Vec3::new(c + a, y, t))),
        _ => (2, Box::new(move |t, a, y| Vec3::new(c + a, y, SIDE - t))),
    }
}

/// Carves the way in from a side, from `t0` to `t1` in, its floor at `y`:
/// a passage, or (`door`) a door's mouth.
fn way(g: &mut Grid, cache: &mut Cache, side: usize, (t0, t1): (f32, f32), y: f32, door: bool) {
    let (axis, at) = frame(side);
    let (w, spring) = if door { (DOOR, DOOR_SPRING) } else { (WIDTH, SPRING) };
    let (p, q) = (at(t0, -w * 0.5, y), at(t1, w * 0.5, y));
    let (lo, hi) = (p.min(q), p.max(q));
    // (Across the way: z for a way along x, x for one along z.)
    let (x, n, depth) = if axis == 0 { (lo.z, lo.x, hi.x - lo.x) } else { (lo.x, lo.z, hi.z - lo.z) };
    g.arch(axis, x, w, y, spring, n, depth, cache);
}

/// The tiles built into `parts`: their surfaces (all of each tile's box: a
/// face on the chasm is its own), the slopes over their flights, their
/// lamps.
pub(in crate::chasm) fn build(tiles: &[Tile], parts: &mut Parts) {
    let mut cache = Cache(Default::default());
    // (In runs of a few, each run its own mesh: see `Parts::chunks`.)
    let mut chunk = super::super::Geometry::default();
    for (k, tile) in tiles.iter().enumerate() {
        if k % RUN == 0 && !chunk.indices.is_empty() {
            parts.chunks.push(std::mem::take(&mut chunk));
        }
        let mut g = Grid::solid(Vec3::ZERO, Vec3::new(SIDE, tile.height, SIDE));
        let mut slopes: Vec<Vec<Vec3>> = Vec::new();
        let mut lamp_at = None;
        // A flight: two sides facing each other at different storeys (or
        // the entrance's one side, climbing to the top).
        let flight = if tile.up {
            tile.arms.first().map(|a| (*a, Arm { side: a.side ^ 1, floor: tile.height - RISER, kind: Opening::Pass }))
        } else {
            tile.arms.iter().find_map(|a| tile.arms.iter().find(|b| b.side == a.side ^ 1 && b.floor > a.floor + 1.0).map(|b| (*a, *b)))
        };
        if let Some((a, b)) = flight {
            let (axis, at) = frame(a.side);
            let rise = b.floor - a.floor;
            let risers = (rise / RISER).round() as i32;
            let slices = (SIDE / 0.25) as i32;
            for k in 0..slices {
                let y = a.floor + RISER * ((k * risers) / slices) as f32;
                let t = k as f32 * 0.25;
                let door = (a.kind == Opening::Door && t < MOUTH) || (b.kind == Opening::Door && t >= SIDE - MOUTH);
                way(&mut g, &mut cache, a.side, (t, t + 0.25), y, door);
                if tile.up {
                    let (p, q) = (at(t, -WIDTH * 0.5, y), at(t + 0.25, WIDTH * 0.5, tile.height));
                    g.fill(p.min(q), p.max(q), Filler::Empty);
                }
            }
            let _ = axis;
            // (Its slope: from the floor at the one side to the floor at
            // the other, through every step's edge, a little over them.)
            let mut corners = Vec::new();
            for (t, y) in [(0.0, a.floor + LIFT), (SIDE, b.floor + LIFT)] {
                for s in [-WIDTH * 0.5, WIDTH * 0.5] {
                    corners.push(tile.origin + at(t, s, y));
                    corners.push(tile.origin + at(t, s, y - 0.5));
                }
            }
            slopes.push(corners);
            lamp_at = Some(at(SIDE * 0.5, 0.0, (a.floor + b.floor) * 0.5 + 4.0));
        }
        // The level ways: each side in to the middle, and across it.
        for arm in &tile.arms {
            if flight.is_some_and(|(a, b)| a.side == arm.side || b.side == arm.side) {
                continue;
            }
            let reach = SIDE * 0.5 + WIDTH * 0.5;
            if arm.kind == Opening::Door {
                way(&mut g, &mut cache, arm.side, (0.0, MOUTH), arm.floor, true);
                way(&mut g, &mut cache, arm.side, (MOUTH, reach), arm.floor, false);
            } else {
                way(&mut g, &mut cache, arm.side, (0.0, reach), arm.floor, false);
            }
            lamp_at.get_or_insert(Vec3::new(SIDE * 0.5, arm.floor + 3.9, SIDE * 0.5));
        }
        let g = Grid { origin: tile.origin, ..g };
        g.mesh(&mut chunk);
        parts.walkable.extend(g.walkable(super::HEADROOM));
        parts.slopes.extend(slopes);
        if tile.lamp
            && let Some(p) = lamp_at
        {
            let p = tile.origin + p;
            // (As strong as a room's main lamp for its size: weaker, the
            // passage was black but round the lamp.)
            let range = 16.0;
            parts.lights.push((p, range, 0.0006 * (range / 14.0f32).powi(2) * 6.0));
            parts.glow.oriented(p, Vec3::X * 0.15, Vec3::Y * 0.15, Vec3::Z * 0.15);
        }
    }
    if !chunk.indices.is_empty() {
        parts.chunks.push(chunk);
    }
}
