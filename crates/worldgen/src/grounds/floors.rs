//! A megastructure's floors (the user, round 2: `stacked` showed levels as a
//! district's identity, but one plate on one pillar read as trees, a
//! savanna): over a floor of plates, decks continuous over many supports,
//! each level a square grid turned its own way, its edge straight-stepped
//! where a noise ends it, holes where it dips.
//!
//! Round 3 still read as trees, trunk and crown (the user): every deck on
//! its own columns, set back under its middle. Now the loads are carried as
//! a structure's are: cores (great shafts through several levels), walls
//! under stretches of the decks' edges, piers of every width and clusters of
//! columns, and ribs under the decks, so their undersides read as structure.

use glam::DVec2;

use super::{Builder, SPAN, rand, voronoi};
use crate::noise::{Fbm, hash01};

/// The levels: each its height (m), how much of it is there (a noise's
/// threshold: higher, less), its turn (degrees), and its tone.
const LEVELS: [(f64, f64, f64, f32); 3] = [(44.0, -0.05, 0.0, 0.16), (92.0, 0.12, 22.0, 0.19), (146.0, 0.28, -14.0, 0.22)];
/// The decks' grid (m) and thickness (m); their ribs' width and depth (m).
const CELL: f64 = 24.0;
const THICK: f64 = 5.0;
const RIB: (f64, f64) = (1.6, 2.6);
/// Piers every so many cells, their width (m, least and most), the chance
/// one is a cluster of four columns instead (each so wide), and their tone.
const EVERY: i32 = 3;
const PIER: (f64, f64) = (4.0, 12.0);
const CLUSTER: (f64, f64) = (0.25, 2.6);
const PIER_TONE: f32 = 0.1;
/// Walls under the decks' edges: in stretches of so many cells, so many of
/// them walled; so thick (m).
const STRETCH: i32 = 4;
const WALLED: f64 = 0.2;
const WALL: f64 = 3.0;
/// Cores: on a grid of so many metres, the chance of one, their width (m,
/// least and most), how far over the highest deck they rise (m).
const CORES: f64 = 320.0;
const CORE: (f64, f64, f64) = (0.65, 18.0, 34.0);
const CROWN: f64 = 6.0;

/// A level: its height, frame, mask.
struct Level {
    height: f64,
    threshold: f64,
    centre: DVec2,
    x: DVec2,
    z: DVec2,
    mask: Fbm,
}

impl Level {
    fn at(&self, i: i32, j: i32) -> DVec2 {
        self.centre + self.x * (i as f64 * CELL) + self.z * (j as f64 * CELL)
    }

    /// Whether cell (`i`, `j`) has deck.
    fn solid(&self, i: i32, j: i32) -> bool {
        let c = self.at(i, j) + (self.x + self.z) * (CELL * 0.5);
        (c - self.centre).abs().max_element() <= SPAN * 0.5 && self.mask.sample2(c.x as f32, c.y as f32, crate::WORLD_SIZE) as f64 > self.threshold
    }

    /// Whether there is deck over a point.
    fn over(&self, p: DVec2) -> bool {
        let q = p - self.centre;
        self.solid((q.dot(self.x) / CELL).floor() as i32, (q.dot(self.z) / CELL).floor() as i32)
    }

    /// A rectangle in its frame: from `a` along x by `w`, along z by `d`.
    fn rect(&self, a: DVec2, w: f64, d: f64) -> Vec<DVec2> {
        vec![a, a + self.x * w, a + self.x * w + self.z * d, a + self.z * d]
    }
}

pub fn build(b: &mut Builder, seed: u32) {
    let origin = b.centre - DVec2::splat(SPAN * 0.5);
    // The floor: plates.
    let n = (SPAN / 48.0).ceil() as i32;
    for j in 0..n {
        for i in 0..n {
            let (_, cell) = voronoi(seed, 90, origin, 48.0, 0.8, i, j);
            let cell: Vec<DVec2> = cell.into_iter().map(|p| p.clamp(origin, origin + DVec2::splat(SPAN))).collect();
            b.shade(seed, (i, j), 0.11, 0.02);
            b.prism(&cell, 4.0, -1.0, false);
        }
    }
    let levels: Vec<Level> = LEVELS
        .iter()
        .enumerate()
        .map(|(k, &(height, threshold, turn, _))| {
            let x = DVec2::from_angle(turn.to_radians());
            Level { height, threshold, centre: b.centre, x, z: x.perp(), mask: Fbm::new(420.0, 3, 0.5, seed ^ (0x90 + k as u32)) }
        })
        .collect();
    // The cores, first: through every level over them, a little above the
    // highest. (Piers keep clear of them.)
    let mut cores: Vec<(DVec2, f64)> = Vec::new();
    let m = (SPAN / CORES).ceil() as i32;
    for j in 0..m {
        for i in 0..m {
            let r = |k: i32| rand(seed, i, j, 700 + k);
            if r(1) > CORE.0 {
                continue;
            }
            let p = origin + (DVec2::new(i as f64, j as f64) + 0.5 + DVec2::new(r(2) - 0.5, r(3) - 0.5) * 0.6) * CORES;
            let Some(top) = levels.iter().filter(|l| l.over(p)).map(|l| l.height).reduce(f64::max) else { continue };
            let w = CORE.1 + (CORE.2 - CORE.1) * r(4);
            let l = &levels[0];
            b.shade(seed, (i, j + 700), PIER_TONE + 0.02, 0.015);
            b.prism(&l.rect(p - (l.x + l.z) * (w * 0.5), w, w), top + CROWN, -1.0, false);
            cores.push((p, w));
        }
    }
    for (k, level) in levels.iter().enumerate() {
        let tone = LEVELS[k].3;
        let height = level.height;
        let half = (SPAN * 0.72 / CELL).ceil() as i32;
        let key = |a: i32, c: i32, s: i32| hash01(a, c, s + 31 * k as i32, seed) as f64;
        // (Supports first: they find the levels under them, not this one.)
        // Piers and clusters, where the deck is all round them.
        for j in -half..=half {
            for i in -half..=half {
                if i.rem_euclid(EVERY) != 0 || j.rem_euclid(EVERY) != 0 || !(level.solid(i, j) && level.solid(i - 1, j) && level.solid(i, j - 1) && level.solid(i - 1, j - 1)) {
                    continue;
                }
                let p = level.at(i, j);
                if cores.iter().any(|&(c, w)| c.distance(p) < w + CELL) {
                    continue;
                }
                b.shade(seed, (i, j + 9000 * (k as i32 + 1)), PIER_TONE, 0.015);
                if key(i, j, 1) < CLUSTER.0 {
                    let (w, gap) = (CLUSTER.1, CLUSTER.1 * 1.6);
                    for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                        let c = p + (level.x * dx + level.z * dz) * (gap * 0.5);
                        let under = b.top.at(c);
                        b.prism(&level.rect(c - (level.x + level.z) * (w * 0.5), w, w), height - THICK, under - 1.0, false);
                    }
                } else {
                    let w = PIER.0 + (PIER.1 - PIER.0) * key(i, j, 2);
                    let under = b.top.at(p);
                    b.prism(&level.rect(p - (level.x + level.z) * (w * 0.5), w, w), height - THICK, under - 1.0, false);
                }
            }
        }
        // Walls under stretches of its edges, inside them.
        for j in -half..=half {
            for i in -half..=half {
                if !level.solid(i, j) {
                    continue;
                }
                // (Each side: its open neighbour, where the side starts, which
                // way along it, which way in.)
                let sides = [((i, j - 1), level.at(i, j), level.x, level.z, i, 0), ((i, j + 1), level.at(i, j + 1), level.x, -level.z, i, 1), ((i - 1, j), level.at(i, j), level.z, level.x, j, 2), ((i + 1, j), level.at(i + 1, j), level.z, -level.x, j, 3)];
                for ((ni, nj), a, along, inward, run, side) in sides {
                    let line = if side < 2 { j * 2 + side } else { i * 2 + side };
                    if level.solid(ni, nj) || key(run.div_euclid(STRETCH), line, 3 + side) > WALLED {
                        continue;
                    }
                    let wall = vec![a, a + along * CELL, a + along * CELL + inward * WALL, a + inward * WALL];
                    let under = b.top.at(a + along * (CELL * 0.5) + inward * (WALL * 0.5));
                    b.shade(seed, (run.div_euclid(STRETCH), line + 4000 * (k as i32 + 1)), tone - 0.03, 0.01);
                    b.prism(&wall, height - THICK, under - 1.0, false);
                }
            }
        }
        // The deck, by runs along each row: its top and underside, its edges,
        // and ribs under it across each run, one every cell.
        for j in -half..=half {
            let mut i = -half;
            while i <= half {
                if !level.solid(i, j) {
                    i += 1;
                    continue;
                }
                let start = i;
                while i <= half && level.solid(i, j) {
                    i += 1;
                }
                // (Each run a little its own tone: the deck's panels.)
                b.shade(seed, (start + 1000 * k as i32, j), tone, 0.012);
                let quad = [level.at(start, j), level.at(i, j), level.at(i, j + 1), level.at(start, j + 1)];
                b.top(&quad, height);
                b.underside(&quad, height - THICK);
                b.wall(quad[3], quad[0], height - THICK, height, -level.x);
                b.wall(quad[1], quad[2], height - THICK, height, level.x);
                for c in start..i {
                    if !level.solid(c, j - 1) {
                        b.wall(level.at(c, j), level.at(c + 1, j), height - THICK, height, -level.z);
                    }
                    if !level.solid(c, j + 1) {
                        b.wall(level.at(c, j + 1), level.at(c + 1, j + 1), height - THICK, height, level.z);
                    }
                }
                b.shade(seed, (start + 1000 * k as i32, j + 50), tone - 0.02, 0.01);
                for c in start..=i {
                    let a = level.at(c, j) - level.x * (RIB.0 * 0.5);
                    let rib = level.rect(a, RIB.0, CELL);
                    b.prism(&rib, height - THICK, height - THICK - RIB.1, true);
                }
            }
        }
    }
}
