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
//! for its tell, and is there, as far on into the node as its range takes
//! it (`Nodes::land`); then rests before it may again. Teleporting is a
//! creature's own, case by case (the user, 2026-11-14: applied to all it
//! won't look good): only those with a look for it have it, authored
//! bodies by their shade (`combat/hunter/clip/shade.rs`); the others'
//! range is 0 (the glows they once had at both ends did not work, and
//! are gone). Off a lip onto a lower node within
//! `DROP_GAP` it drops instead (`drops`, the user, 2026-11-14: every enemy
//! may drop down anywhere): one way, any depth, no tell; the rig's leap,
//! its pose and landing.

use bevy::platform::collections::HashMap;
use std::{sync::Arc, time::Instant};

use bevy::{
    math::DVec2,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use geo::{
    Buffer, Simplify,
    algorithm::buffer::{BufferStyle, LineJoin},
};
use geo::{BoundingRect, ClosestPoint, Distance, Euclidean, MultiPolygon, Point, Polygon};
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

/// Drops (the user, 2026-11-14: every enemy may drop down anywhere, one
/// move for all; the rig's leap is its pose and landing): how far across
/// a lip a walker steps off (m, between the walking space's edges above
/// and below); how much further on it may land (m); how fast up it goes
/// as it does (m/s, each tried in turn: enough to clear the lip before it
/// sinks below it); what one counts as, planning (m walked, and its way
/// across).
const DROP_GAP: f64 = 3.0;
const DROP_ON: f64 = 4.0;
const DROP_UP: [f64; 3] = [1.5, 3.0, 4.5];

/// A leap's upward speed (m/s), as given for gravity of 25 m/s²: as long
/// in the air at the leaps' gravity (`rig::leap_gravity`; at 60, the
/// kicks off a lip were too short to clear it).
fn kick(up: f64) -> f64 {
    up * crate::rig::leap_gravity() as f64 / 25.0
}
const DROP_COST: f32 = 2.0;
/// Hops: how far across (m), and how far over its ledge it rises (m).
const HOP_GAP: f64 = 1.5;
const HOP_CLEAR: f64 = 0.4;
/// How far back from its ledge's foot a hop may take off (m, tried in turn).
const HOP_BACK: [f64; 5] = [0.0, 0.5, 1.0, 1.5, 2.0];
/// A falling body's reach round its root (m, the beast's: the rig sweeps
/// its capsule, `combat/hunter.rs`), and how high over its root the sweep
/// goes (the rig's step up): ground no higher than that is cleared.
const FALL_RADIUS: f64 = 0.45;
const FALL_LIFT: f64 = crate::combat::CREATURE_STEP as f64;

/// How long a leap of `up` m/s takes to come down `h` m (s; below 0, up
/// onto a ledge `-h` high, on its way down: at its top, if it reaches no
/// higher).
fn fall_time(up: f64, h: f64) -> f64 {
    let g = crate::rig::leap_gravity() as f64;
    (up + (up * up + 2.0 * g * h).max(0.0).sqrt()) / g
}

/// Whether a leap of `up` m/s from `a` (its feet `ay` high), aimed by its
/// fall time at `q` (`by` high), comes down on node `onto` at every frame
/// up to the longest of `frames` (s), its body (`FALL_RADIUS` round it, a
/// drop's, `body`) never through the plates' tops more than `FALL_LIFT`,
/// nor a pillar. Exact, by its curve, no stepping: a frame of `dt` steps it
/// `g dt t / 2` below its exact curve at time `t` (the rig's: its velocity,
/// then its root), so the longest frame's curve is the lowest; along its
/// path across, the pieces under it (clipped, convex, `Space::hulls`), each
/// a flat or tilted top, so the curve less a top is concave, judged at its
/// ends. Its root above every top it passes till the lowest curve comes
/// down to `by` (a hop rising into its ledge, no), there on `onto`; its
/// body likewise, against the tops grown by its radius; no gap under it (a
/// canal: no). (Stepped at 1/30 and 1/240 s, with a ground lookup a step
/// and eight about the body, 170 000 falls in the stacks took 5 s of work;
/// the work the problem needs, tens of milliseconds, memory/essential-cost.)
#[allow(clippy::too_many_arguments)]
fn fall_clear(space: &Space, world: &PlateWorld, a: DVec2, ay: f64, q: DVec2, by: f64, up: f64, onto: u32, frames: &[f64], body: bool) -> bool {
    // (A hop up a ledge stands at its foot, the ledge within its body's
    // reach: the body's for a drop's look alone.)
    let ring = body && by < ay;
    let g = crate::rig::leap_gravity() as f64;
    let dt = frames.iter().copied().fold(0.0, f64::max);
    let whole = fall_time(up, ay - by);
    if whole.is_nan() || whole <= 0.0 {
        return false;
    }
    // The lowest curve: `ay + w t - g t² / 2`, down to `by` at `low`.
    let w = up - g * dt / 2.0;
    let disc = w * w + 2.0 * g * (ay - by);
    if disc < 0.0 {
        return false;
    }
    let low = (w + disc.sqrt()) / g;
    let end = (low / whole).min(1.0);
    let y = |u: f64| {
        let t = u * whole;
        ay + w * t - g * t * t / 2.0
    };
    let span = q - a;
    // (Clear above `top` all along `u0`..`u1`, but for touching it at the
    // end, coming down there, no higher than where it lands.)
    let clear = |u0: f64, u1: f64, top: &dyn Fn(f64) -> f64, lift: f64| -> bool {
        let first = if u0 <= 1e-9 { 1e-4f64.min(u1) } else { u0 };
        let ok = |u: f64| if u >= end - 1e-9 { y(u) + lift >= top(u) - 1e-6 } else { y(u) + lift > top(u) };
        ok(first) && ok(u1)
    };
    let pad = span.length() * 0.5 + FALL_RADIUS + 1.0;
    let mut under: Vec<(f64, f64)> = Vec::new();
    for i in space.pieces_near(a + span * 0.5, pad) {
        let piece = &space.pieces[i];
        if piece.hole {
            continue;
        }
        let top = |u: f64| world.plate_top(&piece.plate, a + span * u);
        if let Some((u0, u1)) = nodes::along(&space.hulls[i], a, span, 0.0) {
            let (u0, u1) = (u0.max(0.0), u1.min(end));
            if u0 < u1 {
                under.push((u0, u1));
                if !clear(u0, u1, &top, 0.0) {
                    return false;
                }
            }
        }
        if ring
            && let Some((u0, u1)) = nodes::along(&space.hulls[i], a, span, FALL_RADIUS)
        {
            let (u0, u1) = (u0.max(0.0), u1.min(end));
            if u0 < u1 && !clear(u0, u1, &top, FALL_LIFT) {
                return false;
            }
        }
    }
    // (A pillar's obstacle, already grown by a walker's radius, by the rest
    // of the body's: none in reach.)
    if ring {
        let (lo, hi) = (a.min(q) - pad, a.max(q) + pad);
        let mut ks: Vec<usize> = nodes::cells(lo, hi, 0.0).flat_map(|c| space.blocked.get(&c).into_iter().flatten().copied()).collect();
        ks.sort();
        ks.dedup();
        let more = (FALL_RADIUS - space.fit.radius).max(0.0);
        for k in ks {
            for block in &space.blocks[k].0 {
                let hull: Vec<DVec2> = block.exterior().0.iter().map(|c| DVec2::new(c.x, c.y)).collect();
                if nodes::along(&nodes::ccw(hull), a, span, more).is_some_and(|(u0, u1)| u0.max(0.0) < u1.min(end)) {
                    return false;
                }
            }
        }
    }
    // No gap under its path (a canal's cut, a hole).
    under.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut at = 0.0;
    for (u0, u1) in under {
        if u0 > at + 1e-3 {
            return false;
        }
        at = at.max(u1);
    }
    if at < end - 1e-3 {
        return false;
    }
    // Down, at the longest frame (soonest), on `onto`; at the shortest, `q`.
    space.node_at_preferring(a + span * end, Some(onto)).is_some_and(|f| f.0 == onto)
}

/// The frames a fall is checked at (s): a long one loses height, so every
/// frame between these does as well.
const FRAMES: [f64; 2] = [1.0 / 30.0, 1.0 / 240.0];

/// The drops off every lip of `space`: one way, from a point on the edge
/// of a node's walking space, within `DROP_GAP` of a lower node's, onto a
/// point `INSET` inside that one, the nearest whose fall is clear: the
/// rig's leap, by its gravity, its root never coming down on ground
/// short of there and its body never swept into any (any within
/// `FALL_RADIUS` of it and `FALL_LIFT` over it), nor into a pillar. The
/// nearest few along each edge (`nodes::HOP_CELL`), as teleports.
/// And hops (the user, 2026-11-14: one case, to judge how its look would
/// be made): up onto a ledge within `HOP_GAP` across and as high as you
/// climb it without the hook (`player::hookless_reach`), its leap the
/// least that clears it by `HOP_CLEAR`, a metre a second at a time.
fn drops(space: &Space, world: &PlateWorld) -> Vec<(u32, u32, Hop)> {
    let climb = space.fit.climb;
    let links = space.links(world, DROP_GAP, Some(nodes::HOP_CELL), |gap, rise| {
        if rise < -climb {
            gap - DROP_GAP
        } else if rise > climb && gap <= HOP_GAP && rise <= crate::player::hookless_reach(gap) {
            gap - HOP_GAP
        } else {
            f64::INFINITY
        }
    });
    let on = |n: u32, q: DVec2| space.node_at_preferring(q, Some(n)).is_some_and(|f| f.0 == n);
    let candidates: Vec<(u32, u32, nodes::Link)> = links.iter().filter(|((_, b), _)| space.node_area[*b as usize] >= LAND_LEAST).flat_map(|(&(a, b), list)| list.iter().filter(|l| l.short <= 0.0).map(move |l| (a, b, *l))).collect();
    nodes::par_chunks(&candidates, |c| {
                    c.iter()
                        .filter_map(|&(a, b, l)| {
                            let gap = l.a.distance(l.b);
                            let dir = (l.b - l.a).try_normalize()?;
                            // (A hop takes off a step or two back: its leap
                            // goes across at one speed, and from the foot of
                            // its ledge it met the edge still rising.)
                            let backs: &[f64] = if l.b.length() > 0.0 && space.under(l.b).is_some_and(|i| world.plate_top(&space.pieces[i].plate, l.b) > l.ay) { &HOP_BACK } else { &[0.0] };
                            // (The gentlest leap first, from as far back as
                            // it takes: from the foot of its ledge, the first
                            // that cleared it sprang twice its height.)
                            let hop = backs.len() > 1;
                            let ups: Vec<f64> = if hop { (0..9).map(|k| (2.0 * crate::rig::leap_gravity() as f64 * HOP_CLEAR).sqrt() + kick(0.5) * k as f64).collect() } else { DROP_UP.map(kick).to_vec() };
                            // (Where it may land, found once, nearest first:
                            // on its node, and `INSET` all round it, by where
                            // nine paths along it cross the node, its own and
                            // eight about it.)
                            let (s0, s1) = (gap, gap + DROP_ON);
                            let main = space.node_along(b, l.a, dir, s0, s1);
                            let rays: Vec<Vec<(f64, f64)>> = if main.is_empty() {
                                Vec::new()
                            } else {
                                std::iter::once(main).chain((0..8).map(|k| space.node_along(b, l.a + DVec2::from_angle(k as f64 * std::f64::consts::FRAC_PI_4) * INSET, dir, s0, s1))).collect()
                            };
                            let inside = |spans: &[(f64, f64)], s: f64| spans.iter().any(|&(u0, u1)| u0 <= s && s <= u1);
                            let spots: Vec<(DVec2, f64)> = (0..)
                                .map(|k| gap + k as f64 * LAND_STEP)
                                .take_while(|s| *s <= gap + DROP_ON)
                                .filter(|&s| !rays.is_empty() && rays.iter().all(|r| inside(r, s)))
                                .map(|s| l.a + dir * s)
                                .filter_map(|q| space.under(q).map(|i| (q, world.plate_top(&space.pieces[i].plate, q))))
                                .collect();
                            let froms: Vec<DVec2> = backs.iter().map(|&back| l.a - dir * back).filter(|&from| from == l.a || on(a, from)).collect();
                            for up0 in ups {
                                for &from in &froms {
                                    for &(q, by) in &spots {
                                        let up = if hop { (up0 * up0 + 2.0 * crate::rig::leap_gravity() as f64 * (by - l.ay)).sqrt() } else { up0 };
                                        if fall_clear(space, world, from, l.ay, q, by, up, b, &FRAMES, true) {
                                            let d = (q.distance_squared(from) + (l.ay - by).powi(2)).sqrt();
                                            return Some((a, b, Hop { a: from, ay: l.ay as f32, b: q, by: by as f32, d: d as f32, leap: Some(up as f32) }));
                                        }
                                    }
                                }
                            }
                            None
                        })
                        .collect::<Vec<_>>()
    })
    .into_iter()
    .flatten()
    .collect()
}

/// The teleports between nodes (the user, 2026-11-15: teleporting a
/// creature's own, case by case), made only once one is wanted: their hops
/// (`Space::hops`, nodes' edges `FARTHEST` m apart), each node's neighbours
/// by them and the nearest a teleport between them goes (for `route`,
/// `reaches`), and the portals of them and the drops and hops together, to
/// plan by. (Made for every square, they were a third of its building, 1.4
/// s of work in the stacks, used by none but the gladiator.)
struct Teleports {
    adjacent: Vec<Vec<(u32, f32)>>,
    portals: Portals,
}

/// A district's nodes, ready to walk: the walking space, the hops between
/// nodes, and a 2D navmesh of each (landmass's, in this square's own frame,
/// from `origin`: f32 is fine there, not 8 km out).
pub(crate) struct Nodes {
    world: Arc<PlateWorld>,
    pub(crate) space: Space,
    /// Nodes whose navmesh, its triangles as kept, is in parts (a way
    /// across it not found): none, `node_reach` checks.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) cut: usize,
    /// The teleports: made the first time a walker that teleports plans (a
    /// gladiator's; see `Teleports`).
    teleports: std::sync::OnceLock<Teleports>,
    /// The drops and hops, kept for the teleports' portals.
    leaps: Vec<(u32, u32, Hop)>,
    /// Each node's drops and hops (one way): onto which nodes, and the
    /// least rise of those (below 0, a drop).
    leaps_to: Vec<Vec<(u32, f32)>>,
    /// Where drops and hops leave from and land, to plan by, for walkers
    /// that do not teleport (most).
    walks: Portals,
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
///
/// (By spade directly, in about n log n. geo's `constrained_triangulation`
/// tests every pair of edges for crossings, again after each it splits, and
/// every triangle against the whole outline: a node of 8125 fragments in the
/// stacks, an outline of 134 615 points, never finished, hours of it. The
/// triangles inside are found by crossing the outline: from outside, each
/// edge of it crossed turns in to out or back, which holds for holes by
/// construction.)
///
/// (Snapped first, as geo did, within `SNAP`: corners that near one another
/// one corner, and an edge that near a corner split there. Unsnapped, a
/// hole all but touching another left a hair of a triangle between them, of
/// no area, left out, and the triangles either side of it no longer met:
/// nodes cut in two, ways in them not found, in the tiers.)
fn triangles(poly: &Polygon<f64>) -> Option<(Vec<DVec2>, Vec<[usize; 3]>, usize)> {
    const SNAP: f64 = TRIANGLE_SNAP;
    use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation, handles::FixedVertexHandle};
    const CELL: f64 = 1.0;
    let key = |p: DVec2| ((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64);
    let mut points: Vec<DVec2> = Vec::new();
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let mut rings: Vec<Vec<usize>> = Vec::new();
    for ring in std::iter::once(poly.exterior()).chain(poly.interiors()) {
        // (Closed: its last point its first.)
        let n = ring.0.len().saturating_sub(1);
        let mut ids: Vec<usize> = Vec::new();
        for c in &ring.0[..n] {
            let p = DVec2::new(c.x, c.y);
            let (kx, kz) = key(p);
            let near = (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (kx + dx, kz + dz))).flat_map(|k| grid.get(&k).into_iter().flatten().copied()).find(|&i| points[i].distance(p) < SNAP);
            let id = near.unwrap_or_else(|| {
                points.push(p);
                grid.entry((kx, kz)).or_default().push(points.len() - 1);
                points.len() - 1
            });
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
        while ids.len() > 1 && ids.first() == ids.last() {
            ids.pop();
        }
        if ids.len() >= 3 {
            rings.push(ids);
        }
    }
    if points.len() < 3 || rings.is_empty() {
        return None;
    }
    // Each edge split at the corners within `SNAP` of it; an edge twice
    // over (where two rings meet along it) is no edge: in and out on either
    // side alike.
    let mut count: HashMap<(usize, usize), usize> = HashMap::new();
    let mut cells: Vec<(i64, i64)> = Vec::new();
    for ids in &rings {
        for k in 0..ids.len() {
            let (a, b) = (ids[k], ids[(k + 1) % ids.len()]);
            let (pa, pb) = (points[a], points[b]);
            let d = pb - pa;
            let len2 = d.length_squared();
            let mut on: Vec<(f64, usize)> = Vec::new();
            // (The cells along it, each once: a step's nine looked up again
            // at the next, most of a triangulation's work.)
            let steps = (d.length() / CELL).ceil() as usize + 1;
            cells.clear();
            for s in 0..=steps {
                let (kx, kz) = key(pa + d * (s as f64 / steps as f64));
                cells.extend((-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (kx + dx, kz + dz))));
            }
            cells.sort_unstable();
            cells.dedup();
            for i in cells.iter().flat_map(|k| grid.get(k).into_iter().flatten().copied()) {
                if i == a || i == b || len2 == 0.0 {
                    continue;
                }
                let t = (points[i] - pa).dot(d) / len2;
                if t > 0.0 && t < 1.0 && (pa + d * t).distance(points[i]) < SNAP {
                    on.push((t, i));
                }
            }
            on.sort_by(|x, y| x.0.total_cmp(&y.0));
            on.dedup_by_key(|x| x.1);
            let chain: Vec<usize> = std::iter::once(a).chain(on.into_iter().map(|x| x.1)).chain(std::iter::once(b)).collect();
            for w in chain.windows(2) {
                if w[0] != w[1] {
                    *count.entry((w[0].min(w[1]), w[0].max(w[1]))).or_default() += 1;
                }
            }
        }
    }
    let mut edges: Vec<[usize; 2]> = count.into_iter().filter(|(_, n)| n % 2 == 1).map(|((a, b), _)| [a, b]).collect();
    edges.sort();
    let vertices: Vec<Point2<f64>> = points.iter().map(|p| Point2::new(p.x, p.y)).collect();
    // (Edges crossing others, or one already there, left out by the bulk
    // load and added after, split where they cross.)
    let mut crossing = Vec::new();
    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::try_bulk_load_cdt(vertices, edges, |e| crossing.push(e)).ok()?;
    for [a, b] in crossing {
        if a != b {
            cdt.add_constraint_and_split(FixedVertexHandle::from_index(a), FixedVertexHandle::from_index(b), |p| p);
        }
    }
    // Inside or out: from the faces on the hull (out), across each edge,
    // turned by the outline's.
    let mut inside: Vec<Option<bool>> = vec![None; cdt.num_all_faces()];
    let mut queue = std::collections::VecDeque::new();
    for e in cdt.convex_hull() {
        let face = e.rev().face();
        let Some(f) = face.as_inner() else { continue };
        let i = f.fix().index();
        if inside[i].is_none() {
            inside[i] = Some(cdt.is_constraint_edge(e.as_undirected().fix()));
            queue.push_back(f.fix());
        }
    }
    while let Some(f) = queue.pop_front() {
        let here = inside[f.index()].unwrap();
        for e in cdt.face(f).adjacent_edges() {
            let Some(g) = e.rev().face().as_inner() else { continue };
            let j = g.fix().index();
            if inside[j].is_none() {
                inside[j] = Some(here ^ cdt.is_constraint_edge(e.as_undirected().fix()));
                queue.push_back(g.fix());
            }
        }
    }
    let corners: Vec<DVec2> = cdt.vertices().map(|v| DVec2::new(v.position().x, v.position().y)).collect();
    let (mut out, mut flat) = (Vec::new(), 0);
    for face in cdt.inner_faces() {
        if inside[face.fix().index()] != Some(true) {
            continue;
        }
        let [i, j, k] = face.vertices().map(|v| v.fix().index());
        let (a, b, c) = (corners[i], corners[j], corners[k]);
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

/// How near two corners of an outline are taken for one, and an edge split
/// at a corner, in `triangles` (m). (1 mm, half what an outline is made
/// simpler by: at 0.1 mm, two corners half a millimetre apart either side of
/// a neck a few centimetres wide were kept apart, and the triangles either
/// side met at a corner, not an edge, the node cut in two: four in the
/// broken.)
const TRIANGLE_SNAP: f64 = 1e-3;

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
        // (Each stage's time, for the report: what the work costs, stage by
        // stage, against what it should; see memory/essential-cost.)
        let made_pieces = nodes::pieces(&world, lo, hi);
        let mut times: Vec<(&'static str, f32)> = vec![("pieces", t.elapsed().as_secs_f32())];
        let mut space = Space::build(&world, made_pieces, Fit::WALKER);
        times.extend(space.times.iter().copied());
        let mut stage = Instant::now();
        let mut lap = |name: &'static str, times: &mut Vec<(&'static str, f32)>| {
            times.push((name, stage.elapsed().as_secs_f32()));
            stage = Instant::now();
        };
        // (Nodes too small to land on left out: ground walled in by steps,
        // got onto by no one; in the stacks a third of its nodes, slivers
        // at its pillars' feet, under 1% of its ground, each with its
        // portals to every node 20 m round. One stood on, by a shove, is off
        // the nodes: it walks as on open ground, onto them.)
        space.prune(LAND_LEAST);
        let built = t.elapsed().as_secs_f32();
        lap("prune", &mut times);
        let t_drops = Instant::now();
        let drops = drops(&space, &world);
        let mut leaps_to: Vec<Vec<(u32, f32)>> = vec![Vec::new(); space.nodes()];
        for &(a, b, h) in &drops {
            leaps_to[a as usize].push((b, h.by - h.ay));
        }
        for d in &mut leaps_to {
            d.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)));
            d.dedup_by_key(|x| x.0);
        }
        let dropped = (drops.len(), leaps_to.iter().map(|d| d.iter().filter(|l| l.1 < 0.0).count()).sum::<usize>(), t_drops.elapsed().as_secs_f32());
        lap("drops", &mut times);
        let hops_up: Vec<f32> = drops.iter().filter(|d| d.2.by > d.2.ay).map(|d| d.2.by - d.2.ay).collect();
        let high = hops_up.iter().filter(|r| **r >= 1.5).count();
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
        let made: Vec<Made> = nodes::par_chunks(&of, |c| c.iter().map(|list| outline(list).0.iter().map(triangles).collect::<Made>()).collect::<Vec<_>>()).into_iter().flatten().collect();
        lap("outlines", &mut times);
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
        lap("navmeshes", &mut times);
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
        let walks = Portals::new(&HashMap::new(), &drops, space.nodes());
        lap("portals", &mut times);
        let report = format!(
            "district nodes over {:.0} by {:.0} m round {:.0}, {:.0}: {} pieces, {} nodes ({} of {LAND_LEAST} m² or more), {} drops, between {} node pairs, and {} hops up ({} of 1.5 m or more) ({:.1} s); {} portals, {} walking links between them ({:.1} s); navmesh {} triangles ({} of no area left out, {} outlines not triangulated, {} cut in parts by those left out), {} nodes not one outline; built in {:.1} s ({built:.1} s the walking space); stages {}",
            hi.x - lo.x,
            hi.y - lo.y,
            centre.x,
            centre.y,
            space.pieces.len(),
            space.nodes(),
            space.node_area.iter().filter(|a| **a >= LAND_LEAST).count(),
            dropped.0 - hops_up.len(),
            dropped.1,
            hops_up.len(),
            high,
            dropped.2,
            walks.at.len(),
            walks.links.iter().map(|l| l.len()).sum::<usize>() / 2,
            t_portals.elapsed().as_secs_f32(),
            triangles_n,
            flat,
            untriangulated,
            cut,
            split,
            t.elapsed().as_secs_f32(),
            times.iter().map(|(n, t)| format!("{n} {:.0} ms", t * 1000.0)).collect::<Vec<_>>().join(", ")
        );
        Ok(Nodes { world, space, cut, teleports: Default::default(), leaps: drops, leaps_to, walks, archipelagos, origin, lo, hi, report, drawn })
    }

    /// The teleports (see `Teleports`): made now, the first time asked.
    fn teleports(&self) -> &Teleports {
        self.teleports.get_or_init(|| {
            let hops = self.space.hops(&self.world, FARTHEST);
            let mut adjacent: Vec<Vec<(u32, f32)>> = vec![Vec::new(); self.space.nodes()];
            for (&(a, b), list) in &hops {
                let d = list.iter().map(|h| h.d).fold(f32::INFINITY, f32::min);
                adjacent[a as usize].push((b, d));
                adjacent[b as usize].push((a, d));
            }
            Teleports { adjacent, portals: Portals::new(&hops, &self.leaps, self.space.nodes()) }
        })
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
        let steps = archipelago.find_path(&a, &b, &std::collections::HashMap::new(), PermittedAnimationLinks::All).map_err(|e| format!("no way: {e:?}"))?;
        let back = |p: Vec2| (DVec2::new(p.x as f64, p.y as f64) + self.origin).as_vec2();
        Ok(steps
            .into_iter()
            .map(|s| match s {
                PathStep::Waypoint(p) => back(p),
                PathStep::AnimationLink { end_point, .. } => back(end_point),
            })
            .collect())
    }

    /// Whether node `to` is reached from `from`: by teleports of no more
    /// than `range` (landing on nodes of `LAND_LEAST` or more, but for
    /// `to`), drops, and hops of no more than `climb`.
    pub(crate) fn reaches(&self, from: u32, to: u32, range: f32, climb: f32) -> bool {
        self.reach_from(from, range, climb, Some(to), false)[to as usize]
    }

    /// As `reaches`, in the frame: never making the teleports (see
    /// `reach_from`).
    fn reaches_now(&self, from: u32, to: u32, range: f32, climb: f32) -> bool {
        self.reach_from(from, range, climb, Some(to), true)[to as usize]
    }

    /// The nodes reached from `from` (as `reaches`), all of them; or till
    /// `stop` is.
    fn reach_from(&self, from: u32, range: f32, climb: f32, stop: Option<u32>, now: bool) -> Vec<bool> {
        let to = stop.unwrap_or(u32::MAX);
        let n = self.space.nodes();
        let mut seen = vec![false; n];
        // (In the frame, `now`: by the teleports only if made already;
        // made here, a second's work in the frame.)
        let adjacent = if range <= 0.0 { None } else if now { self.teleports.get().map(|t| &t.adjacent) } else { Some(&self.teleports().adjacent) };
        seen[from as usize] = true;
        let mut stack = vec![from];
        while let Some(n) = stack.pop() {
            if n == to {
                return seen;
            }
            let teleports = adjacent.into_iter().flat_map(|a| a[n as usize].iter()).filter(|&&(m, d)| d <= range && (m == to || self.space.node_area[m as usize] >= LAND_LEAST)).map(|&(m, _)| m);
            let leaps = self.leaps_to[n as usize].iter().filter(|&&(_, rise)| rise <= climb).map(|&(m, _)| m);
            for m in teleports.chain(leaps) {
                if !seen[m as usize] {
                    seen[m as usize] = true;
                    stack.push(m);
                }
            }
        }
        seen
    }

    /// For a walker on node `me` (at `mine`) with no way to `you`, going as
    /// it goes: where it keeps watch, the ground it reaches nearest you
    /// (its node, the point); and where it goes if it leaves, ground it
    /// reaches about `AWAY` m from you, further than it is (see
    /// `Stranded`).
    pub(crate) fn stranded(&self, me: u32, mine: Vec2, you: Vec2, range: f32, climb: f32) -> Option<Spots> {
        let reach = self.reach_from(me, range, climb, None, false);
        let ok = |n: u32| reach[n as usize] && self.space.node_area[n as usize] >= LAND_LEAST;
        let y = DVec2::new(you.x as f64, you.y as f64);
        let pt = Point::new(y.x, y.y);
        let mut watch: Option<(f64, u32, DVec2)> = None;
        for r in [24.0, 48.0, 96.0, 192.0] {
            for i in self.space.pieces_near(y, r) {
                for (k, f) in self.space.space[i].iter().enumerate() {
                    let n = self.space.node_of[i][k];
                    if !ok(n) {
                        continue;
                    }
                    let c = match f.closest_point(&pt) {
                        geo::Closest::Intersection(c) | geo::Closest::SinglePoint(c) => DVec2::new(c.x(), c.y()),
                        geo::Closest::Indeterminate => continue,
                    };
                    let d = c.distance(y);
                    if d <= r && watch.is_none_or(|w| d < w.0) {
                        watch = Some((d, n, c));
                    }
                }
            }
            if watch.is_some() {
                break;
            }
        }
        let (_, n, c) = watch?;
        let here = mine.distance(you) as f64;
        let mut away: Option<(f64, u32, DVec2)> = None;
        for i in self.space.pieces_near(y, AWAY + 40.0) {
            for (k, f) in self.space.space[i].iter().enumerate() {
                let m = self.space.node_of[i][k];
                if !ok(m) {
                    continue;
                }
                use geo::InteriorPoint;
                let Some(q) = f.interior_point() else { continue };
                let q = DVec2::new(q.x(), q.y());
                let d = q.distance(y);
                if d < AWAY * 0.7 || d <= here + 10.0 {
                    continue;
                }
                let score = (d - AWAY).abs();
                if away.is_none_or(|a| score < a.0) {
                    away = Some((score, m, q));
                }
            }
        }
        Some(Spots { watch: (n, c.as_vec2()), away: away.map(|(_, m, q)| (m, q.as_vec2())) })
    }

    /// The fewest teleports of no more than `range` from node `from` to
    /// `to`: the nodes on the way, `from` first, `to` last (landing only on
    /// nodes of `LAND_LEAST` or more, but for `to`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn route(&self, from: u32, to: u32, range: f32) -> Option<Vec<u32>> {
        let adjacent = &self.teleports().adjacent;
        let mut came = vec![u32::MAX; adjacent.len()];
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
            for &(m, d) in &adjacent[n as usize] {
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
    pub(crate) fn plan(&self, a: u32, from: Vec2, b: u32, to: Vec2, range: f32, cost: f32, climb: f32) -> Option<(Hop, u32, f32, Aim)> {
        // (None there, known by its nodes first: the portals' search, finding
        // none, went through them all, up to 41 ms in tiers.)
        if !self.reaches(a, b, range, climb) {
            return None;
        }
        let portals = if range > 0.0 { &self.teleports().portals } else { &self.walks };
        let (hop, onto, goal, onward) = portals.plan(&self.space.node_area, a, DVec2::new(from.x as f64, from.y as f64), b, DVec2::new(to.x as f64, to.y as f64), range, cost, climb)?;
        let aim = Aim { onward, short: if onto == b { LAND_SHORT } else { 0.0 } };
        // (A drop lands where it was found clear, not moved.)
        let hop = if hop.leap.is_some() { hop } else { self.land(hop.a, hop.ay, onto, aim, range, aim.left(hop.b)).map_or(hop, |(b, by, d)| Hop { b, by, d, ..hop }) };
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
        Some(Hop { a: here, ay: at.y, b, by, d, leap: None })
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
    /// A drop's: from its lip, its leap; onto where it lands, none back
    /// (`landing`).
    leap: Vec<Option<f32>>,
    landing: Vec<bool>,
    /// Each portal's walking links: to which, how far.
    pub(crate) links: Vec<Vec<(u32, f32)>>,
    /// Each node's portals, by cell (`CELL`), and all of them.
    cells: HashMap<(u32, i32, i32), Vec<u32>>,
    by_node: Vec<Vec<u32>>,
}

/// The cells portals are found by (m); how many a portal is joined to by
/// walking, the nearest of its node; how far a walker's start is joined to
/// them (m): all those within it, or that many nearest.
const CELL: f64 = 16.0;
const NEAREST: usize = 6;
const START: f64 = 48.0;

impl Portals {
    fn new(hops: &HashMap<(u32, u32), Vec<Hop>>, drops: &[(u32, u32, Hop)], nodes: usize) -> Portals {
        let mut p = Portals { node: Vec::new(), at: Vec::new(), y: Vec::new(), twin: Vec::new(), d: Vec::new(), leap: Vec::new(), landing: Vec::new(), links: Vec::new(), cells: HashMap::new(), by_node: vec![Vec::new(); nodes] };
        let pairs = hops.iter().flat_map(|(&(lo, hi), list)| list.iter().map(move |h| (lo, hi, h))).chain(drops.iter().map(|(a, b, h)| (*a, *b, h)));
        for (lo, hi, h) in pairs {
            let i = p.node.len() as u32;
            p.node.extend([lo, hi]);
            p.at.extend([h.a, h.b]);
            p.y.extend([h.ay, h.by]);
            p.twin.extend([i + 1, i]);
            p.d.extend([h.d, h.d]);
            p.leap.extend([h.leap, None]);
            p.landing.extend([false, h.leap.is_some()]);
        }
        for (i, q) in p.at.iter().enumerate() {
            p.by_node[p.node[i] as usize].push(i as u32);
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
        // parts, the parts joined by the shortest links between them, as a
        // tree (Kruskal's way), from the edges of the portals' Delaunay
        // triangulation, which hold every such link. The nearest pair
        // between the part so far and all the rest, every pair weighed, part
        // after part, cost the stacks 0.7 s, its portals no more than the
        // tiers'.)
        use spade::{DelaunayTriangulation, Point2, Triangulation};
        fn root(parent: &mut [u32], mut i: u32) -> u32 {
            while parent[i as usize] != i {
                let up = parent[i as usize];
                parent[i as usize] = parent[up as usize];
                i = up;
            }
            i
        }
        let mut parent: Vec<u32> = (0..p.at.len() as u32).collect();
        for list in &p.by_node {
            if list.len() < 2 {
                continue;
            }
            for &i in list {
                for &(j, _) in &links[i as usize] {
                    let (x, y) = (root(&mut parent, i), root(&mut parent, j));
                    parent[x as usize] = y;
                }
            }
            let r0 = root(&mut parent, list[0]);
            if list.iter().all(|&i| root(&mut parent, i) == r0) {
                continue;
            }
            // (Portals at one spot are one vertex: joined where they are.)
            let mut tri: DelaunayTriangulation<Point2<f64>> = DelaunayTriangulation::new();
            let mut first: Vec<u32> = Vec::new();
            let mut edges: Vec<(f64, u32, u32)> = Vec::new();
            for &i in list {
                let q = p.at[i as usize];
                let v = tri.insert(Point2::new(q.x, q.y)).expect("portal not finite").index();
                if v < first.len() {
                    edges.push((0.0, first[v], i));
                } else {
                    first.push(i);
                }
            }
            for e in tri.undirected_edges() {
                let [u, v] = e.vertices();
                let (i, j) = (first[u.index()], first[v.index()]);
                edges.push((p.at[i as usize].distance(p.at[j as usize]), i, j));
            }
            edges.sort_by(|x, y| x.0.total_cmp(&y.0));
            for (d, i, j) in edges {
                let (x, y) = (root(&mut parent, i), root(&mut parent, j));
                if x != y {
                    parent[x as usize] = y;
                    links[i as usize].push((j, d as f32));
                    links[j as usize].push((i, d as f32));
                }
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
        let all = &self.by_node[n as usize];
        let mut found: Vec<(u32, f64)> = Vec::new();
        // (A node of few portals: all of them weighed. Fewer than `k`, the
        // rings went on to the last, 16 000 cells, a portal: the stacks'
        // pillar tops, its portals half a second.)
        if all.len() <= 4 * k.max(8) {
            found.extend(all.iter().map(|&i| (i, self.at[i as usize].distance(q))));
            found.sort_by(|a, b| a.1.total_cmp(&b.1));
            let kth = found.get(k.saturating_sub(1)).map_or(f64::INFINITY, |f| f.1);
            found.retain(|f| f.1 <= kth.max(within));
            return found;
        }
        let (cx, cz) = ((q.x / CELL).floor() as i32, (q.y / CELL).floor() as i32);
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
            if (found.len() >= k && found[k - 1].1 <= reach && within <= reach) || found.len() == all.len() {
                break;
            }
        }
        let kth = found.get(k.saturating_sub(1)).map_or(f64::INFINITY, |f| f.1);
        found.retain(|f| f.1 <= kth.max(within));
        found
    }

    #[allow(clippy::too_many_arguments)]
    fn plan(&self, area: &[f64], a: u32, from: DVec2, b: u32, to: DVec2, range: f32, cost: f32, hop: f32) -> Option<(Hop, u32, f32, DVec2)> {
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
            if !self.landing[i as usize] && (m == b || area[m as usize] >= LAND_LEAST) {
                if self.leap[i as usize].is_some() {
                    if self.y[t as usize] - self.y[i as usize] <= hop {
                        next.push((t, gi + DROP_COST + self.at[i as usize].distance(self.at[t as usize]) as f32));
                    }
                } else if self.d[i as usize] <= range {
                    next.push((t, gi + cost));
                }
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
        Some((Hop { a: self.at[u], ay: self.y[u], b: self.at[v], by: self.y[v], d: self.d[u], leap: self.leap[u] }, self.node[v], goal, onward))
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
    // (The old ones let go on another core: in the stacks they are some
    // hundreds of thousands of portals, and freeing them held up a frame.)
    if let Some(old) = districts.nodes.replace(Arc::new(nodes)) {
        AsyncComputeTaskPool::get().spawn(async move { drop(old) }).detach();
    }
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

/// A plan asked for: from node `me` (at `mine`) to node `there` (at
/// `spot`), going as it goes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ask {
    me: u32,
    mine: Vec2,
    there: u32,
    spot: Vec2,
    range: f32,
    cost: f32,
    hop: f32,
}

/// A way found across a node: the node, from and to, its corners.
type WayFound = (u32, Vec2, Vec2, Option<Vec<Vec2>>);

/// A teleport chosen, told by its nodes and where it leaves from.
type SoonerKey = (u32, u32, DVec2);

/// Going from where it is asked for: the teleport chosen (its key, its
/// hop), the node it lands on, its aim, where it is, its range.
#[derive(Clone, Copy, Debug)]
struct SoonerAsk {
    key: SoonerKey,
    planned: Hop,
    onto: u32,
    aim: Aim,
    at: Vec3,
    range: f32,
}

/// A walker with no way to you (the user, 2026-11-15: never just stand
/// there): it keeps watch, wandering near the ground it reaches nearest
/// you, looking now and then whether it has a way now; hurt meanwhile, it
/// leaves, off to ground far from you, and is gone (see `gone`).
#[derive(Clone, Copy, Debug, Default)]
enum Stranded {
    #[default]
    Chasing,
    /// Its spots being found (and, hurt meanwhile, to leave once they are).
    Asking(bool),
    Watch(Watch),
    Leave((u32, Vec2)),
}

/// Keeping watch: where you were when it was found, where it keeps watch
/// and where it would leave to (see `Nodes::stranded`); when it looks
/// again; the spot it wanders to, and how long yet it keeps to it.
#[derive(Clone, Copy, Debug)]
struct Watch {
    you: Vec2,
    spots: Spots,
    retry: f32,
    wander: Vec2,
    linger: f32,
}

/// See `Nodes::stranded`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spots {
    watch: (u32, Vec2),
    away: Option<(u32, Vec2)>,
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
    /// Drops: leaps (the rig's), from where it stands, with this velocity,
    /// across and up, to come down where its drop lands (`drops`).
    Leap(Vec3),
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
/// How long a walker with no way to you waits before looking again (s).
const NO_WAY: f32 = 1.0;
/// A walker stranded (see `Stranded`): how often it looks whether it has a
/// way to you now (s), how far you go before its spots are found anew (m),
/// how far it wanders from where it keeps watch (m) and how long it keeps
/// to each spot (s), how far off it goes, leaving (m).
const RETRY: f32 = 2.0;
const REFRESH: f32 = 10.0;
const WANDER: f32 = 6.0;
const LINGER: (f32, f32) = (3.0, 6.0);
const AWAY: f64 = 110.0;
/// How far on along its way a walker heads for (m).
const LOOK: f32 = 0.6;
/// How near where a teleport (or a drop) leaves from it must be to go (m).
const AT_HOP: f32 = 0.6;
/// How often a walker on its way to where a teleport leaves from looks
/// whether it may go from where it is (s).
const SOONER: f32 = 0.25;

/// A walker on the nodes: walking, dropping, hopping (if given) and
/// teleporting (if it may: range 0, never): its teleports' range (3D, m),
/// its tell before one (s), its rest after one (s); what it is doing,
/// its way and plan (kept a while, not found anew every frame: a choice
/// flipping between two every frame shakes a body, see
/// memory/vibration-causes).
#[derive(Component)]
pub(crate) struct Blink {
    pub(crate) range: f32,
    pub(crate) tell: f32,
    pub(crate) rest: f32,
    /// How fast it glides along a teleport's line (m/s).
    pub(crate) speed: f32,
    /// Whether its body has a shade to split off (see `clip/shade.rs`).
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
    /// A teleport made this frame (from, to).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) made: Option<(Vec3, Vec3)>,
    pub(crate) teleports: u32,
    pub(crate) drops: u32,
    /// No way found from its node to yours (which), looked for again in
    /// how long (s).
    no_way: Option<(u32, u32, f32)>,
    /// A plan asked for (from its node, where, to yours, where), being made
    /// in the background, and made (see `plan_in_background`).
    ask: Option<Ask>,
    task: Option<Task<(Ask, Option<(Hop, u32, f32, Aim)>)>>,
    answer: Option<(Ask, Option<(Hop, u32, f32, Aim)>)>,
    /// Its way across its node, the same (`follow`): asked for (its node,
    /// from, to), being found, found; and the goal none was found to.
    way_ask: Option<(u32, Vec2, Vec2)>,
    way_task: Option<Task<WayFound>>,
    way_answer: Option<WayFound>,
    way_failed: Option<(u32, Vec2)>,
    /// Going from where it is (`Nodes::sooner`), the same: asked for (the
    /// teleport chosen, by its nodes and where it leaves), found.
    sooner_ask: Option<SoonerAsk>,
    sooner_task: Option<Task<(SoonerKey, Option<Hop>)>>,
    sooner_answer: Option<(SoonerKey, Option<Hop>)>,
    /// With no way to you (see `Stranded`): what it does; its spots asked
    /// for (its node, where, you), being found; the health it last had (a
    /// blow, felt); whether it has left.
    stranded: Stranded,
    strand_ask: Option<(u32, Vec2, Vec2)>,
    strand_task: Option<Task<(Vec2, Option<Spots>)>>,
    felt: f32,
    gone: bool,
    /// How high it hops up onto a ledge (m; 0, it doesn't): see `drops`.
    pub(crate) hop: f32,
}

impl Blink {
    pub(crate) fn new(range: f32, tell: f32, rest: f32, cost: f32, speed: f32) -> Blink {
        Blink { range: range.min(FARTHEST as f32), tell, rest, speed: speed.max(1.0), shaded: false, cost, phase: Phase::Walk, resting: 0.0, node: None, way: Vec::new(), way_at: 0, way_to: Vec2::splat(f32::MAX), chosen: None, sooner: 0.0, made: None, teleports: 0, drops: 0, no_way: None, ask: None, task: None, answer: None, way_ask: None, way_task: None, way_answer: None, way_failed: None, sooner_ask: None, sooner_task: None, sooner_answer: None, stranded: Stranded::Chasing, strand_ask: None, strand_task: None, felt: f32::MAX, gone: false, hop: 0.0 }
    }

    /// Hopping up onto ledges this high (m; see `drops`).
    pub(crate) fn with_hop(self, hop: f32) -> Blink {
        Blink { hop, ..self }
    }

    /// Its health now: a blow felt, with no way to you, it leaves (see
    /// `Stranded`).
    pub(crate) fn felt(&mut self, health: f32) {
        if health < self.felt {
            self.stranded = match self.stranded {
                Stranded::Asking(_) => Stranded::Asking(true),
                Stranded::Watch(w) => w.spots.away.map_or(Stranded::Watch(w), Stranded::Leave),
                s => s,
            };
        }
        self.felt = health;
    }

    /// Whether it has no way to you (see `Stranded`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn stranded(&self) -> bool {
        !matches!(self.stranded, Stranded::Chasing)
    }

    /// Whether it has left (see `Stranded`): gone where it went.
    pub(crate) fn gone(&self) -> bool {
        self.gone
    }

    /// Where it goes, stranded (see `Stranded`): keeping watch, a spot near
    /// where it does, now and then another; leaving, where it leaves to;
    /// else where it was going (`there`, `spot`). Keeping watch, it looks
    /// every `RETRY` s whether it has a way to you now (by the nodes: a
    /// fraction of a millisecond), and has its spots found anew if you
    /// have gone `REFRESH` m.
    fn stranded_goal(&mut self, nodes: &Nodes, me: u32, mine: Vec2, at: Vec3, there: u32, spot: Vec2, dt: f32) -> (u32, Vec2) {
        let (range, hop) = (self.range, self.hop);
        match &mut self.stranded {
            Stranded::Watch(w) => {
                w.retry -= dt;
                if w.retry <= 0.0 {
                    w.retry = RETRY;
                    if nodes.reaches_now(me, there, range, hop) {
                        self.stranded = Stranded::Chasing;
                        self.no_way = None;
                        return (there, spot);
                    }
                    if w.you.distance(spot) > REFRESH && self.strand_task.is_none() {
                        self.strand_ask = Some((me, mine, spot));
                    }
                }
                w.linger -= dt;
                if w.linger <= 0.0 {
                    let r = |k: i32| worldgen::noise::hash01((at.x * 7.0) as i32, (at.z * 7.0) as i32, k, 0x7a3);
                    w.linger = LINGER.0 + (LINGER.1 - LINGER.0) * r(0);
                    let a = r(1) * std::f32::consts::TAU;
                    let q = w.spots.watch.1 + Vec2::new(a.cos(), a.sin()) * WANDER * r(2).sqrt();
                    let y = nodes.world.walk_height_at(q.x, q.y);
                    w.wander = match nodes.locate(Vec3::new(q.x, y, q.y), 0.0, Some(w.spots.watch.0)) {
                        Some((n, q)) if n == w.spots.watch.0 => q,
                        _ => w.spots.watch.1,
                    };
                }
                (w.spots.watch.0, w.wander)
            }
            Stranded::Leave(away) => *away,
            _ => (there, spot),
        }
    }

    /// Its plan asked for (`steer`), made on another core; one made,
    /// taken, for `steer` to use. Once a frame, after `steer`.
    pub(crate) fn plan_in_background(&mut self, nodes: &Arc<Nodes>) {
        if let Some(task) = self.task.as_mut()
            && let Some(made) = block_on(poll_once(task))
        {
            self.task = None;
            self.answer = Some(made);
        }
        if self.task.is_none()
            && let Some(a) = self.ask.take()
        {
            let nodes = nodes.clone();
            self.task = Some(AsyncComputeTaskPool::get().spawn(async move { (a, nodes.plan(a.me, a.mine, a.there, a.spot, a.range, a.cost, a.hop)) }));
        }
        if let Some(task) = self.way_task.as_mut()
            && let Some(found) = block_on(poll_once(task))
        {
            self.way_task = None;
            self.way_answer = Some(found);
        }
        if self.way_task.is_none()
            && let Some((n, from, to)) = self.way_ask.take()
        {
            let nodes = nodes.clone();
            self.way_task = Some(AsyncComputeTaskPool::get().spawn(async move { (n, from, to, nodes.way(n, from, to)) }));
        }
        if let Some(task) = self.sooner_task.as_mut()
            && let Some(found) = block_on(poll_once(task))
        {
            self.sooner_task = None;
            self.sooner_answer = Some(found);
        }
        if self.sooner_task.is_none()
            && let Some(a) = self.sooner_ask.take()
        {
            let nodes = nodes.clone();
            self.sooner_task = Some(AsyncComputeTaskPool::get().spawn(async move { (a.key, nodes.sooner(a.planned, a.onto, a.aim, a.at, a.range)) }));
        }
        if let Some(task) = self.strand_task.as_mut()
            && let Some((you, spots)) = block_on(poll_once(task))
        {
            self.strand_task = None;
            self.found_spots(you, spots);
        }
        if self.strand_task.is_none()
            && let Some((me, mine, you)) = self.strand_ask.take()
        {
            let (nodes, range, hop) = (nodes.clone(), self.range, self.hop);
            self.strand_task = Some(AsyncComputeTaskPool::get().spawn(async move { (you, nodes.stranded(me, mine, you, range, hop)) }));
        }
    }

    /// Its spots, found (see `Stranded`): it keeps watch (or leaves, if hurt
    /// meanwhile); none, it stands, as it did.
    fn found_spots(&mut self, you: Vec2, spots: Option<Spots>) {
        let Some(spots) = spots else {
            self.stranded = Stranded::Chasing;
            return;
        };
        self.stranded = match self.stranded {
            Stranded::Asking(true) => spots.away.map_or(Stranded::Watch(Watch { you, spots, retry: RETRY, wander: spots.watch.1, linger: 0.0 }), Stranded::Leave),
            Stranded::Watch(w) => Stranded::Watch(Watch { you, spots, ..w }),
            Stranded::Leave(a) => Stranded::Leave(a),
            _ => Stranded::Watch(Watch { you, spots, retry: RETRY, wander: spots.watch.1, linger: 0.0 }),
        };
    }

    /// As `plan_in_background`, at once (for the tests, frame by frame).
    #[cfg(test)]
    fn plan_now(&mut self, nodes: &Nodes) {
        if let Some(a) = self.ask.take() {
            self.answer = Some((a, nodes.plan(a.me, a.mine, a.there, a.spot, a.range, a.cost, a.hop)));
        }
        if let Some((n, from, to)) = self.way_ask.take() {
            self.way_answer = Some((n, from, to, nodes.way(n, from, to)));
        }
        if let Some(a) = self.sooner_ask.take() {
            self.sooner_answer = Some((a.key, nodes.sooner(a.planned, a.onto, a.aim, a.at, a.range)));
        }
        if let Some((me, mine, you)) = self.strand_ask.take() {
            let spots = nodes.stranded(me, mine, you, self.range, self.hop);
            self.found_spots(you, spots);
        }
    }

    /// Its node, way and teleport forgotten (the nodes built anew), as it
    /// was made but for its settings.
    fn forget(&mut self) {
        *self = Blink { shaded: self.shaded, teleports: self.teleports, drops: self.drops, hop: self.hop, ..Blink::new(self.range, self.tell, self.rest, self.cost, self.speed) };
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
    fn follow(&mut self, node: u32, at: Vec3, from: Vec2, to: Vec2) -> Steer {
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
        // (Found in the background, not the frame: up to 2.6 ms in stacks,
        // a walker. Meanwhile on along the way it has, or standing if it
        // has none. A goal none was found to not asked again till it moves.)
        if self.way_to.distance(to) > MOVED || self.way.len() < 2 || off {
            match self.way_answer.take().filter(|w| w.0 == node && w.2.distance(to) <= MOVED) {
                Some((_, _, _, found)) => {
                    self.way_failed = found.is_none().then_some((node, to));
                    self.way = found.map(|w| std::iter::once(from).chain(w).collect()).unwrap_or_default();
                    self.way_at = 0;
                    self.way_to = to;
                }
                None => {
                    if self.way_task.is_none() && self.way_failed.is_none_or(|f| f.0 != node || f.1.distance(to) > MOVED) {
                        self.way_ask = Some((node, from, to));
                    }
                    if self.way.len() < 2 {
                        return Steer::Stand;
                    }
                }
            }
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
        if self.gone {
            return Steer::Stand;
        }
        let (there, spot) = self.stranded_goal(nodes, me, mine, at, there, spot, dt);
        if there == me {
            self.chosen = None;
            let s = self.follow(me, at, mine, spot);
            // (Leaving, there: gone.)
            if s == Steer::There && matches!(self.stranded, Stranded::Leave(_)) {
                self.gone = true;
            }
            return s;
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
                // (No way there: not looked for again for `NO_WAY` s, unless
                // it or you are on another node. Looked for every frame,
                // with no teleports, a walker cost milliseconds a frame.)
                if self.no_way.is_some_and(|(a, b, left)| a == me && b == there && left > 0.0) {
                    if let Some(w) = self.no_way.as_mut() {
                        w.2 -= dt;
                    }
                    return Steer::Stand;
                }
                // (Its plan made in the background, not in the frame: one
                // could take 14 ms in stacks, and every walker after you
                // made one as you stepped onto another node, the user,
                // 2026-11-15: a game breaker. It stands meanwhile, a frame
                // or two; the plan used if it is still to the point.)
                let fits = |a: &Ask| a.me == me && a.there == there && a.spot.distance(spot) < REPLAN;
                let Some((_, found)) = self.answer.take().filter(|(a, _)| fits(a)) else {
                    if self.task.is_none() {
                        self.ask = Some(Ask { me, mine, there, spot, range: self.range, cost: self.cost, hop: self.hop });
                    }
                    return Steer::Stand;
                };
                let Some((hop, onto, _, aim)) = found else {
                    self.no_way = Some((me, there, NO_WAY));
                    // (No way to you: its spots found, to keep watch.)
                    if matches!(self.stranded, Stranded::Chasing) && self.strand_task.is_none() {
                        self.stranded = Stranded::Asking(false);
                        self.strand_ask = Some((me, mine, spot));
                    }
                    return Steer::Stand;
                };
                self.no_way = None;
                self.chosen = Some((me, onto, hop, aim, there, spot));
                hop
            }
        };
        let mut leave = Vec2::new(hop.a.x as f32, hop.a.y as f32);
        // A drop: at its lip, at once (no tell, no rest: a fall is no
        // teleport), from where it stands, to land where it was found clear.
        if let Some(up) = hop.leap {
            let off = leave.distance(at.xz_());
            if off < AT_HOP {
                // (From where it stands, up to `AT_HOP` short of its lip:
                // the hop found clear from its lip came down on it there, in
                // play, so the least that comes down where it was to, found
                // at a long frame only; else as planned. The rig, turning as
                // it goes, came no nearer. At both frames, a frame took
                // 17 ms; only till a metre below its lip, a step lower down
                // caught it.)
                let (from, q, onto) = (DVec2::new(at.x as f64, at.z as f64), hop.b, self.chosen.map_or(me, |c| c.1));
                let ups = std::iter::once(up).chain((1..=6).map(|k| up + kick(1.5) as f32 * k as f32));
                let up = ups.into_iter().find(|&u| fall_clear(&nodes.space, &nodes.world, from, at.y as f64, q, hop.by as f64, u as f64, onto, &FRAMES[..1], false)).unwrap_or(up);
                let v = (q.as_vec2() - at.xz_()) / fall_time(up as f64, (at.y - hop.by) as f64) as f32;
                self.node = Some(onto);
                self.chosen = None;
                self.way.clear();
                self.drops += 1;
                return Steer::Leap(Vec3::new(v.x, up, v.y));
            }
            return match self.follow(me, at, mine, leave) {
                Steer::There => Steer::Stand,
                s => s,
            };
        }
        // (Or from here, if it does as well from here: see `sooner`.)
        self.sooner -= dt;
        let mut hop = hop;
        // (Found in the background, as its way: up to 2.8 ms a walker in
        // stacks. Its answer used if it is for the teleport still chosen,
        // and from where it nearly is.)
        if self.resting <= 0.0 && self.sooner <= 0.0 && leave.distance(at.xz_()) >= AT_HOP
            && let Some(c) = self.chosen
        {
            self.sooner = SOONER;
            if self.sooner_task.is_none() {
                self.sooner_ask = Some(SoonerAsk { key: (c.0, c.1, hop.a), planned: hop, onto: c.1, aim: c.3, at, range: self.range });
            }
        }
        if let Some((key, found)) = self.sooner_answer.take()
            && let Some(c) = self.chosen
            && key == (c.0, c.1, c.2.a)
            && let Some(now) = found
            && now.a.as_vec2().distance(at.xz_()) < AT_HOP
        {
            hop = now;
            leave = now.a.as_vec2();
        }
        if leave.distance(at.xz_()) < AT_HOP && self.resting <= 0.0 {
            self.phase = Phase::Tell(hop, self.tell, self.chosen.map_or(me, |c| c.1));
            return Steer::Stand;
        }
        match self.follow(me, at, mine, leave) {
            Steer::There => Steer::Stand,
            s => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Area, Contains, LineString};

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
        // (And where you may stand to be chased: anywhere on the walking
        // space, small nodes too, as you can stand there; chased from
        // spots on nodes it lands on. Small ones, under `LAND_LEAST`, told
        // apart: drawn from big ones only, the test once missed them.)
        let mut anywhere: Vec<Vec2> = Vec::new();
        while spots.len() < 2000 || anywhere.len() < 2000 {
            let q = DVec2::new(lo.x + (hi.x - lo.x) * rng.next() as f64, lo.y + (hi.y - lo.y) * rng.next() as f64);
            if let Some((n, ..)) = nodes.space.node_at(q) {
                if nodes.space.node_area[n as usize] >= LAND_LEAST && spots.len() < 2000 {
                    spots.push(q.as_vec2());
                }
                if anywhere.len() < 2000 {
                    anywhere.push(q.as_vec2());
                }
            }
        }
        let small = |q: Vec2| nodes.space.node_at(DVec2::new(q.x as f64, q.y as f64)).is_some_and(|(n, ..)| nodes.space.node_area[n as usize] < LAND_LEAST);
        println!("where you stand: {} of {} spots on small nodes (under {LAND_LEAST} m²)", anywhere.iter().filter(|q| small(**q)).count(), anywhere.len());
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
        // (BLINK_RANGE=0: no teleports, as most creatures; BLINK_HOP=2.45:
        // hopping up ledges too, see `drops`.)
        let range: f32 = std::env::var("BLINK_RANGE").ok().and_then(|v| v.parse().ok()).unwrap_or(10.0);
        let hop: f32 = std::env::var("BLINK_HOP").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let (dt, speed) = (1.0 / 30.0, 5.0);
        let (mut reached, mut chases, mut strayed, mut walked, mut worst_stray) = (0, 0, 0.0f32, 0.0f32, (0.0f64, Vec2::ZERO));
        // (Chases to small nodes: how many, how many reached.)
        let (mut to_small, mut reached_small) = (0, 0);
        let (mut teleports, mut fewest, mut extra) = (0u32, 0usize, Vec::new());
        // (Not reached, how many kept watch, and how far from the goal.)
        let (mut watched, mut kept_watch) = (0usize, Vec::<f32>::new());
        // (Drops made, and the deepest.)
        let (mut drops, mut deepest, mut hops, mut highest) = (0u32, 0.0f32, 0u32, 0.0f32);
        let mut cost: Vec<f32> = Vec::new();
        // (Each teleport's length, and whether it landed inset, not on the
        // edge: see `Nodes::land`.)
        let (mut lengths, mut inset) = (Vec::<f32>::new(), 0usize);
        let mut failed: Vec<String> = Vec::new();
        while chases < std::env::var("BLINK_CHASES").ok().and_then(|v| v.parse().ok()).unwrap_or(300) {
            let (a, b) = (spots[(rng.next() * spots.len() as f32) as usize % spots.len()], anywhere[(rng.next() * anywhere.len() as f32) as usize % anywhere.len()]);
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
            to_small += small(b) as usize;
            let goal = Vec3::new(b.x, on(b).unwrap().0 + crate::player::EYE, b.y);
            let (na, nb) = (nodes.locate(Vec3::new(a.x, on(a).unwrap().0, a.y), 0.0, None).unwrap().0, nodes.locate(goal, crate::player::EYE, None).unwrap().0);
            let least = nodes.route(na, nb, range).map(|r| r.len() - 1);
            let mut blink = Blink::new(range, 0.6, 1.5, 8.0, 20.0).with_hop(hop);
            let mut at = Vec3::new(a.x, on(a).unwrap().0, a.y);
            let mut done = false;
            let mut why = String::new();
            let mut trace: Vec<String> = Vec::new();
            let mut last: Option<(Option<u32>, String)> = None;
            for frame in 0..(120.0 / dt) as usize {
                let t = Instant::now();
                let steer = blink.steer(&nodes, at, goal, crate::player::EYE, dt);
                cost.push(t.elapsed().as_secs_f32() * 1e6);
                blink.plan_now(&nodes);
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
                    // (Stranded, there is where it keeps watch, not the goal.)
                    Steer::There if blink.stranded() => {}
                    Steer::There => {
                        done = true;
                        break;
                    }
                    Steer::Leap(v) => {
                        // (As the rig leaps: by its gravity, stepped as it
                        // is, down onto whatever ground is under it.)
                        let onto = blink.node;
                        let (mut p, mut y, mut vy) = (Vec2::new(at.x, at.z), at.y, v.y);
                        let landed = loop {
                            vy -= crate::rig::leap_gravity() * dt;
                            p += Vec2::new(v.x, v.z) * dt;
                            y += vy * dt;
                            let q = DVec2::new(p.x as f64, p.y as f64);
                            let floor = world.walk_height_at(q.x as f32, q.y as f32);
                            if y <= floor && vy < 0.0 {
                                break Some(Vec3::new(p.x, floor, p.y));
                            }
                            if y < at.y - 1000.0 {
                                break None;
                            }
                        };
                        let Some(to) = landed else {
                            why = format!("fell through from {at:.1}");
                            break;
                        };
                        let landed_on = nodes.locate(to, 0.0, onto).map(|n| n.0);
                        if landed_on != onto || on(Vec2::new(to.x, to.z)).is_none_or(|(_, off)| off > 0.05) {
                            why = format!("dropped from {at:.1} onto {to:.1}, node {landed_on:?}, not {onto:?}");
                            break;
                        }
                        if to.y < at.y {
                            drops += 1;
                            deepest = deepest.max(at.y - to.y);
                        } else {
                            hops += 1;
                            highest = highest.max(to.y - at.y);
                        }
                        at = to;
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
            if !done && blink.stranded() {
                watched += matches!(blink.stranded, Stranded::Watch(_)) as usize;
                kept_watch.push(at.distance(Vec3::new(b.x, at.y, b.y)));
            }
            if done {
                reached += 1;
                reached_small += small(b) as usize;
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
        println!("drops: {drops}, the deepest {deepest:.1} m; hops: {hops}, the highest {highest:.1} m");
        lengths.sort_by(f32::total_cmp);
        lengths.push(f32::NAN); // (none in a district without teleports: told as NaN, not a panic)
        let lengths = &lengths[..(lengths.len() - 1).max(1)];
        println!("teleports: length median {:.1} m, tenth {:.1} m, ninetieth {:.1} m; {inset} of {} landed {INSET} m or more inside the walking space", lengths[lengths.len() / 2], lengths[lengths.len() / 10], lengths[lengths.len() * 9 / 10], lengths.len());
        println!("chases to small nodes: {reached_small} of {to_small} reached");
        kept_watch.sort_by(f32::total_cmp);
        println!("not reached: {} stranded, {watched} keeping watch, from the goal median {:.0} m, most {:.0} m", kept_watch.len(), kept_watch.get(kept_watch.len() / 2).copied().unwrap_or(f32::NAN), kept_watch.last().copied().unwrap_or(f32::NAN));
        for f in &failed {
            println!("  not reached: {f}");
        }
        // Planning alone, to see what it costs: from spot to spot, 20 to
        // 150 m, on two nodes.
        let mut plans: Vec<f32> = Vec::new();
        let mut unplanned: Vec<String> = Vec::new();
        // (A few tries for each: on ground nearly all one node, floor's once
        // its pit had a stair, pairs on two nodes were near none, and this
        // went on for ever.)
        for _ in 0..200_000 {
            if plans.len() == 1000 {
                break;
            }
            let (a, b) = (spots[(rng.next() * spots.len() as f32) as usize % spots.len()], anywhere[(rng.next() * anywhere.len() as f32) as usize % anywhere.len()]);
            let node = |q: Vec2| nodes.space.node_at(DVec2::new(q.x as f64, q.y as f64)).map(|n| n.0).unwrap();
            if !(20.0..=150.0).contains(&a.distance(b)) || node(a) == node(b) {
                continue;
            }
            let t = Instant::now();
            let found = nodes.plan(node(a), a, node(b), b, range, 8.0, hop);
            plans.push(t.elapsed().as_secs_f32() * 1e6);
            if found.is_none() {
                unplanned.push(format!("{a:.1} to {b:.1}{}", if small(b) { " (small)" } else { "" }));
            }
        }
        plans.sort_by(f32::total_cmp);
        let made = plans.len();
        plans.push(f32::NAN); // (none on ground all one node: told as NaN)
        let plans = &plans[..(plans.len() - 1).max(1)];
        println!("plans: median {:.0} µs, 90th {:.0} µs, 99th {:.0} µs, most {:.0} µs; {} of {} found none: {:?}", plans[plans.len() / 2], plans[plans.len() * 9 / 10], plans[plans.len() * 99 / 100], plans.last().unwrap(), unplanned.len(), made, unplanned.iter().take(8).collect::<Vec<_>>());
        // (All told first, then judged: one district's whole picture, not
        // the first thing wrong.)
        assert_eq!(reached, chases, "chases not reached");
        // (Off it no more than the corners it cuts, at 30 cm: within the
        // band of its radius round the walking space, its body clear of
        // any edge it can't cross.)
        assert!(worst_stray.0 <= 0.3, "{:.2} m off the walking space, at {:.1}", worst_stray.0, worst_stray.1);
        assert!(unplanned.is_empty(), "plans not found");
    }

    /// A square with a grid of round holes (the stacks' pillars, in small):
    /// its triangles cover it exactly, holes left out, and quickly.
    #[test]
    fn triangles_cover_holes() {
        let square = |o: DVec2, r: f64, n: usize| -> LineString<f64> { LineString::from((0..=n).map(|k| { let a = k as f64 / n as f64 * std::f64::consts::TAU; (o.x + r * a.cos(), o.y + r * a.sin()) }).collect::<Vec<_>>()) };
        let outer = LineString::from(vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0), (0.0, 0.0)]);
        let holes: Vec<LineString<f64>> = (0..30).flat_map(|i| (0..30).map(move |j| (i, j))).map(|(i, j)| square(DVec2::new(2.0 + i as f64 * 3.2, 2.0 + j as f64 * 3.2), 1.0, 48).into_iter().rev().collect()).collect();
        let poly = Polygon::new(outer, holes);
        let t = Instant::now();
        let (pts, tris, flat) = triangles(&poly).expect("triangulated");
        let took = t.elapsed().as_secs_f32();
        let covered: f64 = tris.iter().map(|t| (pts[t[1]] - pts[t[0]]).perp_dot(pts[t[2]] - pts[t[0]]) / 2.0).sum();
        assert!(tris.iter().all(|t| (pts[t[1]] - pts[t[0]]).perp_dot(pts[t[2]] - pts[t[0]]) > 0.0), "counter-clockwise");
        assert!((covered - poly.unsigned_area()).abs() < 1e-6 * poly.unsigned_area(), "covered {covered}, the polygon {}", poly.unsigned_area());
        // (Slivers between holes' points in a row, of no area: left out,
        // counted, as from geo; the cover above holds without them.)
        assert!(flat < tris.len() / 100, "{flat} slivers");
        assert!(took < 1.0, "{took} s for {} points", pts.len());
    }

    /// Whether `poly` lies at the edge of what was built (its big plates,
    /// `built`): a corner of it within 0.6 m of a big plate not built,
    /// where a way on may lie. (By the square asked for, a slot open onto a
    /// big plate not built, its centre outside it, was taken for a trap.)
    fn at_built_edge(world: &PlateWorld, built: &std::collections::HashSet<(i32, i32)>, poly: &Polygon<f64>) -> bool {
        poly.exterior().0.iter().any(|c| [(0.6, 0.0), (-0.6, 0.0), (0.0, 0.6), (0.0, -0.6)].iter().any(|(dx, dz)| !built.contains(&world.big_site(c.x + dx, c.y + dz))))
    }

    /// Every node of every district, not a sample (chases drawn by area
    /// all but never land on small ones): which ones a walker cannot reach
    /// from the main body of nodes, teleporting no more than 10 m, or 20 m,
    /// and dropping.
    /// The main body: the nodes reached from the biggest, landing only on
    /// nodes of `LAND_LEAST` or more; a node is reached if it is in it or a
    /// teleport from it lands there (where you may stand, though it may
    /// not land there otherwise).
    /// (`cargo test -p game --profile fast node_reach -- --ignored --nocapture`)
    #[test]
    #[ignore]
    fn node_reach() {
        let world = Arc::new(PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library")));
        let centres = [("floor", (10880.0, 128.0)), ("tiers", (5248.0, 640.0)), ("stacks", (12928.0, 128.0)), ("broken", (7296.0, 640.0)), ("uplands", (9344.0, 4736.0)), ("highlands", (9344.0, 2688.0))];
        // (NODE_REACH_AT=x,z: a square round that spot alone.)
        let at: Option<Vec<f64>> = std::env::var("NODE_REACH_AT").ok().map(|v| v.split(',').map(|x| x.parse().unwrap()).collect());
        let centres: Vec<(&str, (f64, f64))> = match &at {
            Some(a) => vec![("there", (a[0], a[1]))],
            None => centres.to_vec(),
        };
        for (name, centre) in centres {
            let nodes = Nodes::build(world.clone(), DVec2::from(centre), SIDE, false).unwrap();
            assert_eq!(nodes.cut, 0, "{name}: nodes whose navmesh is in parts");
            let area = &nodes.space.node_area;
            let total: f64 = area.iter().sum();
            let built: std::collections::HashSet<(i32, i32)> = nodes.space.pieces.iter().map(|p| p.plate.sites[0]).collect();
            // (Every node's triangles cover its outline exactly: no more, no
            // less, holes and all; slivers of no area apart.)
            let mut of: Vec<Vec<&Polygon<f64>>> = vec![Vec::new(); area.len()];
            for (i, frags) in nodes.space.space.iter().enumerate() {
                for (k, f) in frags.iter().enumerate() {
                    of[nodes.space.node_of[i][k] as usize].push(f);
                }
            }
            let (mut worst, mut off, mut none) = ((0.0f64, 0usize), 0, 0);
            for (n, list) in of.iter().enumerate() {
                for poly in outline(list).0 {
                    let Some((pts, tris, _)) = triangles(&poly) else {
                        none += 1;
                        continue;
                    };
                    let covered: f64 = tris.iter().map(|t| (pts[t[1]] - pts[t[0]]).perp_dot(pts[t[2]] - pts[t[0]]) / 2.0).sum();
                    let want = poly.unsigned_area();
                    let e = (covered - want).abs() / want.max(1e-9);
                    // (Off by no more than its corners moved, snapped: its
                    // edges' length by `TRIANGLE_SNAP`. By a millionth and a
                    // square centimetre, fixed, at 0.1 mm snapping: one in
                    // the stacks at 1 mm, two parts in 10⁶ of 133 m round.)
                    let round: f64 = std::iter::once(poly.exterior()).chain(poly.interiors()).map(|r| r.lines().map(|l| (l.end - l.start).x.hypot((l.end - l.start).y)).sum::<f64>()).sum();
                    if (covered - want).abs() > round * TRIANGLE_SNAP {
                        off += 1;
                    }
                    if e > worst.0 {
                        worst = (e, n);
                    }
                }
            }
            println!("{name}: triangles against outlines: {off} off by more than their corners moved, snapped, {none} not triangulated; worst {:.2e} (node {})", worst.0, worst.1);
            assert_eq!((off, none), (0, 0), "{name}: nodes' triangles not covering their outlines");
            let biggest = (0..area.len()).max_by(|a, b| area[*a].total_cmp(&area[*b])).unwrap();
            for range in [10.0f32, 20.0] {
                let mut main = vec![false; area.len()];
                main[biggest] = true;
                let mut queue = std::collections::VecDeque::from([biggest]);
                while let Some(n) = queue.pop_front() {
                    // (Teleports either way, drops one way.)
                    let next = nodes.teleports().adjacent[n].iter().filter(|(_, d)| *d <= range).map(|(m, _)| *m).chain(nodes.leaps_to[n].iter().filter(|l| l.1 < 0.0).map(|l| l.0));
                    for m in next {
                        if !main[m as usize] && area[m as usize] >= LAND_LEAST {
                            main[m as usize] = true;
                            queue.push_back(m as usize);
                        }
                    }
                }
                let reached: Vec<bool> = (0..area.len()).map(|n| main[n] || nodes.teleports().adjacent[n].iter().any(|&(m, d)| d <= range && main[m as usize])).collect();
                let mut cut: Vec<usize> = (0..area.len()).filter(|n| !reached[*n]).collect();
                cut.sort_by(|a, b| area[*b].total_cmp(&area[*a]));
                let cut_area: f64 = cut.iter().map(|n| area[*n]).sum();
                let big_cut = cut.iter().filter(|n| area[**n] >= LAND_LEAST).count();
                // (Where each node not reached lies, how high, and whether it
                // touches the edge of the square built: there it may be
                // reached from outside it.)
                let mut place: HashMap<usize, (DVec2, f64, bool)> = HashMap::new();
                for (i, frags) in nodes.space.space.iter().enumerate() {
                    for (f, poly) in frags.iter().enumerate() {
                        let n = nodes.space.node_of[i][f] as usize;
                        if reached[n] || place.contains_key(&n) {
                            continue;
                        }
                        use geo::Centroid;
                        let Some(c) = poly.centroid() else { continue };
                        let c = DVec2::new(c.x(), c.y());
                        let edge = at_built_edge(&world, &built, poly);
                        place.insert(n, (c, world.plate_top(&nodes.space.pieces[i].plate, c), edge));
                    }
                }
                let inner: Vec<usize> = cut.iter().copied().filter(|n| !place.get(n).is_some_and(|p| p.2)).collect();
                let inner_area: f64 = inner.iter().map(|n| area[*n]).sum();
                println!("  of them inside the square (not touching its edge): {}, {:.0} m², {:.2}% of the ground; the biggest: {:?}", inner.len(), inner_area, 100.0 * inner_area / total, inner.iter().take(6).map(|n| { let p = place[n]; format!("{:.0} m² at {:.0},{:.0} y {:.1}", area[*n], p.0.x, p.0.y, p.1) }).collect::<Vec<_>>());
                println!(
                    "{name} at {range} m: {} nodes, {:.0} m²; {} not reached ({} of {LAND_LEAST} m² or more), {:.0} m², {:.2}% of the ground; the biggest of them {:?} m²",
                    area.len(),
                    total,
                    cut.len(),
                    big_cut,
                    cut_area,
                    100.0 * cut_area / total,
                    cut.iter().take(6).map(|n| area[*n].round()).collect::<Vec<_>>()
                );
            }
        }
    }

    /// Where hops up onto ledges are, the highest first: to see one in
    /// play (`spawn KIND hop=2.45` below, you above).
    /// (`HOPS_AT=x,z cargo test -p game --profile fast hop_spots -- --ignored --nocapture`)
    #[test]
    #[ignore]
    fn hop_spots() {
        let world = Arc::new(PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library")));
        let at: Vec<f64> = std::env::var("HOPS_AT").unwrap_or("7296,640".into()).split(',').map(|x| x.parse().unwrap()).collect();
        // (BUILD_THREADS=1: the build on one thread, each stage's own work.)
        if let Some(n) = std::env::var("BUILD_THREADS").ok().and_then(|v| v.parse().ok()) {
            nodes::THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
        }
        let nodes = Nodes::build(world, DVec2::new(at[0], at[1]), SIDE, false).unwrap();
        println!("{}", nodes.report);
        // (Ledges within reach, found clear or not, by height.)
        let links = nodes.space.links(&nodes.world, HOP_GAP, Some(nodes::HOP_CELL), |gap, rise| if rise > nodes.space.fit.climb && gap <= HOP_GAP && rise <= crate::player::hookless_reach(gap) { gap - HOP_GAP } else { f64::INFINITY });
        let mut band = [0usize; 5];
        for l in links.values().flatten().filter(|l| l.short <= 0.0) {
            band[(((l.by - l.ay) / 0.5) as usize).min(4)] += 1;
        }
        println!("ledges within reach, by half metre of height: {band:?}");
        let p = &nodes.walks;
        let mut up: Vec<usize> = (0..p.at.len()).filter(|&i| p.leap[i].is_some() && p.y[p.twin[i] as usize] > p.y[i]).collect();
        up.sort_by(|a, b| (p.y[p.twin[*b] as usize] - p.y[*b]).total_cmp(&(p.y[p.twin[*a] as usize] - p.y[*a])));
        for &i in up.iter().take(12) {
            let t = p.twin[i] as usize;
            println!("  {:.2} m up: from {:.1},{:.1} y {:.1} to {:.1},{:.1} y {:.1}, leaping {:.1} m/s", p.y[t] - p.y[i], p.at[i].x, p.at[i].y, p.y[i], p.at[t].x, p.at[t].y, p.y[t], p.leap[i].unwrap());
        }
    }

    /// Traps (the user, 2026-11-14: no ground where a player without the
    /// hook can get stuck, a rule for the generator; none, asserted): every node of the
    /// player's walking space (its width, step and height) in each
    /// district, linked one way where a running jump and a mantle
    /// (`player::hookless_reach`) or a drop takes it to another; a trap is
    /// a node got into from the main body (the nodes the biggest one
    /// reaches) with no way back to it. Each group of them listed: where,
    /// how high, and the least it falls short of climbing out.
    /// (`cargo test -p game --profile fast player_traps -- --ignored --nocapture`;
    /// `NODE_REACH_AT=x,z` for one square.)
    #[test]
    #[ignore]
    fn player_traps() {
        use crate::navmesh::nodes::{self, Fit, Space};
        use std::collections::VecDeque;
        let world = Arc::new(PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&crate::structures::load_library().expect("the structure library")));
        let centres = [("floor", (10880.0, 128.0)), ("tiers", (5248.0, 640.0)), ("stacks", (12928.0, 128.0)), ("broken", (7296.0, 640.0)), ("uplands", (9344.0, 4736.0)), ("highlands", (9344.0, 2688.0))];
        let at: Option<Vec<f64>> = std::env::var("NODE_REACH_AT").ok().filter(|v| !v.is_empty()).map(|v| v.split(',').map(|x| x.parse().unwrap()).collect());
        let centres: Vec<(&str, (f64, f64))> = match &at {
            Some(a) => vec![("there", (a[0], a[1]))],
            None => centres.to_vec(),
        };
        let (radius, climb, height) = crate::player::FIT;
        // (Across no more than this, m: a running drop of 20 m lands 12 m
        // out; farther ones are left out, which can only show a trap where
        // there is none, never hide one.)
        const FAR: f64 = 15.0;
        let mut found = 0;
        for (name, centre) in centres {
            let t = Instant::now();
            let column = worldgen::column_size(0) as f64;
            let centre = DVec2::from(centre);
            let (lo, hi) = (((centre - SIDE / 2.0) / column).floor() * column, ((centre + SIDE / 2.0) / column).ceil() * column);
            let space = Space::build(&world, nodes::pieces(&world, lo, hi), Fit { radius, climb, height });
            let links: HashMap<(u32, u32), nodes::Link> = space.links(&world, FAR, None, |gap, rise| rise - crate::player::hookless_reach(gap)).into_iter().map(|(k, v)| (k, v[0])).collect();
            let took = t.elapsed().as_secs_f32();
            let area = &space.node_area;
            let n = area.len();
            let built: std::collections::HashSet<(i32, i32)> = space.pieces.iter().map(|p| p.plate.sites[0]).collect();
            let (mut out, mut into): (Vec<Vec<u32>>, Vec<Vec<u32>>) = (vec![Vec::new(); n], vec![Vec::new(); n]);
            for (&(a, b), l) in &links {
                if l.short <= 0.0 {
                    out[a as usize].push(b);
                    into[b as usize].push(a);
                }
            }
            let flood = |from: usize, next: &Vec<Vec<u32>>| {
                let mut seen = vec![false; n];
                seen[from] = true;
                let mut queue = VecDeque::from([from]);
                while let Some(k) = queue.pop_front() {
                    for &m in &next[k] {
                        if !seen[m as usize] {
                            seen[m as usize] = true;
                            queue.push_back(m as usize);
                        }
                    }
                }
                seen
            };
            let biggest = (0..n).max_by(|a, b| area[*a].total_cmp(&area[*b])).unwrap();
            let (got_to, back) = (flood(biggest, &out), flood(biggest, &into));
            // Each node's place: a point in it, how high, whether it touches
            // the edge of the square (where a way out may lie beyond it).
            let mut place: Vec<Option<(DVec2, f64, bool)>> = vec![None; n];
            for (i, frags) in space.space.iter().enumerate() {
                for (f, poly) in frags.iter().enumerate() {
                    let k = space.node_of[i][f] as usize;
                    let r = poly.bounding_rect().unwrap();
                    let edge = at_built_edge(&world, &built, poly);
                    let was = place[k];
                    use geo::InteriorPoint;
                    let c = poly.interior_point().map_or(DVec2::new(r.min().x, r.min().y), |p| DVec2::new(p.x(), p.y()));
                    let y = world.plate_top(&space.pieces[i].plate, c);
                    place[k] = Some(match was {
                        Some((c0, y0, e0)) => (c0, y0, e0 || edge),
                        None => (c, y, edge),
                    });
                }
            }
            let trapped: Vec<bool> = (0..n).map(|k| got_to[k] && !back[k]).collect();
            // Groups: trapped nodes one move apart, either way.
            let mut group = vec![usize::MAX; n];
            let mut groups: Vec<Vec<usize>> = Vec::new();
            for k in 0..n {
                if !trapped[k] || group[k] != usize::MAX {
                    continue;
                }
                let g = groups.len();
                let mut members = vec![k];
                group[k] = g;
                let mut queue = VecDeque::from([k]);
                while let Some(j) = queue.pop_front() {
                    for &m in out[j].iter().chain(&into[j]) {
                        let m = m as usize;
                        if trapped[m] && group[m] == usize::MAX {
                            group[m] = g;
                            members.push(m);
                            queue.push_back(m);
                        }
                    }
                }
                groups.push(members);
            }
            // The least each group falls short of climbing back to ground
            // with a way to the main body.
            let mut short: Vec<Option<nodes::Link>> = vec![None; groups.len()];
            for (&(a, b), l) in &links {
                if trapped[a as usize] && back[b as usize] {
                    let g = group[a as usize];
                    if short[g].is_none_or(|s| l.short < s.short) {
                        short[g] = Some(*l);
                    }
                }
            }
            // Each group's bounds.
            let mut bounds = vec![(DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)); groups.len()];
            for (i, frags) in space.space.iter().enumerate() {
                for (f, poly) in frags.iter().enumerate() {
                    let k = space.node_of[i][f] as usize;
                    if trapped[k] {
                        let r = poly.bounding_rect().unwrap();
                        let b = &mut bounds[group[k]];
                        b.0 = b.0.min(DVec2::new(r.min().x, r.min().y));
                        b.1 = b.1.max(DVec2::new(r.max().x, r.max().y));
                    }
                }
            }
            let total: f64 = area.iter().sum();
            let unentered: f64 = (0..n).filter(|k| !got_to[*k]).map(|k| area[k]).sum();
            let mut list: Vec<(f64, usize)> = groups.iter().enumerate().map(|(g, m)| (m.iter().map(|k| area[*k]).sum::<f64>(), g)).collect();
            list.sort_by(|a, b| b.0.total_cmp(&a.0));
            let inside: Vec<&(f64, usize)> = list.iter().filter(|(_, g)| !groups[*g].iter().any(|k| place[*k].is_some_and(|p| p.2))).collect();
            println!(
                "{name}: {n} player nodes, {total:.0} m², built and linked in {took:.1} s; {} not got into from the main body ({unentered:.0} m², {:.2}%: plateaus, perches); traps: {} groups ({} of 1 m² or more), {} at the square's edge left out",
                (0..n).filter(|k| !got_to[*k]).count(),
                100.0 * unentered / total,
                inside.len(),
                inside.iter().filter(|(a, _)| *a >= 1.0).count(),
                list.len() - inside.len()
            );
            for &&(a, g) in inside.iter().take(12) {
                let big = *groups[g].iter().max_by(|x, y| area[**x].total_cmp(&area[**y])).unwrap();
                let (c, y, _) = place[big].unwrap();
                let ys: Vec<f64> = groups[g].iter().filter_map(|k| place[*k].map(|p| p.1)).collect();
                let (y0, y1) = (ys.iter().copied().fold(f64::INFINITY, f64::min), ys.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                let (b0, b1) = bounds[g];
                let way = short[g].map_or("no way out within reach".to_string(), |l| format!("out needs {:.2} m more climb: from {:.0},{:.0} y {:.1} to {:.0},{:.0} y {:.1}", l.short, l.a.x, l.a.y, l.ay, l.b.x, l.b.y, l.by));
                println!("  {a:.1} m² in {} nodes at {:.0},{:.0} y {y:.1} (from {y0:.1} to {y1:.1}), over x {:.0} to {:.0}, z {:.0} to {:.0}; {way}", groups[g].len(), c.x, c.y, b0.x, b1.x, b0.y, b1.y);
            }
            found += inside.iter().filter(|(a, _)| *a >= 1.0).count();
        }
        println!("traps of 1 m² or more, in all: {found}");
        // (The user's rule: none; pits have stairs, wells are raised.)
        assert_eq!(found, 0, "ground a player without the hook is stuck in");
    }
}
