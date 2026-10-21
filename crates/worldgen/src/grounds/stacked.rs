//! Ground in layers (after BLAME!'s strata): a floor of plates, and over it
//! decks of plates at set heights, in broken stretches where a noise says,
//! standing on pillars down to whatever is under them. More than one height
//! under a point: the land as a megastructure's floors, voids between them.
//!
//! Round 1 (the user: the best at first glance, the voids the idea) had its
//! decks too full and too close: from above, cracked flat ground. Now fewer
//! and smaller the higher, far apart, so you see down through them.

use glam::DVec2;

use super::{Builder, SPAN, voronoi};
use crate::noise::Fbm;

/// The layers: each its height (m), its plates' spacing (m), how much of it
/// is there (a noise's threshold: higher, less), and its tone.
const LAYERS: [(f64, f64, f64, f32); 4] = [(4.0, 48.0, -2.0, 0.11), (46.0, 64.0, 0.15, 0.16), (96.0, 80.0, 0.3, 0.19), (150.0, 96.0, 0.4, 0.22)];
/// The decks' thickness (m), how much a plate's height strays (m, either
/// way), the chance a plate stands on a pillar, the pillars' width (m,
/// least and most), and their tone.
const THICK: f64 = 4.0;
const STRAY: f64 = 2.0;
const PILLAR: f64 = 0.45;
const WIDTH: (f64, f64) = (6.0, 14.0);
const PILLAR_TONE: f32 = 0.1;

pub fn build(b: &mut Builder, seed: u32) {
    let world = crate::WORLD_SIZE;
    let origin = b.centre - DVec2::splat(SPAN * 0.5);
    let r = |i: i32, j: i32, k: i32| super::rand(seed, i, j, k);
    for (layer, &(height, spacing, threshold, tone)) in LAYERS.iter().enumerate() {
        let mask = Fbm::new(600.0, 3, 0.5, seed ^ (0x70 + layer as u32));
        let n = (SPAN / spacing).ceil() as i32;
        let key = layer as i32 * 10;
        // (Each plate's plan first, then all of them: pillars find the
        // layers under them, not this one.)
        let mut plates = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let (s, cell) = voronoi(seed, key, origin, spacing, 0.8, i, j);
                if (mask.sample2(s.x as f32, s.y as f32, world) as f64) < threshold {
                    continue;
                }
                let cell: Vec<DVec2> = cell.into_iter().map(|p| p.clamp(origin, origin + DVec2::splat(SPAN))).collect();
                let top = (height + (r(i, j, key + 3) * 2.0 - 1.0) * STRAY).round();
                plates.push((i, j, s, cell, top, r(i, j, key + 4) < PILLAR, WIDTH.0 + (WIDTH.1 - WIDTH.0) * r(i, j, key + 5)));
            }
        }
        for (i, j, s, cell, top, pillar, width) in plates {
            if layer == 0 {
                b.shade(seed, (i, j), tone, 0.02);
                b.prism(&cell, top, -1.0, false);
                continue;
            }
            if pillar {
                let under = b.top.at(s);
                let half = width * 0.5;
                let column: Vec<DVec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|&(x, z)| s + DVec2::new(x, z) * half).collect();
                b.shade(seed, (i, j + 7000), PILLAR_TONE, 0.015);
                b.prism(&column, top - THICK, under - 1.0, false);
            }
            b.shade(seed, (i + 5000 * layer as i32, j), tone, 0.025);
            b.prism(&cell, top, top - THICK, true);
        }
    }
}
