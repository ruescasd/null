//! Walkers on district ground: they walk within a navigable node and
//! teleport between nodes (the user, 2026-11-03: districts *require*
//! teleporting, their steps leave most of the ground unreachable on foot;
//! a key risk, so this prototype, to feel it: walker or blinker?).
//!
//! `--opt blink` builds the nodes (`nodes.rs`) over a square of `SIDE`
//! round where you start, in the background, and anew round you when you
//! come near its edge (`NEAR_EDGE`); `--opt nodedraw` draws them,
//! a colour each (the small ones dark red). A hunter on them (`Blink`)
//! walks its node by landmass's ways across a 2D mesh of it; you on
//! another node, it plans the fewest teleports there (between the nearest
//! points of nodes' edges), walks toward where the first leaves from (going
//! from where it is as soon as that does as well: `Nodes::sooner`), stands
//! for its tell (a glow where it stands and where it will land), and is
//! there, as far on into the node as its range takes it (`Nodes::land`);
//! then rests before it may again.

use std::{collections::HashMap, sync::Arc, time::Instant};

use bevy::{
    math::DVec2,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use geo::{
    Buffer, Simplify,
    algorithm::buffer::{BufferStyle, LineJoin},
};
use geo::{BoundingRect, ClosestPoint, Distance, Euclidean, MultiPolygon, Point, Polygon, TriangulateDelaunay, algorithm::triangulate_delaunay::DelaunayTriangulationConfig};
use bevy_landmass::coords::TwoD as XY;
use landmass::{ArchipelagoOptions, FromAgentRadius, PathStep, PermittedAnimationLinks};
use worldgen::plates::PlateWorld;

use super::nodes::{self, Fit, Hop, Space};
use crate::Args;

/// The side of the square of district ground built round where you start
/// (m).
const SIDE: f64 = 600.0;
/// The farthest a teleport goes (m): hops are kept to it.
pub(crate) const FARTHEST: f64 = 20.0;
/// The least node a teleport lands on (m²), unless it is where you are:
/// slivers of a few square decimetres between steps, else.
const LAND_LEAST: f64 = 10.0;

/// A district's nodes, ready to walk: the walking space, the hops between
/// nodes, and a 2D navmesh of each (landmass's, in this square's own frame,
/// from `origin`: f32 is fine there, not 8 km out).
pub(crate) struct Nodes {
    world: Arc<PlateWorld>,
    pub(crate) space: Space,
    /// Each node's neighbours, and the nearest a teleport between them goes
    /// (for `route`).
    #[cfg_attr(not(test), allow(dead_code))]
    adjacent: Vec<Vec<(u32, f32)>>,
    /// Where teleports leave from and land, to plan by.
    portals: Portals,
    /// Each node's navmesh, its own (one for all, a way begun on the edge
    /// of a node was begun on the one beside it, a few millimetres off, and
    /// not found).
    archipelagos: Vec<Option<landmass::Archipelago<XY>>>,
    origin: DVec2,
    pub(crate) lo: DVec2,
    pub(crate) hi: DVec2,
    /// What was built, said once.
    pub(crate) report: String,
    /// The nodes drawn (`--opt nodedraw`): triangles' corners and colours.
    drawn: Option<(Vec<[f32; 3]>, Vec<[f32; 4]>)>,
}

/// `poly`'s triangles, by constrained Delaunay triangulation (well shaped:
/// earcut's slivers along an edge's corners in a row were of no area in
/// f32, and left holes in the mesh, ways not found): its corners and each
/// triangle's, counter-clockwise; how many came out of no area, and were
/// left out; none if it could not be triangulated.
fn triangles(poly: &Polygon<f64>) -> Option<(Vec<DVec2>, Vec<[usize; 3]>, usize)> {
    let tris = poly.constrained_triangulation(DelaunayTriangulationConfig { snap_radius: 1e-4 }).ok()?;
    let mut index: HashMap<(u64, u64), usize> = HashMap::new();
    let mut corners: Vec<DVec2> = Vec::new();
    let mut at = |c: geo::Coord<f64>| {
        *index.entry((c.x.to_bits(), c.y.to_bits())).or_insert_with(|| {
            corners.push(DVec2::new(c.x, c.y));
            corners.len() - 1
        })
    };
    let mut out = Vec::new();
    let mut flat = 0;
    for t in tris {
        let (i, j, k) = (at(t.v1()), at(t.v2()), at(t.v3()));
        let (a, b, c) = (DVec2::new(t.v1().x, t.v1().y), DVec2::new(t.v2().x, t.v2().y), DVec2::new(t.v3().x, t.v3().y));
        let area = (b - a).perp_dot(c - a) / 2.0;
        if area.abs() < 1e-6 {
            flat += 1;
        } else if area > 0.0 {
            out.push([i, j, k]);
        } else {
            out.push([i, k, j]);
        }
    }
    Some((corners, out, flat))
}

/// A node's outline: its fragments as one (they meet along the stretches
/// they were joined across). (Each grown 2 mm first: on the grid of 1/1024
/// m, two fragments' edges along one stretch are off each other by up to
/// half a millimetre, and left apart by the union, the node in pieces. Then
/// made simpler, within 2 mm: corners in a row but for that rounding made
/// slivers, of no area in f32, and many more triangles.)
pub(crate) fn outline(fragments: &[&Polygon<f64>]) -> MultiPolygon<f64> {
    let grown: Vec<MultiPolygon<f64>> = fragments.iter().map(|f| f.buffer_with_style(BufferStyle::new(0.002).line_join(LineJoin::Bevel))).collect();
    geo::unary_union(grown.iter()).simplify(0.002)
}

/// How many parts `tris` make, joined edge to edge.
fn parts(tris: &[[usize; 3]]) -> usize {
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut by_edge: HashMap<(usize, usize), usize> = HashMap::new();
    for (t, tri) in tris.iter().enumerate() {
        for k in 0..3 {
            let e = (tri[k].min(tri[(k + 1) % 3]), tri[k].max(tri[(k + 1) % 3]));
            match by_edge.get(&e) {
                Some(&u) => {
                    let (x, y) = (find(&mut parent, u), find(&mut parent, t));
                    parent[x] = y;
                }
                None => {
                    by_edge.insert(e, t);
                }
            }
        }
    }
    (0..tris.len()).map(|t| find(&mut parent, t)).collect::<std::collections::HashSet<_>>().len()
}

impl Nodes {
    /// Over a square of `side` round `centre` (to the columns' edges), for
    /// a walker; drawn, asked for.
    pub(crate) fn build(world: Arc<PlateWorld>, centre: DVec2, side: f64, draw: bool) -> Result<Nodes, String> {
        let t = Instant::now();
        let column = worldgen::column_size(0) as f64;
        let (lo, hi) = (((centre - side / 2.0) / column).floor() * column, ((centre + side / 2.0) / column).ceil() * column);
        let space = Space::build(&world, nodes::pieces(&world, lo, hi), Fit::WALKER);
        let built = t.elapsed().as_secs_f32();
        let hops = space.hops(&world, FARTHEST);
        let mut adjacent: Vec<Vec<(u32, f32)>> = vec![Vec::new(); space.nodes()];
        for (&(a, b), list) in &hops {
            let d = list.iter().map(|h| h.d).fold(f32::INFINITY, f32::min);
            adjacent[a as usize].push((b, d));
            adjacent[b as usize].push((a, d));
        }
        // Each node's outline: its fragments as one (they meet along the
        // stretches they were joined across), triangulated.
        let mut of: Vec<Vec<&Polygon<f64>>> = vec![Vec::new(); space.nodes()];
        for (i, s) in space.space.iter().enumerate() {
            for (k, f) in s.iter().enumerate() {
                of[space.node_of[i][k] as usize].push(f);
            }
        }
        let origin = (lo + hi) / 2.0;
        let (mut flat, mut split, mut untriangulated, mut cut, mut triangles_n) = (0, 0, 0, 0, 0);
        let mut archipelagos = Vec::new();
        // (Each node's on its own: on every core, in chunks.)
        type Made = Vec<Option<(Vec<DVec2>, Vec<[usize; 3]>, usize)>>;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let made: Vec<Made> = std::thread::scope(|scope| {
            let chunk = of.len().div_ceil(threads * 8).max(1);
            let handles: Vec<_> = of.chunks(chunk).map(|c| scope.spawn(move || c.iter().map(|list| outline(list).0.iter().map(triangles).collect::<Made>()).collect::<Vec<_>>())).collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        for (n, outline) in made.into_iter().enumerate() {
            let (mut vertices, mut polygons): (Vec<Vec2>, Vec<Vec<usize>>) = (Vec::new(), Vec::new());
            if outline.len() > 1 {
                split += 1;
            }
            for poly in outline {
                let Some((corners, tris, f)) = poly else {
                    untriangulated += 1;
                    continue;
                };
                flat += f;
                let base = vertices.len();
                vertices.extend(corners.iter().map(|q| (*q - origin).as_vec2()));
                // (A sliver whose corners, in f32, lie in a row or turn the
                // wrong way: left out, and counted, as landmass refuses it;
                // by landmass's own test, no stricter: stricter, 39 were
                // left out that it takes, and two nodes cut in two.)
                let mut kept: Vec<[usize; 3]> = Vec::new();
                for t in &tris {
                    let v = t.map(|i| vertices[base + i]);
                    let turns = (0..3).all(|k| {
                        let (l, c, r) = (v[(k + 2) % 3], v[k], v[(k + 1) % 3]);
                        let (le, re) = (l - c, r - c);
                        let cross = re.perp_dot(le);
                        cross > 0.0 || (cross.abs() <= 1e-5 * re.length() * le.length() && re.dot(le) < 0.0)
                    });
                    if turns {
                        polygons.push(t.iter().map(|i| base + i).collect());
                        kept.push(*t);
                    } else {
                        flat += 1;
                    }
                }
                // (Its triangles, as kept, one part, joined edge to edge,
                // as the outline is: else a way across it is not found.)
                if parts(&kept) > 1 {
                    cut += 1;
                }
            }
            triangles_n += polygons.len();
            if polygons.is_empty() {
                archipelagos.push(None);
                continue;
            }
            let mesh = landmass::NavigationMesh::<XY> { vertices, polygon_type_indices: vec![0; polygons.len()], polygons, height_mesh: None };
            let mesh = mesh.validate().map_err(|e| format!("node {n}'s navmesh: {e}"))?;
            let mut archipelago = landmass::Archipelago::<XY>::new(ArchipelagoOptions::from_agent_radius(Fit::WALKER.radius as f32));
            archipelago.add_island(landmass::Island::new(landmass::Transform { translation: Vec2::ZERO, rotation: 0.0 }, Arc::new(mesh)));
            archipelago.update(0.0);
            archipelagos.push(Some(archipelago));
        }
        let drawn = draw.then(|| {
            let (mut corners, mut colours) = (Vec::new(), Vec::new());
            for (i, s) in space.space.iter().enumerate() {
                for (k, f) in s.iter().enumerate() {
                    let n = space.node_of[i][k];
                    let colour = if space.node_area[n as usize] < LAND_LEAST {
                        [0.35, 0.0, 0.0, 1.0]
                    } else {
                        let h = (n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
                        let c = |s: u32| 0.25 + 0.75 * ((h >> s) & 0xff) as f32 / 255.0;
                        [c(8), c(24), c(40), 1.0]
                    };
                    let Some((pts, tris, _)) = triangles(f) else { continue };
                    for t in tris {
                        for i_ in t {
                            let q = pts[i_];
                            corners.push([q.x as f32, world.plate_top(&space.pieces[i].plate, q) as f32 + 0.04, q.y as f32]);
                            colours.push(colour);
                        }
                    }
                }
            }
            (corners, colours)
        });
        let t_portals = Instant::now();
        let portals = Portals::new(&hops, space.nodes());
        let report = format!(
            "district nodes over {:.0} by {:.0} m round {:.0}, {:.0}: {} pieces, {} nodes ({} of {LAND_LEAST} m² or more), {} node pairs within {FARTHEST} m; {} portals, {} walking links between them ({:.1} s); navmesh {} triangles ({} of no area left out, {} outlines not triangulated, {} cut in parts by those left out), {} nodes not one outline; built in {:.1} s ({built:.1} s the walking space)",
            hi.x - lo.x,
            hi.y - lo.y,
            centre.x,
            centre.y,
            space.pieces.len(),
            space.nodes(),
            space.node_area.iter().filter(|a| **a >= LAND_LEAST).count(),
            hops.len(),
            portals.at.len(),
            portals.links.iter().map(|l| l.len()).sum::<usize>() / 2,
            t_portals.elapsed().as_secs_f32(),
            triangles_n,
            flat,
            untriangulated,
            cut,
            split,
            t.elapsed().as_secs_f32()
        );
        Ok(Nodes { world, space, adjacent, portals, archipelagos, origin, lo, hi, report, drawn })
    }

    /// The node a body at `p` is on, and the point on its walking space
    /// nearest it: its feet `above` over its ground (0 a creature's feet, a
    /// player's eyes `player::EYE`). In it, there (on the edge of two, the
    /// one it is known to be on, `prefer`); else the nearest within 2 m
    /// whose ground is near its feet.
    pub(crate) fn locate(&self, p: Vec3, above: f32, prefer: Option<u32>) -> Option<(u32, Vec2)> {
        let q = DVec2::new(p.x as f64, p.z as f64);
        let feet = (p.y - above) as f64;
        if let Some((n, i, _)) = self.space.node_at_preferring(q, prefer)
            && (self.world.plate_top(&self.space.pieces[i].plate, q) - feet).abs() < 1.5
        {
            return Some((n, p.xz_()));
        }
        let pt = Point::new(q.x, q.y);
        let mut best: Option<(f64, u32, DVec2)> = None;
        for i in self.space.pieces_near(q, 2.0) {
            let off = (self.world.plate_top(&self.space.pieces[i].plate, q) - feet).abs();
            if off > 1.5 {
                continue;
            }
            for (k, f) in self.space.space[i].iter().enumerate() {
                if f.bounding_rect().is_none_or(|r| r.min().x - 2.0 > q.x || r.max().x + 2.0 < q.x || r.min().y - 2.0 > q.y || r.max().y + 2.0 < q.y) {
                    continue;
                }
                let d = Euclidean.distance(f, &pt);
                if d <= 2.0 && best.is_none_or(|b| d + off < b.0) {
                    let near = match f.closest_point(&pt) {
                        geo::Closest::Intersection(c) | geo::Closest::SinglePoint(c) => DVec2::new(c.x(), c.y()),
                        geo::Closest::Indeterminate => continue,
                    };
                    best = Some((d + off, self.space.node_of[i][k], near));
                }
            }
        }
        best.map(|(_, n, near)| (n, near.as_vec2()))
    }

    /// The way across the walking space from `from` to `to` (both on one
    /// node): its corners, the last `to`; none if there is none.
    pub(crate) fn way(&self, node: u32, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
        self.way_or_why(node, from, to).ok()
    }

    /// As `way`, or why there is none.
    pub(crate) fn way_or_why(&self, node: u32, from: Vec2, to: Vec2) -> Result<Vec<Vec2>, String> {
        let archipelago = self.archipelagos.get(node as usize).and_then(|a| a.as_ref()).ok_or_else(|| format!("node {node} has no navmesh"))?;
        let local = |p: Vec2| (DVec2::new(p.x as f64, p.y as f64) - self.origin).as_vec2();
        let reach = 0.5f32;
        let a = archipelago.sample_point(local(from), &reach).map_err(|e| format!("from {from:.2} off the mesh: {e:?}"))?;
        let b = archipelago.sample_point(local(to), &reach).map_err(|e| format!("to {to:.2} off the mesh: {e:?}"))?;
        let steps = archipelago.find_path(&a, &b, &HashMap::new(), PermittedAnimationLinks::All).map_err(|e| format!("no way: {e:?}"))?;
        let back = |p: Vec2| (DVec2::new(p.x as f64, p.y as f64) + self.origin).as_vec2();
        Ok(steps
            .into_iter()
            .map(|s| match s {
                PathStep::Waypoint(p) => back(p),
                PathStep::AnimationLink { end_point, .. } => back(end_point),
            })
            .collect())
    }

    /// The fewest teleports of no more than `range` from node `from` to
    /// `to`: the nodes on the way, `from` first, `to` last (landing only on
    /// nodes of `LAND_LEAST` or more, but for `to`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn route(&self, from: u32, to: u32, range: f32) -> Option<Vec<u32>> {
        let mut came = vec![u32::MAX; self.adjacent.len()];
        came[from as usize] = from;
        let mut queue = std::collections::VecDeque::from([from]);
        while let Some(n) = queue.pop_front() {
            if n == to {
                let mut way = vec![to];
                while *way.last().unwrap() != from {
                    way.push(came[*way.last().unwrap() as usize]);
                }
                way.reverse();
                return Some(way);
            }
            for &(m, d) in &self.adjacent[n as usize] {
                if d <= range && came[m as usize] == u32::MAX && (m == to || self.space.node_area[m as usize] >= LAND_LEAST) {
                    came[m as usize] = n;
                    queue.push_back(m);
                }
            }
        }
        None
    }

    /// The best way from `from` (on node `a`) to `to` (on node `b`, not
    /// `a`), walking and teleporting no more than `range`, each teleport
    /// counted as `cost` metres walked: its first teleport, and the whole
    /// way's cost (m). By A* over the portals, walking between them in a
    /// straight line (a little short, in a winding node). Its teleport
    /// lands where `land` finds, not on the portal planned by; and what it
    /// aims for, landed, for `sooner`.
    pub(crate) fn plan(&self, a: u32, from: Vec2, b: u32, to: Vec2, range: f32, cost: f32) -> Option<(Hop, u32, f32, Aim)> {
        let (hop, onto, goal, onward) = self.portals.plan(&self.space.node_area, a, DVec2::new(from.x as f64, from.y as f64), b, DVec2::new(to.x as f64, to.y as f64), range, cost)?;
        let aim = Aim { onward, short: if onto == b { LAND_SHORT } else { 0.0 } };
        let hop = self.land(hop.a, hop.ay, onto, aim, range, aim.left(hop.b)).map_or(hop, |(b, by, d)| Hop { b, by, d, ..hop });
        Some((hop, onto, goal, aim))
    }

    /// A teleport from `from` (its feet `fy` high) onto `onto`, landing
    /// nearer `aim` than `worst` (`Aim::left`): where it lands, how high,
    /// how far it goes (3D). The point of `onto` within `range` of `from`
    /// nearest the aim, `INSET` m or more inside its walking space; sought
    /// every `LAND_STEP` m, nearest first. (On the nearest point of its
    /// edge, as planned by, it went from the foot of a wall to its lip
    /// every time, never on into the plate, the user, 2026-11-04.)
    fn land(&self, from: DVec2, fy: f32, onto: u32, aim: Aim, range: f32, worst: f64) -> Option<(DVec2, f32, f32)> {
        let range = range as f64;
        let steps = (range / LAND_STEP).floor() as i32;
        let mut spots: Vec<(f64, DVec2)> = Vec::new();
        for i in -steps..=steps {
            for j in -steps..=steps {
                let q = from + DVec2::new(i as f64, j as f64) * LAND_STEP;
                let s = aim.left(q);
                if s < worst && q.distance(from) <= range {
                    spots.push((s, q));
                }
            }
        }
        spots.sort_by(|x, y| x.0.total_cmp(&y.0));
        let on = |q: DVec2| self.space.node_at_preferring(q, Some(onto)).filter(|f| f.0 == onto);
        spots.into_iter().find_map(|(_, q)| {
            let (_, i, _) = on(q)?;
            let y = self.world.plate_top(&self.space.pieces[i].plate, q);
            let d = (q.distance_squared(from) + (y - fy as f64).powi(2)).sqrt();
            (d <= range && (0..8).all(|k| on(q + DVec2::from_angle(k as f64 * std::f64::consts::TAU / 8.0) * INSET).is_some())).then_some((q, y as f32, d as f32))
        })
    }

    /// `planned` made from where it is (`at`, its feet), on its way to
    /// where it leaves from, if it lands no further from its aim than going
    /// on there would leave it to walk, all told: then it goes from here,
    /// not the foot of a wall or a lip. (Only within `range` of where it
    /// leaves from: the node it lands on is beyond it.) (Where it leaves
    /// from is half the story, the user, 2026-11-04.)
    pub(crate) fn sooner(&self, planned: Hop, onto: u32, aim: Aim, at: Vec3, range: f32) -> Option<Hop> {
        let here = DVec2::new(at.x as f64, at.z as f64);
        let on = here.distance(planned.a);
        if on > range as f64 {
            return None;
        }
        let (b, by, d) = self.land(here, at.y, onto, aim, range, aim.left(planned.b) + on + 1e-6)?;
        Some(Hop { a: here, ay: at.y, b, by, d })
    }

    /// Whether `p` lies inside the square built.
    pub(crate) fn covers(&self, p: Vec3) -> bool {
        (p.x as f64) > self.lo.x && (p.x as f64) < self.hi.x && (p.z as f64) > self.lo.y && (p.z as f64) < self.hi.y
    }

    /// `p` (x, z) kept on `node`'s walking space: itself if it is on it,
    /// else the nearest point of it within `KEEP` m; none if none is (the
    /// body is somewhere else altogether: leave it be). (Outdoors nothing
    /// stops a creature, it has no walls and stands on the ground's height
    /// wherever it is: it walked up a 2 m riser, carried on by its own pace
    /// into one as it stood telling, the user, 2026-11-04.)
    pub(crate) fn keep_on(&self, node: u32, p: Vec2) -> Option<Vec2> {
        let q = DVec2::new(p.x as f64, p.y as f64);
        let pt = Point::new(q.x, q.y);
        let mut best: Option<(f64, DVec2)> = None;
        for i in self.space.pieces_near(q, KEEP) {
            for (k, f) in self.space.space[i].iter().enumerate() {
                if self.space.node_of[i][k] != node || f.bounding_rect().is_none_or(|r| r.min().x - KEEP > q.x || r.max().x + KEEP < q.x || r.min().y - KEEP > q.y || r.max().y + KEEP < q.y) {
                    continue;
                }
                let d = Euclidean.distance(f, &pt);
                if d <= nodes::ROUNDING {
                    return Some(p);
                }
                if d <= KEEP && best.is_none_or(|b| d < b.0) {
                    let near = match f.closest_point(&pt) {
                        geo::Closest::Intersection(c) | geo::Closest::SinglePoint(c) => DVec2::new(c.x(), c.y()),
                        geo::Closest::Indeterminate => continue,
                    };
                    best = Some((d, near));
                }
            }
        }
        best.map(|(_, near)| near.as_vec2())
    }
}

/// How far off its node a body is brought back onto it (m).
const KEEP: f64 = 3.0;

/// A teleport's landing (see `Nodes::landing`): how far in from the edges
/// of the walking space (m), how far apart the spots sought (m), and how
/// near the end of its way (you) it lands at most (m).
const INSET: f64 = 1.0;
const LAND_STEP: f64 = 0.5;
const LAND_SHORT: f64 = 3.0;

/// What a teleport's landing aims for: where it goes next, landed (where it
/// leaves that node from, or the end of its way), and how near that it
/// stops (`LAND_SHORT` short of you).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Aim {
    onward: DVec2,
    short: f64,
}

impl Aim {
    /// How far from `q` it is left to walk (m, as the crow flies).
    fn left(&self, q: DVec2) -> f64 {
        (q.distance(self.onward) - self.short).abs()
    }
}

/// Where teleports leave from and land, to plan by: each hop's two ends,
/// a portal each (its twin the other end), joined to the nearest portals of
/// its own node by walking, in a straight line.
pub(crate) struct Portals {
    /// Each portal's node, where it is, its height; its twin, and how far
    /// the teleport between them goes.
    node: Vec<u32>,
    pub(crate) at: Vec<DVec2>,
    y: Vec<f32>,
    twin: Vec<u32>,
    d: Vec<f32>,
    /// Each portal's walking links: to which, how far.
    pub(crate) links: Vec<Vec<(u32, f32)>>,
    /// Each node's portals, by cell (`CELL`).
    cells: HashMap<(u32, i32, i32), Vec<u32>>,
}

/// The cells portals are found by (m); how many a portal is joined to by
/// walking, the nearest of its node; how far a walker's start is joined to
/// them (m): all those within it, or that many nearest.
const CELL: f64 = 16.0;
const NEAREST: usize = 6;
const START: f64 = 48.0;

impl Portals {
    fn new(hops: &HashMap<(u32, u32), Vec<Hop>>, nodes: usize) -> Portals {
        let mut p = Portals { node: Vec::new(), at: Vec::new(), y: Vec::new(), twin: Vec::new(), d: Vec::new(), links: Vec::new(), cells: HashMap::new() };
        for (&(lo, hi), list) in hops {
            for h in list {
                let i = p.node.len() as u32;
                p.node.extend([lo, hi]);
                p.at.extend([h.a, h.b]);
                p.y.extend([h.ay, h.by]);
                p.twin.extend([i + 1, i]);
                p.d.extend([h.d, h.d]);
            }
        }
        for (i, q) in p.at.iter().enumerate() {
            p.cells.entry((p.node[i], (q.x / CELL).floor() as i32, (q.y / CELL).floor() as i32)).or_default().push(i as u32);
        }
        // Each joined to the `NEAREST` of its own node.
        let mut links: Vec<Vec<(u32, f32)>> = vec![Vec::new(); p.at.len()];
        for i in 0..p.at.len() {
            for (j, d) in p.nearest(p.node[i], p.at[i], NEAREST + 1, 0.0) {
                if j as usize != i {
                    links[i].push((j, d as f32));
                    links[j as usize].push((i as u32, d as f32));
                }
            }
        }
        // (Each node's portals one part: where the nearest leave them in
        // two, the nearest pair between the parts joined too.)
        let mut by_node: Vec<Vec<u32>> = vec![Vec::new(); nodes];
        for (i, &n) in p.node.iter().enumerate() {
            by_node[n as usize].push(i as u32);
        }
        for list in &by_node {
            let Some(&first) = list.first() else { continue };
            loop {
                let mut part: std::collections::HashSet<u32> = std::collections::HashSet::from([first]);
                let mut stack = vec![first];
                while let Some(i) = stack.pop() {
                    for &(j, _) in &links[i as usize] {
                        if part.insert(j) {
                            stack.push(j);
                        }
                    }
                }
                if part.len() == list.len() {
                    break;
                }
                let (inside, outside): (Vec<u32>, Vec<u32>) = list.iter().partition(|i| part.contains(i));
                let mut best = (f64::INFINITY, 0, 0);
                for &i in &inside {
                    for &j in &outside {
                        let d = p.at[i as usize].distance(p.at[j as usize]);
                        if d < best.0 {
                            best = (d, i, j);
                        }
                    }
                }
                links[best.1 as usize].push((best.2, best.0 as f32));
                links[best.2 as usize].push((best.1, best.0 as f32));
            }
        }
        for l in &mut links {
            l.sort_by_key(|x| x.0);
            l.dedup_by_key(|x| x.0);
        }
        p.links = links;
        p
    }

    /// The `k` portals of node `n` nearest `q`, and all within `within` (m).
    fn nearest(&self, n: u32, q: DVec2, k: usize, within: f64) -> Vec<(u32, f64)> {
        let (cx, cz) = ((q.x / CELL).floor() as i32, (q.y / CELL).floor() as i32);
        let mut found: Vec<(u32, f64)> = Vec::new();
        // (Ring by ring outwards, till `k` are found nearer than the ring
        // reaches, and it reaches `within`; or 64 rings, a kilometre.)
        for r in 0i32..64 {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dz.abs() != r {
                        continue;
                    }
                    for &i in self.cells.get(&(n, cx + dx, cz + dz)).into_iter().flatten() {
                        found.push((i, self.at[i as usize].distance(q)));
                    }
                }
            }
            // (Cells reached all round: `r` whole cells past `q`'s own.)
            let reach = r as f64 * CELL;
            found.sort_by(|a, b| a.1.total_cmp(&b.1));
            if found.len() >= k && found[k - 1].1 <= reach && within <= reach {
                break;
            }
        }
        let kth = found.get(k.saturating_sub(1)).map_or(f64::INFINITY, |f| f.1);
        found.retain(|f| f.1 <= kth.max(within));
        found
    }

    #[allow(clippy::too_many_arguments)]
    fn plan(&self, area: &[f64], a: u32, from: DVec2, b: u32, to: DVec2, range: f32, cost: f32) -> Option<(Hop, u32, f32, DVec2)> {
        use std::{cmp::Reverse, collections::BinaryHeap};
        const GOAL: u32 = u32::MAX;
        let h = |q: DVec2| q.distance(to) as f32;
        let mut g: HashMap<u32, f32> = HashMap::new();
        let mut came: HashMap<u32, u32> = HashMap::new();
        // (Costs, all positive, ordered as their bits.)
        let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
        for (i, d) in self.nearest(a, from, NEAREST, START) {
            let d = d as f32;
            if g.get(&i).is_none_or(|&x| d < x) {
                g.insert(i, d);
                heap.push(Reverse(((d + h(self.at[i as usize])).to_bits(), i)));
            }
        }
        let mut goal = f32::INFINITY;
        while let Some(Reverse((f, i))) = heap.pop() {
            let f = f32::from_bits(f);
            if i == GOAL || f >= goal {
                break;
            }
            let gi = g[&i];
            // (Met before, nearer.)
            if f > gi + h(self.at[i as usize]) + 1e-3 {
                continue;
            }
            let n = self.node[i as usize];
            let mut next: Vec<(u32, f32)> = self.links[i as usize].iter().map(|&(j, d)| (j, gi + d)).collect();
            let t = self.twin[i as usize];
            let m = self.node[t as usize];
            if self.d[i as usize] <= range && (m == b || area[m as usize] >= LAND_LEAST) {
                next.push((t, gi + cost));
            }
            if n == b {
                let c = gi + self.at[i as usize].distance(to) as f32;
                if c < goal {
                    goal = c;
                    came.insert(GOAL, i);
                    heap.push(Reverse((c.to_bits(), GOAL)));
                }
            }
            for (j, c) in next {
                if g.get(&j).is_none_or(|&x| c < x) {
                    g.insert(j, c);
                    came.insert(j, i);
                    heap.push(Reverse(((c + h(self.at[j as usize])).to_bits(), j)));
                }
            }
        }
        if !goal.is_finite() {
            return None;
        }
        // The first teleport on the way: back from the goal, the last pair
        // of twins met.
        let mut first = None;
        let mut at = came[&GOAL];
        while let Some(&before) = came.get(&at) {
            if self.twin[before as usize] == at && self.node[before as usize] != self.node[at as usize] {
                first = Some((before, at));
            }
            at = before;
        }
        let (u, v) = first?;
        // (And where it goes next, landed: where it leaves its landing node
        // from, or `to`.)
        let mut way = vec![GOAL];
        let mut at = came[&GOAL];
        way.push(at);
        while let Some(&before) = came.get(&at) {
            way.push(before);
            at = before;
        }
        way.reverse();
        let from_v = way.iter().position(|&i| i == v)?;
        let onward = way[from_v..].windows(2).find_map(|w| match w {
            [_, GOAL] => Some(to),
            [i, j] if self.twin[*i as usize] == *j && self.node[*i as usize] != self.node[*j as usize] => Some(self.at[*i as usize]),
            _ => None,
        })?;
        let (u, v) = (u as usize, v as usize);
        Some((Hop { a: self.at[u], ay: self.y[u], b: self.at[v], by: self.y[v], d: self.d[u] }, self.node[v], goal, onward))
    }
}

/// District ground's nodes: being built, then built.
#[derive(Resource, Default)]
pub(crate) struct Districts {
    building: Option<Task<Result<Nodes, String>>>,
    pub(crate) nodes: Option<Arc<Nodes>>,
}

/// How near the edge of the square built you may come before it is built
/// anew round you (m): a hunter you meet there should have its nodes.
const NEAR_EDGE: f64 = 150.0;

/// `--opt blink`: the nodes asked for round where you are; and again,
/// round where you are then, when you come within `NEAR_EDGE` of the edge
/// of those built, or leave them (`tp` far, say: the user, 2026-11-04, in
/// the broken, not where they had been built, saw gladiators walk over
/// walls). The old ones kept till the new are in.
pub(crate) fn ask(mut commands: Commands, args: Res<Args>, world: Option<Res<crate::terrain::WorldGen>>, districts: Option<ResMut<Districts>>, player: Query<&Transform, With<crate::player::Player>>) {
    if !args.opt("blink") {
        return;
    }
    let (Some(world), Ok(player)) = (world, player.single()) else { return };
    let at = DVec2::new(player.translation.x as f64, player.translation.z as f64);
    if let Some(districts) = districts.as_deref() {
        let Some(nodes) = districts.nodes.as_deref() else { return };
        if districts.building.is_some() || (at.cmpgt(nodes.lo + NEAR_EDGE).all() && at.cmplt(nodes.hi - NEAR_EDGE).all()) {
            return;
        }
    }
    let world = world.0.clone();
    let draw = args.opt("nodedraw");
    info!("district nodes: building over {SIDE:.0} m round {:.0}, {:.0}", at.x, at.y);
    let building = Some(AsyncComputeTaskPool::get().spawn(async move { Nodes::build(world, at, SIDE, draw) }));
    match districts {
        Some(mut districts) => districts.building = building,
        None => commands.insert_resource(Districts { building, nodes: None }),
    }
}

/// The nodes drawn (`--opt nodedraw`), gone when they are built anew.
#[derive(Component)]
pub(crate) struct Drawn;

/// Built: said, and drawn, asked for; in place of any before, every
/// walker's plans on those forgotten. (Not built: said loudly, and fatal.)
pub(crate) fn ready(mut commands: Commands, districts: Option<ResMut<Districts>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, drawn: Query<Entity, With<Drawn>>, mut blinks: Query<&mut Blink>) {
    let Some(mut districts) = districts else { return };
    let Some(task) = districts.building.as_mut() else { return };
    let Some(built) = block_on(poll_once(task)) else { return };
    districts.building = None;
    let mut nodes = built.unwrap_or_else(|e| panic!("district nodes not built: {e}"));
    info!("{}", nodes.report);
    for e in &drawn {
        commands.entity(e).despawn();
    }
    if let Some((corners, colours)) = nodes.drawn.take() {
        let mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, corners).with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours).with_computed_flat_normals();
        let material = materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, cull_mode: None, ..default() });
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), Transform::IDENTITY, Drawn));
    }
    // (Their nodes, ways and teleports were the old nodes'.)
    if districts.nodes.is_some() {
        for mut blink in &mut blinks {
            blink.forget();
        }
    }
    districts.nodes = Some(Arc::new(nodes));
}

/// Across: `p`'s x and z.
trait Across {
    fn xz_(self) -> Vec2;
}

impl Across for Vec3 {
    fn xz_(self) -> Vec2 {
        Vec2::new(self.x, self.z)
    }
}

/// What a blinking walker does now.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Steer {
    /// Walks this way (across, of length 1).
    Toward(Vec3),
    /// Its teleport under way: its shade (or itself, with none) here,
    /// gliding along the line (its feet; through whatever is in the way).
    /// It stays where it left till it is there.
    Glide(Vec3),
    /// Stands: telling, or with nowhere to go.
    Stand,
    /// Is there.
    There,
    /// Is moved, at once, to here (its feet).
    Snap(Vec3),
    /// Isn't on the nodes (nor near them): goes as in the open.
    Off,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Walk,
    /// Telling, before it goes; how long yet; the node it lands on.
    Tell(Hop, f32, u32),
    /// Gliding along its teleport's line, through whatever is in the way
    /// (its shade, see `clip/shade.rs`); how far along (m); the node it
    /// lands on.
    Glide(Hop, f32, u32),
}

/// How often a walker's way and plan are found again (s), unless its goal
/// moves further than `MOVED` (m).
const MOVED: f32 = 1.0;
const REPLAN: f32 = 8.0;
/// How far on along its way a walker heads for (m).
const LOOK: f32 = 0.6;
/// How near where a teleport leaves from it must be to go (m).
const AT_HOP: f32 = 0.6;
/// How often a walker on its way to where a teleport leaves from looks
/// whether it may go from where it is (s).
const SOONER: f32 = 0.25;

/// A walker that teleports between nodes: its teleports' range (3D, m),
/// its tell before one (s), its rest after one (s); what it is doing,
/// its way and plan (kept a while, not found anew every frame: a choice
/// flipping between two every frame shakes a body, see
/// memory/vibration-causes); what began this frame, for the glows.
#[derive(Component)]
pub(crate) struct Blink {
    pub(crate) range: f32,
    pub(crate) tell: f32,
    pub(crate) rest: f32,
    /// How fast it glides along a teleport's line (m/s).
    pub(crate) speed: f32,
    /// Whether its body has a shade to split off (see `clip/shade.rs`):
    /// then no glows.
    pub(crate) shaded: bool,
    /// How many metres of walking a teleport is worth, planning.
    pub(crate) cost: f32,
    phase: Phase,
    resting: f32,
    /// The node it is on, as far as it knows (where it landed, say).
    pub(crate) node: Option<u32>,
    /// The way being walked (from where it was), how far along it (the
    /// stretch it is on), for which goal.
    way: Vec<Vec2>,
    way_at: usize,
    way_to: Vec2,
    /// The teleport chosen next: from which node, onto which, what its
    /// landing aims for; for which goal (its node, where).
    chosen: Option<(u32, u32, Hop, Aim, u32, Vec2)>,
    /// How long till it next looks whether it may go from where it is
    /// (s, every `SOONER`; see `Nodes::sooner`).
    sooner: f32,
    /// A tell begun (where it leaves from, where it lands, how long), or a
    /// teleport made (from, to), this frame.
    pub(crate) begun: Option<(Vec3, Vec3, f32)>,
    pub(crate) made: Option<(Vec3, Vec3)>,
    pub(crate) teleports: u32,
}

impl Blink {
    pub(crate) fn new(range: f32, tell: f32, rest: f32, cost: f32, speed: f32) -> Blink {
        Blink { range: range.min(FARTHEST as f32), tell, rest, speed: speed.max(1.0), shaded: false, cost, phase: Phase::Walk, resting: 0.0, node: None, way: Vec::new(), way_at: 0, way_to: Vec2::splat(f32::MAX), chosen: None, sooner: 0.0, begun: None, made: None, teleports: 0 }
    }

    /// Its node, way and teleport forgotten (the nodes built anew), as it
    /// was made but for its settings.
    fn forget(&mut self) {
        *self = Blink { shaded: self.shaded, teleports: self.teleports, ..Blink::new(self.range, self.tell, self.rest, self.cost, self.speed) };
    }

    /// Whether it is telling (standing, about to go).
    pub(crate) fn telling(&self) -> bool {
        matches!(self.phase, Phase::Tell(..) | Phase::Glide(..))
    }

    /// Whether it is gliding along a teleport's line.
    pub(crate) fn gliding(&self) -> bool {
        matches!(self.phase, Phase::Glide(..))
    }

    /// Gliding along a teleport's line: where along it (its feet), and the
    /// way it goes (of length 1).
    pub(crate) fn glide(&self) -> Option<(Vec3, Vec3)> {
        match self.phase {
            Phase::Glide(hop, along, _) => {
                let (from, to) = (Vec3::new(hop.a.x as f32, hop.ay, hop.a.y as f32), Vec3::new(hop.b.x as f32, hop.by, hop.b.y as f32));
                let way = (to - from).normalize_or(Vec3::Z);
                Some((from + way * along.min(from.distance(to)), way))
            }
            _ => None,
        }
    }

    /// Its tell, as its shade splits off it: where it lands, and how far
    /// through the tell (0..1).
    pub(crate) fn splitting(&self) -> Option<(Vec3, f32)> {
        match self.phase {
            Phase::Tell(hop, left, _) => Some((Vec3::new(hop.b.x as f32, hop.by, hop.b.y as f32), (1.0 - left / self.tell.max(1e-3)).clamp(0.0, 1.0))),
            _ => None,
        }
    }

    /// Along its way to `to` (on its node), from `at` (`from` on the
    /// walking space): which way to go. Toward a point `LOOK` on along its
    /// way from the nearest to it: always on along it. (Dropping each
    /// corner within 30 cm of it, at a neck a few millimetres wide it went
    /// back and forth for ever, its way found anew from either side.)
    fn follow(&mut self, nodes: &Nodes, node: u32, at: Vec3, from: Vec2, to: Vec2) -> Steer {
        // (Found anew only when its goal moves, or it is off it: found anew
        // every half second, landmass's ways from two points a metre apart
        // went round a pillar on either side, and it went back and forth
        // between them for ever.)
        let off = self.way.get(self.way_at..self.way_at + 2).is_none_or(|w| {
            let (a, b, p) = (w[0], w[1], at.xz_());
            let ab = b - a;
            let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
            (a + ab * t).distance(p) > MOVED
        });
        if self.way_to.distance(to) > MOVED || self.way.len() < 2 || off {
            self.way = nodes.way(node, from, to).map(|w| std::iter::once(from).chain(w).collect()).unwrap_or_default();
            self.way_at = 0;
            self.way_to = to;
        }
        let p = at.xz_();
        let Some(&end) = self.way.last() else { return Steer::Stand };
        if self.way.len() < 2 || p.distance(end) < 0.3 {
            return Steer::There;
        }
        // (The nearest point of the next few stretches.)
        let mut best = (f32::MAX, self.way_at, Vec2::ZERO);
        for k in self.way_at..(self.way.len() - 1).min(self.way_at + 4) {
            let (a, b) = (self.way[k], self.way[k + 1]);
            let ab = b - a;
            let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
            let q = a + ab * t;
            if q.distance(p) < best.0 {
                best = (q.distance(p), k, q);
            }
        }
        self.way_at = best.1;
        let (mut k, mut q, mut left) = (best.1, best.2, LOOK);
        while k + 1 < self.way.len() {
            let r = q.distance(self.way[k + 1]);
            if r >= left {
                q += (self.way[k + 1] - q) / r * left;
                break;
            }
            left -= r;
            q = self.way[k + 1];
            k += 1;
        }
        let d = q - p;
        if d.length() < 1e-4 { Steer::There } else { Steer::Toward(Vec3::new(d.x, 0.0, d.y).normalize()) }
    }

    /// What it does, its feet at `at`, going for `goal` (`above` over its
    /// ground: 0 a spot's, `player::EYE` your eyes'), `dt` on.
    pub(crate) fn steer(&mut self, nodes: &Nodes, at: Vec3, goal: Vec3, above: f32, dt: f32) -> Steer {
        if let Phase::Tell(hop, left, onto) = self.phase {
            if left > dt {
                self.phase = Phase::Tell(hop, left - dt, onto);
                return Steer::Stand;
            }
            self.phase = Phase::Glide(hop, 0.0, onto);
            return Steer::Glide(Vec3::new(hop.a.x as f32, hop.ay, hop.a.y as f32));
        }
        // Along its teleport's line, in a straight line, at its speed; there,
        // landed.
        if let Phase::Glide(hop, along, onto) = self.phase {
            let (from, to) = (Vec3::new(hop.a.x as f32, hop.ay, hop.a.y as f32), Vec3::new(hop.b.x as f32, hop.by, hop.b.y as f32));
            let along = along + self.speed * dt;
            let length = from.distance(to);
            if along < length {
                self.phase = Phase::Glide(hop, along, onto);
                return Steer::Glide(from.lerp(to, along / length));
            }
            self.phase = Phase::Walk;
            self.node = Some(onto);
            self.resting = self.rest;
            self.teleports += 1;
            self.way.clear();
            self.chosen = None;
            let to = Vec3::new(hop.b.x as f32, hop.by, hop.b.y as f32);
            self.made = Some((Vec3::new(hop.a.x as f32, hop.ay, hop.a.y as f32), to));
            return Steer::Snap(to);
        }
        self.resting -= dt;
        let Some((me, mine)) = nodes.locate(at, 0.0, self.node) else { return Steer::Off };
        self.node = Some(me);
        // (Where it is going: its node, or the nearest one to it.)
        let Some((there, spot)) = nodes.locate(goal, above, None) else { return Steer::Stand };
        if there == me {
            self.chosen = None;
            return self.follow(nodes, me, at, mine, spot);
        }
        // Another node: the first teleport of the best way there, kept till
        // it is made, unless its goal moves to another node or `REPLAN` m
        // off (found anew every second, two ways about as good won in
        // turn, and it walked back and forth between them); to where it
        // leaves from, and then its tell.
        let keep = self.chosen.filter(|c| c.0 == me && c.4 == there && c.5.distance(spot) < REPLAN);
        let hop = match keep {
            Some(c) => c.2,
            None => {
                let Some((hop, onto, _, aim)) = nodes.plan(me, mine, there, spot, self.range, self.cost) else { return Steer::Stand };
                self.chosen = Some((me, onto, hop, aim, there, spot));
                hop
            }
        };
        let mut leave = Vec2::new(hop.a.x as f32, hop.a.y as f32);
        // (Or from here, if it does as well from here: see `sooner`.)
        self.sooner -= dt;
        let mut hop = hop;
        if self.resting <= 0.0 && self.sooner <= 0.0 && leave.distance(at.xz_()) >= AT_HOP
            && let Some(c) = self.chosen
        {
            self.sooner = SOONER;
            if let Some(now) = nodes.sooner(hop, c.1, c.3, at, self.range) {
                hop = now;
                leave = at.xz_();
            }
        }
        if leave.distance(at.xz_()) < AT_HOP && self.resting <= 0.0 {
            self.phase = Phase::Tell(hop, self.tell, self.chosen.map_or(me, |c| c.1));
            self.begun = Some((Vec3::new(hop.a.x as f32, hop.ay, hop.a.y as f32), Vec3::new(hop.b.x as f32, hop.by, hop.b.y as f32), self.tell));
            return Steer::Stand;
        }
        match self.follow(nodes, me, at, mine, leave) {
            Steer::There => Steer::Stand,
            s => s,
        }
    }
}

/// A glow: a teleport's tell, where it leaves from and where it lands, or
/// its flash as it goes; how old, how long it lasts, how big it grows.
#[derive(Component)]
pub(crate) struct Glow {
    age: f32,
    life: f32,
    size: f32,
}

/// The glows' look.
#[derive(Resource)]
pub(crate) struct GlowLook {
    mesh: Handle<Mesh>,
    tell: Handle<StandardMaterial>,
    flash: Handle<StandardMaterial>,
}

pub(crate) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let glow = |c: Color| StandardMaterial { base_color: c, emissive: c.to_linear() * 6.0, unlit: true, alpha_mode: AlphaMode::Add, ..default() };
    commands.insert_resource(GlowLook { mesh: meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap()), tell: materials.add(glow(Color::srgb(0.35, 0.6, 1.0))), flash: materials.add(glow(Color::srgb(0.9, 0.95, 1.0))) });
}

/// Each tell begun this frame, a glow at both ends, growing; each teleport
/// made, a flash at both.
pub(crate) fn tells(mut commands: Commands, look: Option<Res<GlowLook>>, mut blinks: Query<&mut Blink>) {
    let Some(look) = look else { return };
    for mut b in &mut blinks {
        if let Some((from, to, tell)) = b.begun.take().filter(|_| !b.shaded) {
            for at in [from, to] {
                commands.spawn((Mesh3d(look.mesh.clone()), MeshMaterial3d(look.tell.clone()), Transform::from_translation(at + Vec3::Y * 1.2).with_scale(Vec3::splat(0.05)), Glow { age: 0.0, life: tell, size: 0.9 }));
            }
        }
        if let Some((from, to)) = b.made.take().filter(|_| !b.shaded) {
            for at in [from, to] {
                commands.spawn((Mesh3d(look.mesh.clone()), MeshMaterial3d(look.flash.clone()), Transform::from_translation(at + Vec3::Y * 1.2).with_scale(Vec3::splat(1.2)), Glow { age: 0.0, life: 0.18, size: -1.2 }));
            }
        }
    }
}

/// Glows grow (a tell) or shrink (a flash), and are gone.
pub(crate) fn glows(mut commands: Commands, time: Res<Time>, mut glows: Query<(Entity, &mut Glow, &mut Transform)>) {
    for (e, mut g, mut t) in &mut glows {
        g.age += time.delta_secs();
        if g.age >= g.life {
            commands.entity(e).despawn();
            continue;
        }
        let k = g.age / g.life;
        t.scale = Vec3::splat(if g.size > 0.0 { 0.05 + g.size * k * k } else { -g.size * (1.0 - k) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Contains;

    /// A walker blinking after a target on tiers (the most broken-up
    /// ground but stacks), moved as its steering says, its feet on the
    /// plates: every chase reached, never off the walking space between
    /// teleports, every teleport within its range and onto the walking
    /// space, and its teleports against the fewest there are.
    /// (`cargo test -p game --profile fast blink_walks -- --ignored --nocapture`;
    /// `BLINK_DISTRICT=stacks` and so on for another.)
    #[test]
    #[ignore]
    fn blink_walks() {
        let world = Arc::new(PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library")));
        let name = std::env::var("BLINK_DISTRICT").unwrap_or_else(|_| "tiers".into());
        let centres = [("floor", (10880.0, 128.0)), ("tiers", (5248.0, 640.0)), ("stacks", (12928.0, 128.0)), ("broken", (7296.0, 640.0)), ("uplands", (9344.0, 4736.0)), ("highlands", (9344.0, 2688.0))];
        let centre = DVec2::from(centres.iter().find(|c| c.0 == name).expect("BLINK_DISTRICT: a district's name").1);
        let nodes = Nodes::build(world.clone(), centre, SIDE, false).unwrap();
        println!("{}", nodes.report);
        // Where a body stands: on the walking space (within 5 cm of it), its
        // feet on the plate there.
        let on = |q: Vec2| -> Option<(f32, f64)> {
            let d = DVec2::new(q.x as f64, q.y as f64);
            let i = nodes.space.under(d)?;
            let pt = Point::new(d.x, d.y);
            let off = nodes.space.pieces_near(d, 0.1).into_iter().flat_map(|j| nodes.space.space[j].iter()).map(|f| if f.contains(&pt) { 0.0 } else { Euclidean.distance(f, &pt) }).fold(f64::INFINITY, f64::min);
            Some((world.plate_top(&nodes.space.pieces[i].plate, d) as f32, off))
        };
        // Chases: spots on the walking space at least 100 m inside the
        // square, each to another 20 to 150 m off.
        let mut rng = super::super::zz_world::Rng(0x5eed_b11c_0de5_0001);
        let (lo, hi) = (nodes.lo + 100.0, nodes.hi - 100.0);
        let mut spots: Vec<Vec2> = Vec::new();
        while spots.len() < 2000 {
            let q = DVec2::new(lo.x + (hi.x - lo.x) * rng.next() as f64, lo.y + (hi.y - lo.y) * rng.next() as f64);
            if nodes.space.node_at(q).is_some_and(|(n, ..)| nodes.space.node_area[n as usize] >= LAND_LEAST) {
                spots.push(q.as_vec2());
            }
        }
        // (Ways within a node first: pairs of spots on one node.)
        let mut why: HashMap<String, usize> = HashMap::new();
        let (mut asked, mut found) = (0, 0);
        for _ in 0..200000 {
            if asked == 300 {
                break;
            }
            let (a, b) = (spots[(rng.next() * spots.len() as f32) as usize % spots.len()], spots[(rng.next() * spots.len() as f32) as usize % spots.len()]);
            let node = |q: Vec2| nodes.space.node_at(DVec2::new(q.x as f64, q.y as f64)).map(|n| n.0);
            if node(a) != node(b) || a.distance(b) < 5.0 {
                continue;
            }
            asked += 1;
            match nodes.way_or_why(node(a).unwrap(), a, b) {
                Ok(_) => found += 1,
                Err(e) => {
                    println!("  no way, {a:.2} to {b:.2}: {e}");
                    *why.entry(e.split(':').next().unwrap_or("").to_string()).or_default() += 1;
                }
            }
        }
        println!("ways within a node: {found} of {asked} found; none: {why:?}");
        assert_eq!(found, asked, "ways within a node not found");
        let range = 10.0;
        let (dt, speed) = (1.0 / 30.0, 5.0);
        let (mut reached, mut chases, mut strayed, mut walked, mut worst_stray) = (0, 0, 0.0f32, 0.0f32, (0.0f64, Vec2::ZERO));
        let (mut teleports, mut fewest, mut extra) = (0u32, 0usize, Vec::new());
        let mut cost: Vec<f32> = Vec::new();
        // (Each teleport's length, and whether it landed inset, not on the
        // edge: see `Nodes::land`.)
        let (mut lengths, mut inset) = (Vec::<f32>::new(), 0usize);
        let mut failed: Vec<String> = Vec::new();
        while chases < std::env::var("BLINK_CHASES").ok().and_then(|v| v.parse().ok()).unwrap_or(300) {
            let (a, b) = (spots[(rng.next() * spots.len() as f32) as usize % spots.len()], spots[(rng.next() * spots.len() as f32) as usize % spots.len()]);
            // (BLINK_ONLY=x,z,x,z: that chase alone, every frame told from
            // BLINK_FROM s on, to see what it does.)
            let only: Option<Vec<f32>> = std::env::var("BLINK_ONLY").ok().map(|v| v.split(',').map(|x| x.parse().unwrap()).collect());
            let (a, b) = match &only {
                Some(o) => (Vec2::new(o[0], o[1]), Vec2::new(o[2], o[3])),
                None => (a, b),
            };
            if !(20.0..=150.0).contains(&a.distance(b)) {
                continue;
            }
            chases += 1;
            let goal = Vec3::new(b.x, on(b).unwrap().0 + crate::player::EYE, b.y);
            let (na, nb) = (nodes.locate(Vec3::new(a.x, on(a).unwrap().0, a.y), 0.0, None).unwrap().0, nodes.locate(goal, crate::player::EYE, None).unwrap().0);
            let least = nodes.route(na, nb, range).map(|r| r.len() - 1);
            let mut blink = Blink::new(range, 0.6, 1.5, 8.0, 20.0);
            let mut at = Vec3::new(a.x, on(a).unwrap().0, a.y);
            let mut done = false;
            let mut why = String::new();
            let mut trace: Vec<String> = Vec::new();
            let mut last: Option<(Option<u32>, String)> = None;
            for frame in 0..(120.0 / dt) as usize {
                let t = Instant::now();
                let steer = blink.steer(&nodes, at, goal, crate::player::EYE, dt);
                cost.push(t.elapsed().as_secs_f32() * 1e6);
                // (What it did, each time its node or what it does changes.)
                if only.is_some() && frame as f32 * dt >= std::env::var("BLINK_FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0) && frame as f32 * dt < std::env::var("BLINK_FROM").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) + 1.0 {
                    let mine = nodes.locate(at, 0.0, blink.node);
                    println!("      {:.2} s at {:.2} on {mine:?}: {steer:?}; way at {} of {:?}", frame as f32 * dt, Vec2::new(at.x, at.z), blink.way_at, blink.way.iter().take(5).map(|w| format!("{w:.2}")).collect::<Vec<_>>());
                }
                let now = (blink.node, format!("{:?}", std::mem::discriminant(&steer)));
                if (last.as_ref() != Some(&now) || frame % 150 == 0) && trace.len() < 80 {
                    let route = now.0.and_then(|n| nodes.route(n, nb, range));
                    trace.push(format!("{:.1} s at {at:.1} node {:?} (route {:?}): {steer:?}{}", frame as f32 * dt, now.0, route, blink.chosen.map(|c| format!(", chosen {}->{} from {:.1} to {:.1}", c.0, c.1, c.2.a, c.2.b)).unwrap_or_default()));
                    last = Some(now);
                }
                match steer {
                    Steer::Toward(way) => {
                        let to = Vec2::new(at.x, at.z) + Vec2::new(way.x, way.z) * speed * dt;
                        let Some((y, off)) = on(to) else {
                            why = format!("walked off the pieces at {to:.1}");
                            break;
                        };
                        walked += speed * dt;
                        if off > 0.05 {
                            strayed += speed * dt;
                            if off > worst_stray.0 {
                                worst_stray = (off, to);
                            }
                        }
                        at = Vec3::new(to.x, y, to.y);
                    }
                    Steer::Snap(to) => {
                        let (leave, land) = blink.made.take().unwrap();
                        let Some((y, off)) = on(Vec2::new(to.x, to.z)) else {
                            let d = DVec2::new(to.x as f64, to.z as f64);
                            let pt = Point::new(d.x, d.y);
                            let near: Vec<String> = nodes.space.pieces_near(d, 3.0).into_iter().map(|i| format!("piece {i} (hole {}, pillar {}) {:.4} m off, fragments {:?}", nodes.space.pieces[i].hole, nodes.space.pieces[i].pillar, Euclidean.distance(&nodes::polygon(&nodes.space.pieces[i].poly), &pt), nodes.space.space[i].iter().map(|f| format!("{:.4}", Euclidean.distance(f, &pt))).collect::<Vec<_>>())).filter(|l| !l.contains("fragments []")).collect();
                            why = format!("teleported off the pieces, to {to:.3} (from {leave:.3}); near: {near:?}");
                            break;
                        };
                        assert!(leave.distance(land) <= range + 1e-3, "a teleport of {:.2} m, from {leave:.1} to {land:.1}", leave.distance(land));
                        assert!(Vec2::new(leave.x, leave.z).distance(Vec2::new(at.x, at.z)) <= AT_HOP, "a teleport from {at:.1}, {leave:.1} its start");
                        assert!(off <= 0.05, "a teleport onto {off:.2} m off the walking space, at {to:.1}");
                        lengths.push(leave.distance(land));
                        let q = DVec2::new(to.x as f64, to.z as f64);
                        if (0..8).all(|k| nodes.space.node_at(q + DVec2::from_angle(k as f64 * std::f64::consts::TAU / 8.0) * (INSET - 0.01)).is_some()) {
                            inset += 1;
                        }
                        at = Vec3::new(to.x, y, to.z);
                    }
                    Steer::There => {
                        done = true;
                        break;
                    }
                    Steer::Stand | Steer::Glide(_) => {}
                    Steer::Off => {
                        why = format!("off the nodes at {at:.1}");
                        break;
                    }
                }
                if Vec2::new(at.x, at.z).distance(b) < 1.0 {
                    done = true;
                    break;
                }
            }
            if done {
                reached += 1;
                teleports += blink.teleports;
                if let Some(l) = least {
                    fewest += l;
                    extra.push(blink.teleports as i64 - l as i64);
                }
            } else if failed.len() < 12 {
                if failed.len() < 2 {
                    for t in &trace {
                        println!("    {t}");
                    }
                }
                failed.push(format!("{a:.1} to {b:.1} ({:.0} m, fewest {least:?}): {}; {} teleports, at {:.1}", a.distance(b), if why.is_empty() { "not there in 120 s".into() } else { why }, blink.teleports, at));
            }
        }
        extra.sort();
        cost.sort_by(f32::total_cmp);
        println!(
            "{name}: {reached} of {chases} chases reached; teleports {teleports}, the fewest there are {fewest} (over them, median {}, worst tenth {}, most {}); walked {walked:.0} m, {strayed:.1} m of it more than 5 cm off the walking space (worst {:.2} m, at {:.1}); steering per frame: median {:.0} µs, 99th {:.0} µs, most {:.0} µs",
            extra.get(extra.len() / 2).copied().unwrap_or(0),
            extra.get(extra.len() * 9 / 10).copied().unwrap_or(0),
            extra.last().copied().unwrap_or(0),
            worst_stray.0,
            worst_stray.1,
            cost[cost.len() / 2],
            cost[cost.len() * 99 / 100],
            cost.last().unwrap()
        );
        lengths.sort_by(f32::total_cmp);
        println!("teleports: length median {:.1} m, tenth {:.1} m, ninetieth {:.1} m; {inset} of {} landed {INSET} m or more inside the walking space", lengths[lengths.len() / 2], lengths[lengths.len() / 10], lengths[lengths.len() * 9 / 10], lengths.len());
        for f in &failed {
            println!("  not reached: {f}");
        }
        assert_eq!(reached, chases, "chases not reached");
        // (Off it no more than the corners it cuts, at 30 cm: within the
        // band of its radius round the walking space, its body clear of
        // any edge it can't cross.)
        assert!(worst_stray.0 <= 0.3, "{:.2} m off the walking space, at {:.1}", worst_stray.0, worst_stray.1);
        // Planning alone, to see what it costs: from spot to spot, 20 to
        // 150 m, on two nodes.
        let mut plans: Vec<f32> = Vec::new();
        while plans.len() < 1000 {
            let (a, b) = (spots[(rng.next() * spots.len() as f32) as usize % spots.len()], spots[(rng.next() * spots.len() as f32) as usize % spots.len()]);
            let node = |q: Vec2| nodes.space.node_at(DVec2::new(q.x as f64, q.y as f64)).map(|n| n.0).unwrap();
            if !(20.0..=150.0).contains(&a.distance(b)) || node(a) == node(b) {
                continue;
            }
            let t = Instant::now();
            let found = nodes.plan(node(a), a, node(b), b, range, 8.0);
            plans.push(t.elapsed().as_secs_f32() * 1e6);
            assert!(found.is_some(), "no plan from {a:.1} to {b:.1}");
        }
        plans.sort_by(f32::total_cmp);
        println!("plans: median {:.0} µs, 90th {:.0} µs, 99th {:.0} µs, most {:.0} µs", plans[plans.len() / 2], plans[plans.len() * 9 / 10], plans[plans.len() * 99 / 100], plans.last().unwrap());
    }
}
