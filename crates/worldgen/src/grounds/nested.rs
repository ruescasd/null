//! The structures' fractal rule (see `ifs.rs`) run at the scale of the
//! land: the patch split into a grid of blocks, some kept, each risen or
//! sunk a little and turned a few degrees, split again inside, down to a
//! few metres; between them channels, where the block they were cut from
//! shows, a little lower. Plateaus within plateaus, off-kilter.
//!
//! The user (round 2): hints at an industrial sector, a partial enclosure;
//! can it, or a city-like derivation, escape the flight-simulator look
//! (building-sized boxes standing up, spread evenly)? Two variants to find
//! out: `contrast` (most blocks left blank, a few split all the way down:
//! knots against bare plateaus) and `sunken` (the rule cut downward: pits
//! within pits, ribs between them, excavated ground, not buildings).

use glam::DVec2;

use super::{Builder, SPAN, rand};

/// How deep it splits; per depth: how far a block may rise or sink (m),
/// the channels' depth (m), and the gaps between blocks (of a cell).
const DEPTH: u32 = 5;
const RISE: [f64; 6] = [0.0, 60.0, 28.0, 12.0, 5.0, 2.0];
const CHANNEL: [f64; 6] = [0.0, 12.0, 7.0, 4.0, 2.0, 1.0];
const GAP: (f64, f64) = (0.05, 0.14);
/// The chance a cell is kept (else a channel's floor, open), and its turn
/// (degrees, either way).
const KEEP: f64 = 0.78;
const TWIST: f64 = 7.0;

/// A variant: the chance a block stops splitting (from depth 2, or from 1
/// if a knot can stop it), and the chance a block is a knot (never stops).
pub struct Mode {
    stop: f64,
    from: u32,
    knot: f64,
}

pub const RAISED: Mode = Mode { stop: 0.15, from: 2, knot: 0.0 };
pub const CONTRAST: Mode = Mode { stop: 0.75, from: 1, knot: 0.3 };

pub fn build(b: &mut Builder, seed: u32, mode: Mode) {
    let mut id = 0;
    block(b, seed, &mode, &mut id, b.centre, DVec2::splat(SPAN * 0.5), 0.0, 40.0, 0, false);
}

fn corners(m: DVec2, h: DVec2, turn: f64) -> Vec<DVec2> {
    let (x, z) = (DVec2::from_angle(turn), DVec2::from_angle(turn).perp());
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|&(a, c)| m + x * (a * h.x) + z * (c * h.y)).collect()
}

/// A block: its middle, half extents, turn (radians), top, depth, and
/// whether it is a knot.
#[allow(clippy::too_many_arguments)]
fn block(b: &mut Builder, seed: u32, mode: &Mode, id: &mut i32, middle: DVec2, half: DVec2, turn: f64, top: f64, depth: u32, knot: bool) {
    *id += 1;
    let me = *id;
    let r = |k: i32| rand(seed, me, depth as i32, k);
    let knot = knot || (depth >= 1 && r(9) < mode.knot);
    let leaf = depth == DEPTH || (!knot && depth >= mode.from && r(1) < mode.stop) || half.min_element() < 4.0;
    let d = (depth as usize + 1).min(5);
    let floor = if leaf { top } else { top - CHANNEL[d] };
    // (Each block its own tone, as the plates.)
    b.shade(seed, (me, depth as i32), 0.17, 0.045);
    b.prism(&corners(middle, half, turn), floor, -1.0, false);
    if leaf {
        return;
    }
    let axes = (DVec2::from_angle(turn), DVec2::from_angle(turn).perp());
    let (nx, nz) = (2 + (r(2) * 3.0) as usize, 2 + (r(3) * 3.0) as usize);
    let cell = DVec2::new(half.x * 2.0 / nx as f64, half.y * 2.0 / nz as f64);
    let gap = GAP.0 + (GAP.1 - GAP.0) * r(4);
    for j in 0..nz {
        for i in 0..nx {
            let k = (j * nx + i) as i32 * 8;
            if r(10 + k) > KEEP {
                continue;
            }
            let local = DVec2::new((i as f64 + 0.5) * cell.x - half.x, (j as f64 + 0.5) * cell.y - half.y);
            let at = middle + axes.0 * local.x + axes.1 * local.y;
            if depth == 0 && !b.on(at) {
                continue;
            }
            let h = cell * 0.5 * (1.0 - gap);
            let twist = (r(11 + k) * 2.0 - 1.0) * TWIST.to_radians();
            // (Risen or sunk, never below its channel's floor.)
            let rise = ((r(12 + k) * 2.0 - 1.0) * RISE[d]).max(-CHANNEL[d] + 1.0);
            block(b, seed, mode, id, at, h, turn + twist, (floor + CHANNEL[d] + rise).round(), depth + 1, knot);
        }
    }
}

/// How deep each depth's pits sink (m, least and most) under the ground
/// they are cut from, the ribs between them by depth (of a cell: thick
/// main ribs, thin minor ones), and the chance a cell is cut (else left
/// whole, at its parent's level). (Round 3: one width of rib, shallow pits,
/// most cells cut: from above, a facade's grid of windows. Now fewer pits,
/// some dropping into shafts, the ribs a hierarchy.)
const SINK: [(f64, f64); 6] = [(0.0, 0.0), (30.0, 110.0), (10.0, 40.0), (4.0, 12.0), (2.0, 5.0), (1.0, 2.0)];
const RIBS: [(f64, f64); 3] = [(0.18, 0.3), (0.1, 0.18), (0.04, 0.1)];
const CUT: f64 = 0.5;
/// The plateau the pits are cut into (m).
const PLATEAU: f64 = 160.0;

/// The rule cut downward: from a plateau, each block split into a grid of
/// pits sunk under it, ribs left standing between them at its level; pits
/// split again inside; square (no turn), so the ribs run straight.
pub fn sunken(b: &mut Builder, seed: u32) {
    let mut id = 0;
    pit(b, seed, &mut id, b.centre, DVec2::splat(SPAN * 0.5), PLATEAU, 0);
}

fn pit(b: &mut Builder, seed: u32, id: &mut i32, middle: DVec2, half: DVec2, top: f64, depth: u32) {
    *id += 1;
    let me = *id;
    let r = |k: i32| rand(seed, me, depth as i32, k);
    let whole = |b: &mut Builder| {
        b.shade(seed, (me, depth as i32), 0.17, 0.045);
        b.prism(&corners(middle, half, 0.0), top, -1.0, false);
    };
    if depth == DEPTH || half.min_element() < 4.0 || (depth >= 2 && r(1) < 0.2) {
        whole(b);
        return;
    }
    let d = (depth as usize + 1).min(5);
    let (nx, nz) = (2 + (r(2) * 3.0) as usize, 2 + (r(3) * 3.0) as usize);
    let cell = DVec2::new(half.x * 2.0 / nx as f64, half.y * 2.0 / nz as f64);
    let ribs = RIBS[(depth as usize).min(2)];
    let rib = (ribs.0 + (ribs.1 - ribs.0) * r(4)) * cell.min_element();
    // The ribs, at the block's level along its grid's lines; between them
    // its cells, each sunk (a pit, cut again) or left whole.
    let lo = middle - half;
    b.shade(seed, (me, depth as i32), 0.19, 0.04);
    for i in 0..=nx {
        let x = lo.x + i as f64 * cell.x;
        let strip = [DVec2::new(x - rib * 0.5, lo.y), DVec2::new(x + rib * 0.5, lo.y), DVec2::new(x + rib * 0.5, lo.y + half.y * 2.0), DVec2::new(x - rib * 0.5, lo.y + half.y * 2.0)];
        let strip: Vec<DVec2> = strip.iter().map(|p| p.clamp(lo, middle + half)).collect();
        b.prism(&strip, top, -1.0, false);
    }
    for j in 0..=nz {
        let z = lo.y + j as f64 * cell.y;
        let strip = [DVec2::new(lo.x, z - rib * 0.5), DVec2::new(lo.x + half.x * 2.0, z - rib * 0.5), DVec2::new(lo.x + half.x * 2.0, z + rib * 0.5), DVec2::new(lo.x, z + rib * 0.5)];
        let strip: Vec<DVec2> = strip.iter().map(|p| p.clamp(lo, middle + half)).collect();
        b.prism(&strip, top, -1.0, false);
    }
    for j in 0..nz {
        for i in 0..nx {
            let k = (j * nx + i) as i32 * 8;
            let at = lo + DVec2::new((i as f64 + 0.5) * cell.x, (j as f64 + 0.5) * cell.y);
            if depth == 0 && !b.on(at) {
                continue;
            }
            let h = cell * 0.5 - DVec2::splat(rib * 0.5);
            if h.min_element() <= 0.5 {
                continue;
            }
            let (lo_s, hi_s) = SINK[d];
            let sunk = if r(10 + k) < CUT { (top - lo_s - (hi_s - lo_s) * r(11 + k)).round() } else { top };
            pit(b, seed, id, at, h, sunk, depth + 1);
        }
    }
}
