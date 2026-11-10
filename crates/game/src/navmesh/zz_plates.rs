//! Step 2c of docs/outdoor-navigation.md: navigable nodes built by
//! construction from the generator's plates, for the biped.
//!
//! Outdoors the plates are a flat map seen from above (no plate over
//! another), so a body's walking space is exact in 2D: every plate piece,
//! less a band of the body's radius round every edge it can't cross (a step
//! over its climb, a canal's cut, a pillar). Nodes are what is left, joined
//! across the edges it can cross. A pillar plate is ground at its base,
//! less its grown prisms within a body's height of it; each prism's top is a
//! perch, a node of its own (walkers don't step between stacked slabs: they
//! teleport). A canal's cut is an edge, for now.
//!
//! `ZZ_DISTRICT=broken ZZ_SIDE=400 cargo test -p game --profile fast
//! zz_plates -- --ignored --nocapture`

use std::collections::HashMap;

use bevy::math::DVec2;
use geo::{Area, BooleanOps, BoundingRect, Distance, Buffer, Contains, Intersects, LineString, MultiLineString, MultiPolygon, Point, Polygon};
use worldgen::{district::District, plates::PlateWorld};

use super::nodes::{self, BIN, Fit, Piece, Space, Stretch, cells, polygon, section};

use super::zz_world::env;

/// The walker's radius, climb and height (m).
const RADIUS_: f64 = 0.35;
const CLIMB: f64 = 0.6;
/// The radius and height used (m): as set, or ZZ_radius(), ZZ_height() (to
/// hold the plates to Recast's rounding: its height is ceil(2.6 / 0.0875)
/// = 30 cells, 2.625 m).
fn radius() -> f64 {
    std::env::var("ZZ_RADIUS").ok().and_then(|v| v.parse().ok()).unwrap_or(RADIUS_)
}
fn height() -> f64 {
    std::env::var("ZZ_HEIGHT").ok().and_then(|v| v.parse().ok()).unwrap_or(HEIGHT_)
}
/// The climb used (m): `CLIMB`, or ZZ_CLIMB. (Recast keeps the climb in
/// whole cells: floor(0.6 / 0.0875) = 6, 0.525 m. Held against Detour, the
/// plates take the same.)
fn climb() -> f64 {
    std::env::var("ZZ_CLIMB").ok().and_then(|v| v.parse().ok()).unwrap_or(CLIMB)
}
const HEIGHT_: f64 = 2.6;
/// How far beyond the patch the pieces are gathered (m): its nodes, and
/// for 2d Detour's terrain, see the same ground there. (At 20 m, Detour's
/// terrain reached past the pieces, and its ways round through that strip
/// were taken for disagreements.)
const REACH: f64 = 200.0;
/// (Pillars whose perches the boolean library failed on: told, never
/// hidden.)
static FAILED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// (Strays shown, ZZ_DEEP.)
static DEEP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// (Time spent in the perches' parts, µs.)
static SPENT: [std::sync::atomic::AtomicU64; 3] = [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)];

#[test]
#[ignore]
fn zz_plates() {
    let world = PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library"));
    let name = std::env::var("ZZ_DISTRICT").unwrap_or_else(|_| "broken".into());
    let district = District::ALL.into_iter().find(|d| d.name() == name).expect("ZZ_DISTRICT: a district's name");
    let centres = [("floor", (10880.0, 128.0)), ("tiers", (5248.0, 640.0)), ("stacks", (12928.0, 128.0)), ("broken", (7296.0, 640.0)), ("uplands", (9344.0, 4736.0)), ("highlands", (9344.0, 2688.0))];
    let centre = DVec2::from(centres.iter().find(|c| c.0 == name).unwrap().1);
    let side = env("ZZ_SIDE", 400.0) as f64;
    let (lo, hi) = (centre - side / 2.0, centre + side / 2.0);
    let t = std::time::Instant::now();
    // The pieces round the patch, their walking space and nodes.
    // (The plates of the columns `columns` meshes over the patch and
    // `REACH` round it: those whose biggest plate's site lies in them, the
    // same set the terrain's mesh has.)
    let column = worldgen::column_size(0) as f64;
    let (box_lo, box_hi) = (((centre - side / 2.0 - REACH) / column).floor() * column, ((centre + side / 2.0 + REACH) / column).ceil() * column);
    let built = Space::build(&world, nodes::pieces(&world, box_lo, box_hi), Fit { radius: radius(), climb: climb(), height: height() });
    let (pieces, edges, space, node_of, joins) = (&built.pieces, &built.edges, &built.space, &built.node_of, built.joins);
    let (blocks, blocked) = (&built.blocks, &built.blocked);
    let near = |poly: &[DVec2]| built.closed_near(poly);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    println!("walking space: {} fragments, {} nodes, in {:.1} s", space.iter().map(|s| s.len()).sum::<usize>(), built.nodes(), t.elapsed().as_secs_f32());
    // (How many pillar prisms stand on a footprint that isn't convex: their
    // obstacle, the hull of their cross-sections, then reaches past them.)
    let (mut prisms_all, mut concave) = (0, 0);
    for p in pieces.iter().filter(|p| p.pillar) {
        for q in &p.prisms {
            prisms_all += 1;
            let n = q.0.len();
            let turns: Vec<f64> = (0..n).map(|i| (q.0[(i + 1) % n] - q.0[i]).perp_dot(q.0[(i + 2) % n] - q.0[(i + 1) % n])).collect();
            if turns.iter().any(|t| *t > 1e-9) && turns.iter().any(|t| *t < -1e-9) {
                concave += 1;
            }
        }
    }
    if prisms_all > 0 {
        println!("pillar prisms: {prisms_all}, {concave} on a footprint that isn't convex");
    }
    // The definition's tests. Every open stretch is met by one from the
    // piece across, the same ends; and inside the patch nothing lies
    // across a stretch only where a canal's cut is (a few centimetres out:
    // in the canal).
    let mut ends: HashMap<(usize, usize), Vec<(DVec2, DVec2)>> = HashMap::new();
    for e in edges.iter().filter(|e| e.open) {
        ends.entry((e.piece, e.other.unwrap())).or_default().push((e.a, e.b));
    }
    let unmatched = edges
        .iter()
        .filter(|e| e.open)
        .filter(|e| !ends.get(&(e.other.unwrap(), e.piece)).is_some_and(|v| v.iter().any(|&(a, b)| (a.distance(e.b) < 1e-3 && b.distance(e.a) < 1e-3) || (a.distance(e.a) < 1e-3 && b.distance(e.b) < 1e-3))))
        .count();
    let inner = |q: DVec2| q.x > lo.x && q.x < hi.x && q.y > lo.y && q.y < hi.y;
    let canal = |q: DVec2| world.canals.surface(q, &|c| world.canal_floor(c)).is_some();
    let gaps: Vec<&Stretch> = edges
        .iter()
        .filter(|e| e.other.is_none() && inner(e.a) && inner(e.b))
        .filter(|e| {
            let m = (e.a + e.b) / 2.0;
            let out = (e.b - e.a).normalize().perp() * 0.05;
            !canal(m + out) && !canal(m - out)
        })
        .collect();
    let holes = pieces.iter().filter(|p| p.hole && p.poly.iter().all(|&q| inner(q))).count();
    // (ZZ_HOLES: what the mesh has under each hole: a floor within 2 m of
    // its plate's top, something deeper, or nothing, the void.)
    if std::env::var("ZZ_HOLES").is_ok() {
        use avian3d::prelude::Collider;
        let made: Vec<Collider> = super::zz_world::columns(&world, bevy::math::Vec2::new(centre.x as f32, centre.y as f32), side as f32).into_iter().filter_map(|(v, i)| Collider::try_trimesh(v, i).ok()).collect();
        let (mut floor, mut deep, mut void) = (0, 0, 0);
        for p in pieces.iter().filter(|p| p.hole && p.poly.iter().all(|&q| inner(q))) {
            let c = p.poly.iter().fold(DVec2::ZERO, |s, q| s + *q) / p.poly.len() as f64;
            let top = world.plate_top(&p.plate, c) as f32;
            let from = bevy::math::Vec3::new(c.x as f32, top + 0.5, c.y as f32);
            let hit = made.iter().filter_map(|m| m.cast_ray(bevy::math::Vec3::ZERO, bevy::math::Quat::IDENTITY, from, bevy::math::Vec3::NEG_Y, 1000.0, true)).map(|(t, _)| t).fold(f32::INFINITY, f32::min);
            if hit <= 2.5 {
                floor += 1;
            } else if hit.is_finite() {
                deep += 1;
            } else {
                void += 1;
            }
        }
        println!("under the holes: a floor within 2 m {floor}, something deeper {deep}, nothing (the void) {void}");
    }
    println!(
        "tests: {unmatched} open stretches not met from across; {holes} holes (pieces left out of the mesh) inside the patch; {} stretches inside it with nothing across and no canal{}",
        gaps.len(),
        gaps.iter().take(3).map(|e| format!("; {:.2},{:.2} to {:.2},{:.2}", e.a.x, e.a.y, e.b.x, e.b.y)).collect::<String>()
    );
    println!("{name} ({district:?}): {} pieces ({} pillars), {} stretches ({} open, {} with nothing across) in {:.1} s", pieces.len(), pieces.iter().filter(|p| p.pillar).count(), edges.len(), edges.iter().filter(|e| e.open).count(), edges.iter().filter(|e| e.other.is_none()).count(), t.elapsed().as_secs_f32());

    let t = std::time::Instant::now();
    // Each node's area, within the patch.
    let clip = polygon(&[lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)]);
    let mut areas: HashMap<usize, f64> = HashMap::new();
    let mut walkable = 0.0;
    for (i, s) in space.iter().enumerate() {
        for (k, f) in s.iter().enumerate() {
            let a = f.intersection(&clip).unsigned_area();
            let root = node_of[i][k] as usize;
            *areas.entry(root).or_default() += a;
            walkable += a;
        }
    }
    // (The pieces' own ground within the patch, pillars aside, to set the
    // shares by, as the raster's walkable ground.)
    let ground: f64 = pieces.iter().filter(|p| !p.pillar && !p.hole).map(|p| polygon(&p.poly).intersection(&clip).unsigned_area()).sum();
    println!("measured in {:.1} s", t.elapsed().as_secs_f32());
    // (ZZ_PAIRS=dir: for 2d, the patch's terrain and settings as the bench
    // takes them, and pairs of points on the walking space, at least 200 m
    // inside the patch, each with whether its node is the other's: near
    // pairs (to 150 m) and far (to 1000 m). Then Detour, built by Recast on
    // the same terrain, is asked the same (detour/main.cpp's `nodes`).)
    if let Ok(dir) = std::env::var("ZZ_PAIRS") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let meshes = super::zz_world::columns(&world, bevy::math::Vec2::new(centre.x as f32, centre.y as f32), (side + 2.0 * REACH) as f32);
        let (mut vertices, mut triangles): (Vec<[f32; 3]>, Vec<[u32; 3]>) = (Vec::new(), Vec::new());
        for (v, t) in &meshes {
            let base = vertices.len() as u32;
            triangles.extend(t.iter().map(|t| t.map(|i| i + base)));
            vertices.extend(v.iter().map(|v| v.to_array()));
        }
        let mut bytes = Vec::new();
        bytes.extend((vertices.len() as u32).to_le_bytes());
        bytes.extend((triangles.len() as u32).to_le_bytes());
        for v in &vertices {
            for c in v {
                bytes.extend(c.to_le_bytes());
            }
        }
        for t in &triangles {
            for i in t {
                bytes.extend(i.to_le_bytes());
            }
        }
        std::fs::write(dir.join("terrain.bin"), bytes).unwrap();
        std::fs::copy("C:/work/projects/null/target/navbench/settings.txt", dir.join("settings.txt")).unwrap();
        // Points: fragments by area, a point in each by trial.
        let mut rng = super::zz_world::Rng(0x2545_f491_4f6c_dd1d);
        let core = |q: DVec2| q.x > lo.x + 200.0 && q.x < hi.x - 200.0 && q.y > lo.y + 200.0 && q.y < hi.y - 200.0;
        let frags: Vec<(usize, usize, f64, geo::Rect<f64>)> = space.iter().enumerate().flat_map(|(i, s)| s.iter().enumerate().filter_map(move |(k, f)| f.bounding_rect().map(|r| (i, k, f.unsigned_area(), r)))).collect();
        let total: f64 = frags.iter().map(|f| f.2).sum();
        let mut points: Vec<(DVec2, f64, usize)> = Vec::new();
        while points.len() < 20000 {
            let mut pick = rng.next() as f64 * total;
            let Some(&(i, k, _, r)) = frags.iter().find(|f| { pick -= f.2; pick <= 0.0 }) else { continue };
            for _ in 0..50 {
                let q = DVec2::new(r.min().x + (r.max().x - r.min().x) * rng.next() as f64, r.min().y + (r.max().y - r.min().y) * rng.next() as f64);
                if core(q) && space[i][k].contains(&Point::new(q.x, q.y)) {
                    points.push((q, world.plate_top(&pieces[i].plate, q), node_of[i][k] as usize));
                    break;
                }
            }
        }
        let mut lines = String::new();
        let mut same = [0usize; 2];
        for (reach, count) in [(150.0, 3000), (1000.0, 1000)] {
            let mut made = 0;
            while made < count {
                let a = points[(rng.next() as f64 * points.len() as f64) as usize % points.len()];
                let b = points[(rng.next() as f64 * points.len() as f64) as usize % points.len()];
                if a.0.distance(b.0) > reach || a.0.distance(b.0) < 2.0 {
                    continue;
                }
                made += 1;
                same[(a.2 == b.2) as usize] += 1;
                lines.push_str(&format!("{} {} {} {} {} {} {}\n", a.0.x, a.1, a.0.y, b.0.x, b.1, b.0.y, (a.2 == b.2) as u8));
            }
        }
        std::fs::write(dir.join("pairs.txt"), lines).unwrap();
        println!("pairs for 2d written to {}: {} on one node, {} on two", dir.display(), same[1], same[0]);
    }
    // (ZZ_TELEPORT: how much teleporting the district needs, as
    // docs/outdoor-navigation.md's teleport measure defines it: 2000
    // chases of 20 to 150 m over the nodes, the fewest teleports each takes
    // when a walker may teleport to any node within R metres, R 3, 5, 10,
    // 20; and the node edges the straight line crosses.)
    if std::env::var("ZZ_TELEPORT").is_ok() {
        let t = std::time::Instant::now();
        let node_area = &built.node_area;
        // Each pair of nodes' least distance (3D), where 20 m or less.
        const FAR: f64 = 20.0;
        let hops = built.hops(&world, FAR);
        let near: HashMap<(u32, u32), f64> = hops.iter().map(|(k, v)| (*k, v.iter().map(|h| h.d as f64).fold(f64::INFINITY, f64::min))).collect();
        println!("teleport measure: {} nodes, {} hops, {} node pairs within {FAR} m, in {:.1} s", node_area.len(), hops.values().map(|v| v.len()).sum::<usize>(), near.len(), t.elapsed().as_secs_f32());
        // Chases: points on the walking space at least 200 m inside the
        // patch, each to another 20 to 150 m away.
        let mut rng = super::zz_world::Rng(0x0d1b_2c3a_4f5e_6d7c);
        let core = |q: DVec2| q.x > lo.x + 200.0 && q.x < hi.x - 200.0 && q.y > lo.y + 200.0 && q.y < hi.y - 200.0;
        let frags: Vec<(usize, usize, f64, geo::Rect<f64>)> = space.iter().enumerate().flat_map(|(i, s)| s.iter().enumerate().filter_map(move |(k, f)| f.bounding_rect().map(|r| (i, k, f.unsigned_area(), r)))).collect();
        let total: f64 = frags.iter().map(|f| f.2).sum();
        let mut spots: Vec<(DVec2, u32)> = Vec::new();
        while spots.len() < 20000 {
            let mut pick = rng.next() as f64 * total;
            let Some(&(i, k, _, r)) = frags.iter().find(|f| {
                pick -= f.2;
                pick <= 0.0
            }) else {
                continue;
            };
            for _ in 0..50 {
                let q = DVec2::new(r.min().x + (r.max().x - r.min().x) * rng.next() as f64, r.min().y + (r.max().y - r.min().y) * rng.next() as f64);
                if core(q) && space[i][k].contains(&Point::new(q.x, q.y)) {
                    spots.push((q, node_of[i][k]));
                    break;
                }
            }
        }
        let mut chases: Vec<((DVec2, u32), (DVec2, u32))> = Vec::new();
        while chases.len() < 2000 {
            let a = spots[(rng.next() as f64 * spots.len() as f64) as usize % spots.len()];
            let b = spots[(rng.next() as f64 * spots.len() as f64) as usize % spots.len()];
            let d = a.0.distance(b.0);
            if (20.0..=150.0).contains(&d) {
                chases.push((a, b));
            }
        }
        // The straight line's node crossings, every 25 cm.
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, p) in pieces.iter().enumerate() {
            let (mut a, mut b) = (p.poly[0], p.poly[0]);
            for q in &p.poly {
                a = a.min(*q);
                b = b.max(*q);
            }
            for c in cells(a, b, 0.0) {
                grid.entry(c).or_default().push(i);
            }
        }
        let node_at = |q: DVec2| -> Option<u32> {
            let i = grid.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32))?.iter().copied().find(|&i| polygon(&pieces[i].poly).contains(&Point::new(q.x, q.y)))?;
            let k = space[i].iter().position(|f| f.contains(&Point::new(q.x, q.y)))?;
            Some(node_of[i][k])
        };
        let mut crossings: Vec<f64> = chases
            .iter()
            .map(|(a, b)| {
                let n = (a.0.distance(b.0) / 0.25).ceil() as usize;
                let (mut last, mut count) = (Some(a.1), 0);
                for k in 1..=n {
                    if let Some(m) = node_at(a.0.lerp(b.0, k as f64 / n as f64)) {
                        if last.is_some_and(|l| l != m) {
                            count += 1;
                        }
                        last = Some(m);
                    }
                }
                count as f64 / a.0.distance(b.0) * 100.0
            })
            .collect();
        crossings.sort_by(f64::total_cmp);
        let same = chases.iter().filter(|(a, b)| a.1 == b.1).count();
        println!("chases: 2000 of 20 to 150 m; start and target on one node {:.0}%; the straight line crosses node edges, per 100 m: median {:.1}, worst tenth {:.1}", same as f64 / 20.0, crossings[crossings.len() / 2], crossings[crossings.len() * 9 / 10]);
        // The fewest teleports, by range, over the node graph.
        for least in [0.0, 10.0] {
            for range in [3.0, 5.0, 10.0, 20.0] {
                let mut adjacent: Vec<Vec<u32>> = vec![Vec::new(); node_area.len()];
                for (&(a, b), &d) in &near {
                    if d <= range {
                        if node_area[b as usize] >= least {
                            adjacent[a as usize].push(b);
                        }
                        if node_area[a as usize] >= least {
                            adjacent[b as usize].push(a);
                        }
                    }
                }
                let (adjacent, near) = (&adjacent, &near);
                let hops: Vec<Option<(usize, f64)>> = std::thread::scope(|scope| {
                    let chunk = chases.len().div_ceil(threads).max(1);
                    let handles: Vec<_> = chases
                        .chunks(chunk)
                        .map(|c| {
                            scope.spawn(move || {
                                c.iter()
                                    .map(|(a, b)| {
                                        // (Breadth first: the fewest teleports. The
                                        // target's own node is always a landing.)
                                        let mut seen = vec![u32::MAX; adjacent.len()];
                                        let mut queue = std::collections::VecDeque::from([a.1]);
                                        seen[a.1 as usize] = 0;
                                        while let Some(n) = queue.pop_front() {
                                            if n == b.1 {
                                                return Some((seen[n as usize] as usize, a.0.distance(b.0)));
                                            }
                                            for &m in &adjacent[n as usize] {
                                                if seen[m as usize] == u32::MAX {
                                                    seen[m as usize] = seen[n as usize] + 1;
                                                    queue.push_back(m);
                                                }
                                            }
                                            if least > 0.0 && seen[b.1 as usize] == u32::MAX && near.get(&(n.min(b.1), n.max(b.1))).is_some_and(|d| *d <= range) {
                                                seen[b.1 as usize] = seen[n as usize] + 1;
                                                queue.push_back(b.1);
                                            }
                                        }
                                        None
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .collect();
                    handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
                });
                let reached: Vec<(usize, f64)> = hops.iter().flatten().copied().collect();
                let mut counts: Vec<usize> = reached.iter().map(|r| r.0).collect();
                counts.sort();
                let per100 = reached.iter().map(|r| r.0 as f64 / r.1 * 100.0).sum::<f64>() / reached.len().max(1) as f64;
                println!(
                    "  range {range:>4} m{}: unreachable {:.1}%; teleports a chase median {}, worst tenth {}, most {}; teleports per 100 m {:.2}",
                    if least > 0.0 { ", landing on nodes of 10 m2 or more" } else { "" },
                    (hops.len() - reached.len()) as f64 / hops.len() as f64 * 100.0,
                    counts.get(counts.len() / 2).copied().unwrap_or(0),
                    counts.get(counts.len() * 9 / 10).copied().unwrap_or(0),
                    counts.last().copied().unwrap_or(0),
                    per100
                );
            }
        }
    }
    // (ZZ_CHECK=dir: Detour's answers there (dir/detour.txt) held against
    // the plates. Each way Detour found that the plates' nodes don't have,
    // followed every 10 cm over the pieces: where it leaves the walking
    // space, and the steps it crosses between pieces.)
    if let Ok(dir) = std::env::var("ZZ_CHECK") {
        let dir = std::path::PathBuf::from(dir);
        let pairs = std::fs::read_to_string(dir.join("pairs.txt")).unwrap();
        let answers = std::fs::read_to_string(dir.join("detour.txt")).unwrap();
        // Pieces by where they lie, to find the one under a point.
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, p) in pieces.iter().enumerate() {
            let (mut a, mut b) = (p.poly[0], p.poly[0]);
            for q in &p.poly {
                a = a.min(*q);
                b = b.max(*q);
            }
            for c in cells(a, b, 0.0) {
                grid.entry(c).or_default().push(i);
            }
        }
        let under = |q: DVec2| -> Option<usize> { grid.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32))?.iter().copied().find(|&i| polygon(&pieces[i].poly).contains(&Point::new(q.x, q.y))) };
        let mut shown = 0;
        // (Every way Detour found, followed every 10 cm: how much of it
        // lies more than 5 cm outside the walking space, or in a canal.)
        let (mut walked, mut strayed, mut canal_walked, mut worst) = (0.0, 0.0, 0.0, (0.0, DVec2::ZERO));
        // (How far out, where out: to 15 cm (Recast's edge error, 1.3
        // cells), to the radius, beyond it, and over no piece at all.)
        let mut depths = [0.0f64; 4];
        for answer in answers.lines() {
            let a: Vec<f64> = answer.split_whitespace().map(|v| v.parse().unwrap()).collect();
            if a[1] == 0.0 {
                continue;
            }
            let corners: Vec<DVec2> = a[3..].chunks(3).map(|c| DVec2::new(c[0], c[2])).collect();
            let mut out_here = 0.0;
            for w in corners.windows(2) {
                let n = (w[0].distance(w[1]) / 0.1).ceil().max(1.0) as usize;
                for k in 0..n {
                    let q = w[0].lerp(w[1], (k as f64 + 0.5) / n as f64);
                    let step = w[0].distance(w[1]) / n as f64;
                    walked += step;
                    if canal(q) {
                        canal_walked += step;
                        continue;
                    }
                    let pt = Point::new(q.x, q.y);
                    let inside = under(q).is_some_and(|i| space[i].iter().any(|f| f.contains(&pt) || geo::Euclidean.distance(f, &pt) <= 0.05));
                    if !inside {
                        strayed += step;
                        out_here += step;
                        let depth = under(q).map(|i| {
                            // (To the nearest walking space of its piece or
                            // those next to it.)
                            let near: Vec<usize> = grid.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32)).cloned().unwrap_or_default();
                            std::iter::once(i).chain(near).flat_map(|j| space[j].iter()).map(|f| geo::Euclidean.distance(f, &pt)).fold(f64::INFINITY, f64::min)
                        });
                        depths[match depth { Some(d) if d <= 0.15 => 0, Some(d) if d <= radius() => 1, Some(_) => 2, None => 3 }] += step;
                        // (ZZ_DEEP: the first few strays beyond the radius,
                        // what is there.)
                        if std::env::var("ZZ_DEEP").is_ok() && depth.is_some_and(|d| d > radius()) && DEEP.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 12 {
                            let i = under(q).unwrap();
                            let y = a[3..].chunks(3).zip(a[3..].chunks(3).skip(1)).find(|(c0, c1)| { let (p0, p1) = (DVec2::new(c0[0], c0[2]), DVec2::new(c1[0], c1[2])); (q - p0).perp_dot(p1 - p0).abs() < 1e-6 * p1.distance(p0).max(1.0) || p0.distance(q) + q.distance(p1) <= p0.distance(p1) + 1e-6 }).map(|(c0, _)| c0[1]).unwrap_or(f64::NAN);
                            let near_blocks = blocked.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32)).map(|v| v.iter().filter(|&&k| blocks[k].contains(&pt)).count()).unwrap_or(0);
                            println!("  stray {:.2} m out at {:.2}, {:.2}: way's height {:.2}; over piece {} (pillar {}, ground {:.2}, prisms {}); inside {} pillar obstacles; piece's walking space {} fragments", depth.unwrap(), q.x, q.y, y, i, pieces[i].pillar, world.plate_top(&pieces[i].plate, q), pieces[i].prisms.len(), near_blocks, space[i].len());
                        }
                    }
                }
            }
            if out_here > worst.0 {
                worst = (out_here, corners[0]);
            }
        }
        println!("Detour's ways held against the walking space: {walked:.0} m walked, {strayed:.1} m ({:.3}%) more than 5 cm outside it (to 15 cm out {:.1} m, to the radius {:.1} m, beyond {:.1} m, over no piece {:.1} m), {canal_walked:.1} m in canals; the worst way {:.1} m outside, from {:.1}, {:.1}", strayed / walked.max(1e-9) * 100.0, depths[0], depths[1], depths[2], depths[3], worst.0, worst.1.x, worst.1.y);
        // (Each pair the two disagree on.)
        for (pair, answer) in pairs.lines().zip(answers.lines()) {
            let a: Vec<f64> = answer.split_whitespace().map(|v| v.parse().unwrap()).collect();
            let (floor_ok, found, same) = (a[0] as u8, a[1] as u8, a[2] as u8);
            if floor_ok == 0 || found == same {
                if floor_ok == 0 && shown < 40 {
                    shown += 1;
                    println!("off Detour's floor: {pair}");
                }
                continue;
            }
            if found == 0 {
                if shown < 40 {
                    shown += 1;
                    // (Where Detour's partial way stopped, and the plates'
                    // open stretches within 3 m of it: their length, what
                    // of it is left past the radius round closed ones, and
                    // the step.)
                    let stop = a.get(3..6).map(|c| DVec2::new(c[0], c[2]));
                    println!("a way only by the plates' nodes: {pair}; Detour stopped at {stop:?}");
                    if let Some(stop) = stop {
                        for e in edges.iter().filter(|e| e.open && e.other.is_some_and(|j| j > e.piece)) {
                            let d = (stop - e.a).dot(e.b - e.a) / (e.b - e.a).length_squared();
                            let near_point = e.a.lerp(e.b, d.clamp(0.0, 1.0));
                            if near_point.distance(stop) > 3.0 {
                                continue;
                            }
                            let line = MultiLineString::new(vec![LineString::from(vec![(e.a.x, e.a.y), (e.b.x, e.b.y)])]);
                            let left: f64 = near(&[e.a, e.b]).buffer(radius()).clip(&line, true).0.iter().map(|l| { let p = &l.0; DVec2::new(p[0].x, p[0].y).distance(DVec2::new(p[p.len() - 1].x, p[p.len() - 1].y)) }).sum();
                            let m = (e.a + e.b) / 2.0;
                            let step = (world.plate_top(&pieces[e.piece].plate, m) - world.plate_top(&pieces[e.other.unwrap()].plate, m)).abs();
                            println!("    open stretch {:.2} m long, {left:.2} m left past the radius, step {step:.2} m, {:.1} m from the stop; pieces {} {} (pillar {} {}, sloped {} {})", e.a.distance(e.b), near_point.distance(stop), e.piece, e.other.unwrap(), pieces[e.piece].pillar, pieces[e.other.unwrap()].pillar, pieces[e.piece].plate.sloped, pieces[e.other.unwrap()].plate.sloped);
                        }
                    }
                }
                continue;
            }
            let corners: Vec<DVec2> = a[3..].chunks(3).map(|c| DVec2::new(c[0], c[2])).collect();
            let (mut outside, mut in_canal, mut worst_step, mut at) = (0.0, 0.0, 0.0, DVec2::ZERO);
            let mut last: Option<usize> = None;
            for w in corners.windows(2) {
                let n = (w[0].distance(w[1]) / 0.1).ceil().max(1.0) as usize;
                for k in 0..=n {
                    let q = w[0].lerp(w[1], k as f64 / n as f64);
                    if canal(q) {
                        in_canal += 0.1;
                        continue;
                    }
                    let Some(i) = under(q) else { outside += 0.1; continue };
                    if !space[i].iter().any(|f| f.contains(&Point::new(q.x, q.y))) {
                        outside += 0.1;
                    }
                    if let Some(j) = last.filter(|&j| j != i) {
                        let step = (world.plate_top(&pieces[i].plate, q) - world.plate_top(&pieces[j].plate, q)).abs();
                        if step > worst_step {
                            worst_step = step;
                            at = q;
                        }
                    }
                    last = Some(i);
                }
            }
            if shown < 40 {
                shown += 1;
                println!("a way only by Detour: {in_canal:.1} m of it in a canal, {outside:.1} m elsewhere outside the walking space; its biggest step between pieces {worst_step:.2} m, at {:.2}, {:.2}", at.x, at.y);
                // (Where the way passes from one node to another, if it never
                // leaves the walking space: the two fragments, and the open
                // stretches between their pieces near there.)
                if outside < 0.05 {
                    let mut was: Option<(usize, usize)> = None;
                    'walk: for w in corners.windows(2) {
                        let n = (w[0].distance(w[1]) / 0.05).ceil().max(1.0) as usize;
                        for k in 0..=n {
                            let q = w[0].lerp(w[1], k as f64 / n as f64);
                            let Some(i) = under(q) else { continue };
                            let Some(f) = space[i].iter().position(|f| f.contains(&Point::new(q.x, q.y))) else { continue };
                            if let Some((pi, pf)) = was {
                                let (x, y) = (node_of[pi][pf], node_of[i][f]);
                                if x != y {
                                    println!("    node changes at {:.2}, {:.2}: piece {pi} fragment {pf} to piece {i} fragment {f}", q.x, q.y);
                                    for e in edges.iter().filter(|e| (e.piece == pi && e.other == Some(i)) || (e.piece == i && e.other == Some(pi))) {
                                        let line = MultiLineString::new(vec![LineString::from(vec![(e.a.x, e.a.y), (e.b.x, e.b.y)])]);
                                        let left: f64 = near(&[e.a, e.b]).buffer(radius()).clip(&line, true).0.iter().map(|l| { let p = &l.0; DVec2::new(p[0].x, p[0].y).distance(DVec2::new(p[p.len() - 1].x, p[p.len() - 1].y)) }).sum();
                                        println!("      stretch {:.2},{:.2} to {:.2},{:.2}: open {}, {left:.3} m left past the radius", e.a.x, e.a.y, e.b.x, e.b.y, e.open);
                                    }
                                    break 'walk;
                                }
                            }
                            was = Some((i, f));
                        }
                    }
                }
            }
        }
    }
    let mut sizes: Vec<f64> = areas.values().copied().filter(|a| *a >= 1.0).collect();
    sizes.sort_by(|a, b| b.total_cmp(a));
    let share = |least: f64| sizes.iter().filter(|a| **a >= least).fold(0.0, |s, a| s + a) / ground * 100.0;
    println!(
        "{} joins; {} nodes of 1 m² or more; walking space {:.0}% of the ground; in nodes of 1 ha or more {:.0}%, of 25 ha or more {:.0}%; largest {:?} m²",
        joins,
        sizes.len(),
        walkable / ground * 100.0,
        share(10_000.0),
        share(250_000.0),
        sizes.iter().take(5).map(|a| a.round()).collect::<Vec<_>>()
    );

    // Perches: each prism's top, less the radius at its edge and less
    // whatever of its pillar lies within a body's height over it; each a
    // node of its own. (And how many pairs of tops touch within a climb of
    // each other, or a top lies within a climb of its ground: steps a
    // walker could take, teleports here.)
    let t = std::time::Instant::now();
    let mine: Vec<&Piece> = pieces.iter().filter(|p| p.pillar && p.poly.iter().all(|&q| inner(q))).collect();
    let count = |p: &Piece| -> (usize, f64, usize, usize) {
        let (mut perches, mut area, mut pairs, mut low) = (0, 0.0, 0, 0);
        let tops: Vec<Polygon<f64>> = p.prisms.iter().map(|q| polygon(&q.1)).collect();
        let t0 = std::time::Instant::now();
        let reach: Vec<Option<(MultiPolygon<f64>, geo::Rect<f64>)>> = tops.iter().map(|t| { let r = t.buffer(2.0 * radius()); r.bounding_rect().map(|b| (r, b)) }).collect();
        SPENT[0].fetch_add(t0.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
        // (Each prism's bounds across, to pass by those nowhere near.)
        let bounds: Vec<(DVec2, DVec2)> = p.prisms.iter().map(|q| q.0.iter().chain(&q.1).fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), v| (a.min(*v), b.max(*v)))).collect();
        let touch = |k: usize, m: usize, pad: f64| bounds[k].0.x - pad <= bounds[m].1.x && bounds[m].0.x <= bounds[k].1.x + pad && bounds[k].0.y - pad <= bounds[m].1.y && bounds[m].0.y <= bounds[k].1.y + pad;
        for (k, q) in p.prisms.iter().enumerate() {
            let y = q.3;
            let over: Vec<Polygon<f64>> = p
                .prisms
                .iter()
                .enumerate()
                .filter(|&(m, o)| m != k && o.3 > y + 1e-3 && o.2 < y + height() && touch(k, m, radius()))
                .filter_map(|(_, o)| section(o, y + 1e-3, y + height()))
                .collect();
            // (Where the body's disc lies on the top and clear of all over
            // it: the top less what is over it, shrunk by the radius, the
            // same as the top shrunk less what is over it grown, and far
            // cheaper.)
            let t0 = std::time::Instant::now();
            let mut left = MultiPolygon::new(vec![tops[k].clone()]);
            for o in &over {
                left = left.difference(o);
            }
            SPENT[2].fetch_add(t0.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
            let t0 = std::time::Instant::now();
            let free = left.buffer(-radius());
            SPENT[1].fetch_add(t0.elapsed().as_micros() as u64, std::sync::atomic::Ordering::Relaxed);
            let kept: Vec<&Polygon<f64>> = free.0.iter().filter(|f| f.unsigned_area() >= 0.5).collect();
            perches += kept.len();
            area += kept.iter().map(|f| f.unsigned_area()).sum::<f64>();
            if y - p.plate.height <= climb() {
                low += 1;
            }
            for (m, o) in p.prisms.iter().enumerate().skip(k + 1) {
                if (o.3 - y).abs() > climb() || !touch(k, m, 2.0 * radius()) {
                    continue;
                }
                let (Some((r, b)), Some(tb)) = (&reach[k], tops[m].bounding_rect()) else { continue };
                if (o.3 - y).abs() <= climb() && b.intersects(&tb) && r.intersects(&tops[m]) {
                    pairs += 1;
                }
            }
        }
        (perches, area, pairs, low)
    };
    let counted: Vec<(usize, f64, usize, usize)> = std::thread::scope(|scope| {
        let chunk = mine.len().div_ceil(threads * 8).max(1);
        let handles: Vec<_> = mine
            .chunks(chunk)
            .map(|c| {
                scope.spawn(move || {
                    c.iter()
                        .map(|p| {
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| count(p))).unwrap_or_else(|_| {
                                FAILED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                (0, 0.0, 0, 0)
                            })
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    // How far up its pillar a walker gets from the ground: the usable tops
    // (as above) joined where within a climb of each other and nearly
    // touching (a gap of the two radii their shrinking leaves), the ground
    // joined to tops within a climb of it; the highest top so reached, over
    // the ground, against its pillar's height. And, for each top, the rise
    // to the nearest top touching it that is higher: how big the steps are.
    let idx: Vec<usize> = (0..pieces.len()).filter(|&i| pieces[i].pillar && pieces[i].poly.iter().all(|&q| inner(q))).collect();
    let climbed = |i: usize| -> (f64, f64, Vec<f64>) {
        let p = &pieces[i];
        let floor = p.plate.height;
        let mut frags: Vec<(f64, Polygon<f64>)> = Vec::new();
        let tops: Vec<Polygon<f64>> = p.prisms.iter().map(|q| polygon(&q.1)).collect();
        for (k, q) in p.prisms.iter().enumerate() {
            let y = q.3;
            let mut left = MultiPolygon::new(vec![tops[k].clone()]);
            for o in p.prisms.iter().enumerate().filter(|&(m, o)| m != k && o.3 > y + 1e-3 && o.2 < y + height()).filter_map(|(_, o)| section(o, y + 1e-3, y + height())) {
                left = left.difference(&o);
            }
            for f in left.buffer(-radius()).0 {
                if f.unsigned_area() >= 0.5 {
                    frags.push((y, f));
                }
            }
        }
        let n = frags.len();
        let mut parent: Vec<usize> = (0..=n).collect();
        fn root(p: &mut [usize], mut i: usize) -> usize {
            while p[i] != i {
                p[i] = p[p[i]];
                i = p[i];
            }
            i
        }
        let near = |a: &Polygon<f64>, b: &Polygon<f64>| geo::Euclidean.distance(a, b) <= 2.0 * radius() + 0.05;
        for a in 0..n {
            if frags[a].0 - floor <= climb() && space[i].iter().any(|g| near(g, &frags[a].1)) {
                let (x, y) = (root(&mut parent, a), root(&mut parent, n));
                parent[x] = y;
            }
            for b in a + 1..n {
                if (frags[a].0 - frags[b].0).abs() <= climb() && near(&frags[a].1, &frags[b].1) {
                    let (x, y) = (root(&mut parent, a), root(&mut parent, b));
                    parent[x] = y;
                }
            }
        }
        let g = root(&mut parent, n);
        let highest = (0..n).filter(|&a| root(&mut parent, a) == g).map(|a| frags[a].0 - floor).fold(0.0, f64::max);
        let tallest = p.prisms.iter().map(|q| q.3 - floor).fold(0.0, f64::max);
        // (The rise to the nearest higher top touching each.)
        let mut rises = Vec::new();
        for (k, q) in p.prisms.iter().enumerate() {
            let up = p.prisms.iter().enumerate().filter(|&(m, o)| m != k && o.3 > q.3 + 1e-3 && geo::Euclidean.distance(&tops[k], &tops[m]) <= 2.0 * radius()).map(|(_, o)| o.3 - q.3).fold(f64::INFINITY, f64::min);
            if up.is_finite() {
                rises.push(up);
            }
        }
        (highest, tallest, rises)
    };
    let results: Vec<(usize, (f64, f64, Vec<f64>))> = std::thread::scope(|scope| {
        let chunk = idx.len().div_ceil(threads * 8).max(1);
        let handles: Vec<_> = idx.chunks(chunk).map(|c| scope.spawn(move || c.iter().filter_map(|&i| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| climbed(i))).ok().map(|r| (i, r))).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let n = results.len().max(1) as f64;
    let share = |f: &dyn Fn(&(f64, f64, Vec<f64>)) -> bool| results.iter().filter(|r| f(&r.1)).count() as f64 / n * 100.0;
    let mut rises: Vec<f64> = results.iter().flat_map(|r| r.1 .2.iter().copied()).collect();
    rises.sort_by(f64::total_cmp);
    let q = |f: f64| rises.get(((rises.len() as f64 * f) as usize).min(rises.len().saturating_sub(1))).copied().unwrap_or(0.0);
    println!(
        "pillars climbed by walking from the ground ({} pillars, {} failed): reach 1.2 m or more {:.0}%, 2.6 m or more {:.0}%, 5 m or more {:.0}%, their top perch {:.0}%; pillars' height median {:.1} m; rise to the next higher touching top: median {:.2} m, quarter {:.2}, three quarters {:.2}, within the climb {:.0}%",
        results.len(),
        idx.len() - results.len(),
        share(&|r| r.0 >= 1.2),
        share(&|r| r.0 >= 2.6),
        share(&|r| r.0 >= 5.0),
        share(&|r| r.1 > 0.0 && r.0 >= r.1 - 1e-6),
        { let mut t: Vec<f64> = results.iter().map(|r| r.1 .1).collect(); t.sort_by(f64::total_cmp); t.get(t.len() / 2).copied().unwrap_or(0.0) },
        q(0.5),
        q(0.25),
        q(0.75),
        rises.iter().filter(|r| **r <= climb()).count() as f64 / rises.len().max(1) as f64 * 100.0
    );
    // (The most climbable pillar, and a median one: where to look.)
    let mut by: Vec<&(usize, (f64, f64, Vec<f64>))> = results.iter().collect();
    by.sort_by(|a, b| a.1 .0.total_cmp(&b.1 .0));
    for (label, r) in [("most climbed", by.last()), ("median", by.get(by.len() / 2))] {
        if let Some((i, (h, t, _))) = r {
            let c = pieces[*i].poly.iter().fold(DVec2::ZERO, |s, q| s + *q) / pieces[*i].poly.len() as f64;
            println!("  {label}: pillar at {:.1}, {:.1}, ground {:.2} m, {:.1} m tall, walked up to {:.1} m", c.x, c.y, pieces[*i].plate.height, t, h);
        }
    }
    println!("perch time (s of all cores): reach {:.1}, shrink {:.1}, headroom {:.1}", SPENT[0].load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6, SPENT[1].load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6, SPENT[2].load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6);
    let (perches, perch_area, near_pairs, low) = counted.iter().fold((0, 0.0, 0, 0), |s, c| (s.0 + c.0, s.1 + c.1, s.2 + c.2, s.3 + c.3));
    println!(
        "perches: {perches} of half a square metre or more, {perch_area:.0} m² in all, on {} pillars ({} failed in the boolean library); {near_pairs} pairs of tops touching within a climb, {low} tops within a climb of their ground (steps not walked: teleports); in {:.1} s",
        mine.len(),
        FAILED.load(std::sync::atomic::Ordering::Relaxed),
        t.elapsed().as_secs_f32()
    );
}
