//! A megastructure's floors (the user, round 2: `stacked` showed levels as a
//! district's identity, but one plate on one pillar read as trees, a
//! savanna): over a floor of plates, decks continuous over many pillars,
//! each level a square grid turned its own way, its edge straight-stepped
//! where a noise ends it, holes where it dips; pillars in a regular rhythm
//! down to whatever is under them.

use glam::DVec2;

use super::{Builder, SPAN, voronoi};
use crate::noise::Fbm;

/// The levels: each its height (m), how much of it is there (a noise's
/// threshold: higher, less), its turn (degrees), and its tone.
const LEVELS: [(f64, f64, f64, f32); 3] = [(44.0, -0.05, 0.0, 0.16), (92.0, 0.12, 22.0, 0.19), (146.0, 0.28, -14.0, 0.22)];
/// The decks' grid (m), thickness (m), their pillars every so many cells,
/// the pillars' width (m), and their tone.
const CELL: f64 = 24.0;
const THICK: f64 = 5.0;
const EVERY: i32 = 3;
const PILLAR: f64 = 6.0;
const PILLAR_TONE: f32 = 0.1;

pub fn build(b: &mut Builder, seed: u32) {
    let world = crate::WORLD_SIZE;
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
    for (level, &(height, threshold, turn, tone)) in LEVELS.iter().enumerate() {
        let mask = Fbm::new(420.0, 3, 0.5, seed ^ (0x90 + level as u32));
        let (x, z) = (DVec2::from_angle(turn.to_radians()), DVec2::from_angle(turn.to_radians()).perp());
        let centre = b.centre;
        let at = |i: i32, j: i32| centre + x * (i as f64 * CELL) + z * (j as f64 * CELL);
        let half = (SPAN * 0.72 / CELL).ceil() as i32;
        let solid = |i: i32, j: i32| {
            let c = at(i, j) + (x + z) * (CELL * 0.5);
            (c - centre).abs().max_element() <= SPAN * 0.5 && mask.sample2(c.x as f32, c.y as f32, world) as f64 > threshold
        };
        // (Pillars first: they find the levels under them, not this one.)
        for j in -half..=half {
            for i in -half..=half {
                if i.rem_euclid(EVERY) != 0 || j.rem_euclid(EVERY) != 0 || !(solid(i, j) && solid(i - 1, j) && solid(i, j - 1) && solid(i - 1, j - 1)) {
                    continue;
                }
                let p = at(i, j);
                let under = b.top.at(p);
                let w = PILLAR * 0.5;
                let column: Vec<DVec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|&(a, c)| p + x * (a * w) + z * (c * w)).collect();
                b.shade(seed, (i, j + 9000 * (level as i32 + 1)), PILLAR_TONE, 0.015);
                b.prism(&column, height - THICK, under - 1.0, false);
            }
        }
        // The deck, by runs along each row: its top and underside, and its
        // walls where the deck ends.
        b.shade(seed, (level as i32, 0), tone, 0.0);
        for j in -half..=half {
            let mut i = -half;
            while i <= half {
                if !solid(i, j) {
                    i += 1;
                    continue;
                }
                let start = i;
                while i <= half && solid(i, j) {
                    i += 1;
                }
                // (Each run a little its own tone: the deck's panels.)
                b.shade(seed, (start + 1000 * level as i32, j), tone, 0.012);
                let quad = [at(start, j), at(i, j), at(i, j + 1), at(start, j + 1)];
                b.top(&quad, height);
                b.underside(&quad, height - THICK);
                b.wall(quad[3], quad[0], height - THICK, height, -x);
                b.wall(quad[1], quad[2], height - THICK, height, x);
                for k in start..i {
                    if !solid(k, j - 1) {
                        b.wall(at(k, j), at(k + 1, j), height - THICK, height, -z);
                    }
                    if !solid(k, j + 1) {
                        b.wall(at(k, j + 1), at(k + 1, j + 1), height - THICK, height, z);
                    }
                }
            }
        }
    }
}
