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

/// How a canyon ground is finished: its walls grooved (strata), and its
/// smallest blocks' tops.
pub struct Style {
    pub grooves: bool,
    pub tops: Tops,
}

/// The smallest blocks (the mesas' tops): stepped boxes (the user, round 4:
/// in danger of the blocky look, the fractal detail helping), or inverted
/// (the user's, round 5: the boxes swapped for spaces, pits sunk in the
/// mesa, the canyons between them left standing as ribs; "better than what
/// we had so far").
pub enum Tops {
    Steps,
    Inverted,
}

pub const PLAIN: Style = Style { grooves: false, tops: Tops::Steps };
pub const BRIDGED: Style = Style { grooves: true, tops: Tops::Inverted };

/// Grooves round the walls (`bridged`; the user, round 4: the ledges read
/// as bars added to the walls; cut in instead, as strata): at heights shared
/// by every wall, one every so many metres (least and most), each so tall
/// and so deep (m, least and most).
const STRATA: (f64, f64) = (12.0, 22.0);
const GROOVE: ((f64, f64), (f64, f64)) = ((2.0, 6.0), (1.5, 5.0));
/// Inverted tops: how deep the pits sink (m, least and most) under the
/// mesa's top.
const PITS: (f64, f64) = (6.0, 24.0);

/// The canyons, finished in `style`.
pub fn build(b: &mut Builder, seed: u32, style: &Style) {
    let origin = b.centre - DVec2::splat(SPAN * 0.5);
    let square = vec![origin, origin + DVec2::new(SPAN, 0.0), origin + DVec2::splat(SPAN), origin + DVec2::new(0.0, SPAN)];
    // The canyons' floor: the land's broad rise, gentle.
    let world = crate::WORLD_SIZE;
    let land = Fbm::new(1200.0, 2, 0.5, seed ^ 0x81);
    let floor = 4.0 + (8.0 * land.sample2(b.centre.x as f32, b.centre.y as f32, world) as f64).round();
    b.shade(seed, (0, 0), 0.13, 0.0);
    b.prism(&square, floor, -1.0, false);
    cut(b, seed, &square, floor, 0, 0, style);
}

/// Cuts `region` (standing at `floor`) into the cells of level `level`.
fn cut(b: &mut Builder, seed: u32, region: &[DVec2], floor: f64, level: usize, parent: i32, style: &Style) {
    let (spacing, width, rise) = LEVELS[level];
    let smallest = level + 1 == LEVELS.len();
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
            let further = !smallest && r(7) >= WHOLE[level + 1];
            if smallest && matches!(style.tops, Tops::Inverted) {
                // (Its cell, ribs round a pit: `floor` is the mesa's top.)
                let cell = within(&cell, region);
                if cell.len() >= 3 {
                    let d = (width.0 + (width.1 - width.0) * r(5)) * 0.5;
                    pit(b, &cell, d, floor, floor - PITS.0 - (PITS.1 - PITS.0) * r(6), floor - PITS.1);
                }
                continue;
            }
            // (A mesa whose top is cut into pits stands lower, the pits'
            // deepest, and its ribs and pits rise from there.)
            let top = if further && level + 2 == LEVELS.len() && matches!(style.tops, Tops::Inverted) {
                cut(b, seed, &block, top, level + 1, i * 1000 + j, style);
                top - PITS.1
            } else {
                top
            };
            if style.grooves && !smallest {
                grooved(b, seed, &block, top, floor);
            } else {
                b.prism(&block, top, floor, false);
            }
            if further && !(level + 2 == LEVELS.len() && matches!(style.tops, Tops::Inverted)) {
                cut(b, seed, &block, top, level + 1, i * 1000 + j, style);
            }
        }
    }
}



/// A cell inverted (`Tops::Inverted`): a pit `d` m in from its sides,
/// its floor at `low`, and round it ribs at `top` (the half of the canyon
/// that was its; its neighbour's half meets it), all standing on `base`.
/// The pit's corners are where its sides' offset lines meet (mitred), so
/// ribs and pit tile the cell exactly; a cell too small for a pit is solid.
fn pit(b: &mut Builder, cell: &[DVec2], d: f64, top: f64, low: f64, base: f64) {
    let n = cell.len();
    let ccw = crate::tilted::area(cell) > 0.0;
    let inward = |i: usize| {
        let (p, q) = (cell[i], cell[(i + 1) % n]);
        (q - p).perp().normalize_or_zero() * if ccw { 1.0 } else { -1.0 }
    };
    let inner: Vec<DVec2> = (0..n)
        .map(|i| {
            let (n0, n1) = (inward((i + n - 1) % n), inward(i));
            cell[i] + (n0 + n1) * (d / (1.0 + n0.dot(n1)).max(0.2))
        })
        .collect();
    // (Too small: an inner side turned back on itself.)
    let fits = (0..n).all(|i| (inner[(i + 1) % n] - inner[i]).dot(cell[(i + 1) % n] - cell[i]) > 1.0);
    if !fits {
        b.prism(cell, top, base, false);
        return;
    }
    for i in 0..n {
        let j = (i + 1) % n;
        b.prism(&[cell[i], cell[j], inner[j], inner[i]], top, base, false);
    }
    b.prism(&inner, low.max(base + 1.0), base, false);
}

/// A block with grooves cut round its walls at the strata's heights (each
/// stratum's height and depth the same on every wall): solid bands, and
/// between them its core, set back.
fn grooved(b: &mut Builder, seed: u32, block: &[DVec2], top: f64, floor: f64) {
    let mut y = floor;
    let mut k = 0;
    let mut at = 0.0;
    loop {
        let stratum = k;
        let r = |n: i32| rand(seed, stratum, n, 880);
        at += STRATA.0 + (STRATA.1 - STRATA.0) * r(1);
        let tall = GROOVE.0.0 + (GROOVE.0.1 - GROOVE.0.0) * r(2);
        let (g0, g1) = (at.round(), (at + tall).round());
        k += 1;
        if g1 > top - 3.0 {
            break;
        }
        if g0 < floor + 3.0 {
            continue;
        }
        let core = super::inset(block, GROOVE.1.0 + (GROOVE.1.1 - GROOVE.1.0) * r(3));
        if core.is_empty() {
            break;
        }
        b.prism(block, g0, y, y > floor);
        b.prism(&core, g1, g0, false);
        y = g1;
    }
    b.prism(block, top, y, y > floor);
}

/// Natural bridges (`bridged`): on canyons of the first two levels (an
/// edge between two cells, low at its middle, high to either side), spans
/// left when the canyon was cut, as thick as strata, at or a little under
/// the lower rim, their ends in the walls. (The round 3 planks, thin and
/// plain, looked polygonal and out of place, the user: added, not of it.)
const SPANS: [f64; 2] = [0.45, 0.25];
const SPAN_WIDTH: (f64, f64) = (14.0, 40.0);
const SPAN_THICK: (f64, f64) = (12.0, 24.0);

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
                    let r = |k: i32| rand(seed, (mid.x * 4.0) as i32, (mid.y * 4.0) as i32, 500 + k);
                    if r(0) > SPANS[level] {
                        continue;
                    }
                    let along = (q - p).normalize_or_zero();
                    let across = along.perp();
                    let at = p + (q - p) * (0.2 + 0.6 * r(1));
                    let span = width.1 + 20.0;
                    let floor = b.top.at(at);
                    let rims = b.top.at(at + across * (span * 0.5 - 6.0)).min(b.top.at(at - across * (span * 0.5 - 6.0)));
                    if rims - floor < 20.0 {
                        continue;
                    }
                    let wide = SPAN_WIDTH.0 + (SPAN_WIDTH.1 - SPAN_WIDTH.0) * r(2);
                    let thick = (SPAN_THICK.0 + (SPAN_THICK.1 - SPAN_THICK.0) * r(3)).min(rims - floor - 6.0);
                    let top = (rims - 6.0 * r(4)).round();
                    let (x, z) = (across * (span * 0.5), along * (wide * 0.5));
                    let outline = vec![at - x - z, at + x - z, at + x + z, at - x + z];
                    b.shade(seed, (at.x as i32, at.y as i32), 0.15, 0.02);
                    b.prism(&outline, top, top - thick, true);
                }
            }
        }
    }
}
