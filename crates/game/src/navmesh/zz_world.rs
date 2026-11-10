//! Spike A1 (the user, 2026-11-01: can the navigation crates hold a world,
//! not two places?). A patch of the world's own terrain, its navmesh in
//! tiles on the world's grid: how long, how big; ways asked across it, near
//! and far, and across its tiles' edges; agents walking it; and its tiles
//! added and taken away round a point moving through it, agents following.
//!
//! `cargo test -p game --profile fast zz_world -- --ignored --nocapture`;
//! `ZZ_SIDE` (m, 2000), `ZZ_X`, `ZZ_Z` (its middle, m), `ZZ_AGENTS` (50).

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::AtomicUsize},
    time::{Duration, Instant},
};

use avian3d::prelude::Collider;
use bevy::prelude::*;
use bevy_landmass::coords::ThreeD;

use super::{Body, Floors, bounds, build, options, tile_side};

pub(super) fn env(name: &str, default: f32) -> f32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

pub(super) struct Rng(pub(super) u64);

impl Rng {
    pub(super) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Mean, 99th and worst of `times` (ms).
pub(super) fn spread(times: &mut [f32]) -> String {
    if times.is_empty() {
        return "none".into();
    }
    times.sort_by(f32::total_cmp);
    let mean = times.iter().sum::<f32>() / times.len() as f32;
    format!("mean {mean:.3} ms, 99th {:.3} ms, worst {:.3} ms", times[times.len() * 99 / 100], times[times.len() - 1])
}

pub(super) fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

#[test]
#[ignore]
fn zz_world() {
    let side = env("ZZ_SIDE", 2000.0);
    let at = Vec2::new(env("ZZ_X", 8192.0), env("ZZ_Z", 8192.0));
    let crowd = env("ZZ_AGENTS", 50.0) as usize;
    let t = Instant::now();
    let world = worldgen::plates::PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library"));
    println!("world made: {:.1} s", t.elapsed().as_secs_f32());

    // Its columns at the finest detail, as terrain.rs makes them, each a
    // collider.
    let t = Instant::now();
    let made: Vec<(Collider, usize)> = columns(&world, at, side).into_iter().filter_map(|(v, i)| { let n = i.len(); Collider::try_trimesh(v, i).ok().map(|c| (c, n)) }).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let triangles: usize = made.iter().map(|m| m.1).sum();
    let colliders: Vec<Collider> = made.into_iter().map(|m| m.0).collect();
    println!("terrain: {} columns, {triangles} triangles, meshed in {:.1} s on {threads} threads", colliders.len(), t.elapsed().as_secs_f32());
    let height = |p: Vec2| Vec3::new(p.x, world.height_at(p.x, p.y), p.y);

    let mut biped = None;
    for body in Body::ALL {
        let t = Instant::now();
        let tiles = build(&colliders, body.size(), bounds(&colliders)).unwrap();
        let built = t.elapsed().as_secs_f32();
        let t = Instant::now();
        let floors = Floors::new(&tiles, body.size().0, false).unwrap_or_else(|e| {
            // (Which polygon, as Recast made it.)
            let bad = bevy_rerecast::rerecast::BAD_TRIANGULATIONS.load(std::sync::atomic::Ordering::Relaxed);
            let nums: Vec<usize> = e.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
            let (k, poly) = (nums[0], nums[1]);
            let (key, tile) = &tiles[k];
            let nvp = tile.polygon.max_vertices_per_polygon as usize;
            let verts: Vec<_> = tile.polygon.polygons[poly * nvp..(poly + 1) * nvp].iter().filter(|&&v| v != u16::MAX).map(|&v| tile.polygon.vertices[v as usize]).collect();
            let sub = &tile.detail.meshes[poly];
            let dv = &tile.detail.vertices[sub.base_vertex_index as usize..][..sub.vertex_count as usize];
            let o = tile.polygon.aabb.min;
            let tris: Vec<String> = tile.detail.triangles[sub.base_triangle_index as usize..][..sub.triangle_count as usize]
                .iter()
                .map(|t| {
                    let [a, b, c] = [t[0], t[2], t[1]].map(|i| dv[i as usize] - o);
                    let (ab, ac) = (Vec2::new(b.x - a.x, -(b.z - a.z)), Vec2::new(c.x - a.x, -(c.z - a.z)));
                    format!("{t:?} {a:.3} {b:.3} {c:.3} cross {:.6}", ab.perp_dot(ac))
                })
                .collect();
            panic!("{e}: tile {key} at {:?}, cell {}; its vertices (cells) {verts:?}; detail {} vertices {dv:?}; triangles {tris:#?}; {bad} contours not triangulated whole", o, tile.polygon.cell_size, dv.len());
        });
        let bytes: usize = tiles.iter().map(|(_, t)| t.polygon.vertices.len() * 6 + t.polygon.polygons.len() * 4 + t.detail.vertices.len() * 12 + t.detail.triangles.len() * 4).sum();
        println!(
            "{body:?}: {} tiles of {:.1} m, {} polygons ({} dropped), built in {built:.1} s ({:.0} ms a tile on one core), joined in {:.2} s; about {:.0} MB as built",
            tiles.len(),
            tile_side(body.size()),
            floors.polygons,
            floors.dropped,
            built * 1000.0 * ((threads / 2).saturating_sub(1).max(1)) as f32 / tiles.len().max(1) as f32,
            t.elapsed().as_secs_f32(),
            bytes as f32 / 1e6
        );
        if body == Body::Biped {
            biped = Some((tiles, floors));
        }
    }
    let (tiles, mut floors) = biped.unwrap();
    if let Ok(pair) = std::env::var("ZZ_PAIR") {
        let n: Vec<f32> = pair.split(',').map(|v| v.parse().unwrap()).collect();
        let (a, b) = (Vec3::new(n[0], n[1], n[2]), Vec3::new(n[3], n[4], n[5]));
        let (a, b) = if n[1].is_nan() { (height(a.xz()), height(b.xz())) } else { (a, b) };
        let profile: Vec<String> = (0..=16).map(|i| { let p = a.lerp(b, i as f32 / 16.0); format!("{:.2}", world.height_at(p.x, p.z)) }).collect();
        eprintln!("terrain across: {}", profile.join(" "));
        let (sa, sb) = (floors.sample(a, 1.5, super::CLIMB).unwrap(), floors.sample(b, 1.5, 1.0).unwrap());
        eprintln!("sampled {} on island {:?}, {} on {:?}", sa.point(), sa.island(), sb.point(), sb.island());
        // (The polygons' edges along the tile line between them.)
        let side = tile_side(Body::Biped.size());
        let line = ((a.x + b.x) / 2.0 / side).round() * side;
        for (key, t) in &tiles {
            let (o, cs, ch) = (t.polygon.aabb.min, t.polygon.cell_size, t.polygon.cell_height);
            let nvp = t.polygon.max_vertices_per_polygon as usize;
            for poly in 0..t.polygon.polygon_count() {
                let vs: Vec<Vec3> = t.polygon.polygons[poly * nvp..][..nvp].iter().filter(|&&v| v != u16::MAX).map(|&v| o + t.polygon.vertices[v as usize].as_vec3() * Vec3::new(cs, ch, cs)).collect();
                for i in 0..vs.len() {
                    let (p, q) = (vs[i], vs[(i + 1) % vs.len()]);
                    if (p.x - line).abs() < 0.05 && (q.x - line).abs() < 0.05 && (p.z - a.z).abs().min((q.z - a.z).abs()) < 40.0 {
                        eprintln!("tile {key} poly {poly}: z {:.3}..{:.3}, y {:.2}/{:.2}", p.z, q.z, p.y, q.y);
                    }
                }
            }
        }
        eprintln!("way {:?}; stalled {}", floors.way(a, b), landmass::STALLED_PATHS.load(std::sync::atomic::Ordering::Relaxed));
        return;
    }
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);

    // Ways asked, near and far.
    let inner = side * 0.45;
    for (name, reach, count) in [("near (to 150 m)", 150.0, 400), ("far (to 1500 m)", 1500.0, 100)] {
        let (mut times, mut found, mut asked, mut ratios) = (Vec::new(), 0, 0, Vec::new());
        while asked < count {
            let a = at + (Vec2::new(rng.next(), rng.next()) - 0.5) * 2.0 * inner;
            let d = Vec2::from_angle(rng.next() * std::f32::consts::TAU) * reach * rng.next().sqrt();
            let b = (a + d).clamp(at - inner, at + inner);
            let (Some(a), Some(b)) = (floors.on(height(a), 3.0), floors.on(height(b), 3.0)) else { continue };
            asked += 1;
            let t = Instant::now();
            let length = floors.way_length(a, b);
            times.push(ms(t.elapsed()));
            if let Some(l) = length {
                found += 1;
                ratios.push(l / a.distance(b).max(1.0));
            }
        }
        ratios.sort_by(f32::total_cmp);
        let ratio = |q: f32| ratios.get(((ratios.len() as f32 * q) as usize).min(ratios.len().saturating_sub(1))).copied().unwrap_or(0.0);
        println!("ways {name}: {found} of {asked} found; way / straight median {:.2}, 95th {:.2}; {}", ratio(0.5), ratio(0.95), spread(&mut times));
    }

    // Where the terrain itself is walkable straight from one to the other
    // (no step over half a metre, no slope over 40 degrees, on a lane a
    // metre wide), a way must be found.
    let walkable = |a: Vec3, b: Vec3| {
        let (flat, n) = ((b - a).xz(), ((b - a).xz().length() / 0.25).ceil() as usize);
        let side = flat.perp().normalize_or_zero() * 0.5;
        [-side, Vec2::ZERO, side].iter().all(|&off| {
            let h = |i: usize| world.height_at(a.x + flat.x * i as f32 / n as f32 + off.x, a.z + flat.y * i as f32 / n as f32 + off.y);
            (0..n).all(|i| (h(i + 1) - h(i)).abs() <= 0.5_f32.min(0.25 * 40f32.to_radians().tan()).max(0.21))
        })
    };
    let (mut straight, mut missed, mut shown) = (0, 0, Vec::new());
    while straight < 300 {
        let a = at + (Vec2::new(rng.next(), rng.next()) - 0.5) * 2.0 * inner;
        let b = (a + Vec2::from_angle(rng.next() * std::f32::consts::TAU) * 40.0 * rng.next().sqrt()).clamp(at - inner, at + inner);
        let (Some(a), Some(b)) = (floors.on(height(a), 0.3), floors.on(height(b), 0.3)) else { continue };
        if !walkable(a, b) {
            continue;
        }
        straight += 1;
        if floors.way(a, b).is_none() {
            missed += 1;
            shown.push(format!("{:.1},{:.1},{:.1} to {:.1},{:.1},{:.1}", a.x, a.y, a.z, b.x, b.y, b.z));
        }
    }
    println!("walkable straight (to 40 m): {straight} pairs, {missed} with no way{}", shown.iter().take(6).map(|s| format!("; {s}")).collect::<String>());

    // Across its tiles' edges, where one flat floor runs on over them: a way
    // must be found, and not a long one round.
    let tile = tile_side(Body::Biped.size());
    let (mut checked, mut cut, mut round, mut shown) = (0, 0, 0, Vec::new());
    let (mut steps, mut false_joins, mut shown_steps) = (0, 0, Vec::new());
    for across in [Vec2::X, Vec2::Y] {
        let along = across.perp();
        let first = ((at - inner).dot(across) / tile).ceil() as i32;
        let last = ((at + inner).dot(across) / tile).floor() as i32;
        for line in first..=last {
            let mut s = (at - inner).dot(along);
            while s < (at + inner).dot(along) {
                s += 5.0;
                let on = across * line as f32 * tile + along * s;
                let (Some(a), Some(b), Some(m)) = (floors.on(height(on - across * 0.8), 0.3), floors.on(height(on + across * 0.8), 0.3), floors.on(height(on), 0.3)) else { continue };
                // (Floors a step too high apart either side, joined straight
                // across: a creature would press into the step.)
                // (A step, not a slope: somewhere across it the ground
                // rises more than a climb within 0.2 m.)
                let profile: Vec<f32> = (0..=8).map(|i| { let p = on - across * 0.8 + across * 0.2 * i as f32; world.height_at(p.x, p.y) }).collect();
                let stepped = profile.windows(2).any(|w| (w[1] - w[0]).abs() > 0.6);
                if (a.y - b.y).abs() > 0.7 && stepped {
                    steps += 1;
                    if floors.way_length(a, b).is_some_and(|l| l < a.distance(b) + 1.0) {
                        false_joins += 1;
                        shown_steps.push(format!("{:.1} m step joined at {:.1},{:.1},{:.1}", (a.y - b.y).abs(), on.x, m.y, on.y));
                    }
                    continue;
                }
                if (a.y - b.y).abs() > 0.7 {
                    continue;
                }
                if (a.y - b.y).abs() > 0.05 || (a.y - m.y).abs() > 0.05 {
                    continue;
                }
                checked += 1;
                match floors.way_length(a, b) {
                    None => {
                        cut += 1;
                        shown.push(format!("cut at {:.1},{:.1},{:.1}", on.x, m.y, on.y));
                    }
                    Some(l) if l > a.distance(b) * 3.0 + 1.0 => {
                        round += 1;
                        shown.push(format!("round {l:.1} m at {:.1},{:.1},{:.1}", on.x, m.y, on.y));
                    }
                    _ => {}
                }
            }
        }
    }
    println!("ways given up straightening so far: {}", landmass::STALLED_PATHS.load(std::sync::atomic::Ordering::Relaxed));
    println!("tile edges with a step over 0.7 m: {steps} checked, {false_joins} joined straight across{}", shown_steps.iter().take(6).map(|s| format!("; {s}")).collect::<String>());
    println!("tile edges on one floor: {checked} checked, {cut} cut, {round} a long way round{}", shown.iter().take(8).map(|s| format!("; {s}")).collect::<String>());

    // Its update alone, no agents: what holding its tiles costs a frame.
    let mut idle: Vec<f32> = (0..300).map(|_| { let t = Instant::now(); floors.archipelago.update(1.0 / 60.0); ms(t.elapsed()) }).collect();
    println!("update with no agents, {} tiles in: {}", floors.tiles.len(), spread(&mut idle));
    let points: Vec<Vec3> = (0..2000).map(|_| height(at + (Vec2::new(rng.next(), rng.next()) - 0.5) * 2.0 * inner)).collect();
    let t = Instant::now();
    let found = points.iter().filter(|p| floors.archipelago.sample_point(**p, &options(0.35).point_sample_distance).is_ok()).count();
    println!("sampling a point: {:.4} ms each ({found} of {} on a floor)", ms(t.elapsed()) / points.len() as f32, points.len());

    // Agents walking it, each to somewhere up to 150 m off: first where
    // there is a way, then where there is none (on another plate).
    for reachable in [true, false] {
        let mut pairs = Vec::new();
        while pairs.len() < crowd {
            let a = at + (Vec2::new(rng.next(), rng.next()) - 0.5) * 2.0 * inner;
            let b = a + Vec2::from_angle(rng.next() * std::f32::consts::TAU) * 150.0 * rng.next().sqrt();
            let (Some(a), Some(b)) = (floors.on(height(a), 1.0), floors.on(height(b), 1.0)) else { continue };
            if floors.way(a, b).is_some() == reachable && a.distance(b) > 20.0 {
                pairs.push((a, b));
            }
        }
        let arch = &mut floors.archipelago;
        let ids: Vec<_> = pairs
            .iter()
            .map(|&(a, b)| {
                let mut agent = landmass::Agent::create(a, Vec3::ZERO, 0.35, 5.0, 7.5);
                agent.current_target = Some(b);
                (arch.add_agent(agent), a)
            })
            .collect();
        let report = walk(arch, &ids, 60 * 40, 1.0 / 60.0, |_, _| None);
        println!("{crowd} agents walking 40 s, {}: {report}", if reachable { "each with a way" } else { "each with none" });
        for (id, _) in &ids {
            arch.remove_agent(*id);
        }
    }

    // Tiles added and taken away round a point walking a way of a
    // kilometre or so at 6 m/s, those within 450 m of it there (the
    // terrain's finest disc); 30 agents after it at 7.5 m/s.
    let route = loop {
        let a = at + (Vec2::new(rng.next(), rng.next()) - 0.5) * 2.0 * inner;
        let b = at + (Vec2::new(rng.next(), rng.next()) - 0.5) * 2.0 * inner;
        let (Some(a), Some(b)) = (floors.on(height(a), 1.0), floors.on(height(b), 1.0)) else { continue };
        if a.xz().distance(b.xz()) < 800.0 {
            continue;
        }
        if let Some(way) = floors.way(a, b) {
            break std::iter::once(a).chain(way).collect::<Vec<_>>();
        }
    };
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
    let mut followers = Vec::new();
    while followers.len() < 30 {
        let p = route[0].xz() + Vec2::from_angle(rng.next() * std::f32::consts::TAU) * (30.0 + 50.0 * rng.next());
        let Some(p) = floors.on(height(p), 1.0) else { continue };
        if floors.way(p, route[0]).is_some() {
            followers.push(p);
        }
    }
    let meshes: HashMap<IVec2, (Vec3, Arc<landmass::ValidNavigationMesh<ThreeD>>)> = tiles.iter().map(|(k, _)| *k).zip(floors.tiles.iter().cloned()).collect();
    let mut arch = landmass::Archipelago::<ThreeD>::new(options(0.35));
    let mut resident: HashMap<IVec2, landmass::IslandId> = HashMap::new();
    let wanted = |p: Vec3| -> HashSet<IVec2> { meshes.keys().copied().filter(|k| ((k.as_vec2() + 0.5) * tile).distance(p.xz()) < 450.0).collect() };
    for k in wanted(route[0]) {
        resident.insert(k, arch.add_island(landmass::Island::new(landmass::Transform { translation: meshes[&k].0, rotation: 0.0 }, meshes[&k].1.clone())));
    }
    arch.update(0.0);
    let ids: Vec<_> = followers.iter().map(|&a| (arch.add_agent(landmass::Agent::create(a, Vec3::ZERO, 0.35, 7.5, 11.0)), a)).collect();
    let seconds = (length / 6.0) as usize;
    let (mut added, mut removed, mut churn) = (0, 0, Vec::new());
    let mut resident_most = 0;
    // (Each frame, before the update: the tiles round where it is.)
    let report = walk(&mut arch, &ids, 60 * seconds, 1.0 / 60.0, |arch, frame| {
        let p = along(6.0 * frame as f32 / 60.0);
        let t = Instant::now();
        let want = wanted(p);
        for k in resident.keys().copied().filter(|k| !want.contains(k)).collect::<Vec<_>>() {
            arch.remove_island(resident.remove(&k).unwrap());
            removed += 1;
        }
        let before = added + removed;
        for k in want {
            resident.entry(k).or_insert_with(|| {
                added += 1;
                arch.add_island(landmass::Island::new(landmass::Transform { translation: meshes[&k].0, rotation: 0.0 }, meshes[&k].1.clone()))
            });
        }
        resident_most = resident_most.max(resident.len());
        churn.push(ms(t.elapsed()));
        if std::env::var("ZZ_SPIKES").is_ok() && added + removed > before {
            eprintln!("tiles changed frame {frame}");
        }
        Some(p)
    });
    let behind: Vec<f32> = ids.iter().map(|(id, _)| arch.get_agent(*id).unwrap().position.distance(*route.last().unwrap())).collect();
    println!(
        "streamed along a way of {length:.0} m over {seconds} s: {added} tiles added, {removed} taken away, at most {resident_most} in; adding and taking away {}; 30 agents after it: {report}; at the end, {} of 30 within 30 m of it",
        spread(&mut churn),
        behind.iter().filter(|d| **d < 30.0).count()
    );
    println!("ways given up straightening in all: {}", landmass::STALLED_PATHS.load(std::sync::atomic::Ordering::Relaxed));
}

/// The world's columns at their finest detail within `side` of `at`, as
/// terrain.rs makes them, in the world's frame: each its vertices and its
/// triangles. (Meshed on every core.)
pub(super) fn columns(world: &worldgen::plates::PlateWorld, at: Vec2, side: f32) -> Vec<(Vec<Vec3>, Vec<[u32; 3]>)> {
    let column = worldgen::column_size(0);
    let (lo, hi) = (((at - side / 2.0) / column).floor().as_ivec2(), ((at + side / 2.0) / column).ceil().as_ivec2());
    let keys: Vec<IVec2> = (lo.y..hi.y).flat_map(|z| (lo.x..hi.x).map(move |x| IVec2::new(x, z))).collect();
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&c) = keys.get(k) else { return out };
                        let mesh = world.mesh_column(0, c.x, c.y);
                        if mesh.is_empty() {
                            continue;
                        }
                        let o = c.as_vec2() * column;
                        let vertices = mesh.positions.iter().map(|p| Vec3::new(p[0] + o.x, p[1], p[2] + o.y)).collect();
                        out.push((vertices, mesh.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect()));
                    }
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

/// `ids`' agents walked `frames` frames of `dt`, each set going as it
/// wants and put on its floor; `target` asked each frame for where they all
/// go (none, each keeps its own). Each frame's update timed; how many
/// frames in each state; and agents that had somewhere to go but were no
/// further on after two seconds.
pub(super) fn walk(arch: &mut landmass::Archipelago<ThreeD>, ids: &[(landmass::AgentId, Vec3)], frames: usize, dt: f32, mut target: impl FnMut(&mut landmass::Archipelago<ThreeD>, usize) -> Option<Vec3>) -> Walked {
    let mut times = Vec::new();
    let mut states: HashMap<String, usize> = HashMap::new();
    let mut history: Vec<Vec<Vec3>> = ids.iter().map(|(_, a)| vec![*a]).collect();
    let mut stuck = HashSet::new();
    let mut jammed = HashSet::new();
    let mut offs = HashSet::new();
    let mut last_target = None;
    for frame in 0..frames {
        if let Some(t) = target(arch, frame) {
            last_target = Some(t);
        }
        if let Some(t) = last_target {
            for (id, _) in ids {
                arch.get_agent_mut(*id).unwrap().current_target = Some(t);
            }
        }
        let t = Instant::now();
        arch.update(dt);
        times.push(ms(t.elapsed()));
        if std::env::var("ZZ_SPIKES").is_ok() && times.last().unwrap() > &2.0 {
            let results = arch.get_pathing_results();
            eprintln!("spike frame {frame}: {:.2} ms; {} paths found this frame, {} nodes explored", times.last().unwrap(), results.len(), results.iter().map(|r| r.explored_nodes).sum::<u32>());
        }
        for (i, (id, _)) in ids.iter().enumerate() {
            let agent = arch.get_agent(*id).unwrap();
            let state = agent.state();
            if std::env::var("ZZ_DEBUG").is_ok() && frame < 3 && i < 4 {
                eprintln!("frame {frame} agent {i} at {} {state:?}, target {:?}, sampled {:?}", agent.position, agent.current_target, arch.sample_point(agent.position, &options(0.35).point_sample_distance).map(|s| s.point()));
            }
            *states.entry(format!("{state:?}")).or_default() += 1;
            let v = *agent.get_desired_velocity();
            // (Across as it wants, up or down to the floor right under it
            // there, within a creature's step up or 3 m down, as its body
            // goes; none there, at its own height.)
            let p = agent.position + v * dt;
            let under = landmass::PointSampleDistance3d { horizontal_distance: 0.0, distance_above: 0.65, distance_below: 3.0, vertical_preference_ratio: 1.0, animation_link_max_vertical_distance: 0.5 };
            let floor = arch.sample_point(p, &under).ok().map_or(p, |s| s.point());
            let at = agent.position;
            if state == landmass::AgentState::AgentNotOnNavMesh && offs.insert(i) && std::env::var("ZZ_OFF").is_ok() && offs.len() < 12 {
                let before = history[i][history[i].len().saturating_sub(3)];
                let wide = landmass::PointSampleDistance3d { horizontal_distance: 5.0, distance_above: 5.0, distance_below: 10.0, vertical_preference_ratio: 2.0, animation_link_max_vertical_distance: 0.5 };
                eprintln!("off {i} frame {frame} at {:.2} (from {before:.2}) wanting {v:.2}; nearest floor {:?}", at, arch.sample_point(at, &wide).map(|s| s.point()));
            }
            let agent = arch.get_agent_mut(*id).unwrap();
            agent.velocity = v;
            agent.position = floor;
            history[i].push(floor);
            let h = &history[i];
            let crowded = history.iter().enumerate().any(|(j, o)| j != i && o.last().unwrap().distance(floor) < 1.5);
            let far = arch.get_agent(*id).unwrap().current_target.is_some_and(|t| t.distance(floor) > 5.0);
            if state == landmass::AgentState::Moving && h.len() > 120 && h[h.len() - 1].distance(h[h.len() - 121]) < 0.5 {
                if crowded || !far {
                    jammed.insert(i);
                } else if stuck.insert(i) && std::env::var("ZZ_STUCK").is_ok() {
                    let tile = 1000.0 / 60.0 * 3.5;
                    let edge = |x: f32| (x / tile - (x / tile).round()).abs() * tile;
                    eprintln!("stuck {i} frame {frame} at {floor:.2} wanting {v:.2}, going to {:?}; {:.2} m off a tile edge across, {:.2} along; neighbours within 1 m: {}", arch.get_agent(*id).unwrap().current_target, edge(floor.x), edge(floor.z), history.iter().enumerate().filter(|(j, o)| *j != i && o.last().unwrap().distance(floor) < 1.0).count());
                }
            }
        }
    }
    let mut states: Vec<_> = states.into_iter().collect();
    states.sort();
    let text = format!("update {}; agent-frames {}; {} stuck alone while moving, {} held up in a crowd or near the end", spread(&mut times.clone()), states.iter().map(|(s, n)| format!("{s} {n}")).collect::<Vec<_>>().join(", "), stuck.len(), jammed.len());
    let started = history.iter().map(|h| h.iter().position(|p| p.distance(h[0]) > 0.1)).collect();
    Walked { text, times, stuck: stuck.len(), jammed: jammed.len(), started, ends: history.iter().map(|h| *h.last().unwrap()).collect() }
}

/// What `walk` saw: said, and as numbers: each frame's update (ms); how
/// many stuck alone and held up; the frame each agent first moved a tenth
/// of a metre on, if it did; where each ended.
pub(super) struct Walked {
    pub(super) text: String,
    pub(super) times: Vec<f32>,
    pub(super) stuck: usize,
    pub(super) jammed: usize,
    pub(super) started: Vec<Option<usize>>,
    pub(super) ends: Vec<Vec3>,
}

impl std::fmt::Display for Walked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}
