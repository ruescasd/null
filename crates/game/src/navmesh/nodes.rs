//! Navigable nodes, built by construction from the generator's plates (the
//! user's direction, 2026-11-02; docs/outdoor-navigation.md): a district's
//! ground is a flat map seen from above (no plate over another), so a
//! body's walking space is exact in 2D: every plate piece, less a band of
//! the body's radius round every edge it can't cross (a step over its
//! climb, a canal's cut, the edge of what was built, a pillar). Nodes are
//! what is left, joined across the edges it can cross. A pillar plate is
//! ground at its base, less its grown prisms within a body's height of it
//! (their tops, perches, are left out: walkers don't get onto them).
//!
//! Walkers walk within a node and teleport between nodes (`district.rs`);
//! the hops a teleport may take are the nodes' nearest edge points.

use std::collections::HashMap;

use bevy::math::DVec2;
use geo::{Area, BooleanOps, BoundingRect, Buffer, Contains, ConvexHull, Coord, Distance, Intersects, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon};
use worldgen::plates::{Plate, PlateWorld};

/// A body's fit (m): its radius, the highest step it climbs, its height.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fit {
    pub(crate) radius: f64,
    pub(crate) climb: f64,
    pub(crate) height: f64,
}

impl Fit {
    /// A walker: the biped's body (`navmesh::Body::Biped`).
    pub(crate) const WALKER: Fit = Fit { radius: 0.35, climb: 0.6, height: 2.6 };
}

/// The threads building the nodes take (in the background, in play): half
/// the cores, each set below the game's (`background`). (Taking them all,
/// the frame fell under 10 fps for the seconds a district's nodes took,
/// the user, 2026-11-15; half, still frames of 100 ms to 1.3 s, till they
/// were set below; one, none, but 19.5 s to build the stacks'.)
pub(crate) fn build_threads() -> usize {
    std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1))
}

/// This thread set below the game's own (each of the nodes' build threads
/// first): with as many busy as half the cores, the game's threads were
/// held up behind them for whole time slices, frames of 100 ms to 1.3 s.
pub(crate) fn background() {
    #[cfg(windows)]
    // SAFETY: setting the calling thread's own priority, by its pseudo
    // handle; nothing is freed or shared.
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_LOWEST};
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST);
    }
}

/// How near two edges must lie to be one border (m).
pub(crate) const ON: f64 = 1e-4;
/// The grid edges and pieces are found by (m).
pub(crate) const BIN: f64 = 8.0;

pub(crate) struct Piece {
    pub(crate) poly: Vec<DVec2>,
    /// Its plate; a pillar's, at its base.
    pub(crate) plate: Plate,
    /// A pillar grown from the library's form.
    pub(crate) pillar: bool,
    /// Left out of the mesh: a hole.
    pub(crate) hole: bool,
    /// A pillar's prisms: each its foot and top outlines (the same points,
    /// in order), and its bottom and top heights.
    pub(crate) prisms: Vec<(Vec<DVec2>, Vec<DVec2>, f64, f64)>,
}

/// A prism's outline between heights `a` and `b` (within its own): the
/// hull of its cross-sections there (it narrows and leans as it rises).
pub(crate) fn section(prism: &(Vec<DVec2>, Vec<DVec2>, f64, f64), a: f64, b: f64) -> Option<Polygon<f64>> {
    let (foot, top, y0, y1) = prism;
    let (a, b) = (a.max(*y0), b.min(*y1));
    if b <= a {
        return None;
    }
    let at = |y: f64| {
        let t = if y1 > y0 { (y - y0) / (y1 - y0) } else { 0.0 };
        foot.iter().zip(top).map(move |(f, u)| f.lerp(*u, t))
    };
    let points: Vec<Point<f64>> = at(a).chain(at(b)).map(|q| Point::new(snap(q.x), snap(q.y))).collect();
    Some(MultiPoint::new(points).convex_hull())
}

/// A stretch of a piece's edge: from `a` to `b`, with the piece across
/// it (none: nothing, a canal's cut or the edge of what was built), and
/// whether a body crosses it.
pub(crate) struct Stretch {
    pub(crate) a: DVec2,
    pub(crate) b: DVec2,
    pub(crate) piece: usize,
    pub(crate) other: Option<usize>,
    pub(crate) open: bool,
}

pub(crate) fn cells(a: DVec2, b: DVec2, pad: f64) -> impl Iterator<Item = (i32, i32)> {
    let (lo, hi) = (a.min(b) - pad, a.max(b) + pad);
    let (x0, z0, x1, z1) = ((lo.x / BIN).floor() as i32, (lo.y / BIN).floor() as i32, (hi.x / BIN).floor() as i32, (hi.y / BIN).floor() as i32);
    (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| (x, z)))
}

/// A coordinate on the grid of 1/1024 m: exact in binary, and Detour's
/// 1/64 m cells a whole number of steps. (The boolean library, i_overlay
/// 4.0.7, broke on a pillar's near-degenerate slabs off it.)
pub(crate) fn snap(v: f64) -> f64 {
    (v * 1024.0).round() / 1024.0
}

pub(crate) fn polygon(points: &[DVec2]) -> Polygon<f64> {
    Polygon::new(LineString::from(points.iter().map(|p| (snap(p.x), snap(p.y))).collect::<Vec<_>>()), vec![])
}

fn bounds(poly: &[DVec2]) -> (DVec2, DVec2) {
    poly.iter().fold((poly[0], poly[0]), |(a, b), q| (a.min(*q), b.max(*q)))
}

/// The plate pieces from `lo` to `hi` (best on columns' edges, the same
/// set the terrain's mesh has), as the generator outlines them: a pillar's
/// prisms grown, its plate taken at its base.
pub(crate) fn pieces(world: &PlateWorld, lo: DVec2, hi: DVec2) -> Vec<Piece> {
    let grows = world.grows_pillars_at_full_detail();
    world
        .plate_pieces(lo, hi)
        .into_iter()
        .map(|(poly, plate, kept)| {
            let pillar = grows && plate.pillar > 0.0;
            let o = poly.iter().fold(DVec2::ZERO, |s, q| s + *q) / poly.len() as f64;
            let at = |v: &bevy::math::Vec2| o + DVec2::new(v.x as f64, v.y as f64);
            let prisms = if pillar { world.pillar_prisms(&poly, &plate, o).into_iter().map(|p| (p.points.iter().map(at).collect(), p.top().iter().map(at).collect(), p.y0 as f64, p.y1 as f64)).collect() } else { Vec::new() };
            let plate = if pillar { Plate { height: plate.height - plate.pillar, pillar: 0.0, sloped: false, tilt: DVec2::ZERO, ..plate } } else { plate };
            Piece { pillar, hole: !kept, poly, plate, prisms }
        })
        .collect()
}

/// Every piece's edges cut where the piece across changes, each stretch
/// open or not for a body climbing `climb`.
pub(crate) fn stretches(world: &PlateWorld, pieces: &[Piece], climb: f64) -> Vec<Stretch> {
    let mut grid: HashMap<(i32, i32), Vec<(usize, usize)>> = HashMap::new();
    for (i, p) in pieces.iter().enumerate() {
        for k in 0..p.poly.len() {
            let (a, b) = (p.poly[k], p.poly[(k + 1) % p.poly.len()]);
            for c in cells(a, b, ON) {
                grid.entry(c).or_default().push((i, k));
            }
        }
    }
    let top = |i: usize, q: DVec2| world.plate_top(&pieces[i].plate, q);
    let mut out = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        for k in 0..p.poly.len() {
            let (a, b) = (p.poly[k], p.poly[(k + 1) % p.poly.len()]);
            let d = b - a;
            let len = d.length();
            if len < 1e-9 {
                continue;
            }
            let dir = d / len;
            // Who lies across it, and along which part (0..1).
            let mut across: Vec<(f64, f64, usize)> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for c in cells(a, b, ON) {
                for &(j, m) in grid.get(&c).into_iter().flatten() {
                    if j == i || !seen.insert((j, m)) {
                        continue;
                    }
                    let q = &pieces[j].poly;
                    let (c0, c1) = (q[m], q[(m + 1) % q.len()]);
                    let off = |x: DVec2| (x - a).perp_dot(dir).abs();
                    if off(c0) > ON || off(c1) > ON {
                        continue;
                    }
                    let (t0, t1) = ((c0 - a).dot(dir) / len, (c1 - a).dot(dir) / len);
                    let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
                    if hi - lo > ON / len {
                        across.push((lo, hi, j));
                    }
                }
            }
            across.sort_by(|x, y| x.0.total_cmp(&y.0));
            // The edge in parts: each with whoever is across, or none.
            let mut parts: Vec<(f64, f64, Option<usize>)> = Vec::new();
            let mut t = 0.0;
            for &(lo, hi, j) in &across {
                if lo > t + ON / len {
                    parts.push((t, lo, None));
                }
                let lo = lo.max(t);
                if hi > lo {
                    parts.push((lo, hi, Some(j)));
                    t = hi;
                }
            }
            if t < 1.0 - ON / len {
                parts.push((t, 1.0, None));
            }
            for (t0, t1, j) in parts {
                let Some(j) = j.filter(|&j| !pieces[j].hole && !p.hole) else {
                    out.push(Stretch { a: a + d * t0, b: a + d * t1, piece: i, other: j, open: false });
                    continue;
                };
                // The step along it, open where within the climb: exact for
                // flat and tilted plates (it changes linearly); sampled every
                // 25 cm on an earthwork.
                let n = if p.plate.sloped || pieces[j].plate.sloped { ((t1 - t0) * len / 0.25).ceil().max(1.0) as usize } else { 1 };
                let mut cuts = vec![t0];
                for s in 0..n {
                    let (u0, u1) = (t0 + (t1 - t0) * s as f64 / n as f64, t0 + (t1 - t0) * (s + 1) as f64 / n as f64);
                    let step = |u: f64| top(i, a + d * u) - top(j, a + d * u);
                    let (s0, s1) = (step(u0), step(u1));
                    for c in [climb, -climb] {
                        if (s0 - c) * (s1 - c) < 0.0 {
                            cuts.push(u0 + (u1 - u0) * (c - s0) / (s1 - s0));
                        }
                    }
                }
                cuts.push(t1);
                cuts.sort_by(f64::total_cmp);
                for w in cuts.windows(2) {
                    if w[1] - w[0] <= 1e-12 {
                        continue;
                    }
                    let m = a + d * ((w[0] + w[1]) / 2.0);
                    out.push(Stretch { a: a + d * w[0], b: a + d * w[1], piece: i, other: Some(j), open: (top(i, m) - top(j, m)).abs() <= climb });
                }
            }
        }
    }
    out
}

/// A teleport's way from one node to another: from `a` (its height `ay`)
/// to `b` (`by`), `d` apart in 3D. Both on the edge of their nodes'
/// walking space: where a body stands. Or a drop (`leap`, its upward
/// speed as it goes, m/s): one way, off a lip down onto `b`, inside the
/// node below (see `district::drops`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hop {
    pub(crate) a: DVec2,
    pub(crate) ay: f32,
    pub(crate) b: DVec2,
    pub(crate) by: f32,
    pub(crate) d: f32,
    pub(crate) leap: Option<f32>,
}

/// A one way move between nodes (`Space::links`): how far it falls short
/// of being made (at most 0, made), from `a` (its height `ay`) to `b`
/// (`by`), on the edges of their nodes' walking space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Link {
    pub(crate) short: f64,
    pub(crate) a: DVec2,
    pub(crate) ay: f64,
    pub(crate) b: DVec2,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) by: f64,
}

/// How far off an edge a point on it may be found (m): half a step of the
/// grid of 1/1024 m, and more. (A fragment's corners, out of the boolean
/// library on snapped outlines, lie up to 0.6 mm outside their piece; a
/// teleport landing there was found on no piece.)
pub(crate) const ROUNDING: f64 = 1e-3;

/// How far apart a node pair's hops are kept, along the edge they leave
/// from (m): the nearest in each cell of this side.
pub(crate) const HOP_CELL: f64 = 8.0;

/// A body's walking space over a set of pieces, and its nodes.
pub(crate) struct Space {
    pub(crate) fit: Fit,
    pub(crate) pieces: Vec<Piece>,
    pub(crate) edges: Vec<Stretch>,
    /// Each piece's walking space, in fragments.
    pub(crate) space: Vec<Vec<Polygon<f64>>>,
    /// Each fragment's node (as `space`).
    pub(crate) node_of: Vec<Vec<u32>>,
    /// Each node's area (m²).
    pub(crate) node_area: Vec<f64>,
    /// How many times two fragments were joined into one node.
    pub(crate) joins: usize,
    /// The pillars' obstacles (their prisms within a body's height of their
    /// ground, grown by the radius), and by cell.
    pub(crate) blocks: Vec<MultiPolygon<f64>>,
    pub(crate) blocked: HashMap<(i32, i32), Vec<usize>>,
    /// The closed stretches by cell (padded by the radius).
    closed: HashMap<(i32, i32), Vec<usize>>,
    /// The pieces by cell.
    grid: HashMap<(i32, i32), Vec<usize>>,
}

impl Space {
    /// Over `pieces`, for a body of `fit`, on every core.
    pub(crate) fn build(world: &PlateWorld, pieces: Vec<Piece>, fit: Fit) -> Space {
        let edges = stretches(world, &pieces, fit.climb);
        let mut closed: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (k, e) in edges.iter().enumerate() {
            if !e.open {
                for c in cells(e.a, e.b, fit.radius) {
                    closed.entry(c).or_default().push(k);
                }
            }
        }
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, p) in pieces.iter().enumerate() {
            let (a, b) = bounds(&p.poly);
            for c in cells(a, b, 0.0) {
                grid.entry(c).or_default().push(i);
            }
        }
        // The pillars' prisms within a body's height of their ground, each
        // with the radius round it: obstacles, to whatever ground they reach.
        let threads = build_threads();
        let pillars: Vec<&Piece> = pieces.iter().filter(|p| p.pillar).collect();
        let made: Vec<MultiPolygon<f64>> = std::thread::scope(|scope| {
            let chunk = pillars.len().div_ceil(threads * 8).max(1);
            let handles: Vec<_> = pillars
                .chunks(chunk)
                .map(|c| {
                    scope.spawn(move || {
                        background();
                        c.iter()
                            .filter_map(|p| {
                                let floor = p.plate.height;
                                // (A slab whose top is within a climb of the
                                // ground is stepped onto, not walked round: no
                                // obstacle. Taken for one, Detour walked over
                                // ground the plates said was blocked.)
                                let hulls: Vec<Polygon<f64>> = p.prisms.iter().filter(|q| q.3 - floor > fit.climb).filter_map(|q| section(q, floor, floor + fit.height)).collect();
                                (!hulls.is_empty()).then(|| MultiPolygon::new(hulls).buffer(fit.radius))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        let mut blocks: Vec<MultiPolygon<f64>> = Vec::new();
        let mut blocked: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for block in made {
            if let Some(r) = block.bounding_rect() {
                for c in cells(DVec2::new(r.min().x, r.min().y), DVec2::new(r.max().x, r.max().y), 0.0) {
                    blocked.entry(c).or_default().push(blocks.len());
                }
                blocks.push(block);
            }
        }
        let mut space = Space { fit, pieces, edges, space: Vec::new(), node_of: Vec::new(), node_area: Vec::new(), joins: 0, blocks, blocked, closed, grid };
        // Each piece's walking space: itself, less the radius round every
        // closed stretch near it, less the pillars' obstacles.
        let one = |p: &Piece| -> Vec<Polygon<f64>> {
            if p.hole {
                return Vec::new();
            }
            let band: MultiPolygon<f64> = space.closed_near(&p.poly).buffer(fit.radius);
            let mut left = polygon(&p.poly).difference(&band);
            let (a, b) = bounds(&p.poly);
            let mut ks: Vec<usize> = cells(a, b, 0.0).flat_map(|c| space.blocked.get(&c).into_iter().flatten().copied()).collect();
            ks.sort();
            ks.dedup();
            for k in ks {
                left = left.difference(&space.blocks[k]);
            }
            left.0
        };
        let chunk = space.pieces.len().div_ceil(threads * 8).max(1);
        let walking: Vec<Vec<Polygon<f64>>> = std::thread::scope(|scope| {
            let handles: Vec<_> = space.pieces.chunks(chunk).map(|c| scope.spawn(|| {
                background();
                c.iter().map(one).collect::<Vec<_>>()
            })).collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        space.space = walking;
        space.join();
        space
    }

    /// Nodes: fragments joined where an open stretch, less the radius
    /// round closed ones, still has length; numbered, with their areas.
    fn join(&mut self) {
        let mut id: Vec<Vec<usize>> = Vec::new();
        let mut n = 0;
        for s in &self.space {
            id.push((n..n + s.len()).collect());
            n += s.len();
        }
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], mut i: usize) -> usize {
            while p[i] != i {
                p[i] = p[p[i]];
                i = p[i];
            }
            i
        }
        let inside = |i: usize, q: DVec2| self.space[i].iter().position(|f| f.contains(&Point::new(q.x, q.y)));
        let mut joins = 0;
        for e in self.edges.iter().filter(|e| e.open && e.other.is_some_and(|j| j > e.piece)) {
            let j = e.other.unwrap();
            let line = MultiLineString::new(vec![LineString::from(vec![(e.a.x, e.a.y), (e.b.x, e.b.y)])]);
            let band = self.closed_near(&[e.a, e.b]).buffer(self.fit.radius);
            let left = band.clip(&line, true);
            let dir = (e.b - e.a).normalize();
            for part in left.0 {
                let pts: Vec<Coord<f64>> = part.0;
                if pts.len() < 2 {
                    continue;
                }
                let (p0, p1) = (DVec2::new(pts[0].x, pts[0].y), DVec2::new(pts[pts.len() - 1].x, pts[pts.len() - 1].y));
                if p0.distance(p1) < 1e-3 {
                    continue;
                }
                // (All along it, every 10 cm: either side's walking space may
                // be in several fragments along one stretch, and each joins
                // the one across. At its midpoint only, fragments were left
                // apart that a way crossed, found against Detour.) (Into the
                // piece: its outline runs one way round, so both sides are
                // tried, and whichever lands in it kept.)
                let n = dir.perp() * 1e-3;
                let steps = (p0.distance(p1) / 0.1).ceil().max(1.0) as usize;
                for k in 0..steps {
                    let m = p0.lerp(p1, (k as f64 + 0.5) / steps as f64);
                    let (fi, fj) = (inside(e.piece, m + n).or(inside(e.piece, m - n)), inside(j, m - n).or(inside(j, m + n)));
                    if let (Some(fi), Some(fj)) = (fi, fj) {
                        let (x, y) = (find(&mut parent, id[e.piece][fi]), find(&mut parent, id[j][fj]));
                        if x != y {
                            parent[x] = y;
                            joins += 1;
                        }
                    }
                }
            }
        }
        let mut number: HashMap<usize, u32> = HashMap::new();
        let mut node_area: Vec<f64> = Vec::new();
        let mut node_of: Vec<Vec<u32>> = Vec::new();
        for (i, s) in self.space.iter().enumerate() {
            let mut of = Vec::new();
            for (k, f) in s.iter().enumerate() {
                let root = find(&mut parent, id[i][k]);
                let n = *number.entry(root).or_insert_with(|| {
                    node_area.push(0.0);
                    node_area.len() as u32 - 1
                });
                node_area[n as usize] += f.unsigned_area();
                of.push(n);
            }
            node_of.push(of);
        }
        self.node_of = node_of;
        self.node_area = node_area;
        self.joins = joins;
    }

    /// Nodes under `least` m² left out of the walking space, the rest
    /// numbered anew.
    pub(crate) fn prune(&mut self, least: f64) {
        let mut number: Vec<Option<u32>> = vec![None; self.node_area.len()];
        let mut area = Vec::new();
        for (n, &a) in self.node_area.iter().enumerate() {
            if a >= least {
                number[n] = Some(area.len() as u32);
                area.push(a);
            }
        }
        for (frags, of) in self.space.iter_mut().zip(self.node_of.iter_mut()) {
            let kept: Vec<(Polygon<f64>, u32)> = frags.drain(..).zip(of.drain(..)).filter_map(|(f, n)| number[n as usize].map(|m| (f, m))).collect();
            for (f, m) in kept {
                frags.push(f);
                of.push(m);
            }
        }
        self.node_area = area;
    }

    /// The closed stretches near `poly` (within the radius of its bounds).
    pub(crate) fn closed_near(&self, poly: &[DVec2]) -> MultiLineString<f64> {
        let (a, b) = bounds(poly);
        let mut ks: Vec<usize> = cells(a, b, self.fit.radius).flat_map(|c| self.closed.get(&c).into_iter().flatten().copied()).collect();
        ks.sort();
        ks.dedup();
        MultiLineString::new(ks.into_iter().map(|k| LineString::from(vec![(self.edges[k].a.x, self.edges[k].a.y), (self.edges[k].b.x, self.edges[k].b.y)])).collect())
    }

    /// How many nodes.
    pub(crate) fn nodes(&self) -> usize {
        self.node_area.len()
    }

    /// The piece under `q` (or within `ROUNDING` of it).
    pub(crate) fn under(&self, q: DVec2) -> Option<usize> {
        let pt = Point::new(q.x, q.y);
        let near = |p: &Polygon<f64>| p.intersects(&pt) || geo::Euclidean.distance(p, &pt) <= ROUNDING;
        self.grid.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32))?.iter().copied().find(|&i| near(&polygon(&self.pieces[i].poly)))
    }

    /// The pieces whose bounds lie within `pad` of `q`.
    pub(crate) fn pieces_near(&self, q: DVec2, pad: f64) -> Vec<usize> {
        let mut out: Vec<usize> = cells(q, q, pad).flat_map(|c| self.grid.get(&c).into_iter().flatten().copied()).collect();
        out.sort();
        out.dedup();
        out
    }

    /// The node whose walking space `q` is in (or on the edge of, within
    /// `ROUNDING`: a teleport lands on the edge of a fragment, and where two
    /// fragments of a node meet, that is a piece's edge too), and which
    /// piece and fragment.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn node_at(&self, q: DVec2) -> Option<(u32, usize, usize)> {
        self.node_at_preferring(q, None)
    }

    /// As `node_at`, and where `q` is on the edge of two nodes, `prefer`, if
    /// it is one. (A walker just landed on the edge of a node was taken for
    /// on the other, and could go nowhere from there.)
    pub(crate) fn node_at_preferring(&self, q: DVec2, prefer: Option<u32>) -> Option<(u32, usize, usize)> {
        let pt = Point::new(q.x, q.y);
        let near = |p: &Polygon<f64>| p.intersects(&pt) || geo::Euclidean.distance(p, &pt) <= ROUNDING;
        let pieces = self.grid.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32))?;
        let mut found = pieces.iter().copied().filter(|&i| near(&polygon(&self.pieces[i].poly))).flat_map(|i| self.space[i].iter().enumerate().filter(|(_, f)| near(f)).map(move |(k, _)| (self.node_of[i][k], i, k)));
        let first = found.next()?;
        if prefer.is_none_or(|n| n == first.0) {
            return Some(first);
        }
        Some(found.find(|f| Some(f.0) == prefer).unwrap_or(first))
    }

    /// The hops between nodes no more than `far` apart (3D): points every
    /// metre round each fragment's edges, each pair of nodes' nearest in
    /// each `HOP_CELL` of the lower-numbered node's side; keyed by the pair,
    /// the lower first, each hop from it.
    pub(crate) fn hops(&self, world: &PlateWorld, far: f64) -> HashMap<(u32, u32), Vec<Hop>> {
        let points = self.edge_points(world);
        let mut cell: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (k, p) in points.iter().enumerate() {
            cell.entry(((p.0.x / far).floor() as i32, (p.0.y / far).floor() as i32)).or_default().push(k);
        }
        let keys: Vec<(i32, i32)> = cell.keys().copied().collect();
        let threads = build_threads();
        type Best = HashMap<((u32, u32), (i32, i32)), Hop>;
        let keep = |out: &mut Best, key: ((u32, u32), (i32, i32)), hop: Hop| {
            let e = out.entry(key).or_insert(hop);
            if hop.d < e.d {
                *e = hop;
            }
        };
        let best: Best = std::thread::scope(|scope| {
            let chunk = keys.len().div_ceil(threads * 4).max(1);
            let (cell, points) = (&cell, &points);
            let handles: Vec<_> = keys
                .chunks(chunk)
                .map(|c| {
                    scope.spawn(move || {
                        background();
                        let mut out: Best = HashMap::new();
                        for &(cx, cz) in c {
                            for &a in &cell[&(cx, cz)] {
                                let pa = points[a];
                                for dz in -1..=1 {
                                    for dx in -1..=1 {
                                        let Some(others) = cell.get(&(cx + dx, cz + dz)) else { continue };
                                        for &b in others {
                                            let pb = points[b];
                                            if pb.2 <= pa.2 {
                                                continue;
                                            }
                                            let d = (pa.0.distance_squared(pb.0) + (pa.1 - pb.1).powi(2)).sqrt();
                                            if d <= far {
                                                let at = ((pa.0.x / HOP_CELL).floor() as i32, (pa.0.y / HOP_CELL).floor() as i32);
                                                keep(&mut out, ((pa.2, pb.2), at), Hop { a: pa.0, ay: pa.1 as f32, b: pb.0, by: pb.1 as f32, d: d as f32, leap: None });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        out
                    })
                })
                .collect();
            let mut all: Best = HashMap::new();
            for h in handles {
                for (k, hop) in h.join().unwrap() {
                    keep(&mut all, k, hop);
                }
            }
            all
        });
        let mut out: HashMap<(u32, u32), Vec<Hop>> = HashMap::new();
        for ((pair, _), hop) in best {
            out.entry(pair).or_default().push(hop);
        }
        out
    }

    /// Points every metre round each fragment's edges: where, how high,
    /// and its node.
    fn edge_points(&self, world: &PlateWorld) -> Vec<(DVec2, f64, u32)> {
        let mut points: Vec<(DVec2, f64, u32)> = Vec::new();
        for (i, s) in self.space.iter().enumerate() {
            for (k, f) in s.iter().enumerate() {
                let n = self.node_of[i][k];
                for ring in std::iter::once(f.exterior()).chain(f.interiors()) {
                    for w in ring.0.windows(2) {
                        let (a, b) = (DVec2::new(w[0].x, w[0].y), DVec2::new(w[1].x, w[1].y));
                        let m = (a.distance(b) / 1.0).ceil().max(1.0) as usize;
                        for j in 0..m {
                            let q = a.lerp(b, j as f64 / m as f64);
                            points.push((q, world.plate_top(&self.pieces[i].plate, q), n));
                        }
                    }
                }
            }
        }
        points
    }

    /// One way moves between nodes: for each ordered pair of nodes with
    /// edge points no more than `far` apart across (2D), the least
    /// `short(gap, rise)` over them (how far the move falls short of
    /// making it: at most 0, it is made), and where from and to; with
    /// `spread`, the least in each cell of that side along the edge it
    /// leaves from, else one. Directed: a drop down is no climb up.
    pub(crate) fn links(&self, world: &PlateWorld, far: f64, spread: Option<f64>, short: impl Fn(f64, f64) -> f64 + Sync) -> HashMap<(u32, u32), Vec<Link>> {
        let points = self.edge_points(world);
        let mut cell: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (k, p) in points.iter().enumerate() {
            cell.entry(((p.0.x / far).floor() as i32, (p.0.y / far).floor() as i32)).or_default().push(k);
        }
        let keys: Vec<(i32, i32)> = cell.keys().copied().collect();
        let threads = build_threads();
        type Best = HashMap<((u32, u32), (i32, i32)), Link>;
        let keep = |out: &mut Best, pair: (u32, u32), l: Link| {
            let at = spread.map_or((0, 0), |c| ((l.a.x / c).floor() as i32, (l.a.y / c).floor() as i32));
            let e = out.entry((pair, at)).or_insert(l);
            if l.short < e.short {
                *e = l;
            }
        };
        std::thread::scope(|scope| {
            let chunk = keys.len().div_ceil(threads * 4).max(1);
            let (cell, points, short) = (&cell, &points, &short);
            let handles: Vec<_> = keys
                .chunks(chunk)
                .map(|c| {
                    scope.spawn(move || {
                        background();
                        let mut out: Best = HashMap::new();
                        for &(cx, cz) in c {
                            for &a in &cell[&(cx, cz)] {
                                let pa = points[a];
                                for dz in -1..=1 {
                                    for dx in -1..=1 {
                                        let Some(others) = cell.get(&(cx + dx, cz + dz)) else { continue };
                                        for &b in others {
                                            let pb = points[b];
                                            if pb.2 == pa.2 {
                                                continue;
                                            }
                                            let gap = pa.0.distance(pb.0);
                                            if gap <= far {
                                                keep(&mut out, (pa.2, pb.2), Link { short: short(gap, pb.1 - pa.1), a: pa.0, ay: pa.1, b: pb.0, by: pb.1 });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        out
                    })
                })
                .collect();
            let mut all: Best = HashMap::new();
            for h in handles {
                for ((pair, _), l) in h.join().unwrap() {
                    keep(&mut all, pair, l);
                }
            }
            let mut out: HashMap<(u32, u32), Vec<Link>> = HashMap::new();
            for ((pair, _), l) in all {
                out.entry(pair).or_default().push(l);
            }
            out
        })
    }
}
