//! A plateau cut by canyons (the user, round 1: turing's corridors, judged
//! from between their walls, and penrose's cracks both pointed here; the
//! curves were not of our world): straight-sided, in a hierarchy. The land
//! is split into big Voronoi cells, each shrunk to leave a deep, wide canyon
//! round it; inside each, smaller cells shrunk again leave narrower,
//! shallower canyons; and again. Each block stands on the one it was cut
//! from, so the canyons step down to the main ones. Some cells are not cut
//! further (broad mesas), some left out (open basins).

use glam::DVec2;

use super::{Builder, SPAN, inset, rand, voronoi, within};
use crate::noise::Fbm;

/// The levels, coarsest first: cells' spacing (m), the canyons' width
/// between them (m, least and most), and how much higher (m) a block stands
/// over the one it is cut from (least and most).
// (Round 2: as deep as the main canyons are wide and more; shallower, the
// plateau read as paving from above and the canyons as ditches.)
const LEVELS: [(f64, (f64, f64), (f64, f64)); 3] = [(560.0, (40.0, 80.0), (70.0, 100.0)), (150.0, (12.0, 24.0), (26.0, 40.0)), (48.0, (4.0, 8.0), (8.0, 16.0))];
/// Of each level's cells: the chance one is left out (an open basin, at
/// its parent's floor) and the chance it is not cut further (a broad mesa).
const OPEN: [f64; 3] = [0.0, 0.12, 0.25];
const WHOLE: [f64; 3] = [0.0, 0.3, 0.45];

pub fn build(b: &mut Builder, seed: u32) {
    let origin = b.centre - DVec2::splat(SPAN * 0.5);
    let square = vec![origin, origin + DVec2::new(SPAN, 0.0), origin + DVec2::splat(SPAN), origin + DVec2::new(0.0, SPAN)];
    // The canyons' floor: the land's broad rise, gentle.
    let world = crate::WORLD_SIZE;
    let land = Fbm::new(1200.0, 2, 0.5, seed ^ 0x81);
    let floor = 4.0 + (8.0 * land.sample2(b.centre.x as f32, b.centre.y as f32, world) as f64).round();
    b.shade(seed, (0, 0), 0.13, 0.0);
    b.prism(&square, floor, -1.0, false);
    cut(b, seed, &square, floor, 0, 0);
}

/// Cuts `region` (standing at `floor`) into the cells of level `level`.
fn cut(b: &mut Builder, seed: u32, region: &[DVec2], floor: f64, level: usize, parent: i32) {
    let (spacing, width, rise) = LEVELS[level];
    let origin = b.centre - DVec2::splat(SPAN * 0.5);
    let (mut lo, mut hi) = (region[0], region[0]);
    for p in region {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let (i0, i1) = (((lo.x - origin.x) / spacing).floor() as i32 - 1, ((hi.x - origin.x) / spacing).ceil() as i32 + 1);
    let (j0, j1) = (((lo.y - origin.y) / spacing).floor() as i32 - 1, ((hi.y - origin.y) / spacing).ceil() as i32 + 1);
    let key = level as i32 * 7 + 3;
    for j in j0..=j1 {
        for i in i0..=i1 {
            let (_, cell) = voronoi(seed, key, origin, spacing, 0.9, i, j);
            let r = |k: i32| rand(seed, i * 31 + parent, j, key * 16 + k);
            if r(4) < OPEN[level] {
                continue;
            }
            let block = inset(&within(&cell, region), (width.0 + (width.1 - width.0) * r(5)) * 0.5);
            if block.is_empty() {
                continue;
            }
            let top = (floor + rise.0 + (rise.1 - rise.0) * r(6)).round();
            // (Each block its own tone, the higher the paler a little.)
            b.shade(seed, (i * 97 + parent, j * 89 + level as i32), 0.14 + 0.02 * level as f32, 0.035);
            b.prism(&block, top, floor, false);
            if level + 1 < LEVELS.len() && r(7) >= WHOLE[level + 1] {
                cut(b, seed, &block, top, level + 1, i * 1000 + j);
            }
        }
    }
}


/// Bridges and decks across the canyons (`bridged`: enclosure and levels):
/// on each canyon of the first two levels (an edge between two cells, low
/// at its middle, high to either side), a few spans at heights between its
/// floor and its rims, their ends buried in its walls; some narrow bridges,
/// some broad decks.
pub fn bridges(b: &mut Builder, seed: u32) {
    let origin = b.centre - DVec2::splat(SPAN * 0.5);
    let mut seen = std::collections::HashSet::new();
    for (level, &(spacing, width, _)) in LEVELS.iter().enumerate().take(2) {
        let key = level as i32 * 7 + 3;
        let n = (SPAN / spacing).ceil() as i32;
        for j in -1..=n {
            for i in -1..=n {
                let (_, cell) = voronoi(seed, key, origin, spacing, 0.9, i, j);
                for e in 0..cell.len() {
                    let (p, q) = (cell[e], cell[(e + 1) % cell.len()]);
                    let mid = (p + q) * 0.5;
                    if !seen.insert(((mid.x * 4.0).round() as i64, (mid.y * 4.0).round() as i64)) || !b.on(mid) {
                        continue;
                    }
                    let along = (q - p).normalize_or_zero();
                    let across = along.perp();
                    let length = p.distance(q);
                    let span = width.1 + 16.0;
                    let r = |k: i32| rand(seed, (mid.x * 4.0) as i32, (mid.y * 4.0) as i32, 500 + k);
                    // (A span every 90 m or so of the canyon, at most five.)
                    for s in 0..((length / 90.0).ceil() as i32).min(5) {
                        let t = 0.1 + 0.8 * r(s * 10);
                        let at = p + (q - p) * t;
                        let floor = b.top.at(at);
                        let rims = b.top.at(at + across * (span * 0.5 - 4.0)).min(b.top.at(at - across * (span * 0.5 - 4.0)));
                        if rims - floor < 14.0 || r(s * 10 + 1) < 0.15 {
                            continue;
                        }
                        let deck = r(s * 10 + 2) < 0.45;
                        let (wide, thick) = if deck { (30.0 + 50.0 * r(s * 10 + 3), 6.0) } else { (6.0 + 6.0 * r(s * 10 + 3), 4.0) };
                        let top = (floor + (rims - floor) * (0.15 + 0.7 * r(s * 10 + 4))).round();
                        let (x, z) = (across * (span * 0.5), along * (wide * 0.5));
                        let outline = vec![at - x - z, at + x - z, at + x + z, at - x + z];
                        b.shade(seed, ((at.x) as i32, (at.y) as i32), if deck { 0.2 } else { 0.24 }, 0.02);
                        b.prism(&outline, top, top - thick, true);
                    }
                }
            }
        }
    }
}
