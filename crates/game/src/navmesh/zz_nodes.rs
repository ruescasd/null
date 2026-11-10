//! Step 1 of docs/outdoor-navigation.md: how the outdoors falls into
//! navigable nodes as the climb rises, per district, and the terrain lever
//! (the same patches with every step capped at the walker's climb).
//!
//! A measurement of the premise, not the build by construction: the
//! terrain's mesh sampled from above every 25 cm (its top, and its slope);
//! cells walkable at 45° or less, neighbours joined within the climb; cells
//! within the body's radius of a drop or wall taken away; nodes the groups
//! left. One patch per district, where it is most of the ground.
//!
//! `cargo test -p game --profile fast zz_nodes -- --ignored --nocapture`;
//! writes target/navbench/nodes.json (ZZ_NODES_OUT).

use std::{
    collections::HashMap,
    fmt::Write as _,
    sync::atomic::{AtomicUsize, Ordering},
};

use avian3d::prelude::{Collider, SimpleCollider};
use bevy::prelude::*;
use worldgen::{district::District, plates::PlateWorld};

use super::zz_world::columns;

/// The raster's cell (m).
const CELL: f32 = 0.25;
/// The walker's radius (m) and steepest walkable slope (degrees).
const RADIUS: f32 = 0.35;
const SLOPE: f32 = 45.0;
/// A patch's side, and the margin left out of its measure (m).
const PATCH: f32 = 1600.0;
const MARGIN: f32 = 100.0;
/// A large node's area, and a walker-sized one's (m²).
const LARGE: f64 = 250_000.0;
const WALKER: f64 = 10_000.0;
/// The climbs measured (m): the walker's, the hunter's step-up, a knee's
/// height (the most a visible step reaches), then context.
const CLIMBS: [f32; 7] = [0.6, 0.65, 0.8, 0.9, 1.2, 1.75, 2.0];
/// The steps measured where nodes part (m), at the walker's climb.
const STEPS: [f32; 5] = [0.8, 1.2, 2.0, 4.0, f32::INFINITY];

/// The top of a patch's ground, every `CELL`: its height (NaN, none), the
/// up part of its normal, and its district.
struct Raster {
    n: usize,
    origin: Vec2,
    h: Vec<f32>,
    up: Vec<f32>,
    district: Vec<u8>,
}

impl Raster {
    fn new(world: &PlateWorld, centre: Vec2) -> Raster {
        let meshes = columns(world, centre, PATCH);
        let made: Vec<Collider> = meshes.into_iter().filter_map(|(v, i)| Collider::try_trimesh(v, i).ok()).collect();
        let column = worldgen::column_size(0);
        let key = |p: Vec2| (p / column).floor().as_ivec2();
        let mut grid: HashMap<IVec2, Vec<usize>> = HashMap::new();
        for (i, c) in made.iter().enumerate() {
            let b = c.aabb(Vec3::ZERO, Quat::IDENTITY);
            let (lo, hi) = (key(b.min.xz()), key(b.max.xz()));
            for z in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    grid.entry(IVec2::new(x, z)).or_default().push(i);
                }
            }
        }
        let side = PATCH - 2.0 * MARGIN;
        let n = (side / CELL) as usize;
        let origin = centre - side / 2.0;
        let (mut h, mut up) = (vec![f32::NAN; n * n], vec![0.0; n * n]);
        let next = AtomicUsize::new(0);
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows: Vec<Vec<(f32, f32)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    scope.spawn(|| {
                        let mut out = Vec::new();
                        loop {
                            let z = next.fetch_add(1, Ordering::Relaxed);
                            if z >= n {
                                return out;
                            }
                            let row: Vec<(f32, f32)> = (0..n)
                                .map(|x| {
                                    let p = origin + (Vec2::new(x as f32, z as f32) + 0.5) * CELL;
                                    let mut best = (f32::NAN, 0.0);
                                    for &i in grid.get(&key(p)).into_iter().flatten() {
                                        if let Some((t, normal)) = made[i].cast_ray(Vec3::ZERO, Quat::IDENTITY, Vec3::new(p.x, 5000.0, p.y), Vec3::NEG_Y, 10000.0, true) {
                                            let top = 5000.0 - t;
                                            if best.0.is_nan() || top > best.0 {
                                                best = (top, normal.y.abs());
                                            }
                                        }
                                    }
                                    best
                                })
                                .collect();
                            out.push((z, row));
                        }
                    })
                })
                .collect();
            let mut all: Vec<(usize, Vec<(f32, f32)>)> = handles.into_iter().flat_map(|t| t.join().unwrap()).collect();
            all.sort_by_key(|r| r.0);
            all.into_iter().map(|r| r.1).collect()
        });
        for (z, row) in rows.into_iter().enumerate() {
            for (x, (top, u)) in row.into_iter().enumerate() {
                h[z * n + x] = top;
                up[z * n + x] = u;
            }
        }
        // (Districts every 4 m, enough for borders kilometres apart.)
        let mut district = vec![0u8; n * n];
        let coarse = 16;
        for z in (0..n).step_by(coarse) {
            for x in (0..n).step_by(coarse) {
                let p = origin + (Vec2::new(x as f32, z as f32) + coarse as f32 / 2.0) * CELL;
                let d = District::ALL.iter().position(|&d| d == world.district(p.x as f64, p.y as f64)).unwrap() as u8;
                for zz in z..(z + coarse).min(n) {
                    for xx in x..(x + coarse).min(n) {
                        district[zz * n + xx] = d;
                    }
                }
            }
        }
        Raster { n, origin, h, up, district }
    }
}

/// What a climb makes of a raster, for one district's cells: its walkable
/// ground (cells), that in large and in walker-sized nodes, its nodes (of
/// a square metre or more) by size, the steps where nodes part, and how
/// much of its large nodes' ground touches the patch's edge.
#[derive(Default, Debug)]
struct Nodes {
    walkable: usize,
    large: usize,
    walker: usize,
    /// Nodes under 100 m², to 10 000, to 250 000, over (count, cells).
    sizes: [(usize, usize); 4],
    /// The longest side of the large nodes' bounds (m), each.
    extents: Vec<f32>,
    large_at_edge: usize,
    steps: [usize; 5],
}

fn nodes(r: &Raster, climb: f32, district: u8) -> Nodes {
    let n = r.n;
    let cos = SLOPE.to_radians().cos();
    let walk: Vec<bool> = (0..n * n).map(|i| r.h[i].is_finite() && r.up[i] >= cos - 1e-4).collect();
    let joined = |i: usize, j: usize| walk[i] && walk[j] && (r.h[i] - r.h[j]).abs() <= climb;
    let neighbours = |i: usize| {
        let (x, z) = (i % n, i / n);
        [(x > 0).then(|| i - 1), (x + 1 < n).then(|| i + 1), (z > 0).then(|| i - n), (z + 1 < n).then(|| i + n)]
    };
    // Distance (cells) from each cell's centre to the nearest edge it
    // can't cross: half a cell at a cell beside one; then a chamfer pass
    // each way. (The patch's own edge is no edge.)
    let mut dist = vec![f32::INFINITY; n * n];
    for i in 0..n * n {
        if !walk[i] {
            dist[i] = 0.0;
        } else if neighbours(i).into_iter().flatten().any(|j| !joined(i, j)) {
            dist[i] = 0.5;
        }
    }
    let diag = std::f32::consts::SQRT_2;
    for z in 0..n {
        for x in 0..n {
            let i = z * n + x;
            let mut d = dist[i];
            if x > 0 {
                d = d.min(dist[i - 1] + 1.0);
            }
            if z > 0 {
                d = d.min(dist[i - n] + 1.0);
                if x > 0 {
                    d = d.min(dist[i - n - 1] + diag);
                }
                if x + 1 < n {
                    d = d.min(dist[i - n + 1] + diag);
                }
            }
            dist[i] = d;
        }
    }
    for z in (0..n).rev() {
        for x in (0..n).rev() {
            let i = z * n + x;
            let mut d = dist[i];
            if x + 1 < n {
                d = d.min(dist[i + 1] + 1.0);
            }
            if z + 1 < n {
                d = d.min(dist[i + n] + 1.0);
                if x + 1 < n {
                    d = d.min(dist[i + n + 1] + diag);
                }
                if x > 0 {
                    d = d.min(dist[i + n - 1] + diag);
                }
            }
            dist[i] = d;
        }
    }
    let clear: Vec<bool> = (0..n * n).map(|i| walk[i] && dist[i] >= RADIUS / CELL).collect();
    // The nodes: clear cells joined.
    let mut parent: Vec<u32> = (0..(n * n) as u32).collect();
    fn find(p: &mut [u32], mut i: u32) -> u32 {
        while p[i as usize] != i {
            p[i as usize] = p[p[i as usize] as usize];
            i = p[i as usize];
        }
        i
    }
    for i in 0..n * n {
        if !clear[i] {
            continue;
        }
        for j in [(i % n + 1 < n).then(|| i + 1), (i / n + 1 < n).then(|| i + n)].into_iter().flatten() {
            if clear[j] && joined(i, j) {
                let (a, b) = (find(&mut parent, i as u32), find(&mut parent, j as u32));
                if a != b {
                    parent[a as usize] = b;
                }
            }
        }
    }
    // Each node: its cells, bounds, whether it touches the patch's edge.
    struct Node {
        cells: usize,
        lo: (usize, usize),
        hi: (usize, usize),
        edge: bool,
    }
    let mut roots = vec![u32::MAX; n * n];
    let mut found: HashMap<u32, Node> = HashMap::new();
    for i in 0..n * n {
        if !clear[i] {
            continue;
        }
        let root = find(&mut parent, i as u32);
        roots[i] = root;
        let (x, z) = (i % n, i / n);
        let node = found.entry(root).or_insert(Node { cells: 0, lo: (x, z), hi: (x, z), edge: false });
        node.cells += 1;
        node.lo = (node.lo.0.min(x), node.lo.1.min(z));
        node.hi = (node.hi.0.max(x), node.hi.1.max(z));
        node.edge |= x == 0 || z == 0 || x + 1 == n || z + 1 == n;
    }
    let area = |cells: usize| cells as f64 * (CELL * CELL) as f64;
    let mut out = Nodes::default();
    let mut counted = std::collections::HashSet::new();
    for i in 0..n * n {
        if r.district[i] != district {
            continue;
        }
        if walk[i] {
            out.walkable += 1;
            // (The steps where nodes part, once a pair, at the walker's
            // climb only.)
            if climb == CLIMBS[0] {
                for j in [(i % n + 1 < n).then(|| i + 1), (i / n + 1 < n).then(|| i + n)].into_iter().flatten() {
                    if walk[j] && !joined(i, j) {
                        let step = (r.h[i] - r.h[j]).abs();
                        out.steps[STEPS.iter().position(|&s| step <= s).unwrap()] += 1;
                    }
                }
            }
        }
        if !clear[i] {
            continue;
        }
        let node = &found[&roots[i]];
        let a = area(node.cells);
        if a >= LARGE {
            out.large += 1;
            if node.edge {
                out.large_at_edge += 1;
            }
        }
        if a >= WALKER {
            out.walker += 1;
        }
        if counted.insert(roots[i]) && a >= 1.0 {
            let bucket = [100.0, WALKER, LARGE, f64::INFINITY].iter().position(|&b| a < b).unwrap();
            out.sizes[bucket].0 += 1;
            out.sizes[bucket].1 += node.cells;
            if a >= LARGE {
                out.extents.push(((node.hi.0 - node.lo.0).max(node.hi.1 - node.lo.1) + 1) as f32 * CELL);
            }
        }
    }
    out
}

/// Where a district is most of the ground: the centre (on a 256 m grid)
/// whose measured square holds most of it, and that share.
fn patch_for(world: &PlateWorld, d: District) -> (Vec2, f32) {
    let size = worldgen::WORLD_SIZE;
    let side = PATCH - 2.0 * MARGIN;
    let mut best = (Vec2::ZERO, -1.0f32);
    let steps = (size / 256.0) as i32;
    for gz in 0..steps {
        for gx in 0..steps {
            let c = Vec2::new(gx as f32 + 0.5, gz as f32 + 0.5) * 256.0;
            let (mut hit, mut all) = (0, 0);
            for k in 0..14 {
                for i in 0..14 {
                    let p = c - side / 2.0 + (Vec2::new(i as f32, k as f32) + 0.5) * side / 14.0;
                    all += 1;
                    hit += (world.district(p.x as f64, p.y as f64) == d) as i32;
                }
            }
            let share = hit as f32 / all as f32;
            if share > best.1 {
                best = (c, share);
            }
        }
    }
    best
}

#[test]
#[ignore]
fn zz_nodes() {
    let library = crate::structures::load_library().expect("the structure library");
    let world = PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&library);
    let capped = {
        let mut w = PlateWorld::new(worldgen::WORLD_SIZE, 1);
        w.step_cap = Some(CLIMBS[0] as f64);
        w.with_sites(&library)
    };
    let mut json = String::from("{");
    for (k, d) in District::ALL.into_iter().enumerate() {
        let (centre, share) = patch_for(&world, d);
        let t = std::time::Instant::now();
        let raster = Raster::new(&world, centre);
        // (How far the world's own height is from its mesh, every 4 m.)
        let (mut off, mut far, mut seen) = (0, 0, 0);
        for z in (0..raster.n).step_by(16) {
            for x in (0..raster.n).step_by(16) {
                let i = z * raster.n + x;
                if raster.h[i].is_finite() {
                    let p = raster.origin + (Vec2::new(x as f32, z as f32) + 0.5) * CELL;
                    let e = (world.height_at(p.x, p.y) - raster.h[i]).abs();
                    seen += 1;
                    off += (e > 0.05) as usize;
                    far += (e > 0.5) as usize;
                }
            }
        }
        println!("\n== {} (patch at {:.0}, {:.0}: {:.0}% of it {}; sampled in {:.0} s; the world's height off its mesh by over 5 cm at {:.1}% of points, over 50 cm at {:.1}%)", d.name(), centre.x, centre.y, share * 100.0, d.name(), t.elapsed().as_secs_f32(), off as f32 / seen.max(1) as f32 * 100.0, far as f32 / seen.max(1) as f32 * 100.0);
        write!(json, "{}\"{}\": {{\"centre\": [{}, {}], \"share\": {share}, \"height_off_5cm\": {}, \"height_off_50cm\": {}, \"climbs\": {{", if k > 0 { ", " } else { "" }, d.name(), centre.x, centre.y, off as f32 / seen.max(1) as f32, far as f32 / seen.max(1) as f32).unwrap();
        let report = |label: &str, raster: &Raster, climb: f32, json: &mut String, first: bool| {
            let m = nodes(raster, climb, k as u8);
            let pct = |c: usize| c as f32 / m.walkable.max(1) as f32 * 100.0;
            let mut ext = m.extents.clone();
            ext.sort_by(f32::total_cmp);
            let band = |p: f32| if p >= 80.0 { "not forced" } else if p >= 40.0 { "mixed" } else { "forced" };
            println!(
                "{label} climb {climb:.2} m: large nodes {:.0}% of walkable ({}), walker-sized {:.0}%; nodes <100 m² {} ({:.0}%), to 1 ha {} ({:.0}%), to 25 ha {} ({:.0}%), larger {} ({:.0}%); large nodes' extent median {:.0} m; large ground at the patch edge {:.0}%",
                pct(m.large),
                if climb <= 0.8 { band(pct(m.large)) } else { "context" },
                pct(m.walker),
                m.sizes[0].0,
                pct(m.sizes[0].1),
                m.sizes[1].0,
                pct(m.sizes[1].1),
                m.sizes[2].0,
                pct(m.sizes[2].1),
                m.sizes[3].0,
                pct(m.sizes[3].1),
                ext.get(ext.len() / 2).copied().unwrap_or(0.0),
                m.large_at_edge as f32 / m.large.max(1) as f32 * 100.0
            );
            if climb == CLIMBS[0] {
                let total: usize = m.steps.iter().sum();
                println!("{label}   steps where nodes part, at 0.6 m: to 0.8 m {:.0}%, to 1.2 {:.0}%, to 2 {:.0}%, to 4 {:.0}%, more {:.0}%", m.steps.map(|s| s as f32 / total.max(1) as f32 * 100.0)[0], m.steps.map(|s| s as f32 / total.max(1) as f32 * 100.0)[1], m.steps.map(|s| s as f32 / total.max(1) as f32 * 100.0)[2], m.steps.map(|s| s as f32 / total.max(1) as f32 * 100.0)[3], m.steps.map(|s| s as f32 / total.max(1) as f32 * 100.0)[4]);
            }
            write!(json, "{}\"{label}{climb}\": {{\"walkable_cells\": {}, \"large\": {}, \"walker\": {}, \"sizes\": {:?}, \"large_extents\": {:?}, \"large_at_edge\": {}, \"steps\": {:?}}}", if first { "" } else { ", " }, m.walkable, m.large, m.walker, m.sizes.map(|s| [s.0, s.1]), ext, m.large_at_edge, m.steps).unwrap();
        };
        for (i, climb) in CLIMBS.into_iter().enumerate() {
            report("", &raster, climb, &mut json, i == 0);
        }
        drop(raster);
        let lever = Raster::new(&capped, centre);
        for climb in [0.6, 0.65, 0.8] {
            report("lever ", &lever, climb, &mut json, false);
        }
        json.push_str("}}");
    }
    json.push('}');
    let out = std::env::var("ZZ_NODES_OUT").unwrap_or_else(|_| "C:/work/projects/null/target/navbench/nodes.json".into());
    std::fs::write(out, json).unwrap();
}

/// Step 2a: where, in stacks, the world's height is off its mesh: at each
/// such point, every surface the mesh has there, and the plate the height
/// comes from.
#[test]
#[ignore]
fn zz_stacks_off() {
    let world = PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library"));
    let centre = Vec2::new(12928.0, 128.0);
    let made: Vec<Collider> = columns(&world, centre, 400.0).into_iter().filter_map(|(v, i)| Collider::try_trimesh(v, i).ok()).collect();
    let mut cache = worldgen::plates::Cache::default();
    let (mut shown, mut kinds): (usize, HashMap<String, usize>) = (0, HashMap::new());
    for k in 0..4000 {
        let p = centre + Vec2::new((k % 63) as f32 * 2.9 - 90.0, (k / 63) as f32 * 2.9 - 90.0);
        let mut tops: Vec<f32> = Vec::new();
        for c in &made {
            // (Every surface down the line, not only the top.)
            let mut from = 5000.0;
            while let Some((t, _)) = c.cast_ray(Vec3::ZERO, Quat::IDENTITY, Vec3::new(p.x, from, p.y), Vec3::NEG_Y, 10000.0, true) {
                tops.push(from - t);
                from = from - t - 0.01;
            }
        }
        tops.sort_by(|a, b| b.total_cmp(a));
        tops.dedup_by(|a, b| (*a - *b).abs() < 0.02);
        let Some(&top) = tops.first() else { continue };
        let h = world.height_at(p.x, p.y);
        if (h - top).abs() <= 0.5 {
            continue;
        }
        let plate = world.plate_at(&mut cache, p.x as f64, p.y as f64, 2);
        let kind = format!("height {} the mesh's top; plate level {}, pillar {}", if h > top { "above" } else { "below" }, plate.level, plate.pillar > 0.0);
        *kinds.entry(kind.clone()).or_default() += 1;
        if shown < 12 {
            shown += 1;
            println!("at {:.1} {:.1}: height_at {h:.2}; mesh surfaces {:?}; plate height {:.2}, level {}, pillar {:.2}, sites {:?}", p.x, p.y, tops.iter().take(5).map(|t| (t * 100.0).round() / 100.0).collect::<Vec<_>>(), plate.height, plate.level, plate.pillar, plate.sites);
        }
    }
    let mut kinds: Vec<_> = kinds.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, n) in kinds {
        println!("{n:5}  {k}");
    }
}
