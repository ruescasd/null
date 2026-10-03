//! Sites: where structures grow out of the world.
//!
//! A grid of cells covers the torus; each cell may hold one site. Whether it
//! does, and what grows there, comes from the `sites` rules in the structure
//! library: each rule names a district, a style, a weight and a range of
//! sizes. Everything is decided by hashing the cell, so the world is the same
//! every time and wraps seamlessly. Sites keep clear of each other, of the
//! canals and of the hand-placed structures.
//!
//! A site stands on a podium: its top sits just above the highest ground
//! under the footprint and its sides reach below the lowest, so nothing
//! floats or is half buried, and on uneven ground it reads as a deliberate
//! acropolis.

use glam::{DVec2, Quat, Vec3};
use serde::Deserialize;

use crate::canal::HALF_WIDTH;
use crate::district::District;
use crate::noise::hash01;
use crate::plates::PlateWorld;
use crate::structure::{self, Library, Placement, Solid};

/// The grid sites are placed on.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SiteGrid {
    /// Distance between cells, in metres (adjusted to fit the world exactly).
    #[serde(default = "default_spacing")]
    pub spacing: f32,
    /// Chance that a cell holds a site at all.
    #[serde(default = "default_chance")]
    pub chance: f32,
}

impl Default for SiteGrid {
    fn default() -> Self {
        Self { spacing: default_spacing(), chance: default_chance() }
    }
}

fn default_spacing() -> f32 {
    768.0
}
fn default_chance() -> f32 {
    0.5
}
fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

/// "In this district, structures of this style grow, this often, this big."
#[derive(Clone, Debug, Deserialize)]
pub struct SiteRule {
    pub district: District,
    pub style: String,
    #[serde(default = "one")]
    pub weight: f32,
    /// Smallest and largest size (x, y, z), metres.
    pub size: ((f32, f32, f32), (f32, f32, f32)),
    /// Stand on a podium (default) or straight on the ground.
    #[serde(default = "yes")]
    pub plinth: bool,
}

/// A planned site: a structure placed in the world (its `at` is in the
/// world's canonical 0..size range).
#[derive(Clone, Debug)]
pub struct Site {
    /// The grid cell, which identifies the site.
    pub cell: (i32, i32),
    pub placement: Placement,
    pub plinth: bool,
}

/// How far a site's centre may stray from its cell's centre, as a fraction
/// of the cell.
const JITTER: f32 = 0.15;
/// A footprint's half-diagonal at most, as a fraction of the cell: with the
/// jitter, neighbours can never touch.
const REACH: f32 = 0.5 - JITTER - 0.02;
/// Clearance between a footprint and a canal's lip, metres.
const CANAL_CLEARANCE: f32 = 12.0;
/// The podium's top clears this fraction of the ground under it.
const HIGH_PERCENTILE: f32 = 0.85;
/// Footprints smaller than this (in either direction) are not worth placing.
const MIN_FOOTPRINT: f32 = 24.0;

/// Cells per side and the cell size.
fn cells(world_size: f32, spacing: f32) -> (i32, f32) {
    let n = (world_size / spacing.max(16.0)).round().max(1.0) as i32;
    (n, world_size / n as f32)
}

/// The site in a grid cell, if any. Cheap: no ground sampling.
pub fn plan(library: &Library, world: &PlateWorld, cell: (i32, i32)) -> Option<Site> {
    let grid = library.site_grid;
    let (n, size) = cells(world.size(), grid.spacing);
    let cell = (cell.0.rem_euclid(n), cell.1.rem_euclid(n));
    let seed = world.seed() ^ 0x517e_5eed;
    let r = |k: i32| hash01(cell.0, k, cell.1, seed);
    if r(0) >= grid.chance {
        return None;
    }
    let x = (cell.0 as f32 + 0.5 + (r(1) * 2.0 - 1.0) * JITTER) * size;
    let z = (cell.1 as f32 + 0.5 + (r(2) * 2.0 - 1.0) * JITTER) * size;
    let district = world.district(x as f64, z as f64);
    let rules: Vec<&SiteRule> = library.sites.iter().filter(|s| s.district == district).collect();
    let total: f32 = rules.iter().map(|s| s.weight.max(0.0)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = r(3) * total;
    let rule = rules
        .iter()
        .copied()
        .find(|s| {
            pick -= s.weight.max(0.0);
            pick <= 0.0
        })
        .unwrap_or(rules[rules.len() - 1]);
    let ((x0, y0, z0), (x1, y1, z1)) = rule.size;
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let (mut sx, sy, mut sz) = (lerp(x0, x1, r(4)), lerp(y0, y1, r(5)), lerp(z0, z1, r(6)));
    // Shrink (keeping proportions) to stay inside the cell and off the canals.
    let mut reach = REACH * size;
    if let Some((center, _, _)) = world.nearest_canal(x, z) {
        let distance = (center - glam::Vec2::new(x, z)).length();
        reach = reach.min(distance - HALF_WIDTH as f32 - CANAL_CLEARANCE);
    }
    // And off the hand-placed structures.
    for other in &library.structures {
        let wrap = |d: f32| d - (d / world.size()).round() * world.size();
        let distance = wrap(other.at.0 - x).hypot(wrap(other.at.1 - z));
        reach = reach.min(distance - other.size.0.hypot(other.size.2) * 0.5 - CANAL_CLEARANCE);
    }
    let half_diagonal = sx.hypot(sz) * 0.5;
    if half_diagonal > reach {
        let k = reach.max(0.0) / half_diagonal;
        sx *= k;
        sz *= k;
    }
    if sx.min(sz) < MIN_FOOTPRINT {
        return None;
    }
    Some(Site {
        cell,
        placement: Placement {
            style: rule.style.clone(),
            at: (x, z),
            size: (sx, sy, sz),
            yaw: r(7) * 360.0,
            seed: (cell.0 as u32).wrapping_mul(73_856_093) ^ (cell.1 as u32).wrapping_mul(19_349_663) ^ seed,
            sink: 0.0,
        },
        plinth: rule.plinth,
    })
}

/// Every site whose cell lies within `radius` of (x, z), wrapping. Cells
/// repeat around the torus, so a small world can list a site twice.
pub fn near(library: &Library, world: &PlateWorld, x: f32, z: f32, radius: f32) -> Vec<Site> {
    let (_, size) = cells(world.size(), library.site_grid.spacing);
    let (cx, cz) = ((x / size).floor() as i32, (z / size).floor() as i32);
    let reach = (radius / size).ceil() as i32 + 1;
    let mut out = Vec::new();
    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let (gx, gz) = (cx + dx, cz + dz);
            let center = ((gx as f32 + 0.5) * size, (gz as f32 + 0.5) * size);
            if (center.0 - x).hypot(center.1 - z) > radius + size {
                continue;
            }
            if let Some(site) = plan(library, world, (gx, gz)) {
                out.push(site);
            }
        }
    }
    out
}

/// Every site in the world.
pub fn all(library: &Library, world: &PlateWorld) -> Vec<Site> {
    let (n, _) = cells(world.size(), library.site_grid.spacing);
    (0..n * n).filter_map(|i| plan(library, world, (i % n, i / n))).collect()
}

/// A built site: the height its structure stands at, and its solids
/// (podium included) relative to (x, base, z).
pub struct Built {
    pub base: f32,
    pub solids: Vec<Solid>,
}

/// Builds a site's structure and podium. Samples the ground under the
/// footprint and runs the structure's rules, so it belongs in the background.
pub fn build(library: &Library, world: &PlateWorld, site: &Site, max_leaves: usize) -> Built {
    let (x, z) = site.placement.at;
    let (sx, _, sz) = site.placement.size;
    if !site.plinth {
        let base = world.height_at(x, z);
        return Built { base, solids: structure::build(library, &site.placement, max_leaves) };
    }
    // The podium is a little larger than the structure.
    let half = Vec3::new(sx * 0.5 + 3.0, 0.0, sz * 0.5 + 3.0);
    let rotation = Quat::from_rotation_y(site.placement.yaw.to_radians());
    // Ground on a grid of about 6 m over the footprint: fine enough to catch
    // most plates.
    let (nx, nz) = (((half.x / 3.0) as usize).clamp(4, 48), ((half.z / 3.0) as usize).clamp(4, 48));
    let mut heights = Vec::with_capacity((nx + 1) * (nz + 1));
    for i in 0..=nx {
        for j in 0..=nz {
            let local = Vec3::new(
                (i as f32 / nx as f32 * 2.0 - 1.0) * half.x,
                0.0,
                (j as f32 / nz as f32 * 2.0 - 1.0) * half.z,
            );
            let w = rotation * local;
            let p = DVec2::new((x + w.x) as f64, (z + w.z) as f64);
            heights.push(world.height_at(p.x as f32, p.y as f32));
        }
    }
    heights.sort_by(f32::total_cmp);
    let low = heights[0];
    // Its top just above the ground, ignoring the odd pillar or mesa (which
    // pokes through instead of lifting the whole site), its foot well below
    // the lowest ground.
    let high = heights[((heights.len() - 1) as f32 * HIGH_PERCENTILE) as usize];
    let base = high + 0.4;
    let bottom = low - 6.0 - base;
    let mut solids = vec![Solid {
        wedge: false,
        center: Vec3::Y * bottom * 0.5,
        rotation,
        half: Vec3::new(half.x, -bottom * 0.5, half.z),
        albedo: 0.11,
    }];
    solids.extend(structure::build(library, &site.placement, max_leaves));
    Built { base, solids }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::parse(
            r#"(
                styles: { "block": (divisions: (2, 2, 2), keep: All, depth: 1, leaves: [(on: Any, module: "box")]) },
                site_grid: (spacing: 768, chance: 0.6),
                sites: [
                    (district: Floor, style: "block", size: ((80, 20, 80), (200, 60, 200))),
                    (district: Tiers, style: "block", size: ((80, 20, 80), (200, 60, 200))),
                    (district: Stacks, style: "block", size: ((80, 20, 80), (200, 60, 200))),
                    (district: Broken, style: "block", size: ((80, 20, 80), (200, 60, 200))),
                ],
            )"#,
        )
        .unwrap()
    }

    #[test]
    fn sites_are_spread_and_do_not_overlap() {
        let world = PlateWorld::new(16384.0, 7);
        let library = library();
        let sites = all(&library, &world);
        assert!(sites.len() > 100, "{} sites", sites.len());
        for (i, a) in sites.iter().enumerate() {
            for b in &sites[i + 1..] {
                let (ax, az) = a.placement.at;
                let (bx, bz) = b.placement.at;
                let wrap = |d: f32| d - (d / 16384.0).round() * 16384.0;
                let distance = wrap(ax - bx).hypot(wrap(az - bz));
                let reach = |s: &Site| s.placement.size.0.hypot(s.placement.size.2) * 0.5;
                assert!(distance > reach(a) + reach(b), "{:?} and {:?} overlap", a.cell, b.cell);
            }
        }
        // Wrapping: the cell past the edge is the first one.
        let first = plan(&library, &world, (0, 0)).map(|s| s.placement.at);
        let n = (16384.0f32 / 768.0).round() as i32;
        assert_eq!(first, plan(&library, &world, (n, 0)).map(|s| s.placement.at));
    }

    #[test]
    fn podiums_stand_above_the_ground() {
        let world = PlateWorld::new(16384.0, 7);
        let library = library();
        let site = all(&library, &world).into_iter().next().unwrap();
        let built = build(&library, &world, &site, 1000);
        let (x, z) = site.placement.at;
        assert!(built.base > world.height_at(x, z));
        let podium = built.solids[0];
        assert!(podium.center.y + podium.half.y <= 1e-3);
        assert!(built.base + podium.center.y - podium.half.y < world.height_at(x, z));
        assert!(built.solids.len() > 1);
    }
}
