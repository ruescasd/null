//! The navigation bench (tools/navbench/CRITERIA.md; the user, 2026-11-01:
//! the same test on every candidate, judged by criteria fixed
//! beforehand): A1's patch of terrain, both bodies' settings and the cases,
//! all from the terrain alone, written for every candidate; then the cases
//! run on landmass, ours.
//!
//! `ZZ_BENCH=<dir> cargo test -p game --profile fast zz_bench -- --ignored
//! --nocapture` writes terrain.bin, settings.txt and cases.txt there, then
//! results/landmass.json.

use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    sync::Arc,
    time::Instant,
};

use avian3d::prelude::{Collider, SimpleCollider};
use bevy::{math::DVec2, prelude::*};
use bevy_landmass::coords::ThreeD;

use super::zz_world::{Rng, columns, env, ms, walk};
use super::{Body, Floors, bounds, build, config, options, tile_side};

/// A group of cases: its name, how far across its points may be put on a
/// floor (m), and its points (pairs, or a route's corners).
struct Group {
    name: &'static str,
    reach: f32,
    points: Vec<Vec<Vec3>>,
}

#[test]
#[ignore]
fn zz_bench() {
    let dir = std::path::PathBuf::from(std::env::var("ZZ_BENCH").expect("ZZ_BENCH, the directory to write the bench into"));
    std::fs::create_dir_all(dir.join("results")).unwrap();
    let side = env("ZZ_SIDE", 2000.0);
    let at = Vec2::new(env("ZZ_X", 8192.0), env("ZZ_Z", 8192.0));
    let world = worldgen::plates::PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library"));
    let meshes = columns(&world, at, side);

    // The terrain: vertex and triangle counts (u32), the vertices (f32 x, y,
    // z, the world's frame), the triangles (u32 each corner).
    let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
    for (v, t) in &meshes {
        let base = vertices.len() as u32;
        triangles.extend(t.iter().map(|t| t.map(|i| i + base)));
        vertices.extend_from_slice(v);
    }
    let mut bytes = Vec::with_capacity(8 + vertices.len() * 12 + triangles.len() * 12);
    bytes.extend((vertices.len() as u32).to_le_bytes());
    bytes.extend((triangles.len() as u32).to_le_bytes());
    for v in &vertices {
        for c in v.to_array() {
            bytes.extend(c.to_le_bytes());
        }
    }
    for t in &triangles {
        for i in t {
            bytes.extend(i.to_le_bytes());
        }
    }
    std::fs::write(dir.join("terrain.bin"), bytes).unwrap();
    println!("terrain: {} vertices, {} triangles", vertices.len(), triangles.len());

    // Each body's settings, as Recast takes them (cells and metres).
    let mut settings = String::new();
    for body in Body::ALL {
        let (radius, height, climb, slope) = body.size();
        let c = config(body.size(), Vec3::ZERO, Vec3::ONE);
        writeln!(
            settings,
            "body {body:?} radius {radius} height {height} climb {climb} slope {slope} cell_size {} cell_height {} walkable_height {} walkable_climb {} walkable_radius {} tile_cells {} border_cells {} tile_side {} min_region_area {} merge_region_area {} max_edge_len {} max_error {} max_vertices {} detail_sample_dist {} detail_sample_max_error {}",
            c.cell_size,
            c.cell_height,
            c.walkable_height,
            c.walkable_climb,
            c.walkable_radius,
            c.tile_size,
            c.border_size,
            tile_side(body.size()),
            c.min_region_area,
            c.merge_region_area,
            c.max_edge_len,
            c.max_simplification_error,
            c.max_vertices_per_polygon,
            c.detail_sample_dist,
            c.detail_sample_max_error,
        )
        .unwrap();
    }
    writeln!(settings, "agent speed 5 max_speed 7.5 follower_speed 7.5 follower_max_speed 11 frame 0.016666667 stream_reach 450 stream_speed 6").unwrap();
    std::fs::write(dir.join("settings.txt"), &settings).unwrap();

    // Heights from the mesh itself, the top surface (the world's
    // `height_at` is not the mesh: up to 7 m off it on some plates).
    let made: Vec<Collider> = meshes.into_iter().filter_map(|(v, i)| Collider::try_trimesh(v, i).ok()).collect();
    let column = worldgen::column_size(0);
    let cell = |p: Vec2| (p / column).floor().as_ivec2();
    let mut grid: HashMap<IVec2, Vec<usize>> = HashMap::new();
    for (i, c) in made.iter().enumerate() {
        let b = c.aabb(Vec3::ZERO, Quat::IDENTITY);
        let (lo, hi) = (cell(b.min.xz()), cell(b.max.xz()));
        for z in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                grid.entry(IVec2::new(x, z)).or_default().push(i);
            }
        }
    }
    let top = |x: f32, z: f32| -> f32 {
        grid.get(&cell(Vec2::new(x, z))).into_iter().flatten().filter_map(|&i| made[i].cast_ray(Vec3::ZERO, Quat::IDENTITY, Vec3::new(x, 5000.0, z), Vec3::NEG_Y, 10000.0, true)).map(|(t, _)| 5000.0 - t).fold(f32::NEG_INFINITY, f32::max)
    };
    let height = |p: Vec2| Vec3::new(p.x, top(p.x, p.y), p.y);
    // (Ground a body could stand on: the terrain within 0.2 m of its height
    // for 0.6 m all round.)
    let standable = |p: Vec3| (0..8).all(|i| { let q = p.xz() + Vec2::from_angle(i as f32 * std::f32::consts::FRAC_PI_4) * 0.6; (top(q.x, q.y) - p.y).abs() <= 0.2 });
    // (As A1: no step over half a metre on a lane a metre wide.)
    let walkable = |a: Vec3, b: Vec3| {
        let (flat, n) = ((b - a).xz(), ((b - a).xz().length() / 0.25).ceil() as usize);
        let side = flat.perp().normalize_or_zero() * 0.5;
        [-side, Vec2::ZERO, side].iter().all(|&off| {
            let h = |i: usize| top(a.x + flat.x * i as f32 / n as f32 + off.x, a.z + flat.y * i as f32 / n as f32 + off.y);
            (0..n).all(|i| (h(i + 1) - h(i)).abs() <= 0.5_f32.min(0.25 * 40f32.to_radians().tan()).max(0.21))
        })
    };
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let inner = side * 0.45;
    let anywhere = |rng: &mut Rng| at + (Vec2::new(rng.next(), rng.next()) - 0.5) * 2.0 * inner;
    let mut groups = Vec::new();

    let mut straight = Vec::new();
    while straight.len() < 300 {
        let a = height(anywhere(&mut rng));
        let b = height((a.xz() + Vec2::from_angle(rng.next() * std::f32::consts::TAU) * 40.0 * rng.next().sqrt()).clamp(at - inner, at + inner));
        if a.xz().distance(b.xz()) > 2.0 && standable(a) && standable(b) && walkable(a, b) {
            straight.push(vec![a, b]);
        }
    }
    groups.push(Group { name: "straight", reach: 0.5, points: straight });

    // Across the biped's tile edges, every 5 m along them.
    let tile = tile_side(Body::Biped.size());
    let (mut flat, mut steps) = (Vec::new(), Vec::new());
    for across in [Vec2::X, Vec2::Y] {
        let along = across.perp();
        for line in ((at - inner).dot(across) / tile).ceil() as i32..=((at + inner).dot(across) / tile).floor() as i32 {
            let mut s = (at - inner).dot(along);
            while s < (at + inner).dot(along) {
                s += 5.0;
                let on = across * line as f32 * tile + along * s;
                let (a, b, m) = (height(on - across * 0.8), height(on + across * 0.8), height(on));
                if !standable(a) || !standable(b) {
                    continue;
                }
                let profile: Vec<f32> = (0..=8).map(|i| { let p = on - across * 0.8 + across * 0.2 * i as f32; top(p.x, p.y) }).collect();
                if (a.y - b.y).abs() > 0.7 && profile.windows(2).any(|w| (w[1] - w[0]).abs() > 0.6) {
                    steps.push(vec![a, b]);
                } else if (a.y - b.y).abs() <= 0.05 && (a.y - m.y).abs() <= 0.05 && walkable(a, b) {
                    flat.push(vec![a, b]);
                }
            }
        }
    }
    groups.push(Group { name: "seam_flat", reach: 0.5, points: flat });
    groups.push(Group { name: "seam_step", reach: 0.5, points: steps });

    for (name, reach, count) in [("near", 150.0, 400), ("far", 1500.0, 100), ("agents", 150.0, 600), ("burst", 1500.0, 800)] {
        let mut pairs = Vec::new();
        while pairs.len() < count {
            let a = anywhere(&mut rng);
            let b = (a + Vec2::from_angle(rng.next() * std::f32::consts::TAU) * reach * rng.next().sqrt()).clamp(at - inner, at + inner);
            let (a, b) = (height(a), height(b));
            let least = match name { "agents" => 20.0, "burst" => 500.0, _ => 0.0 };
            if standable(a) && standable(b) && a.xz().distance(b.xz()) >= least {
                pairs.push(vec![a, b]);
            }
        }
        groups.push(Group { name, reach: 3.0, points: pairs });
    }

    // A1's floors, for the route the streaming run follows (a way landmass
    // found of 800 m or more; a route, not a judgement) and its followers'
    // candidates, 30 to 80 m from its start.
    let mut built: Vec<(Body, Vec<(IVec2, bevy_rerecast::Navmesh)>, Floors, f32, f32)> = Vec::new();
    let workers = (std::thread::available_parallelism().map_or(4, |n| n.get()) / 2).saturating_sub(1).max(1);
    for body in Body::ALL {
        let t = Instant::now();
        let tiles = build(&made, body.size(), bounds(&made)).unwrap();
        let seconds = t.elapsed().as_secs_f32();
        let t = Instant::now();
        let floors = Floors::new(&tiles, body.size().0, false).unwrap();
        built.push((body, tiles, floors, seconds, t.elapsed().as_secs_f32()));
    }
    let floors = &built[0].2;
    let route = loop {
        let (a, b) = (height(anywhere(&mut rng)), height(anywhere(&mut rng)));
        if a.xz().distance(b.xz()) < 800.0 {
            continue;
        }
        let (Some(a), Some(b)) = (floors.on(a, 1.0), floors.on(b, 1.0)) else { continue };
        if let Some(way) = floors.way(a, b) {
            break std::iter::once(a).chain(way).collect::<Vec<_>>();
        }
    };
    let mut followers = Vec::new();
    while followers.len() < 100 {
        let p = height(route[0].xz() + Vec2::from_angle(rng.next() * std::f32::consts::TAU) * (30.0 + 50.0 * rng.next()));
        if standable(p) {
            followers.push(vec![p]);
        }
    }
    groups.push(Group { name: "route", reach: 1.0, points: route.iter().map(|p| vec![*p]).collect() });
    groups.push(Group { name: "followers", reach: 1.0, points: followers });

    // One line a group (`group NAME REACH COUNT`), then one a case, its
    // points' coordinates in a row.
    let mut cases = String::new();
    for g in &groups {
        writeln!(cases, "group {} {} {}", g.name, g.reach, g.points.len()).unwrap();
        for p in &g.points {
            writeln!(cases, "{}", p.iter().map(|v| format!("{} {} {}", v.x, v.y, v.z)).collect::<Vec<_>>().join(" ")).unwrap();
        }
    }
    std::fs::write(dir.join("cases.txt"), cases).unwrap();
    println!("cases: {}", groups.iter().map(|g| format!("{} {}", g.name, g.points.len())).collect::<Vec<_>>().join(", "));

    // Landmass on them.
    let mut json = String::from("{\"engine\": \"landmass 0.9.3 + rerecast 0.4, patched\", \"build\": {");
    for (i, (body, tiles, floors, seconds, joined)) in built.iter().enumerate() {
        let bytes: usize = tiles.iter().map(|(_, t)| t.polygon.vertices.len() * 6 + t.polygon.polygons.len() * 4 + t.detail.vertices.len() * 12 + t.detail.triangles.len() * 4).sum();
        write!(json, "{}\"{body:?}\": {{\"tiles\": {}, \"polygons\": {}, \"seconds\": {seconds}, \"workers\": {workers}, \"ms_per_tile_core\": {}, \"joined_seconds\": {joined}, \"bytes\": {bytes}}}", if i > 0 { ", " } else { "" }, tiles.len(), floors.polygons, seconds * 1000.0 * workers as f32 / tiles.len() as f32).unwrap();
    }
    json.push_str("}, \"queries\": {");
    let group = |name: &str| groups.iter().find(|g| g.name == name).unwrap();
    for (i, name) in ["straight", "seam_flat", "seam_step", "near", "far"].into_iter().enumerate() {
        let g = group(name);
        let rows: Vec<String> = g
            .points
            .iter()
            .map(|p| {
                let (Some(a), Some(b)) = (floors.on(p[0], g.reach), floors.on(p[1], g.reach)) else { return "[-1, 0, 0, 0]".into() };
                let t = Instant::now();
                let length = floors.way_length(a, b);
                let took = ms(t.elapsed());
                format!("[{}, {}, {}, {took}]", length.is_some() as u8, length.unwrap_or(0.0), a.distance(b))
            })
            .collect();
        write!(json, "{}\"{name}\": [{}]", if i > 0 { ", " } else { "" }, rows.join(", ")).unwrap();
    }
    json.push_str("}, \"runs\": {");

    // Agents: the first 50 with a way, then the first 50 with none; then 50
    // given far goals all in one frame.
    let floors = &mut built[0].2;
    let pick = |floors: &Floors, name: &str, reachable: bool, n: usize| -> Vec<(Vec3, Vec3)> {
        group(name).points.iter().filter_map(|p| Some((floors.on(p[0], 3.0)?, floors.on(p[1], 3.0)?))).filter(|&(a, b)| floors.way(a, b).is_some() == reachable).take(n).collect()
    };
    for (i, (name, group_name, reachable, seconds)) in [("reachable", "agents", true, 40), ("unreachable", "agents", false, 40), ("burst", "burst", true, 30)].into_iter().enumerate() {
        let pairs = pick(floors, group_name, reachable, 50);
        let arch = &mut floors.archipelago;
        let ids: Vec<_> = pairs
            .iter()
            .map(|&(a, b)| {
                let mut agent = landmass::Agent::create(a, Vec3::ZERO, 0.35, 5.0, 7.5);
                agent.current_target = Some(b);
                (arch.add_agent(agent), a)
            })
            .collect();
        let walked = walk(arch, &ids, 60 * seconds, 1.0 / 60.0, |_, _| None);
        println!("{name}: {} agents; {walked}", pairs.len());
        let arrived = walked.ends.iter().zip(&pairs).filter(|(e, p)| e.distance(p.1) < 2.0).count();
        let started: Vec<String> = walked.started.iter().map(|s| s.map_or("-1".into(), |f| f.to_string())).collect();
        write!(json, "{}\"{name}\": {{\"agents\": {}, \"frames_ms\": {:?}, \"stuck\": {}, \"jammed\": {}, \"arrived\": {arrived}, \"started_frame\": [{}]}}", if i > 0 { ", " } else { "" }, pairs.len(), walked.times, walked.stuck, walked.jammed, started.join(", ")).unwrap();
        for (id, _) in &ids {
            arch.remove_agent(*id);
        }
    }

    // Streaming: tiles within 450 m of a point walking the route at 6 m/s;
    // 30 followers after it.
    let length: f32 = route.windows(2).map(|w| w[0].distance(w[1])).sum();
    let along = |d: f32| {
        let mut left = d;
        for w in route.windows(2) {
            let l = w[0].distance(w[1]);
            if left <= l {
                return w[0].lerp(w[1], left / l.max(1e-6));
            }
            left -= l;
        }
        *route.last().unwrap()
    };
    let starts: Vec<Vec3> = group("followers").points.iter().filter_map(|p| floors.on(p[0], 1.0)).filter(|&p| floors.way(p, route[0]).is_some()).take(30).collect();
    let tiles = &built[0].1;
    let meshes: HashMap<IVec2, (Vec3, Arc<landmass::ValidNavigationMesh<ThreeD>>)> = tiles.iter().map(|(k, _)| *k).zip(built[0].2.tiles.iter().cloned()).collect();
    let mut arch = landmass::Archipelago::<ThreeD>::new(options(0.35));
    let mut resident: HashMap<IVec2, landmass::IslandId> = HashMap::new();
    let wanted = |p: Vec3| -> HashSet<IVec2> { meshes.keys().copied().filter(|k| ((k.as_vec2() + 0.5) * tile).distance(p.xz()) < 450.0).collect() };
    for k in wanted(route[0]) {
        resident.insert(k, arch.add_island(landmass::Island::new(landmass::Transform { translation: meshes[&k].0, rotation: 0.0 }, meshes[&k].1.clone())));
    }
    arch.update(0.0);
    let ids: Vec<_> = starts.iter().map(|&a| (arch.add_agent(landmass::Agent::create(a, Vec3::ZERO, 0.35, 7.5, 11.0)), a)).collect();
    let seconds = (length / 6.0) as usize;
    let (mut added, mut removed, mut churn, mut most) = (0, 0, Vec::new(), 0);
    let walked = walk(&mut arch, &ids, 60 * seconds, 1.0 / 60.0, |arch, frame| {
        let p = along(6.0 * frame as f32 / 60.0);
        let t = Instant::now();
        let want = wanted(p);
        for k in resident.keys().copied().filter(|k| !want.contains(k)).collect::<Vec<_>>() {
            arch.remove_island(resident.remove(&k).unwrap());
            removed += 1;
        }
        for k in want {
            resident.entry(k).or_insert_with(|| {
                added += 1;
                arch.add_island(landmass::Island::new(landmass::Transform { translation: meshes[&k].0, rotation: 0.0 }, meshes[&k].1.clone()))
            });
        }
        most = most.max(resident.len());
        churn.push(ms(t.elapsed()));
        Some(p)
    });
    let frames: Vec<f32> = walked.times.iter().zip(&churn).map(|(u, c)| u + c).collect();
    let kept = walked.ends.iter().filter(|e| e.distance(*route.last().unwrap()) < 30.0).count();
    println!("stream: {length:.0} m, {added} added, {removed} removed, at most {most}; {walked}; {kept} of {} kept up", starts.len());
    write!(json, ", \"stream\": {{\"agents\": {}, \"frames_ms\": {frames:?}, \"churn_ms\": {churn:?}, \"added\": {added}, \"removed\": {removed}, \"resident_max\": {most}, \"stuck\": {}, \"jammed\": {}, \"kept_up\": {kept}, \"started_frame\": [{}]}}", starts.len(), walked.stuck, walked.jammed, walked.started.iter().map(|s| s.map_or("-1".into(), |f| f.to_string())).collect::<Vec<_>>().join(", ")).unwrap();
    write!(json, "}}, \"stalled\": {}}}", landmass::STALLED_PATHS.load(std::sync::atomic::Ordering::Relaxed)).unwrap();
    std::fs::write(dir.join("results/landmass.json"), json).unwrap();
}

/// Which districts the bench's patch covers, against the whole world's
/// share of each (a 10 m grid over the patch; a 64 m one over the world).
#[test]
#[ignore]
fn zz_districts() {
    let world = worldgen::plates::PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library"));
    let (side, at) = (env("ZZ_SIDE", 2000.0) as f64, Vec2::new(env("ZZ_X", 8192.0), env("ZZ_Z", 8192.0)).as_dvec2());
    let share = |step: f64, lo: DVec2, extent: f64| {
        let mut counts = HashMap::new();
        let n = (extent / step) as i32;
        for i in 0..n {
            for k in 0..n {
                *counts.entry(world.district(lo.x + (i as f64 + 0.5) * step, lo.y + (k as f64 + 0.5) * step).name()).or_insert(0usize) += 1;
            }
        }
        let total = (n * n) as f64;
        let mut v: Vec<_> = counts.into_iter().map(|(d, c)| format!("{d} {:.0}%", c as f64 / total * 100.0)).collect();
        v.sort();
        v.join(", ")
    };
    println!("patch: {}", share(10.0, at - side / 2.0, side));
    println!("world: {}", share(64.0, DVec2::ZERO, worldgen::WORLD_SIZE as f64));
}
