//! Navigable nodes, built by construction from the generator's plates (the
//! user's direction, 2026-11-02; docs/outdoor-navigation.md): a district's
//! ground is a flat map seen from above (no plate over another), so a
//! body's walking space is exact in 2D: every plate piece, less a band of
//! the body's radius round every edge it can't cross (a step over its
//! climb, a canal's cut, the edge of what was built, a pillar). Nodes are
//! what is left, joined across the edges it can cross. A pillar a body
//! would walk into is solid, its whole plate (the user, 2026-11-16: its
//! panels sunk into faces flush with its plate, nothing of its plate is
//! walked on; a walker's centre reached at most 15 cm into a recess):
//! its edges walls, like a hole's. (Before, its plate less each of its
//! slabs within a body's height, grown: ~11 overlapping regions a pillar,
//! half the walking space's cuts, for strips at most 15 cm wide.)
//!
//! Correct by construction, by the plates' own structure (the user,
//! 2026-11-16): the plates are one planar map of convex cells
//! (`worldgen::planar`); the walking space is each cell less the convex
//! regions a body keeps out of, in convex parts whose corners are known by
//! what they are; the nodes are those parts joined edge to edge, and their
//! own meshes (`Space`). No stage has a tolerance of its own.
//!
//! Walkers walk within a node and teleport between nodes (`district.rs`);
//! the hops a teleport may take are the nodes' nearest edge points.

use bevy::platform::collections::HashMap;

use bevy::math::DVec2;
use geo::{ConvexHull, MultiPoint, Point};
use worldgen::planar::{self, Corner, Geometry, Key, Line};
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
    match THREADS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1)),
        n => n,
    }
}

/// `f` over `items` in chunks, on `build_threads` threads (at the
/// background's priority), the chunks' answers in order. (A thread for every
/// chunk, eight for each build thread, all at once: every core, not half,
/// and a build "on one thread" on eight.)
pub(crate) fn par_chunks<T: Sync, R: Send>(items: &[T], f: impl Fn(&[T]) -> R + Sync) -> Vec<R> {
    let threads = build_threads();
    let chunk = items.len().div_ceil(threads * 8).max(1);
    let chunks: Vec<&[T]> = items.chunks(chunk).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<(usize, R)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads.min(chunks.len()))
            .map(|_| {
                scope.spawn(|| {
                    background();
                    let mut mine = Vec::new();
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(c) = chunks.get(k) else { break };
                        mine.push((k, f(c)));
                    }
                    mine
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    out.sort_by_key(|x| x.0);
    out.into_iter().map(|x| x.1).collect()
}

/// The build's threads, if set (0: as `build_threads` says): a test's,
/// one, to measure each stage's work.
pub(crate) static THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

/// The grid edges and pieces are found by (m).
pub(crate) const BIN: f64 = 8.0;

pub(crate) struct Piece {
    /// Its corners (see `worldgen::planar`), stitched: a corner of a piece
    /// beside it on an edge of its is one of its, so pieces beside one
    /// another share their edges corner for corner.
    pub(crate) corners: Vec<Corner>,
    /// Their places.
    pub(crate) poly: Vec<DVec2>,
    /// Its plate; a pillar's, at its base.
    pub(crate) plate: Plate,
    /// A pillar grown from the library's form.
    pub(crate) pillar: bool,
    /// Left out of the mesh: a hole.
    pub(crate) hole: bool,
    /// A pillar's prisms within reach of its ground (see `pieces`): each its
    /// foot and top outlines (the same points, in order), and its bottom and
    /// top heights.
    pub(crate) prisms: Vec<(Vec<DVec2>, Vec<DVec2>, f64, f64)>,
}

/// A stretch of a piece's edge: from `a` to `b`, with the piece across
/// it (none: nothing, a canal's cut or the edge of what was built), and
/// whether a body crosses it.
pub(crate) struct Stretch {
    pub(crate) a: DVec2,
    pub(crate) b: DVec2,
    /// The line it lies on (see `worldgen::planar`).
    pub(crate) line: Line,
    pub(crate) piece: usize,
    pub(crate) other: Option<usize>,
    pub(crate) open: bool,
}

/// `poly` counter-clockwise, its last point not its first.
pub(crate) fn ccw(mut poly: Vec<DVec2>) -> Vec<DVec2> {
    if poly.len() > 1 && poly[0].distance(poly[poly.len() - 1]) < 1e-12 {
        poly.pop();
    }
    let twice: f64 = (0..poly.len()).map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()])).sum();
    if twice < 0.0 {
        poly.reverse();
    }
    poly
}

/// The part of the path `a` + `span` u (u in 0..1, and beyond) within
/// convex `hull` (counter-clockwise) grown by `grow` (each edge moved out
/// by it, and kept within its bounds grown by as much: a little more than
/// `grow` at its corners, no more than 1.5 times): from and to which u, if
/// any. (Edges moved out alone meet far off at a sharp corner: a sliver's
/// reached a fall's path from 2.7 m away, the falls' check taking it for
/// ground under the body, 37 drops in 340 000 not found.)
pub(crate) fn along(hull: &[DVec2], a: DVec2, span: DVec2, grow: f64) -> Option<(f64, f64)> {
    let (mut u0, mut u1) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut side = |out: DVec2, num: f64| {
        let den = span.dot(out);
        if den.abs() < 1e-15 {
            num >= 0.0
        } else {
            if den > 0.0 {
                u1 = u1.min(num / den);
            } else {
                u0 = u0.max(num / den);
            }
            true
        }
    };
    for i in 0..hull.len() {
        let (p, q) = (hull[i], hull[(i + 1) % hull.len()]);
        let Some(out) = (-(q - p).perp()).try_normalize() else { continue };
        if !side(out, grow - (a - p).dot(out)) {
            return None;
        }
    }
    if grow > 0.0 && !hull.is_empty() {
        let (lo, hi) = bounds(hull);
        for (out, at) in [(DVec2::X, hi.x + grow), (DVec2::NEG_X, -(lo.x - grow)), (DVec2::Y, hi.y + grow), (DVec2::NEG_Y, -(lo.y - grow))] {
            if !side(out, at - a.dot(out)) {
                return None;
            }
        }
    }
    (u0 <= u1).then_some((u0, u1))
}


/// Every point within `r` of the hull of `points`, and no more than the
/// grid's rounding beyond: the hull, on the grid (whole numbers of
/// `planar::GRID`), counter-clockwise, of each point moved `ROUND_STEPS`
/// ways round a circle that much wider (see `Keep::round`).
fn grown(points: impl IntoIterator<Item = DVec2>, r: f64) -> Vec<(i64, i64)> {
    thread_local! { static TURNS: std::cell::RefCell<Vec<DVec2>> = const { std::cell::RefCell::new(Vec::new()) }; }
    let n = round_steps();
    let k = (r + std::f64::consts::FRAC_1_SQRT_2 / planar::GRID) / (std::f64::consts::PI / n as f64).cos();
    let mut at: Vec<(i64, i64)> = TURNS.with(|turns| {
        let mut turns = turns.borrow_mut();
        if turns.len() != n {
            *turns = (0..n).map(|j| DVec2::from_angle((j as f64 + 0.5) * std::f64::consts::TAU / n as f64)).collect();
        }
        points.into_iter().flat_map(|p| turns.iter().map(move |u| p + *u * k)).map(|q| ((q.x * planar::GRID).round() as i64, (q.y * planar::GRID).round() as i64)).collect()
    });
    let hull = grid_hull(&mut at);
    assert!(hull.len() >= 3, "a grown region of some area: {hull:?}");
    hull
}

/// Where grid lines are: from what they are alone (a region's `Cut`s).
struct GridOnly;

impl Geometry for GridOnly {
    fn site(&self, _: usize, _: (i32, i32)) -> DVec2 {
        unreachable!("grid lines only")
    }

    fn line(&self, l: Line) -> (DVec2, DVec2) {
        match l {
            Line::Grid { a, b, c } => planar::grid_line(a, b, c),
            _ => unreachable!("grid lines only"),
        }
    }
}

pub(crate) fn cells(a: DVec2, b: DVec2, pad: f64) -> impl Iterator<Item = (i32, i32)> {
    let (lo, hi) = (a.min(b) - pad, a.max(b) + pad);
    let (x0, z0, x1, z1) = ((lo.x / BIN).floor() as i32, (lo.y / BIN).floor() as i32, (hi.x / BIN).floor() as i32, (hi.y / BIN).floor() as i32);
    (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| (x, z)))
}

fn bounds(poly: &[DVec2]) -> (DVec2, DVec2) {
    poly.iter().fold((poly[0], poly[0]), |(a, b), q| (a.min(*q), b.max(*q)))
}

/// How far up from a pillar's ground its prisms are kept (m): over the
/// tallest body's height (a walker's 2.6 m), all a body on its ground
/// meets. (All kept, 43 a pillar in the stacks, each made into outlines,
/// for the two or three below a body's head.)
pub(crate) const BODY_REACH: f64 = 4.0;

/// The plate pieces from `lo` to `hi` (best on columns' edges, the same
/// set the terrain's mesh has), as the generator outlines them: a pillar's
/// prisms grown, those starting within `BODY_REACH` of its ground kept, its
/// plate taken at its base.
pub(crate) fn pieces(world: &PlateWorld, lo: DVec2, hi: DVec2) -> Vec<Piece> {
    pieces_up_to(world, lo, hi, BODY_REACH)
}

/// As `pieces`, a pillar's prisms starting within `reach` of its ground.
pub(crate) fn pieces_up_to(world: &PlateWorld, lo: DVec2, hi: DVec2, reach: f64) -> Vec<Piece> {
    let grows = world.grows_pillars_at_full_detail();
    // (In strips, on the build's threads: each big plate is its site's
    // strip's, so the same pieces, in the same order. On one thread, a
    // third of the build's time in play.)
    const STRIPS: usize = 16;
    let strips: Vec<(DVec2, DVec2)> = (0..STRIPS)
        .map(|k| {
            let x = |k: usize| if k == STRIPS { hi.x } else { lo.x + (hi.x - lo.x) * k as f64 / STRIPS as f64 };
            (DVec2::new(x(k), lo.y), DVec2::new(x(k + 1), hi.y))
        })
        .collect();
    // (A pillar's form grown in its strip's thread too: after them, on one,
    // half the stacks' pieces in play.)
    let piece = |(corners, plate, kept): (Vec<Corner>, Plate, bool)| {
        let poly = planar::points(&corners);
        let pillar = grows && plate.pillar > 0.0;
        let o = poly.iter().fold(DVec2::ZERO, |s, q| s + *q) / poly.len() as f64;
        let at = |v: &bevy::math::Vec2| o + DVec2::new(v.x as f64, v.y as f64);
        let floor = plate.height - plate.pillar;
        let prisms = if pillar { world.pillar_prisms(&poly, &plate, o).into_iter().filter(|p| (p.y0 as f64) < floor + reach).map(|p| (p.points.iter().map(at).collect(), p.top().iter().map(at).collect(), p.y0 as f64, p.y1 as f64)).collect() } else { Vec::new() };
        let plate = if pillar { Plate { height: plate.height - plate.pillar, pillar: 0.0, sloped: false, tilt: DVec2::ZERO, ..plate } } else { plate };
        Piece { pillar, hole: !kept, corners, poly, plate, prisms }
    };
    let mut pieces: Vec<Piece> = par_chunks(&strips, |c| c.iter().flat_map(|&(a, b)| world.plate_pieces(a, b)).map(piece).collect::<Vec<_>>()).into_iter().flatten().collect();
    // (Stitched all together: a piece's neighbour may be in another strip.)
    let mut corners: Vec<Vec<Corner>> = pieces.iter_mut().map(|p| std::mem::take(&mut p.corners)).collect();
    planar::stitch(&mut corners, world);
    for (p, c) in pieces.iter_mut().zip(corners) {
        p.poly = planar::points(&c);
        p.corners = c;
    }
    pieces
}

/// Every piece's edges, each with the piece across it (none: a canal's cut,
/// a hole, the edge of what was built), cut where the step across passes a
/// body's `climb`, each stretch open or not. By the planar map (see
/// `worldgen::planar`): an edge's piece across is its twin's, the same two
/// corners the other way round, found by what they are, never by how near.
/// `solid`: pieces nothing crosses onto (`Space::build`'s pillars), their
/// edges walls, as a hole's are.
pub(crate) fn stretches(world: &PlateWorld, pieces: &[Piece], climb: f64, solid: &[bool]) -> Vec<Stretch> {
    let mut twin: HashMap<(Key, Key), usize> = HashMap::new();
    for (i, p) in pieces.iter().enumerate() {
        for k in 0..p.corners.len() {
            twin.insert((p.corners[k].key, p.corners[(k + 1) % p.corners.len()].key), i);
        }
    }
    let top = |i: usize, q: DVec2| world.plate_top(&pieces[i].plate, q);
    let mut out = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        let n = p.corners.len();
        for k in 0..n {
            let (u, w) = (p.corners[k], p.corners[(k + 1) % n]);
            let (a, b) = (u.at, w.at);
            let other = twin.get(&(w.key, u.key)).copied().filter(|&j| j != i);
            let Some(j) = other.filter(|&j| !pieces[j].hole && !p.hole && !solid[i] && !solid[j]) else {
                out.push(Stretch { a, b, line: u.next, piece: i, other: None, open: false });
                continue;
            };
            // The step along it, open where within the climb: exact for
            // flat and tilted plates (it changes linearly); sampled every
            // 25 cm on an earthwork. (From the lesser piece's side, the
            // greater's the same cuts the other way round: each wall the
            // same from either side.)
            let (lo, hi, from) = if i < j { (a, b, i) } else { (b, a, j) };
            let d = hi - lo;
            let len = d.length();
            let m = if p.plate.sloped || pieces[j].plate.sloped { (len / 0.25).ceil().max(1.0) as usize } else { 1 };
            let to = if from == i { j } else { i };
            let step = |u: f64| top(from, lo + d * u) - top(to, lo + d * u);
            let mut cuts = vec![0.0];
            for s in 0..m {
                let (u0, u1) = (s as f64 / m as f64, (s + 1) as f64 / m as f64);
                let (s0, s1) = (step(u0), step(u1));
                for c in [climb, -climb] {
                    if (s0 - c) * (s1 - c) < 0.0 {
                        cuts.push(u0 + (u1 - u0) * (c - s0) / (s1 - s0));
                    }
                }
            }
            cuts.push(1.0);
            cuts.sort_by(f64::total_cmp);
            let mut parts: Vec<Stretch> = cuts
                .windows(2)
                .filter(|w| w[1] - w[0] > 1e-12)
                .map(|w| {
                    let (x, y) = (if w[0] == 0.0 { lo } else { lo + d * w[0] }, if w[1] == 1.0 { hi } else { lo + d * w[1] });
                    Stretch { a: x, b: y, line: u.next, piece: i, other: Some(j), open: step((w[0] + w[1]) / 2.0).abs() <= climb }
                })
                .collect();
            if from != i {
                parts.reverse();
                for s in &mut parts {
                    std::mem::swap(&mut s.a, &mut s.b);
                }
            }
            out.extend(parts);
        }
    }
    out
}

/// What lies about a stretch of ground (`Space::about`).
pub(crate) struct About {
    pub(crate) pieces: Vec<usize>,
    pub(crate) blocks: Vec<usize>,
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

/// How far off a node a point may be found on it (m): a point on its edge,
/// computed, lands a hair either side.
pub(crate) const ROUNDING: f64 = 1e-3;

/// How far apart a node pair's hops are kept, along the edge they leave
/// from (m): the nearest in each cell of this side.
pub(crate) const HOP_CELL: f64 = 8.0;

/// The ways round a keep-out region's round edges are made (`Keep::round`):
/// 6, or `--set round_steps=N` in play. (The user, 2026-11-16: 6 for 12,
/// a wall's round end up to 0.404 m from it for 0.362, 4 cm walkers would
/// not show; each step a cut where a piece goes on past a wall's end, the
/// bulk of the walking space's work: walking 58-66 ms for 76-84 a big
/// square, a quarter fewer parts.)
pub(crate) static ROUND_STEPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(6);

fn round_steps() -> usize {
    ROUND_STEPS.load(std::sync::atomic::Ordering::Relaxed)
}

/// A region a body's centre keeps out of: a closed wall's band. Convex, counter-clockwise, its corners on the grid
/// (`planar::GRID`), no three in a line; each edge's line by what it is
/// (`Line::Grid`: two regions' edges on one line are on one line), and
/// whether the region is on its normal's side, with its `Cut`, and its
/// normal turned inwards with where the line is along it (`clear`); and
/// its edges, longest first (`less`).
pub(crate) struct Keep {
    pub(crate) ring: Vec<DVec2>,
    lines: Vec<(Line, bool)>,
    cuts: Vec<planar::Cut>,
    facing: Vec<(DVec2, f64, f64)>,
    order: Vec<usize>,
    lo: DVec2,
    hi: DVec2,
}

impl Keep {
    /// Every point within `r` of the hull of `points`, and no more than
    /// the grid's rounding beyond (half its diagonal): the hull, on the
    /// grid, of each point moved `ROUND_STEPS` ways round a circle that
    /// much wider, each edge touching it. (A corner moved onto the grid
    /// moves its edges no more than half the grid's diagonal: round the
    /// circle of `r`, never in it.) A wall's band from its two ends.
    pub(crate) fn round(points: impl IntoIterator<Item = DVec2>, r: f64) -> Keep {
        let hull = grown(points, r);
        let n = hull.len();
        let ring: Vec<DVec2> = hull.iter().map(|&(x, y)| DVec2::new(x as f64, y as f64) / planar::GRID).collect();
        let lines: Vec<(Line, bool)> = (0..n).map(|i| Line::grid(hull[i], hull[(i + 1) % n])).collect();
        let cuts = lines.iter().map(|&(l, _)| planar::Cut::new(l, &GridOnly)).collect();
        let facing = lines
            .iter()
            .map(|&(l, inside)| {
                let (p, n) = GridOnly.line(l);
                let n = if inside { n } else { -n };
                (n, n.dot(p), 1e-6 * n.length())
            })
            .collect();
        let (lo, hi) = bounds(&ring);
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&i, &j| ring[(j + 1) % n].distance_squared(ring[j]).total_cmp(&ring[(i + 1) % n].distance_squared(ring[i])).then(i.cmp(&j)));
        Keep { ring, lines, cuts, facing, order, lo, hi }
    }

    /// Turned so that above 0 is inside, below outside: `s`, a side of
    /// edge `k`'s line by its `Cut`.
    fn inward(&self, k: usize, s: f64) -> f64 {
        if self.lines[k].1 { s } else { -s }
    }

    /// Whether all of the box `lo`..`hi` is on one side of edge `k`'s line,
    /// by more than a micrometre: inside (`Some(true)`), outside
    /// (`Some(false)`); else `None`, its corners to be tried one by one.
    /// (Its corners' rounding, far within a micrometre, the same each
    /// corner's side says; two products where they lie far from the line,
    /// most of them. Every corner against every line: a third of the
    /// walking space's work.)
    fn clear(&self, k: usize, lo: DVec2, hi: DVec2) -> Option<bool> {
        let (n, o, margin) = self.facing[k];
        let far = DVec2::new(if n.x > 0.0 { hi.x } else { lo.x }, if n.y > 0.0 { hi.y } else { lo.y });
        let near = DVec2::new(if n.x > 0.0 { lo.x } else { hi.x }, if n.y > 0.0 { lo.y } else { hi.y });
        if n.dot(near) - o > margin {
            Some(true)
        } else if n.dot(far) - o < -margin {
            Some(false)
        } else {
            None
        }
    }
}

/// A corner while a piece's walking space is made (`Walk`): where, and its
/// point and the line of its edge on, by the piece's own numbers
/// (`Walk::keys`, `Walk::lines`): 24 bytes. (As a `Corner`, 112, its key
/// 64: copied in every cut, hashed and compared in every merge, the cost
/// of a cut 33 times a plain clip's.)
#[derive(Clone, Copy, Debug)]
struct Spot {
    at: DVec2,
    point: u32,
    line: u32,
}

/// An edge by its points' numbers, from and to; or two lines'.
fn pair(a: u32, b: u32) -> u64 {
    (a as u64) << 32 | b as u64
}

fn spot_bounds(p: &[Spot]) -> (DVec2, DVec2) {
    p.iter().fold((p[0].at, p[0].at), |(a, b), s| (a.min(s.at), b.max(s.at)))
}

/// A piece's walking space as regions are taken from it (`walking`), kept
/// from piece to piece for its buffers. Its points by what they are (`keys`,
/// one number a point: `names`; where two lines cross, by their numbers:
/// `crossings`, each pair named once) and their places; its lines (one
/// number a line). Its parts (convex, counter-clockwise), their bounds,
/// whether still one; and every edge of theirs, by its points' numbers, to
/// its part.
#[derive(Default)]
pub(crate) struct Walk {
    keys: Vec<Key>,
    ats: Vec<DVec2>,
    names: HashMap<Key, u32>,
    crossings: HashMap<u64, u32>,
    lines: Vec<Line>,
    line_ids: HashMap<Line, u32>,
    parts: Vec<Vec<Spot>>,
    bounds: Vec<(DVec2, DVec2)>,
    alive: Vec<bool>,
    edges: HashMap<u64, u32>,
    pool: Vec<Vec<Spot>>,
    sides: Vec<f64>,
}

impl Walk {
    fn reset(&mut self) {
        self.keys.clear();
        self.ats.clear();
        self.names.clear();
        self.crossings.clear();
        self.lines.clear();
        self.line_ids.clear();
        for p in self.parts.drain(..) {
            self.pool.push(p);
        }
        self.bounds.clear();
        self.alive.clear();
        self.edges.clear();
    }

    fn point(&mut self, key: Key, at: DVec2) -> u32 {
        let next = self.keys.len() as u32;
        let p = *self.names.entry(key).or_insert(next);
        if p == next {
            self.keys.push(key);
            self.ats.push(at);
        }
        p
    }

    fn line(&mut self, l: Line) -> u32 {
        let next = self.lines.len() as u32;
        let i = *self.line_ids.entry(l).or_insert(next);
        if i == next {
            self.lines.push(l);
        }
        i
    }

    /// Where lines `a` and `b` cross: named by what it is the first time,
    /// by the two numbers after.
    fn crossing<G: Geometry + ?Sized>(&mut self, a: u32, b: u32, geometry: &G) -> (u32, DVec2) {
        let k = if a < b { pair(a, b) } else { pair(b, a) };
        if let Some(&p) = self.crossings.get(&k) {
            return (p, self.ats[p as usize]);
        }
        let key = Key::of(self.lines[a as usize], self.lines[b as usize]);
        let p = self.point(key, geometry.corner(key));
        self.crossings.insert(k, p);
        (p, self.ats[p as usize])
    }

    fn side(&self, cut: &planar::Cut, s: &Spot) -> f64 {
        cut.side_at(s.at, &self.keys[s.point as usize])
    }

    fn add(&mut self, p: Vec<Spot>) -> u32 {
        let i = self.parts.len() as u32;
        let n = p.len();
        for k in 0..n {
            self.edges.insert(pair(p[k].point, p[(k + 1) % n].point), i);
        }
        self.bounds.push(spot_bounds(&p));
        self.alive.push(true);
        self.parts.push(p);
        i
    }

    fn remove(&mut self, i: u32) {
        let p = std::mem::take(&mut self.parts[i as usize]);
        let n = p.len();
        // (Those a part made from it has taken over, left.)
        for k in 0..n {
            let e = pair(p[k].point, p[(k + 1) % n].point);
            if self.edges.get(&e) == Some(&i) {
                self.edges.remove(&e);
            }
        }
        self.alive[i as usize] = false;
        self.pool.push(p);
    }

    /// Whether `keep` and `part` (its bounds `lo`..`hi`) are apart: the
    /// part outside one of the region's edges, or the region outside one of
    /// the part's. (An edge of no length has no line: passed over.)
    fn apart(&self, keep: &Keep, part: &[Spot], (lo, hi): (DVec2, DVec2)) -> bool {
        let m = keep.ring.len();
        if (0..m).any(|k| match keep.clear(k, lo, hi) {
            Some(inside) => !inside,
            None => part.iter().all(|s| keep.inward(k, self.side(&keep.cuts[k], s)) <= 0.0),
        }) {
            return true;
        }
        let n = part.len();
        (0..n).any(|i| {
            let (a, e) = (part[i].at, part[(i + 1) % n].at - part[i].at);
            e != DVec2::ZERO && keep.ring.iter().all(|v| e.perp_dot(*v - a) <= 0.0)
        })
    }

    /// Part `i` less region `keep`: what of it lies outside, in convex
    /// pieces, into `out`. Untouched (`apart`), nothing. Else cut along each
    /// of the region's edges' lines in turn, the longest first: the piece
    /// outside each kept, the rest carried on; what is left at the end is
    /// inside, gone. (Longest first, a band's sides: the rest no wider than
    /// the band, the cuts after them, round its ends, stay near it. In the
    /// ring's order, its ends' cuts went across the whole part: 14 parts a
    /// piece in the tiers.) Whether it was cut.
    fn less<G: Geometry + ?Sized>(&mut self, keep: &Keep, i: u32, geometry: &G, out: &mut Vec<Vec<Spot>>) -> bool {
        let (lo, hi) = self.bounds[i as usize];
        if self.apart(keep, &self.parts[i as usize], (lo, hi)) {
            return false;
        }
        let mut rest = self.pool.pop().unwrap_or_default();
        rest.clear();
        rest.extend_from_slice(&self.parts[i as usize]);
        let mut spare = self.pool.pop().unwrap_or_default();
        let mut sides = std::mem::take(&mut self.sides);
        let (mut lo, mut hi) = (lo, hi);
        for &k in &keep.order {
            match keep.clear(k, lo, hi) {
                Some(true) => continue,
                Some(false) => {
                    out.push(rest);
                    self.pool.push(spare);
                    self.sides = sides;
                    return true;
                }
                None => {}
            }
            let (mut inside, mut outside) = (false, false);
            sides.clear();
            for s in &rest {
                let side = self.side(&keep.cuts[k], s);
                sides.push(side);
                let s = keep.inward(k, side);
                inside |= s > 0.0;
                outside |= s < 0.0;
            }
            if !outside {
                continue;
            }
            if !inside {
                out.push(rest);
                self.pool.push(spare);
                self.sides = sides;
                return true;
            }
            // Outside, kept; inside, on to the next line.
            let l = self.line(keep.lines[k].0);
            let mut piece = self.pool.pop().unwrap_or_default();
            if keep.lines[k].1 {
                self.split(&rest, l, &sides, geometry, &mut piece, &mut spare);
            } else {
                self.split(&rest, l, &sides, geometry, &mut spare, &mut piece);
            }
            out.push(piece);
            std::mem::swap(&mut rest, &mut spare);
            (lo, hi) = spot_bounds(&rest);
        }
        self.pool.push(rest);
        self.pool.push(spare);
        self.sides = sides;
        true
    }

    /// `poly` cut by line `l` both ways at once, `sides` its corners' (by
    /// the line's `Cut`): below it into `below`, above into `above`, as
    /// `planar::split` makes them.
    fn split<G: Geometry + ?Sized>(&mut self, poly: &[Spot], l: u32, sides: &[f64], geometry: &G, below: &mut Vec<Spot>, above: &mut Vec<Spot>) {
        below.clear();
        above.clear();
        let m = poly.len();
        for i in 0..m {
            let (a, j) = (poly[i], (i + 1) % m);
            let (sa, sb) = (sides[i], sides[j]);
            if sa < 0.0 {
                below.push(a);
                if sb > 0.0 {
                    let (point, at) = self.crossing(a.line, l, geometry);
                    below.push(Spot { at, point, line: l });
                    above.push(Spot { at, point, line: a.line });
                }
            } else if sa > 0.0 {
                above.push(a);
                if sb < 0.0 {
                    let (point, at) = self.crossing(a.line, l, geometry);
                    above.push(Spot { at, point, line: l });
                    below.push(Spot { at, point, line: a.line });
                }
            } else {
                // On the line: a corner of both, going on along it from
                // whichever leaves it.
                below.push(Spot { line: if sb > 0.0 { l } else { a.line }, ..a });
                above.push(Spot { line: if sb < 0.0 { l } else { a.line }, ..a });
            }
        }
    }

    /// Parts beside one another across whole edges (the same two points,
    /// each way round) joined where their union is convex, till no two are
    /// (Hertel and Mehlhorn's): as few parts as the walking space's own
    /// corners call for, however the cuts made them. (Each region's cuts
    /// split every part the regions before it left near it: where regions
    /// overlap, a pillar's slabs, bands round one corner, parts multiplied,
    /// 52 GB in the stacks.) Only `fresh` parts, and what they are joined
    /// into, are tried: no two others could be joined after the merge
    /// before. Two parts beside one another along part of an edge alone
    /// never make one convex part: whole edges are all it looks for.
    fn merge(&mut self, fresh: &mut Vec<u32>) {
        while let Some(i) = fresh.pop() {
            if !self.alive[i as usize] {
                continue;
            }
            let a = &self.parts[i as usize];
            let n = a.len();
            let joined = (0..n).find_map(|k| {
                let j = *self.edges.get(&pair(a[(k + 1) % n].point, a[k].point))?;
                (j != i).then(|| union(a, k, &self.parts[j as usize])).flatten().map(|u| (j, u))
            });
            if let Some((j, u)) = joined {
                let made = self.add(u);
                self.remove(i);
                self.remove(j);
                fresh.push(made);
            }
        }
    }
}

/// `a` and `b` (convex, counter-clockwise), `a`'s edge `k` the other way
/// round one of `b`'s, as one, if convex: the run of edges they share left
/// out, the turns where it ends no more than straight (straight by what
/// their lines are, going on the same way, or turning left).
fn union(a: &[Spot], k: usize, b: &[Spot]) -> Option<Vec<Spot>> {
    let (na, nb) = (a.len() as isize, b.len() as isize);
    let at = |p: &[Spot], i: isize| p[i.rem_euclid(p.len() as isize) as usize];
    // `a`'s edges k0..=k1 shared with `b`'s u1..=u0 (going down).
    let u = (0..nb).find(|&u| at(b, u).point == at(a, k as isize + 1).point && at(b, u + 1).point == at(a, k as isize).point)?;
    let (mut k0, mut k1, mut u0, mut u1) = (k as isize, k as isize, u, u);
    while k1 - k0 + 2 < na.min(nb) && at(a, k1 + 2).point == at(b, u1 - 1).point {
        k1 += 1;
        u1 -= 1;
    }
    while k1 - k0 + 2 < na.min(nb) && at(a, k0 - 1).point == at(b, u0 + 2).point {
        k0 -= 1;
        u0 += 1;
    }
    // Where it starts (`s`: `a`'s k0, `b`'s u0 + 1) and ends (`e`: `a`'s
    // k1 + 1, `b`'s u1), each turn from one's edge onto the other's.
    // (On one line, straight only going on the same way: back along it,
    // the two lie side by side further, their corners there not the same.)
    let turn = |from: Spot, here: Spot, to: Spot| {
        let (u, v) = (here.at - from.at, to.at - here.at);
        if from.line == here.line { u.dot(v) > 0.0 } else { u.perp_dot(v) > 0.0 }
    };
    if !turn(at(a, k0 - 1), at(b, u0 + 1), at(b, u0 + 2)) || !turn(at(b, u1 - 1), at(a, k1 + 1), at(a, k1 + 2)) {
        return None;
    }
    // `a` from `e` round to before `s`, then `b` from `s` round to before
    // `e`.
    let mut out = Vec::with_capacity((na + nb) as usize);
    let mut i = k1 + 1;
    while (i - k0).rem_euclid(na) != 0 {
        out.push(at(a, i));
        i += 1;
    }
    let mut j = u0 + 1;
    while (j - u1).rem_euclid(nb) != 0 {
        out.push(at(b, j));
        j += 1;
    }
    Some(out)
}

/// How far a wall's band's sides lie from it, at most, by the radius: the
/// round ends' steps circumscribe it (1 / cos(180° / steps): 6, 1.155),
/// and the grid's rounding, with room.
fn band_reach() -> f64 {
    1.0 / (std::f64::consts::PI / round_steps() as f64).cos() + 0.015
}

/// The wall on `line` from `a` to `b` or past them, if any.
fn wall_along(line: Line, a: DVec2, b: DVec2, on_line: &HashMap<Line, Vec<(f64, f64, u32)>>, dirs: &HashMap<Line, (DVec2, DVec2)>, walls: &[(DVec2, DVec2)]) -> Option<u32> {
    let (o, d) = *dirs.get(&line)?;
    let (ta, tb) = ((a - o).dot(d), (b - o).dot(d));
    let (lo, hi) = (ta.min(tb), ta.max(tb));
    // (The wall's span by its ends, as the side's: the same points, the
    // same arithmetic.)
    on_line.get(&line)?.iter().find(|&&(_, _, w)| {
        let (a, b) = walls[w as usize];
        let (u, v) = ((a - o).dot(d), (b - o).dot(d));
        u.min(v) <= lo && hi <= u.max(v)
    }).map(|&(_, _, w)| w)
}

/// A piece (`corners`, convex, counter-clockwise) less the regions `keeps`
/// (those that may touch it, in order), in convex parts: each region not
/// apart from the piece taken from the parts the regions before it left,
/// those it cut merged again (`Walk::merge`). And how many regions touched
/// it (their bounds met). `walk` its buffers.
///
/// First, `straight`: walls along whole edges of the piece, each a band's
/// side facing it (its edge `k`), the piece cut by it alone, its band not
/// in `keeps`. Exact where the wall goes on past each end of the edge, or
/// the piece's other edge there is a wall too: within a convex piece, a
/// point's nearest point of such a wall is across from it, never its end.
/// (Its band's round ends cut there anyway, slivers of their steps' poke
/// past its sides, up to 1.2 cm at 12: 13% of the tiers' cuts, 35% of the
/// broken's.)
pub(crate) fn walking<'a, G: Geometry + ?Sized>(walk: &mut Walk, corners: &[Corner], straight: &[(&Keep, usize)], keeps: impl Iterator<Item = &'a Keep>, geometry: &G) -> (Vec<Vec<Corner>>, usize) {
    walk.reset();
    let mut piece = walk.pool.pop().unwrap_or_default();
    piece.clear();
    for c in corners {
        let (point, line) = (walk.point(c.key, c.at), walk.line(c.next));
        piece.push(Spot { at: c.at, point, line });
    }
    let mut sides = std::mem::take(&mut walk.sides);
    for &(keep, k) in straight {
        let (mut inside, mut outside) = (false, false);
        sides.clear();
        for s in &piece {
            let side = walk.side(&keep.cuts[k], s);
            sides.push(side);
            let v = keep.inward(k, side);
            inside |= v > 0.0;
            outside |= v < 0.0;
        }
        if !inside {
            continue;
        }
        if !outside {
            piece.clear();
            break;
        }
        let l = walk.line(keep.lines[k].0);
        let (mut below, mut above) = (walk.pool.pop().unwrap_or_default(), walk.pool.pop().unwrap_or_default());
        walk.split(&piece, l, &sides, geometry, &mut below, &mut above);
        // (Outside: below its line where the region is above.)
        let (kept, other) = if keep.lines[k].1 { (below, above) } else { (above, below) };
        walk.pool.push(std::mem::replace(&mut piece, kept));
        walk.pool.push(other);
    }
    walk.sides = sides;
    if piece.len() < 3 {
        walk.pool.push(piece);
        return (Vec::new(), straight.len());
    }
    let whole = spot_bounds(&piece);
    let (lo, hi) = whole;
    let mut live = vec![walk.add(piece)];
    let first_part = live[0];
    let (mut cut, mut fresh): (Vec<Vec<Spot>>, Vec<u32>) = Default::default();
    let mut outline: Vec<Spot> = Vec::new();
    outline.extend_from_slice(&walk.parts[first_part as usize]);
    let mut touches = straight.len();
    for keep in keeps {
        if !(keep.lo.cmple(hi).all() && keep.hi.cmpge(lo).all()) {
            continue;
        }
        touches += 1;
        // (Apart from the piece, from every part of it: 42 700 of the
        // stacks' 93 000 regions did nothing, each tried on every part.)
        if walk.apart(keep, &outline, whole) {
            continue;
        }
        let mut changed = false;
        for &i in &live {
            let (a, b) = walk.bounds[i as usize];
            if keep.lo.cmple(b).all() && keep.hi.cmpge(a).all() && walk.less(keep, i, geometry, &mut cut) {
                for p in cut.drain(..) {
                    fresh.push(walk.add(p));
                }
                walk.remove(i);
                changed = true;
            }
        }
        // (A part wholly inside it: gone, nothing made.)
        if !changed {
            continue;
        }
        let first = walk.alive.len() as u32 - fresh.len() as u32;
        walk.merge(&mut fresh);
        // (Those left, and those made since.)
        live.retain(|&i| walk.alive[i as usize]);
        live.extend((first..walk.alive.len() as u32).filter(|&i| walk.alive[i as usize]));
        if live.is_empty() {
            break;
        }
    }
    let out = live
        .iter()
        .map(|&i| walk.parts[i as usize].iter().map(|s| Corner { at: s.at, key: walk.keys[s.point as usize], next: walk.lines[s.line as usize], number: s.point }).collect())
        .collect();
    (out, touches)
}

/// A part's edge along its line (`Space::build`'s nodes): the line's
/// number, whether the edge goes the line's way (its part on its left
/// going that way, so on one side of it), its span along it (`t0` < `t1`),
/// its ends there, and its part.
#[derive(Clone, Copy)]
struct Along {
    line: u32,
    up: bool,
    t0: f64,
    t1: f64,
    p0: DVec2,
    p1: DVec2,
    part: u32,
}

/// One line's edges on one side (`these`) against those on the other
/// (`others`), each side in order along it, none over another: for each
/// overlap, `meet` (the two edges, and the overlap's ends, in order along
/// the line, each a corner of one of them); each stretch of these with
/// none across it, a rim (in its part's order round).
fn along_line(these: &[Along], others: &[Along], meet: &mut dyn FnMut(&Along, &Along, DVec2, DVec2), rims: &mut Vec<Rim>) {
    let mut rim = |e: &Along, x: DVec2, y: DVec2| rims.push(if e.up { Rim { a: x, b: y, part: e.part } } else { Rim { a: y, b: x, part: e.part } });
    for e in these {
        let (mut at, mut at_p) = (e.t0, e.p0);
        let first = others.partition_point(|o| o.t1 <= e.t0);
        for o in others[first..].iter().take_while(|o| o.t0 < e.t1) {
            if o.t0 > at {
                rim(e, at_p, o.p0);
            }
            let (x, y) = (if e.t0 >= o.t0 { e.p0 } else { o.p0 }, if e.t1 <= o.t1 { e.p1 } else { o.p1 });
            meet(e, o, x, y);
            if o.t1 >= e.t1 {
                (at, at_p) = (e.t1, e.p1);
                break;
            }
            (at, at_p) = (o.t1, o.p1);
        }
        if at < e.t1 {
            rim(e, at_p, e.p1);
        }
    }
}

/// The convex hull of `points` (whole numbers), counter-clockwise, no three
/// corners in a line: exact.
fn grid_hull(points: &mut Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    points.sort_unstable();
    points.dedup();
    let turn = |o: (i64, i64), a: (i64, i64), b: (i64, i64)| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut hull: Vec<(i64, i64)> = Vec::with_capacity(points.len() + 1);
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &(i64, i64)>> = if pass == 0 { Box::new(points.iter()) } else { Box::new(points.iter().rev()) };
        for &q in iter {
            while hull.len() >= start + 2 && turn(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 0 {
                hull.pop();
            }
            hull.push(q);
        }
        hull.pop();
    }
    hull
}

/// A convex part of the walking space: its corners (counter-clockwise),
/// bounds, piece, node, and the parts beside it: each with the edge between
/// (from this part's side, in its order: the other's on its right going
/// across).
pub(crate) struct Part {
    pub(crate) poly: Vec<DVec2>,
    pub(crate) lo: DVec2,
    pub(crate) hi: DVec2,
    pub(crate) piece: u32,
    pub(crate) node: u32,
    pub(crate) portals: Vec<(u32, DVec2, DVec2)>,
}

/// A stretch of a node's edge (`Space::rims`): from `a` to `b`, the node on
/// its left, on `part`.
#[derive(Clone, Copy)]
pub(crate) struct Rim {
    pub(crate) a: DVec2,
    pub(crate) b: DVec2,
    pub(crate) part: u32,
}

/// A body's walking space over a set of pieces, and its nodes.
///
/// By the generator's structure (the user, 2026-11-16: the essential work,
/// by construction): every piece is a convex cell of the plates' planar map
/// (`worldgen::planar`), its corners and lines known by what they are. A
/// body keeps out of convex regions: a band of its radius round every edge
/// it can't cross (each wall once, those along one line one; a solid
/// pillar's plate's edges walls, see the module's), `Keep`.
/// Each piece's walking space is the piece less the regions touching it,
/// cut into convex parts along their edges' lines; every corner of a part
/// is known by what it is too, so parts beside one another share their
/// edges corner for corner (stitched, `planar::stitch`). A node is parts
/// joined edge to edge (an edge's twin, by its corners' keys): nothing
/// joins across a wall, its band between. The parts are the node's mesh as
/// they are: ways across it are found on them (`way`), exact, no rounding.
/// (Before: one overlay of every piece and band on an integer grid, its
/// shapes triangulated, a mesh in f32 for each node: general machinery for
/// work the plates' structure makes unnecessary.)
pub(crate) struct Space {
    pub(crate) fit: Fit,
    pub(crate) pieces: Vec<Piece>,
    /// The stretches (for its checks: `district::tests::as_defined`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) edges: Vec<Stretch>,
    pub(crate) parts: Vec<Part>,
    /// Each node's area (m²), parts, and edge.
    pub(crate) node_area: Vec<f64>,
    pub(crate) members: Vec<Vec<u32>>,
    pub(crate) rims: Vec<Vec<Rim>>,
    /// The parts by cell, and the rims (node, rim).
    part_cells: HashMap<(i32, i32), Vec<u32>>,
    rim_cells: HashMap<(i32, i32), Vec<(u32, u32)>>,
    /// The solid pillars (see the module's), each its plate grown by the
    /// radius (one hull: its corners, counter-clockwise, last not first), by
    /// cell: what a falling body keeps clear of (`district::fall_clear`).
    pub(crate) blocked: HashMap<(i32, i32), Vec<usize>>,
    pub(crate) block_rings: Vec<Vec<Vec<DVec2>>>,
    /// Each piece's bounds.
    pub(crate) boxes: Vec<(DVec2, DVec2)>,
    /// The pieces by cell.
    grid: HashMap<(i32, i32), Vec<usize>>,
    /// How long each stage of its building took (s), in order.
    pub(crate) times: Vec<(&'static str, f32)>,
    /// Each piece's convex hull, counter-clockwise: what a fall's path
    /// crosses (see `district::fall_clear`).
    pub(crate) hulls: Vec<Vec<DVec2>>,
    /// The keep-out regions made: walls' bands, and all.
    pub(crate) walls: usize,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) keeps: usize,
    /// Pieces and regions touching, each region tried against the piece's
    /// parts.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) touches: usize,
    /// Edges found twice the same way round (two parts over one another):
    /// none, by construction (`district::tests::node_reach` checks).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) overlaps: usize,
}

impl Space {
    /// Over `pieces`, for a body of `fit`, on the build's threads.
    pub(crate) fn build(world: &PlateWorld, pieces: Vec<Piece>, fit: Fit) -> Space {
        let mut times = Vec::new();
        let clock = std::time::Instant::now();
        let lap = |name: &'static str, times: &mut Vec<(&'static str, f32)>| {
            times.push((name, clock.elapsed().as_secs_f32() - times.iter().map(|t: &(&str, f32)| t.1).sum::<f32>()));
        };
        // A pillar a body would walk into (a slab of it within its height
        // of the ground, over its climb: a lower one is stepped onto):
        // solid, its whole plate.
        let solid: Vec<bool> = pieces.iter().map(|p| p.pillar && p.prisms.iter().any(|q| q.3 - p.plate.height > fit.climb && q.2 < p.plate.height + fit.height)).collect();
        let edges = stretches(world, &pieces, fit.climb, &solid);
        lap("edges", &mut times);
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, p) in pieces.iter().enumerate() {
            let (a, b) = bounds(&p.poly);
            for c in cells(a, b, 0.0) {
                grid.entry(c).or_default().push(i);
            }
        }
        let hulls: Vec<Vec<DVec2>> = pieces.iter().map(|p| ccw(MultiPoint::new(p.poly.iter().map(|q| Point::new(q.x, q.y)).collect()).convex_hull().exterior().0.iter().map(|c| DVec2::new(c.x, c.y)).collect())).collect();
        let boxes = hulls.iter().map(|h| bounds(h)).collect();
        // The keep-out regions. The walls: each once (the same stretches
        // from either side), and those that go on from one another along
        // one line, by what it is, one.
        let mut along: Vec<(Line, f64, f64, DVec2, DVec2)> = Vec::new();
        let mut dirs: HashMap<Line, (DVec2, DVec2)> = HashMap::new();
        for e in edges.iter().filter(|e| !e.open && e.other.is_none_or(|j| e.piece < j)) {
            let (p, d) = *dirs.entry(e.line).or_insert_with(|| {
                let (p, n) = world.line(e.line);
                (p, n.perp())
            });
            let (ta, tb) = ((e.a - p).dot(d), (e.b - p).dot(d));
            along.push(if ta <= tb { (e.line, ta, tb, e.a, e.b) } else { (e.line, tb, ta, e.b, e.a) });
        }
        along.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)));
        let mut walls: Vec<(DVec2, DVec2)> = Vec::new();
        let mut on_line: HashMap<Line, Vec<(f64, f64, u32)>> = HashMap::new();
        let mut k = 0;
        while k < along.len() {
            let (line, t0, mut t1, a, mut b) = along[k];
            k += 1;
            while k < along.len() && along[k].0 == line && along[k].1 <= t1 {
                if along[k].2 > t1 {
                    (t1, b) = (along[k].2, along[k].4);
                }
                k += 1;
            }
            on_line.entry(line).or_default().push((t0, t1, walls.len() as u32));
            walls.push((a, b));
        }
        let keeps: Vec<Keep> = par_chunks(&walls, |c| c.iter().map(|&(a, b)| Keep::round([a, b], fit.radius)).collect::<Vec<_>>()).into_iter().flatten().collect();
        let wall_count = keeps.len();
        let pillars: Vec<usize> = (0..pieces.len()).filter(|&i| solid[i]).collect();
        let block_rings: Vec<Vec<Vec<DVec2>>> = par_chunks(&pillars, |c| c.iter().map(|&i| vec![grown(pieces[i].poly.iter().copied(), fit.radius).into_iter().map(|(x, y)| DVec2::new(x as f64, y as f64) / planar::GRID).collect()]).collect::<Vec<_>>()).into_iter().flatten().collect();
        let mut blocked: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (k, rings) in block_rings.iter().enumerate() {
            let (lo, hi) = bounds(&rings[0]);
            for c in cells(lo, hi, 0.0) {
                blocked.entry(c).or_default().push(k);
            }
        }
        let mut keep_cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, k) in keeps.iter().enumerate() {
            for c in cells(k.lo, k.hi, 0.0) {
                keep_cells.entry(c).or_default().push(i as u32);
            }
        }
        lap("obstacles", &mut times);
        // Each piece walked on less the regions touching it, in convex
        // parts (in order: the same parts whatever the threads).
        let walked: Vec<usize> = (0..pieces.len()).filter(|&i| !pieces[i].hole && !solid[i]).collect();
        // Each piece's stretches (in its order round, from `stretches`).
        let mut first = vec![0usize; pieces.len() + 1];
        for e in &edges {
            first[e.piece + 1] += 1;
        }
        for i in 0..pieces.len() {
            first[i + 1] += first[i];
        }
        let made: Vec<(Vec<(u32, Vec<Corner>)>, usize)> = par_chunks(&walked, |c| {
            let mut near: Vec<u32> = Vec::new();
            let mut straight: Vec<(&Keep, usize)> = Vec::new();
            let mut walk = Walk::default();
            let (mut out, mut touches) = (Vec::new(), 0);
            for &i in c {
                let p = &pieces[i];
                let (lo, hi) = bounds(&p.poly);
                near.clear();
                near.extend(cells(lo, hi, 0.0).flat_map(|c| keep_cells.get(&c).into_iter().flatten().copied()));
                near.sort_unstable();
                near.dedup();
                // A wall along a whole side of its (every stretch on one
                // line, stitched into many edges or not), by a straight cut
                // where that is exact (see `walking`), its band left out.
                straight.clear();
                let own = &edges[first[i]..first[i + 1]];
                let m = own.len();
                let o = p.poly.iter().fold(DVec2::ZERO, |s, q| s + *q) / p.poly.len() as f64;
                if let Some(start) = (0..m).find(|&j| own[j].line != own[(j + m - 1) % m].line) {
                    let mut j = start;
                    loop {
                        let mut end = j;
                        while own[(end + 1) % m].line == own[j].line && (end + 1) % m != start {
                            end = (end + 1) % m;
                        }
                        let next = (end + 1) % m;
                        let side = if j <= end { &own[j..=end] } else { &[] };
                        let (before, after) = (&own[(j + m - 1) % m], &own[next]);
                        if !side.is_empty()
                            && side.iter().all(|e| !e.open)
                            && let Some(w) = wall_along(own[j].line, own[j].a, own[end].b, &on_line, &dirs, &walls)
                        {
                            let (wa, wb) = walls[w as usize];
                            // At each end of the side, exact if the piece's
                            // edge there (on another line) is a wall too:
                            // within the convex piece a point's nearest
                            // wall is across from it. Or if the wall goes
                            // on past the end as far as the piece reaches
                            // past it within the band: at a corner of more
                            // than a right angle, the band's width by its
                            // cotangent.
                            let fine = |q: DVec2, from: DVec2, next: &Stretch, far: DVec2| {
                                if !next.open {
                                    return true;
                                }
                                let u = (q - from).normalize();
                                let past = [wa, wb].iter().map(|x| (*x - q).dot(u)).fold(f64::NEG_INFINITY, f64::max);
                                let v = (far - q).normalize();
                                let (c, s) = (v.dot(u), v.perp_dot(u).abs());
                                c <= 0.0 || past * s >= band_reach() * fit.radius * c
                            };
                            let (sa, sb) = (own[j].a, own[end].b);
                            if fine(sa, sb, before, before.a) && fine(sb, sa, after, after.b) {
                                // Its band's side facing the piece, along
                                // the wall.
                                let keep = &keeps[w as usize];
                                let along = (wb - wa).normalize_or_zero();
                                let r = keep.ring.len();
                                if let Some(k) = (0..r).find(|&k| {
                                    let d = (keep.ring[(k + 1) % r] - keep.ring[k]).normalize_or_zero();
                                    d.perp_dot(along).abs() < 1e-3 && keep.facing[k].0.dot(o) - keep.facing[k].1 < 0.0
                                }) {
                                    straight.push((keep, k));
                                    near.retain(|&x| x != w);
                                }
                            }
                        }
                        j = next;
                        if j == start {
                            break;
                        }
                    }
                }
                let (parts, touched) = walking(&mut walk, &p.corners, &straight, near.iter().map(|&k| &keeps[k as usize]), world);
                touches += touched;
                out.extend(parts.into_iter().map(|part| (i as u32, part)));
            }
            (out, touches)
        });
        let touches = made.iter().map(|m| m.1).sum();
        let (mut piece_of, corners): (Vec<u32>, Vec<Vec<Corner>>) = made.into_iter().flat_map(|m| m.0).unzip();
        lap("walking", &mut times);
        // Nodes: parts beside one another, joined. Two parts are beside one
        // another where edges of theirs lie on one line, by what it is, on
        // its two sides, their spans along it overlapping: every edge, by
        // its line and how far along it, sorted once, and each line's two
        // sides walked together (`along_line`). Where they overlap, a
        // portal; where an edge has none across it, the node's rim. (Each
        // part stitched with the corners of those beside it and its edges'
        // twins found by their corners' keys: a hash of every corner, 0.5 s
        // in the stacks, for what one sort says.)
        let mut ids: HashMap<Line, u32> = HashMap::new();
        let mut places: Vec<(DVec2, DVec2)> = Vec::new();
        let mut along: Vec<Along> = Vec::with_capacity(corners.iter().map(|c| c.len()).sum());
        for (i, c) in corners.iter().enumerate() {
            let n = c.len();
            for k in 0..n {
                let (a, b) = (c[k].at, c[(k + 1) % n].at);
                let id = *ids.entry(c[k].next).or_insert_with(|| {
                    let (p, m) = world.line(c[k].next);
                    places.push((p, m.perp()));
                    places.len() as u32 - 1
                });
                let (p, d) = places[id as usize];
                let (ta, tb) = ((a - p).dot(d), (b - p).dot(d));
                if ta != tb {
                    let up = ta < tb;
                    along.push(if up { Along { line: id, up, t0: ta, t1: tb, p0: a, p1: b, part: i as u32 } } else { Along { line: id, up, t0: tb, t1: ta, p0: b, p1: a, part: i as u32 } });
                }
            }
        }
        along.sort_unstable_by(|x, y| x.line.cmp(&y.line).then(x.up.cmp(&y.up)).then(x.t0.total_cmp(&y.t0)));
        let mut parent: Vec<u32> = (0..corners.len() as u32).collect();
        fn find(p: &mut [u32], mut i: u32) -> u32 {
            while p[i as usize] != i {
                p[i as usize] = p[p[i as usize] as usize];
                i = p[i as usize];
            }
            i
        }
        let mut portals: Vec<Vec<(u32, DVec2, DVec2)>> = vec![Vec::new(); corners.len()];
        let mut rims_of: Vec<Rim> = Vec::new();
        let mut overlaps = 0;
        let mut start = 0;
        while start < along.len() {
            let mut end = start + 1;
            while end < along.len() && along[end].line == along[start].line {
                end += 1;
            }
            let run = &along[start..end];
            let split = run.partition_point(|e| !e.up);
            let (down, up) = run.split_at(split);
            for side in [down, up] {
                overlaps += side.windows(2).filter(|w| w[0].t1 > w[1].t0 + 1e-9).count();
            }
            along_line(down, up, &mut |e, o, x, y| {
                let (a, b) = (find(&mut parent, e.part), find(&mut parent, o.part));
                parent[a as usize] = b;
                // (Each part's way across, in its own order round.)
                portals[e.part as usize].push((o.part, if e.up { x } else { y }, if e.up { y } else { x }));
                portals[o.part as usize].push((e.part, if o.up { x } else { y }, if o.up { y } else { x }));
            }, &mut rims_of);
            along_line(up, down, &mut |_, _, _, _| {}, &mut rims_of);
            start = end;
        }
        let mut parts: Vec<Part> = corners
            .iter()
            .zip(portals)
            .enumerate()
            .map(|(i, (c, mut portals))| {
                // (Beside one part along two edges in a line, its corner
                // between them straight: one way across, end to end.)
                portals.sort_by_key(|p| p.0);
                let mut joined: Vec<(u32, DVec2, DVec2)> = Vec::with_capacity(portals.len());
                for p in portals {
                    match joined.last_mut() {
                        Some(last) if last.0 == p.0 => {
                            let d = last.2 - last.1;
                            let ends = [last.1, last.2, p.1, p.2];
                            let lo = ends.iter().copied().min_by(|x, y| x.dot(d).total_cmp(&y.dot(d))).unwrap();
                            let hi = ends.iter().copied().max_by(|x, y| x.dot(d).total_cmp(&y.dot(d))).unwrap();
                            (last.1, last.2) = (lo, hi);
                        }
                        _ => joined.push(p),
                    }
                }
                let poly = planar::points(c);
                let (lo, hi) = bounds(&poly);
                Part { poly, lo, hi, piece: piece_of[i], node: 0, portals: joined }
            })
            .collect();
        piece_of.clear();
        let mut number: HashMap<u32, u32> = HashMap::new();
        for i in 0..parts.len() {
            let root = find(&mut parent, i as u32);
            let next = number.len() as u32;
            parts[i].node = *number.entry(root).or_insert(next);
        }
        let mut rims: Vec<Vec<Rim>> = vec![Vec::new(); number.len()];
        for r in rims_of {
            rims[parts[r.part as usize].node as usize].push(r);
        }
        let mut space = Space {
            fit,
            pieces,
            edges,
            parts,
            node_area: Vec::new(),
            members: Vec::new(),
            rims,
            part_cells: HashMap::new(),
            rim_cells: HashMap::new(),
            blocked,
            block_rings,
            boxes,
            grid,
            times: Vec::new(),
            hulls,
            walls: wall_count,
            keeps: keeps.len(),
            touches,
            overlaps,
        };
        space.index();
        lap("nodes", &mut times);
        space.times = times;
        space
    }

    /// Each node's parts and area, and the parts and rims by cell.
    fn index(&mut self) {
        let n = self.rims.len();
        self.members = vec![Vec::new(); n];
        self.node_area = vec![0.0; n];
        self.part_cells.clear();
        for (i, p) in self.parts.iter().enumerate() {
            self.members[p.node as usize].push(i as u32);
            self.node_area[p.node as usize] += poly_area(&p.poly);
            for c in cells(p.lo, p.hi, ROUNDING) {
                self.part_cells.entry(c).or_default().push(i as u32);
            }
        }
        self.rim_cells.clear();
        for (node, rim) in self.rims.iter().enumerate() {
            for (k, r) in rim.iter().enumerate() {
                for cell in cells(r.a, r.b, ROUNDING) {
                    self.rim_cells.entry(cell).or_default().push((node as u32, k as u32));
                }
            }
        }
    }

    /// Nodes under `least` m² left out of the walking space, the rest
    /// numbered anew.
    pub(crate) fn prune(&mut self, least: f64) {
        let mut number: Vec<Option<u32>> = vec![None; self.node_area.len()];
        let mut n = 0u32;
        for (k, &a) in self.node_area.iter().enumerate() {
            if a >= least {
                number[k] = Some(n);
                n += 1;
            }
        }
        let mut renumber: Vec<u32> = vec![u32::MAX; self.parts.len()];
        let mut m = 0u32;
        for (i, p) in self.parts.iter().enumerate() {
            if number[p.node as usize].is_some() {
                renumber[i] = m;
                m += 1;
            }
        }
        let parts = std::mem::take(&mut self.parts);
        self.parts = parts
            .into_iter()
            .filter_map(|mut p| {
                p.node = number[p.node as usize]?;
                for x in &mut p.portals {
                    x.0 = renumber[x.0 as usize];
                }
                Some(p)
            })
            .collect();
        let rims = std::mem::take(&mut self.rims);
        self.rims = rims
            .into_iter()
            .enumerate()
            .filter(|(k, _)| number[*k].is_some())
            .map(|(_, r)| r.into_iter().map(|x| Rim { part: renumber[x.part as usize], ..x }).collect())
            .collect();
        self.index();
    }

    /// How many nodes.
    pub(crate) fn nodes(&self) -> usize {
        self.node_area.len()
    }

    /// The piece under `q` (or within `ROUNDING` of it).
    pub(crate) fn under(&self, q: DVec2) -> Option<usize> {
        self.grid.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32))?.iter().copied().find(|&i| in_or_near(&self.pieces[i].poly, q))
    }

    /// How far `q` is from part `i` (0 in it).
    fn off_part(&self, i: u32, q: DVec2) -> f64 {
        let poly = &self.parts[i as usize].poly;
        let n = poly.len();
        if (0..n).all(|k| (poly[(k + 1) % n] - poly[k]).perp_dot(q - poly[k]) >= 0.0) {
            return 0.0;
        }
        (0..n).map(|k| segment_distance(q, poly[k], poly[(k + 1) % n])).fold(f64::INFINITY, f64::min)
    }

    /// The parts listed in `q`'s cell.
    fn parts_at(&self, q: DVec2) -> &[u32] {
        self.part_cells.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32)).map_or(&[], |v| v.as_slice())
    }

    /// The node whose walking space `q` is on (or within `ROUNDING` of).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn node_at(&self, q: DVec2) -> Option<u32> {
        self.node_at_preferring(q, None)
    }

    /// As `node_at`, and where `q` is on the edge of two nodes, `prefer`, if
    /// it is one. (A walker just landed on the edge of a node was taken for
    /// on the other, and could go nowhere from there.)
    pub(crate) fn node_at_preferring(&self, q: DVec2, prefer: Option<u32>) -> Option<u32> {
        let mut found = self.parts_at(q).iter().copied().filter(|&i| self.off_part(i, q) <= ROUNDING).map(|i| self.parts[i as usize].node);
        let first = found.next()?;
        if prefer.is_none_or(|n| n == first) {
            return Some(first);
        }
        Some(found.find(|&n| Some(n) == prefer).unwrap_or(first))
    }

    /// Whether `q` is on node `n` (within `ROUNDING`).
    pub(crate) fn on_node(&self, q: DVec2, n: u32) -> bool {
        self.parts_at(q).iter().any(|&i| self.parts[i as usize].node == n && self.off_part(i, q) <= ROUNDING)
    }

    /// The part of node `n` `q` is on (within `ROUNDING`), and `q`; else
    /// the nearest point of the node within `reach`, and its part.
    fn sample(&self, n: u32, q: DVec2, reach: f64) -> Option<(u32, DVec2)> {
        if let Some(&i) = self.parts_at(q).iter().find(|&&i| self.parts[i as usize].node == n && self.off_part(i, q) <= ROUNDING) {
            return Some((i, q));
        }
        let mut best: Option<(f64, u32, DVec2)> = None;
        for c in cells(q, q, reach) {
            for &(node, k) in self.rim_cells.get(&c).into_iter().flatten() {
                if node != n {
                    continue;
                }
                let r = self.rims[n as usize][k as usize];
                let p = segment_nearest(q, r.a, r.b);
                let d = p.distance(q);
                if d <= reach && best.is_none_or(|x| (d, r.part) < (x.0, x.1)) {
                    best = Some((d, r.part, p));
                }
            }
        }
        best.map(|(_, i, p)| (i, p))
    }

    /// The nearest point of node `n` to `q`, if within `reach`, and how far.
    pub(crate) fn nearest_on(&self, n: u32, q: DVec2, reach: f64) -> Option<(f64, DVec2)> {
        self.sample(n, q, reach).map(|(_, p)| (p.distance(q), p))
    }

    /// Each node within `reach` of `q`, its nearest point to it and how
    /// far, nearest first.
    pub(crate) fn near_nodes(&self, q: DVec2, reach: f64) -> Vec<(u32, f64, DVec2)> {
        let mut best: HashMap<u32, (f64, DVec2)> = HashMap::new();
        for &i in self.parts_at(q) {
            if self.off_part(i, q) == 0.0 {
                best.insert(self.parts[i as usize].node, (0.0, q));
            }
        }
        for c in cells(q, q, reach) {
            for &(node, k) in self.rim_cells.get(&c).into_iter().flatten() {
                let r = self.rims[node as usize][k as usize];
                let p = segment_nearest(q, r.a, r.b);
                let d = p.distance(q);
                if d <= reach && best.get(&node).is_none_or(|x| d < x.0) {
                    best.insert(node, (d, p));
                }
            }
        }
        let mut out: Vec<(u32, f64, DVec2)> = best.into_iter().map(|(n, (d, p))| (n, d, p)).collect();
        out.sort_by(|x, y| x.1.total_cmp(&y.1).then(x.0.cmp(&y.0)));
        out
    }

    /// The parts whose cells meet the box `lo`..`hi`, each once, with their
    /// nodes and middles.
    pub(crate) fn parts_in(&self, lo: DVec2, hi: DVec2) -> Vec<(u32, DVec2)> {
        let mut ps: Vec<u32> = cells(lo, hi, 0.0).flat_map(|c| self.part_cells.get(&c).into_iter().flatten().copied()).collect();
        ps.sort_unstable();
        ps.dedup();
        ps.into_iter()
            .map(|i| {
                let p = &self.parts[i as usize];
                (p.node, p.poly.iter().copied().sum::<DVec2>() / p.poly.len() as f64)
            })
            .collect()
    }

    /// How many parts node `n`'s parts are in, joined across their shared
    /// edges: one, by construction (`node_reach` checks).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn node_pieces(&self, n: u32) -> usize {
        let members = &self.members[n as usize];
        let mut seen: HashMap<u32, ()> = HashMap::new();
        let mut count = 0;
        for &start in members {
            if seen.insert(start, ()).is_some() {
                continue;
            }
            count += 1;
            let mut stack = vec![start];
            while let Some(i) = stack.pop() {
                for &(j, _, _) in &self.parts[i as usize].portals {
                    if self.parts[j as usize].node == n && seen.insert(j, ()).is_none() {
                        stack.push(j);
                    }
                }
            }
        }
        count
    }

    /// What lies within `pad` of the box `lo`..`hi`: the pieces and the
    /// pillars' blocks whose bounds meet it, each once, in order. (Found
    /// once for a drop's every ray and fall, not for each: its landing
    /// spots and falls were most of the drops' work.)
    pub(crate) fn about(&self, lo: DVec2, hi: DVec2, pad: f64) -> About {
        let (lo, hi) = (lo - pad, hi + pad);
        let meets = |b: &(DVec2, DVec2)| b.0.cmple(hi).all() && b.1.cmpge(lo).all();
        let mut pieces: Vec<usize> = cells(lo, hi, 0.0).flat_map(|c| self.grid.get(&c).into_iter().flatten().copied()).filter(|&i| meets(&self.boxes[i])).collect();
        pieces.sort_unstable();
        pieces.dedup();
        let mut blocks: Vec<usize> = cells(lo, hi, 0.0).flat_map(|c| self.blocked.get(&c).into_iter().flatten().copied()).collect();
        blocks.sort_unstable();
        blocks.dedup();
        About { pieces, blocks }
    }

    /// The piece under `q` (as `under`), among `about`'s.
    pub(crate) fn under_in(&self, about: &About, q: DVec2) -> Option<usize> {
        about.pieces.iter().copied().filter(|&i| self.boxes[i].0.cmple(q + ROUNDING).all() && self.boxes[i].1.cmpge(q - ROUNDING).all()).find(|&i| in_or_near(&self.pieces[i].poly, q))
    }

    /// Node `n`'s parts whose bounds meet the box `lo`..`hi`, each once.
    pub(crate) fn node_parts_in(&self, n: u32, lo: DVec2, hi: DVec2, out: &mut Vec<u32>) {
        out.clear();
        out.extend(cells(lo, hi, 0.0).flat_map(|c| self.part_cells.get(&c).into_iter().flatten().copied()).filter(|&i| {
            let p = &self.parts[i as usize];
            p.node == n && p.lo.cmple(hi).all() && p.hi.cmpge(lo).all()
        }));
        out.sort_unstable();
        out.dedup();
    }

    /// Where along `a` + `dir` s (`dir` of length 1), s from `s0` to `s1`,
    /// the parts `parts` (one node's, as `node_parts_in` finds them) are,
    /// each grown by `ROUNDING` at its ends (as a point within it of a node
    /// is on it): their spans, merged, in order, into `out`.
    pub(crate) fn spans_along(&self, parts: &[u32], a: DVec2, dir: DVec2, s0: f64, s1: f64, out: &mut Vec<(f64, f64)>) {
        out.clear();
        let (e0, e1) = (a + dir * s0, a + dir * s1);
        let (lo, hi) = (e0.min(e1) - ROUNDING, e0.max(e1) + ROUNDING);
        for &i in parts {
            let p = &self.parts[i as usize];
            if !(p.lo.cmple(hi).all() && p.hi.cmpge(lo).all()) {
                continue;
            }
            // The line's part inside it (each edge's half-plane).
            let (mut u0, mut u1) = (s0, s1);
            let mut empty = false;
            let n = p.poly.len();
            for k in 0..n {
                let (x, y) = (p.poly[k], p.poly[(k + 1) % n]);
                let m = (y - x).perp();
                let (c, v) = ((a - x).dot(m), dir.dot(m));
                if v.abs() < 1e-15 {
                    if c < 0.0 {
                        empty = true;
                        break;
                    }
                    continue;
                }
                let s = -c / v;
                if v > 0.0 {
                    u0 = u0.max(s);
                } else {
                    u1 = u1.min(s);
                }
            }
            if !empty && u0 <= u1 {
                out.push(((u0 - ROUNDING).max(s0), (u1 + ROUNDING).min(s1)));
            }
        }
        out.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut m = 0;
        for j in 0..out.len() {
            if m > 0 && out[j].0 <= out[m - 1].1 {
                out[m - 1].1 = out[m - 1].1.max(out[j].1);
            } else {
                out[m] = out[j];
                m += 1;
            }
        }
        out.truncate(m);
    }

    /// The way across node `n`'s walking space from `from` to `to`: its
    /// corners, `from` first and `to` last (each, if off the node, its
    /// nearest point within `WAY_REACH`). On the parts as they are: the
    /// fewest metres from part to part across their shared edges (A*, by
    /// the edges' middles), then the shortest line through those edges (the
    /// funnel), its corners the parts' own. Exact: nothing rounded.
    pub(crate) fn way(&self, n: u32, from: DVec2, to: DVec2) -> Result<Vec<DVec2>, String> {
        let (pa, a) = self.sample(n, from, WAY_REACH).ok_or_else(|| format!("from {from:.2} off node {n}"))?;
        let (pb, b) = self.sample(n, to, WAY_REACH).ok_or_else(|| format!("to {to:.2} off node {n}"))?;
        if pa == pb {
            return Ok(vec![a, b]);
        }
        // (Each part's best cost, the part it came from, and where it was
        // entered.)
        let mut came: HashMap<u32, (f64, u32, DVec2)> = HashMap::new();
        came.insert(pa, (0.0, pa, a));
        let mut open = std::collections::BinaryHeap::new();
        let key = |f: f64| std::cmp::Reverse(f.to_bits());
        open.push((key(a.distance(b)), pa));
        let mut done = false;
        while let Some((_, i)) = open.pop() {
            if i == pb {
                done = true;
                break;
            }
            let (g, _, at) = came[&i];
            for &(j, x, y) in &self.parts[i as usize].portals {
                let enter = (x + y) / 2.0;
                let gj = g + at.distance(enter);
                if came.get(&j).is_none_or(|c| gj < c.0) {
                    came.insert(j, (gj, i, enter));
                    open.push((key(gj + enter.distance(b)), j));
                }
            }
        }
        if !done {
            return Err(format!("no way across node {n} from part {pa} to {pb}"));
        }
        let mut chain = vec![pb];
        while *chain.last().unwrap() != pa {
            chain.push(came[chain.last().unwrap()].1);
        }
        chain.reverse();
        // Each edge crossed, its left and right as one goes across.
        let mut gates: Vec<(DVec2, DVec2)> = vec![(a, a)];
        for w in chain.windows(2) {
            let &(_, x, y) = self.parts[w[0] as usize].portals.iter().find(|p| p.0 == w[1]).expect("a portal between parts on the way");
            gates.push((y, x));
        }
        gates.push((b, b));
        Ok(funnel(&gates))
    }

    /// The hops between nodes no more than `far` apart (3D): points every
    /// metre round each node's edges, each pair of nodes' nearest in each
    /// `HOP_CELL` of the lower-numbered node's side; keyed by the pair, the
    /// lower first, each hop from it.
    pub(crate) fn hops(&self, world: &PlateWorld, far: f64) -> HashMap<(u32, u32), Vec<Hop>> {
        let points = self.edge_points(world);
        // (Each cell's points in it, side by side, and its neighbours found
        // once a cell, not once a point: 3 million lookups in the stacks.)
        let mut cell: HashMap<(i32, i32), Vec<(DVec2, f64, u32)>> = HashMap::new();
        for p in &points {
            cell.entry(((p.0.x / far).floor() as i32, (p.0.y / far).floor() as i32)).or_default().push(*p);
        }
        let keys: Vec<(i32, i32)> = cell.keys().copied().collect();
        type Best = HashMap<((u32, u32), (i32, i32)), Hop>;
        let keep = |out: &mut Best, key: ((u32, u32), (i32, i32)), hop: Hop| {
            // (Ties by where: the same whatever the threads, as chunked.)
            let e = out.entry(key).or_insert(hop);
            if (hop.d, hop.a.x, hop.a.y, hop.b.x, hop.b.y) < (e.d, e.a.x, e.a.y, e.b.x, e.b.y) {
                *e = hop;
            }
        };
        let best: Best = {
            let cell = &cell;
            let handles = par_chunks(&keys, |c| {
                        let mut out: Best = HashMap::new();
                        for &(cx, cz) in c {
                            let near: Vec<&Vec<(DVec2, f64, u32)>> = (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (cx + dx, cz + dz))).filter_map(|k| cell.get(&k)).collect();
                            let mut best: Vec<Hop> = Vec::new();
                            let mut to: Vec<u32> = Vec::new();
                            for &pa in &cell[&(cx, cz)] {
                                // (The nearest of each node from this point,
                                // then kept: kept pair by pair, most of it.)
                                best.clear();
                                to.clear();
                                for &pb in near.iter().flat_map(|o| o.iter()) {
                                    if pb.2 <= pa.2 {
                                        continue;
                                    }
                                    let d = (pa.0.distance_squared(pb.0) + (pa.1 - pb.1).powi(2)).sqrt();
                                    if d > far {
                                        continue;
                                    }
                                    let hop = Hop { a: pa.0, ay: pa.1 as f32, b: pb.0, by: pb.1 as f32, d: d as f32, leap: None };
                                    match to.iter().position(|&n| n == pb.2) {
                                        Some(k) => {
                                            if (hop.d, hop.b.x, hop.b.y) < (best[k].d, best[k].b.x, best[k].b.y) {
                                                best[k] = hop;
                                            }
                                        }
                                        None => {
                                            to.push(pb.2);
                                            best.push(hop);
                                        }
                                    }
                                }
                                let at = ((pa.0.x / HOP_CELL).floor() as i32, (pa.0.y / HOP_CELL).floor() as i32);
                                for (&n, &hop) in to.iter().zip(&best) {
                                    keep(&mut out, ((pa.2, n), at), hop);
                                }
                            }
                        }
                        out
            });
            let mut all: Best = HashMap::new();
            for h in handles {
                for (k, hop) in h {
                    keep(&mut all, k, hop);
                }
            }
            all
        };
        let mut out: HashMap<(u32, u32), Vec<Hop>> = HashMap::new();
        for ((pair, _), hop) in best {
            out.entry(pair).or_default().push(hop);
        }
        out
    }

    /// Points every metre round each node's edge: where, how high (on the
    /// piece of the part it bounds), and its node.
    pub(crate) fn edge_points(&self, world: &PlateWorld) -> Vec<(DVec2, f64, u32)> {
        let mut points: Vec<(DVec2, f64, u32)> = Vec::new();
        for (n, rim) in self.rims.iter().enumerate() {
            for r in rim {
                let m = r.a.distance(r.b).ceil().max(1.0) as usize;
                let plate = &self.pieces[self.parts[r.part as usize].piece as usize].plate;
                for j in 0..m {
                    let q = r.a.lerp(r.b, j as f64 / m as f64);
                    points.push((q, world.plate_top(plate, q), n as u32));
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
        if points.is_empty() {
            return HashMap::new();
        }
        // (Each cell's points side by side in one list, sorted by cell, then
        // node: runs of one node; a point's own node's run passed over
        // whole. A grid of cells counted out, not a map of lists: a map, a
        // seventh of the work.)
        let (lo, hi) = points.iter().fold((DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), p| (lo.min(p.0), hi.max(p.0)));
        let (wide, high) = (((hi.x - lo.x) / far).floor() as usize + 1, ((hi.y - lo.y) / far).floor() as usize + 1);
        let at = |q: DVec2| ((q.x - lo.x) / far).floor() as usize + wide * ((q.y - lo.y) / far).floor() as usize;
        let mut first = vec![0usize; wide * high + 1];
        for p in &points {
            first[at(p.0) + 1] += 1;
        }
        for k in 0..wide * high {
            first[k + 1] += first[k];
        }
        let mut list: Vec<(DVec2, f64, u32)> = vec![(DVec2::ZERO, 0.0, 0); points.len()];
        {
            let mut fill = first.clone();
            for p in &points {
                let c = at(p.0);
                list[fill[c]] = *p;
                fill[c] += 1;
            }
        }
        // Runs of one node, each cell's from `first`.
        let mut runs: Vec<(usize, usize, u32)> = Vec::new();
        let mut run_first = vec![0usize; wide * high + 1];
        for c in 0..wide * high {
            let cell = &mut list[first[c]..first[c + 1]];
            cell.sort_by_key(|p| p.2);
            let base = first[c];
            let mut start = 0;
            for k in 1..=cell.len() {
                if k == cell.len() || cell[k].2 != cell[start].2 {
                    runs.push((base + start, base + k, cell[start].2));
                    start = k;
                }
            }
            run_first[c + 1] = runs.len();
        }
        let filled: Vec<usize> = (0..wide * high).filter(|&c| first[c + 1] > first[c]).collect();
        type Best = HashMap<((u32, u32), (i32, i32)), Link>;
        let keep = |out: &mut Best, pair: (u32, u32), l: Link| {
            let at = spread.map_or((0, 0), |c| ((l.a.x / c).floor() as i32, (l.a.y / c).floor() as i32));
            // (Ties by where: the same whatever the threads, as chunked.)
            let e = out.entry((pair, at)).or_insert(l);
            if (l.short, l.a.x, l.a.y, l.b.x, l.b.y) < (e.short, e.a.x, e.a.y, e.b.x, e.b.y) {
                *e = l;
            }
        };
        let far2 = far * far;
        let (list, runs, run_first, short) = (&list, &runs, &run_first, &short);
        let handles = par_chunks(&filled, |c| {
                    let mut out: Best = HashMap::new();
                    let mut best: Vec<Link> = Vec::new();
                    let mut to: Vec<u32> = Vec::new();
                    let mut near: Vec<usize> = Vec::new();
                    for &cell in c {
                        let (cx, cz) = (cell % wide, cell / wide);
                        near.clear();
                        for z in cz.saturating_sub(1)..=(cz + 1).min(high - 1) {
                            for x in cx.saturating_sub(1)..=(cx + 1).min(wide - 1) {
                                near.extend(run_first[z * wide + x]..run_first[z * wide + x + 1]);
                            }
                        }
                        for &(a0, a1, na) in &runs[run_first[cell]..run_first[cell + 1]] {
                            for &pa in &list[a0..a1] {
                                // (The least of each node from this point,
                                // then kept: kept pair by pair, most of it.)
                                best.clear();
                                to.clear();
                                for &r in &near {
                                    let (b0, b1, nb) = runs[r];
                                    if nb == na {
                                        continue;
                                    }
                                    let mut least: Option<Link> = None;
                                    for &pb in &list[b0..b1] {
                                        let d2 = pa.0.distance_squared(pb.0);
                                        if d2 > far2 {
                                            continue;
                                        }
                                        // (No move, none kept: most pairs.)
                                        let s = short(d2.sqrt(), pb.1 - pa.1);
                                        if !s.is_finite() {
                                            continue;
                                        }
                                        if least.is_none_or(|l| (s, pb.0.x, pb.0.y) < (l.short, l.b.x, l.b.y)) {
                                            least = Some(Link { short: s, a: pa.0, ay: pa.1, b: pb.0, by: pb.1 });
                                        }
                                    }
                                    let Some(l) = least else { continue };
                                    match to.iter().position(|&n| n == nb) {
                                        Some(k) => {
                                            if (l.short, l.b.x, l.b.y) < (best[k].short, best[k].b.x, best[k].b.y) {
                                                best[k] = l;
                                            }
                                        }
                                        None => {
                                            to.push(nb);
                                            best.push(l);
                                        }
                                    }
                                }
                                for (&n, &l) in to.iter().zip(&best) {
                                    keep(&mut out, (na, n), l);
                                }
                            }
                        }
                    }
                    out
        });
        let mut all: Best = HashMap::new();
        for h in handles {
            for ((pair, _), l) in h {
                keep(&mut all, pair, l);
            }
        }
        let mut out: HashMap<(u32, u32), Vec<Link>> = HashMap::new();
        for ((pair, _), l) in all {
            out.entry(pair).or_default().push(l);
        }
        out
    }
}

/// How far off its node a way's ends are brought onto it (m).
const WAY_REACH: f64 = 0.5;

/// The shortest line through `gates` (each its left and right as one goes
/// through, the first and last a point: where from and to): its corners,
/// the first and last those points (the funnel: its apex, and the left and
/// right sides of what it sees, narrowed gate by gate; a side crossing the
/// other is a corner, from which it looks on).
fn funnel(gates: &[(DVec2, DVec2)]) -> Vec<DVec2> {
    let start = gates[0].0;
    let mut path = vec![start];
    let (mut apex, mut left, mut right) = (start, start, start);
    let (mut li, mut ri) = (0, 0);
    let mut ai;
    let mut i = 1;
    while i < gates.len() {
        let (pl, pr) = gates[i];
        // The right side, narrowed to `pr` if it is no further right.
        if (right - apex).perp_dot(pr - apex) >= 0.0 {
            if apex == right || (left - apex).perp_dot(pr - apex) < 0.0 {
                right = pr;
                ri = i;
            } else {
                // Over the left side: its end a corner, looked on from.
                if path.last() != Some(&left) {
                    path.push(left);
                }
                (apex, ai) = (left, li);
                (right, ri, left, li) = (apex, ai, apex, ai);
                i = ai + 1;
                continue;
            }
        }
        if (left - apex).perp_dot(pl - apex) <= 0.0 {
            if apex == left || (right - apex).perp_dot(pl - apex) > 0.0 {
                left = pl;
                li = i;
            } else {
                if path.last() != Some(&right) {
                    path.push(right);
                }
                (apex, ai) = (right, ri);
                (right, ri, left, li) = (apex, ai, apex, ai);
                i = ai + 1;
                continue;
            }
        }
        i += 1;
    }
    let end = gates[gates.len() - 1].0;
    if path.last() != Some(&end) {
        path.push(end);
    }
    path
}

/// A polygon's area (counter-clockwise, positive).
fn poly_area(poly: &[DVec2]) -> f64 {
    let o = poly[0];
    (1..poly.len().saturating_sub(1)).map(|k| (poly[k] - o).perp_dot(poly[k + 1] - o)).sum::<f64>() / 2.0
}

/// The nearest point of segment `a`-`b` to `q`.
fn segment_nearest(q: DVec2, a: DVec2, b: DVec2) -> DVec2 {
    let d = b - a;
    let t = if d.length_squared() > 0.0 { ((q - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    a + d * t
}

/// How far `q` is from segment `a`-`b`.
fn segment_distance(q: DVec2, a: DVec2, b: DVec2) -> f64 {
    segment_nearest(q, a, b).distance(q)
}

/// Whether `q` is in polygon `poly` (either way round) or within
/// `ROUNDING` of it.
fn in_or_near(poly: &[DVec2], q: DVec2) -> bool {
    let n = poly.len();
    let mut odd = false;
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (b.x - a.x) * (q.y - a.y) / (b.y - a.y) {
            odd = !odd;
        }
    }
    odd || (0..n).any(|k| segment_distance(q, poly[k], poly[(k + 1) % n]) <= ROUNDING)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A free-standing polygon's corners, each edge its own line (a grid
    /// line through its corners on the grid), placed by `GridOnly`.
    fn free(points: &[(i64, i64)]) -> Vec<Corner> {
        let n = points.len();
        let line = |k: usize| Line::grid(points[k % n], points[(k + 1) % n]).0;
        (0..n).map(|k| Corner::new(DVec2::new(points[k].0 as f64, points[k].1 as f64) / planar::GRID, Key::of(line(k + n - 1), line(k)), line(k))).collect()
    }

    /// A cell less many keep-out regions overlapping one another and its
    /// edges (bands of walls along it and across it, round feet), by the
    /// definition: every part convex and counter-clockwise; no two over one
    /// another (each edge once each way round); each part's edges met
    /// corner for corner by the parts beside it or on the cell's edge or a
    /// region's; and a point is in a part exactly when it is in the cell and
    /// in no region (sampled, away from every edge by a micrometre).
    #[test]
    fn parts_as_defined() {
        let g = planar::GRID as i64;
        let cell = free(&[(0, 0), (40 * g, 3 * g), (37 * g, 30 * g), (2 * g, 33 * g)]);
        let mut keeps = Vec::new();
        let mut seed = 7u64;
        let mut rand = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..40 {
            let a = DVec2::new(rand() * 44.0 - 2.0, rand() * 36.0 - 2.0);
            let b = a + DVec2::from_angle(rand() * 6.3) * rand() * 12.0;
            keeps.push(Keep::round([a, b], 0.35));
        }
        for _ in 0..10 {
            let c = DVec2::new(rand() * 40.0, rand() * 32.0);
            keeps.push(Keep::round((0..5).map(|k| c + DVec2::from_angle(k as f64 * 1.3) * (0.5 + rand())), 0.35));
        }
        // Two along one line, and two round one end (their edges on one
        // line each, by what they are).
        keeps.push(Keep::round([DVec2::new(5.0, 10.0), DVec2::new(15.0, 10.0)], 0.35));
        keeps.push(Keep::round([DVec2::new(18.0, 10.0), DVec2::new(25.0, 10.0)], 0.35));
        keeps.push(Keep::round([DVec2::new(30.0, 20.0), DVec2::new(30.0, 28.0)], 0.35));
        keeps.push(Keep::round([DVec2::new(30.0, 20.0), DVec2::new(36.0, 20.0)], 0.35));
        let (mut parts, _) = walking(&mut Walk::default(), &cell, &[], keeps.iter(), &GridOnly);
        planar::stitch(&mut parts, &GridOnly);
        let mut edges: HashMap<(Key, Key), (usize, Line)> = HashMap::new();
        for p in &parts {
            let n = p.len();
            for k in 0..n {
                let (a, b, c) = (p[k].at, p[(k + 1) % n].at, p[(k + 2) % n].at);
                assert!((b - a).perp_dot(c - b) >= -1e-9, "convex, counter-clockwise: {a} {b} {c}");
                edges.entry((p[k].key, p[(k + 1) % n].key)).or_insert((0, p[k].next)).0 += 1;
            }
        }
        assert!(edges.values().all(|c| c.0 == 1), "no two parts over one another");
        for ((a, b), (_, line)) in &edges {
            if !edges.contains_key(&(*b, *a)) {
                assert!(cell.iter().any(|c| c.next == *line) || keeps.iter().any(|kp| kp.lines.iter().any(|x| x.0 == *line)), "an edge with no part beside it on the cell's edge or a region's: {a:?} {b:?} on {line:?}");
            }
        }
        println!("{} parts", parts.len());
        let inside = |poly: &[DVec2], q: DVec2, by: f64| (0..poly.len()).all(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            (b - a).perp_dot(q - a) / (b - a).length() > by
        });
        let cell_at = planar::points(&cell);
        let mut checked = 0;
        for _ in 0..100_000 {
            let q = DVec2::new(rand() * 42.0 - 1.0, rand() * 35.0 - 1.0);
            let near_edge = |poly: &[DVec2]| (0..poly.len()).any(|k| segment_distance(q, poly[k], poly[(k + 1) % poly.len()]) < 1e-6);
            if near_edge(&cell_at) || keeps.iter().any(|k| near_edge(&k.ring)) {
                continue;
            }
            let want = inside(&cell_at, q, 0.0) && keeps.iter().all(|k| !inside(&k.ring, q, 0.0));
            let got = parts.iter().filter(|p| inside(&planar::points(p), q, -1e-9)).count();
            assert_eq!(got, want as usize, "{q}: in {got} parts");
            checked += 1;
        }
        assert!(checked > 90_000);
    }

    /// A keep-out region round a wall: every point within the radius of it
    /// inside, every point further than its round polygon's corners and the
    /// grid's rounding outside; corners on the grid, convex, no three in a
    /// line.
    #[test]
    fn keep_round_as_defined() {
        let (a, b, r) = (DVec2::new(3.1234, -7.77), DVec2::new(9.5, -2.25), 0.35);
        let k = Keep::round([a, b], r);
        let n = k.ring.len();
        for i in 0..n {
            let (p, q, s) = (k.ring[i], k.ring[(i + 1) % n], k.ring[(i + 2) % n]);
            assert!((q - p).perp_dot(s - q) > 0.0, "convex, no three in a line");
            assert_eq!(p * planar::GRID, (p * planar::GRID).round(), "on the grid");
        }
        let inside = |q: DVec2| (0..n).all(|i| (k.ring[(i + 1) % n] - k.ring[i]).perp_dot(q - k.ring[i]) >= 0.0);
        let h = std::f64::consts::FRAC_1_SQRT_2 / planar::GRID;
        let beyond = (r + h) / (std::f64::consts::PI / round_steps() as f64).cos() + h;
        let mut seed = 3u64;
        let (mut ins, mut outs) = (0, 0);
        for _ in 0..200_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let (x, y) = ((seed >> 40) as f64 / (1u64 << 24) as f64, ((seed >> 16) & 0xff_ffff) as f64 / (1u64 << 24) as f64);
            let q = DVec2::new(a.x.min(b.x) - 1.0 + x * ((a.x - b.x).abs() + 2.0), a.y.min(b.y) - 1.0 + y * ((a.y - b.y).abs() + 2.0));
            let d = segment_distance(q, a, b);
            if d <= r {
                assert!(inside(q), "{q} {d} m off: inside");
                ins += 1;
            } else if d > beyond {
                assert!(!inside(q), "{q} {d} m off: outside");
                outs += 1;
            }
        }
        assert!(ins > 1000 && outs > 1000);
    }

    /// Ways across parts: through a bend, round its inner corner, the
    /// corners the parts' own.
    #[test]
    fn funnel_round_a_corner() {
        // An L of three squares: (0..1, 0..1), (1..2, 0..1), (1..2, 1..2).
        let gates = vec![(DVec2::new(0.2, 0.5), DVec2::new(0.2, 0.5)), (DVec2::new(1.0, 1.0), DVec2::new(1.0, 0.0)), (DVec2::new(1.0, 1.0), DVec2::new(2.0, 1.0)), (DVec2::new(1.5, 1.8), DVec2::new(1.5, 1.8))];
        assert_eq!(funnel(&gates), vec![DVec2::new(0.2, 0.5), DVec2::new(1.0, 1.0), DVec2::new(1.5, 1.8)]);
        let straight = vec![(DVec2::new(0.5, 0.5), DVec2::new(0.5, 0.5)), (DVec2::new(1.0, 1.0), DVec2::new(1.0, 0.0)), (DVec2::new(1.5, 0.5), DVec2::new(1.5, 0.5))];
        assert_eq!(funnel(&straight), vec![DVec2::new(0.5, 0.5), DVec2::new(1.5, 0.5)]);
    }
}
