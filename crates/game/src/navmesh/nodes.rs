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

use bevy::platform::collections::HashMap;

use bevy::math::DVec2;
use geo::{Area, BooleanOps, BoundingRect, ConvexHull, Distance, Intersects, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon};
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

/// The parts of segment `a`-`b` further than `r` from every line of
/// `walls`: by each wall's capsule's interval along it (the line through
/// its rectangle, Liang-Barsky, and its two end circles), merged, and what
/// is left. Exact, as geo's band round them clipped to it was, at a
/// fraction of the cost (0.2 ms an open stretch, the join's 1.4 s in the
/// stacks).
fn clear_of(a: DVec2, b: DVec2, walls: &MultiLineString<f64>, r: f64) -> Vec<(DVec2, DVec2)> {
    let d = b - a;
    let len2 = d.length_squared();
    if len2 < 1e-18 {
        return Vec::new();
    }
    let mut cover: Vec<(f64, f64)> = Vec::new();
    for w in &walls.0 {
        let (c, e) = (DVec2::new(w.0[0].x, w.0[0].y), DVec2::new(w.0[1].x, w.0[1].y));
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        // The end circles: |a + t d - c|² = r².
        for q in [c, e] {
            let f = a - q;
            let (qa, qb, qc) = (len2, 2.0 * f.dot(d), f.length_squared() - r * r);
            let disc = qb * qb - 4.0 * qa * qc;
            if disc >= 0.0 {
                let s = disc.sqrt();
                lo = lo.min((-qb - s) / (2.0 * qa));
                hi = hi.max((-qb + s) / (2.0 * qa));
            }
        }
        // The rectangle: along the wall within its length, across within r.
        let u = e - c;
        let ul = u.length();
        if ul > 1e-12 {
            let (ux, n) = (u / ul, (u / ul).perp());
            let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
            let mut inside = true;
            for (axis, min, max) in [(ux, 0.0, ul), (n, -r, r)] {
                let (p, v) = ((a - c).dot(axis), d.dot(axis));
                if v.abs() < 1e-15 {
                    if p < min || p > max {
                        inside = false;
                    }
                } else {
                    let (s0, s1) = ((min - p) / v, (max - p) / v);
                    t0 = t0.max(s0.min(s1));
                    t1 = t1.min(s0.max(s1));
                }
            }
            if inside && t0 <= t1 {
                lo = lo.min(t0);
                hi = hi.max(t1);
            }
        }
        if lo < hi && hi > 0.0 && lo < 1.0 {
            cover.push((lo.max(0.0), hi.min(1.0)));
        }
    }
    cover.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = Vec::new();
    let mut at = 0.0;
    for (lo, hi) in cover {
        if lo > at {
            out.push((a + d * at, a + d * lo));
        }
        at = at.max(hi);
    }
    if at < 1.0 {
        out.push((a + d * at, b));
    }
    out
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


/// How far two fragments either side of an open stretch must meet along it
/// to be one node (m): a gap a body's centre passes (its radius already
/// taken off), five times what a node's outline is made simpler by (2 mm,
/// `district::outline`), so it is kept in its navmesh. (Found every 10 cm
/// before, a meeting shorter than that joined or not by where the points
/// fell: 134 in the tiers did not.)
const MEET: f64 = 0.01;

/// The parts of the line `a` + `dir` s (`dir` of length 1), s from `s0` to
/// `s1`, inside `f`, each grown by `pad` at its ends: where it crosses `f`'s
/// rings, inside between each pair; pushed onto `spans` (`cross` a buffer).
pub(crate) fn spans_in(f: &Polygon<f64>, a: DVec2, dir: DVec2, s0: f64, s1: f64, pad: f64, cross: &mut Vec<f64>, spans: &mut Vec<(f64, f64)>) {
    cross.clear();
    for ring in std::iter::once(f.exterior()).chain(f.interiors()) {
        for e in ring.lines() {
            let (p, q) = (DVec2::new(e.start.x, e.start.y), DVec2::new(e.end.x, e.end.y));
            let (sp, sq) = (dir.perp_dot(p - a), dir.perp_dot(q - a));
            if (sp > 0.0) != (sq > 0.0) {
                cross.push((p + (q - p) * (sp / (sp - sq)) - a).dot(dir));
            }
        }
    }
    cross.sort_by(f64::total_cmp);
    for w in cross.chunks_exact(2) {
        let (u0, u1) = ((w[0] - pad).max(s0), (w[1] + pad).min(s1));
        if u0 <= u1 {
            spans.push((u0, u1));
        }
    }
}


/// Convex `hulls` (counter-clockwise) less those within another (of two
/// alike, the first kept): what they cover together, the same. (A pillar's
/// stacked slabs, many of one footprint, each a hull to take out: 16 a
/// pillar's piece, most of its walking space's work.)
fn outermost(hulls: Vec<Polygon<f64>>) -> Vec<Polygon<f64>> {
    let pts: Vec<Vec<DVec2>> = hulls.iter().map(|h| ccw(h.exterior().0.iter().map(|c| DVec2::new(c.x, c.y)).collect())).collect();
    let within = |a: &[DVec2], b: &[DVec2]| a.iter().all(|&x| (0..b.len()).all(|k| (b[(k + 1) % b.len()] - b[k]).perp_dot(x - b[k]) >= -1e-9));
    let keep: Vec<bool> = (0..pts.len()).map(|i| !(0..pts.len()).any(|j| j != i && within(&pts[i], &pts[j]) && (j < i || !within(&pts[j], &pts[i])))).collect();
    hulls.into_iter().zip(keep).filter_map(|(h, k)| k.then_some(h)).collect()
}

/// Whether convex `a` (a closed ring, either way round) and convex `b`
/// (counter-clockwise) meet or touch: no edge of either with the other all
/// beyond it. (geo's `intersects`, a pillar's every hull against a piece,
/// was a sixth of the walking space's work in the stacks.)
fn convex_meet(a: &[geo::Coord<f64>], b: &[DVec2]) -> bool {
    let a: Vec<DVec2> = ccw(a.iter().map(|c| DVec2::new(c.x, c.y)).collect());
    let apart = |p: &[DVec2], q: &[DVec2]| (0..p.len()).any(|k| {
        let (u, v) = (p[k], p[(k + 1) % p.len()]);
        q.iter().all(|&x| (v - u).perp_dot(x - u) < 0.0)
    });
    !apart(&a, b) && !apart(b, &a)
}

/// Convex `poly` (counter-clockwise) on the left of line `a`-`b` moved `r`
/// to its left: the part of it further than `r` inside the line.
pub(crate) fn keep_left(poly: &[DVec2], a: DVec2, b: DVec2, r: f64) -> Vec<DVec2> {
    let Some(dir) = (b - a).try_normalize() else { return poly.to_vec() };
    let side = |q: DVec2| dir.perp_dot(q - a) - r;
    let mut out = Vec::with_capacity(poly.len() + 1);
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            out.push(p + (q - p) * (sp / (sp - sq)));
        }
    }
    out
}

/// How near a line a segment's ends lie for it to run along it (m).
const ON_LINE: f64 = 1e-9;
/// How near two ends of the boundary's parts are for one corner (m).
const LINK: f64 = 1e-7;

/// The part of segment `p`-`q` (0 to 1 along it) inside a convex shape
/// given by its edges' `lines` (each a point on it and its direction, of
/// length 1, counter-clockwise), closed: none if none, or only a point. A
/// part running along one of its edges is inside if `same` (the same way
/// round as it) or `opposite`.
fn inside_part(p: DVec2, q: DVec2, lines: &[(DVec2, DVec2)], same: bool, opposite: bool) -> Option<(f64, f64)> {
    let d = q - p;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for &(a, dir) in lines {
        let (s0, s1) = (dir.perp_dot(p - a), dir.perp_dot(q - a));
        if s0.abs() <= ON_LINE && s1.abs() <= ON_LINE {
            if !(if d.dot(dir) > 0.0 { same } else { opposite }) {
                return None;
            }
            continue;
        }
        if s0 < 0.0 && s1 < 0.0 {
            return None;
        }
        if s0 < 0.0 {
            t0 = t0.max(s0 / (s0 - s1));
        } else if s1 < 0.0 {
            t1 = t1.min(s0 / (s0 - s1));
        }
        if t0 >= t1 {
            return None;
        }
    }
    Some((t0, t1))
}

/// `convex_less`'s working space, kept from call to call on each thread.
#[derive(Default)]
struct LessScratch {
    /// Every shape's edges (a point and a direction), `shape`'s first.
    lines: Vec<(DVec2, DVec2)>,
    /// Each shape's corners and edges in them, and its bounds.
    of: Vec<(usize, usize, DVec2, DVec2)>,
    into: Vec<u32>,
    parts: Vec<(DVec2, DVec2)>,
    covered: Vec<(f64, f64)>,
    ends: Vec<(DVec2, u32)>,
    corner: Vec<u32>,
    corners: Vec<DVec2>,
    edges: Vec<(u32, u32)>,
    order: Vec<u32>,
    first: Vec<u32>,
    used: Vec<bool>,
}

/// `convex_less` says why its rings did not close (tests).
#[cfg(test)]
pub(crate) static LESS_DEBUG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

thread_local! {
    static LESS: std::cell::RefCell<LessScratch> = std::cell::RefCell::new(LessScratch::default());
}

/// `shape` (convex, counter-clockwise) less the convex shapes `cut`
/// (each counter-clockwise, its last corner not its first): the walking
/// space's difference, by its boundary. What is left is bounded by the
/// parts of `shape`'s edges no cutter covers and the parts of the cutters'
/// edges inside `shape` that no other cutter covers, these run backwards
/// (what is left on the left of every part); linked end to end into rings,
/// outer ones counter-clockwise, holes clockwise, each hole in the least
/// outer ring round it. Where two shapes run along one line: the same way
/// round, the line is kept once (`shape`'s gives way, then the later
/// cutter's); opposite ways, two cutters meeting there, neither. Exact:
/// its corners where the edges cross (geo's boolean puts them on a grid of
/// about half a millimetre). None if the rings do not close: the caller
/// then makes it by geo's boolean. (geo's boolean, the general case, 20 to
/// 40 µs a piece in the stacks and the broken, was most of the walking
/// space's building; see `walking`.)
pub(crate) fn convex_less(shape: &[DVec2], cut: &[Vec<DVec2>]) -> Option<Vec<Polygon<f64>>> {
    LESS.with(|s| less(&mut s.borrow_mut(), shape, cut))
}

fn less(s: &mut LessScratch, shape: &[DVec2], cut: &[Vec<DVec2>]) -> Option<Vec<Polygon<f64>>> {
    let LessScratch { lines, of, into, parts, covered, ends, corner, corners, edges, order, first, used } = s;
    lines.clear();
    of.clear();
    parts.clear();
    for poly in std::iter::once(shape).chain(cut.iter().map(|c| c.as_slice())) {
        let start = lines.len();
        let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
        let m = poly.len();
        for k in 0..m {
            let (a, b) = (poly[k], poly[(k + 1) % m]);
            lo = lo.min(a);
            hi = hi.max(a);
            if let Some(dir) = (b - a).try_normalize() {
                lines.push((a, dir));
            }
        }
        of.push((start, lines.len(), lo, hi));
    }
    let meets = |a0: DVec2, a1: DVec2, b0: DVec2, b1: DVec2| a0.x <= b1.x + ON_LINE && a0.y <= b1.y + ON_LINE && b0.x <= a1.x + ON_LINE && b0.y <= a1.y + ON_LINE;
    // (The parts of `p`-`q` from `t0` to `t1` not `covered`, kept, run
    // backwards if `back`.)
    let keep = |p: DVec2, q: DVec2, t0: f64, t1: f64, covered: &mut Vec<(f64, f64)>, back: bool, parts: &mut Vec<(DVec2, DVec2)>| {
        covered.sort_unstable_by(|x, y| x.0.total_cmp(&y.0));
        let len = p.distance(q);
        let mut put = |u0: f64, u1: f64| {
            if (u1 - u0) * len > LINK {
                let (a, b) = (p.lerp(q, u0), p.lerp(q, u1));
                parts.push(if back { (b, a) } else { (a, b) });
            }
        };
        let mut at = t0;
        for &(c0, c1) in covered.iter() {
            if c0 > at {
                put(at, c0.min(t1));
            }
            at = at.max(c1);
            if at >= t1 {
                break;
            }
        }
        if at < t1 {
            put(at, t1);
        }
    };
    let (s0, s1, slo, shi) = of[0];
    // `shape`'s edges, less what the cutters cover.
    let n = s1 - s0;
    for k in 0..n {
        let (p, q) = (lines[s0 + k].0, lines[s0 + (k + 1) % n].0);
        let (b0, b1) = (p.min(q), p.max(q));
        covered.clear();
        for &(c0, c1, lo, hi) in &of[1..] {
            if meets(b0, b1, lo, hi)
                && let Some(i) = inside_part(p, q, &lines[c0..c1], true, false)
            {
                covered.push(i);
            }
        }
        keep(p, q, 0.0, 1.0, covered, false, parts);
    }
    // The cutters' edges inside `shape`, less what the others cover.
    for j in 1..of.len() {
        let (c0, c1, lo, hi) = of[j];
        if !meets(lo, hi, slo, shi) {
            continue;
        }
        let m = c1 - c0;
        for k in 0..m {
            let (p, q) = (lines[c0 + k].0, lines[c0 + (k + 1) % m].0);
            let (b0, b1) = (p.min(q), p.max(q));
            if !meets(b0, b1, slo, shi) {
                continue;
            }
            let Some((t0, t1)) = inside_part(p, q, &lines[s0..s1], false, false) else { continue };
            covered.clear();
            for (i, &(o0, o1, olo, ohi)) in of.iter().enumerate().skip(1) {
                if i != j
                    && meets(b0, b1, olo, ohi)
                    && let Some(u) = inside_part(p, q, &lines[o0..o1], j > i, true)
                {
                    covered.push(u);
                }
            }
            keep(p, q, t0, t1, covered, true, parts);
        }
    }
    if parts.is_empty() {
        return Some(Vec::new());
    }
    // Their ends, each the corner it is one with (within `LINK`): by x,
    // each against those before it within `LINK`.
    ends.clear();
    for (k, &(a, b)) in parts.iter().enumerate() {
        ends.push((a, 2 * k as u32));
        ends.push((b, 2 * k as u32 + 1));
    }
    ends.sort_unstable_by(|x, y| x.0.x.total_cmp(&y.0.x));
    corner.clear();
    corner.resize(ends.len(), u32::MAX);
    corners.clear();
    for e in 0..ends.len() {
        let (p, id) = ends[e];
        let mut c = u32::MAX;
        for f in (0..e).rev() {
            let (o, oid) = ends[f];
            if p.x - o.x > LINK {
                break;
            }
            if (p.y - o.y).abs() <= LINK {
                c = corner[oid as usize];
                break;
            }
        }
        if c == u32::MAX {
            corners.push(p);
            c = corners.len() as u32 - 1;
        }
        corner[id as usize] = c;
    }
    edges.clear();
    for k in 0..parts.len() {
        let (a, b) = (corner[2 * k], corner[2 * k + 1]);
        if a != b {
            edges.push((a, b));
        }
    }
    // (A part and one back along it: nothing between them.)
    order.clear();
    order.extend(0..edges.len() as u32);
    order.sort_unstable_by_key(|&e| {
        let (a, b) = edges[e as usize];
        (a.min(b), a.max(b))
    });
    let mut gone = false;
    for w in order.windows(2) {
        let (x, y) = (edges[w[0] as usize], edges[w[1] as usize]);
        if x.0 == y.1 && x.1 == y.0 && x.0 != u32::MAX {
            edges[w[0] as usize] = (u32::MAX, u32::MAX);
            edges[w[1] as usize] = (u32::MAX, u32::MAX);
            gone = true;
        }
    }
    if gone {
        edges.retain(|e| e.0 != u32::MAX);
    }
    // Each corner's parts out (by corner), and as many in as out.
    let n = corners.len();
    first.clear();
    first.resize(n + 1, 0);
    into.clear();
    into.resize(n, 0);
    for &(a, b) in edges.iter() {
        first[a as usize + 1] += 1;
        into[b as usize] += 1;
    }
    for c in 0..n {
        if first[c + 1] != into[c] {
            #[cfg(test)]
            if LESS_DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
                println!("corner {c} at {:?}: {} out, {} in", corners[c], first[c + 1], into[c]);
                for (k, &(a, b)) in parts.iter().enumerate() {
                    println!("  part {k}: {a:?} -> {b:?} ({:.3e} m), corners {} -> {}", a.distance(b), corner[2 * k], corner[2 * k + 1]);
                }
            }
            return None;
        }
        first[c + 1] += first[c];
    }
    order.clear();
    order.extend(0..edges.len() as u32);
    order.sort_unstable_by_key(|&e| edges[e as usize].0);
    // Rings: from each part, on to the one turning furthest left at its
    // end (two pieces meeting at a corner kept apart).
    used.clear();
    used.resize(edges.len(), false);
    let mut rings: Vec<(Vec<DVec2>, f64)> = Vec::new();
    for e0 in 0..edges.len() {
        if used[e0] {
            continue;
        }
        let mut ring: Vec<DVec2> = Vec::new();
        let mut e = e0;
        loop {
            used[e] = true;
            let (a, b) = edges[e];
            ring.push(corners[a as usize]);
            let out = &order[first[b as usize] as usize..first[b as usize + 1] as usize];
            let next = if out.len() == 1 {
                out[0] as usize
            } else {
                let d_in = corners[b as usize] - corners[a as usize];
                let turn = |o: u32| {
                    let (x, y) = edges[o as usize];
                    let d = corners[y as usize] - corners[x as usize];
                    d_in.perp_dot(d).atan2(d_in.dot(d))
                };
                *out.iter().max_by(|&&x, &&y| turn(x).total_cmp(&turn(y)))? as usize
            };
            if next == e0 {
                break;
            }
            if used[next] {
                #[cfg(test)]
                if LESS_DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("ring from part {e0} met a used part {next}");
                }
                return None;
            }
            e = next;
        }
        // (From its first corner: from the origin, 15 km off, the products'
        // rounding was as big as a speck's area, and turned it round.)
        let o = ring[0];
        let area = (1..ring.len().saturating_sub(1)).map(|k| (ring[k] - o).perp_dot(ring[k + 1] - o)).sum::<f64>() / 2.0;
        if area.abs() > 1e-12 {
            rings.push((ring, area));
        }
    }
    // Holes into the least outer ring round them.
    let line = |ring: &[DVec2]| LineString::from(ring.iter().chain(ring.first()).map(|p| (p.x, p.y)).collect::<Vec<_>>());
    if rings.iter().all(|r| r.1 > 0.0) {
        return Some(rings.iter().map(|r| Polygon::new(line(&r.0), Vec::new())).collect());
    }
    let inside = |q: DVec2, ring: &[DVec2]| {
        let mut c = false;
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (b.x - a.x) * (q.y - a.y) / (b.y - a.y) {
                c = !c;
            }
        }
        c
    };
    let outer: Vec<usize> = (0..rings.len()).filter(|&r| rings[r].1 > 0.0).collect();
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); rings.len()];
    for h in (0..rings.len()).filter(|&r| rings[r].1 < 0.0) {
        let ring = &rings[h].0;
        let k = (0..ring.len()).max_by(|&x, &y| ring[x].distance_squared(ring[(x + 1) % ring.len()]).total_cmp(&ring[y].distance_squared(ring[(y + 1) % ring.len()]))).unwrap();
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let q = (a + b) / 2.0 + (b - a).normalize().perp() * 1e-6;
        let Some(o) = outer.iter().copied().filter(|&o| inside(q, &rings[o].0)).min_by(|&x, &y| rings[x].1.total_cmp(&rings[y].1)) else {
            #[cfg(test)]
            if LESS_DEBUG.load(std::sync::atomic::Ordering::Relaxed) {
                println!("hole {h} ({:.3e} m², {} corners) in no outer ring; rings {:?}", rings[h].1, ring.len(), rings.iter().map(|r| (r.1, r.0.len())).collect::<Vec<_>>());
                for (k, &(a, b)) in parts.iter().enumerate() {
                    println!("  part {k}: {a:?} -> {b:?} ({:.3e} m)", a.distance(b));
                }
            }
            return None;
        };
        holes[o].push(h);
    }
    Some(outer.iter().map(|&o| Polygon::new(line(&rings[o].0), holes[o].iter().map(|&h| line(&rings[h].0)).collect())).collect())
}

/// Whether segment `a`-`b` comes within `r` of convex `poly` (counter-
/// clockwise): its band reaches into it.
fn within(poly: &[DVec2], a: DVec2, b: DVec2, r: f64) -> bool {
    let inside = |q: DVec2| (0..poly.len()).all(|k| (poly[(k + 1) % poly.len()] - poly[k]).perp_dot(q - poly[k]) >= 0.0);
    if inside(a) || inside(b) {
        return true;
    }
    let to = |q: DVec2, p0: DVec2, p1: DVec2| {
        let d = p1 - p0;
        let t = if d.length_squared() > 0.0 { ((q - p0).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        q.distance(p0 + d * t)
    };
    (0..poly.len()).any(|k| {
        let (p0, p1) = (poly[k], poly[(k + 1) % poly.len()]);
        // (Crossing, or the nearest of the four ends to the other segment.)
        let (d1, d2) = ((b - a).perp_dot(p0 - a), (b - a).perp_dot(p1 - a));
        let (d3, d4) = ((p1 - p0).perp_dot(a - p0), (p1 - p0).perp_dot(b - p0));
        (d1 * d2 < 0.0 && d3 * d4 < 0.0) || to(a, p0, p1) < r || to(b, p0, p1) < r || to(p0, a, b) < r || to(p1, a, b) < r
    })
}

/// Steps in each half circle of a capsule's ends (`capsule`).
pub(crate) const CAP_STEPS: usize = 6;

/// Segment `a`-`b` grown by `r`, counter-clockwise, its last corner not
/// its first: its sides, each end a half circle in `CAP_STEPS`, its corners
/// just outside the circle (the polygon round it, not in it: never less
/// than `r`). (Each turn round an end from a table: a sine and a cosine
/// for each of its corners, 100 000 capsules in the stacks.)
pub(crate) fn capsule(a: DVec2, b: DVec2, r: f64) -> Vec<DVec2> {
    static TURNS: std::sync::LazyLock<[(f64, f64); CAP_STEPS + 1]> = std::sync::LazyLock::new(|| {
        let k = 1.0 / (std::f64::consts::PI / (2.0 * CAP_STEPS as f64)).cos();
        std::array::from_fn(|j| {
            let t = std::f64::consts::PI * j as f64 / CAP_STEPS as f64;
            let f = if j == 0 || j == CAP_STEPS { 1.0 } else { k };
            (t.cos() * f, t.sin() * f)
        })
    });
    let d = (b - a).normalize_or(DVec2::X) * r;
    let n = d.perp();
    let mut ring = Vec::with_capacity(2 * (CAP_STEPS + 1));
    // (Round `b` from its right to its left, then `a` from its left to its
    // right: counter-clockwise.)
    for (c, from, to) in [(b, -n, d), (a, n, -d)] {
        for &(cos, sin) in TURNS.iter() {
            ring.push(c + from * cos + to * sin);
        }
    }
    ring
}

/// `hull` (convex) grown by `r`, counter-clockwise: the hull of its corners
/// moved eight ways, a little more than `r` (round the circle, not in it).
fn grow(hull: &Polygon<f64>, r: f64) -> Polygon<f64> {
    let k = r / (std::f64::consts::PI / 8.0).cos();
    let points: Vec<Point<f64>> = hull.exterior().0.iter().flat_map(|c| (0..8).map(move |j| { let a = (j as f64 + 0.5) * std::f64::consts::FRAC_PI_4; Point::new(c.x + k * a.cos(), c.y + k * a.sin()) })).collect();
    let mut poly = MultiPoint::new(points).convex_hull();
    if poly.signed_area() < 0.0 {
        poly.exterior_mut(|e| e.0.reverse());
    }
    poly
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

/// `points` as a polygon as they are (`polygon` rounds them).
pub(crate) fn polygon_exact(points: &[DVec2]) -> Polygon<f64> {
    Polygon::new(LineString::from(points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()), vec![])
}

fn bounds(poly: &[DVec2]) -> (DVec2, DVec2) {
    poly.iter().fold((poly[0], poly[0]), |(a, b), q| (a.min(*q), b.max(*q)))
}

/// The plate pieces from `lo` to `hi` (best on columns' edges, the same
/// set the terrain's mesh has), as the generator outlines them: a pillar's
/// prisms grown, its plate taken at its base.
pub(crate) fn pieces(world: &PlateWorld, lo: DVec2, hi: DVec2) -> Vec<Piece> {
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
    par_chunks(&strips, |c| c.iter().flat_map(|&(a, b)| world.plate_pieces(a, b)).collect::<Vec<_>>())
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
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
    let mut near: Vec<(usize, usize)> = Vec::new();
    let mut across: Vec<(f64, f64, usize)> = Vec::new();
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
            across.clear();
            near.clear();
            near.extend(cells(a, b, ON).flat_map(|c| grid.get(&c).into_iter().flatten().copied()).filter(|&(j, _)| j != i));
            near.sort_unstable();
            near.dedup();
            for &(j, m) in &near {
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
    /// Each block's hulls' corners, counter-clockwise, last not first.
    pub(crate) block_rings: Vec<Vec<Vec<DVec2>>>,
    /// Each piece's bounds.
    pub(crate) boxes: Vec<(DVec2, DVec2)>,
    /// The closed stretches by cell (padded by the radius).
    closed: HashMap<(i32, i32), Vec<usize>>,
    /// The pieces by cell.
    grid: HashMap<(i32, i32), Vec<usize>>,
    /// Each piece as a polygon, made once (found where a point is, every
    /// lookup: made anew each time, most of a lookup's cost).
    shapes: Vec<Polygon<f64>>,
    /// How long each stage of its building took (s), in order.
    pub(crate) times: Vec<(&'static str, f32)>,
    /// Each piece's convex hull, counter-clockwise (itself, convex as most
    /// are; one a canal cuts, round it): what a fall's path crosses (see
    /// `district::fall_clear`).
    pub(crate) hulls: Vec<Vec<DVec2>>,
    /// How many shapes were taken out of the pieces, all told (the walking
    /// space's unit of work: `seed_sweep`).
    pub(crate) cutters: std::sync::atomic::AtomicUsize,
    /// Pieces whose walking space `convex_less` could not make (made by
    /// geo's boolean instead): none, `seed_sweep` checks.
    pub(crate) fallbacks: std::sync::atomic::AtomicUsize,
}

impl Space {
    /// Over `pieces`, for a body of `fit`, on every core.
    pub(crate) fn build(world: &PlateWorld, pieces: Vec<Piece>, fit: Fit) -> Space {
        let mut times = Vec::new();
        let clock = std::time::Instant::now();
        let lap = |name: &'static str, times: &mut Vec<(&'static str, f32)>| times.push((name, clock.elapsed().as_secs_f32() - times.iter().map(|t: &(&str, f32)| t.1).sum::<f32>()));
        let edges = stretches(world, &pieces, fit.climb);
        lap("edges", &mut times);
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
        let pillars: Vec<&Piece> = pieces.iter().filter(|p| p.pillar).collect();
        let made: Vec<MultiPolygon<f64>> = par_chunks(&pillars, |c| {
                        c.iter()
                            .filter_map(|p| {
                                let floor = p.plate.height;
                                // (A slab whose top is within a climb of the
                                // ground is stepped onto, not walked round: no
                                // obstacle. Taken for one, Detour walked over
                                // ground the plates said was blocked.)
                                let hulls: Vec<Polygon<f64>> = p.prisms.iter().filter(|q| q.3 - floor > fit.climb).filter_map(|q| section(q, floor, floor + fit.height)).collect();
                                // (Each hull grown by the radius by hand, its
                                // own convex hull: geo's growing, 0.5 ms a
                                // pillar. Overlapping, as they may: they are
                                // taken out at once, see `walking`.)
                                (!hulls.is_empty()).then(|| MultiPolygon::new(outermost(hulls.iter().map(|h| grow(h, fit.radius)).collect())))
                            })
                            .collect::<Vec<_>>()
        })
        .into_iter()
        .flatten()
        .collect();
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
        let shapes: Vec<Polygon<f64>> = pieces.iter().map(|p| polygon(&p.poly)).collect();
        let hulls: Vec<Vec<DVec2>> = shapes.iter().map(|s| ccw(s.convex_hull().exterior().0.iter().map(|c| DVec2::new(c.x, c.y)).collect())).collect();
        let boxes = hulls.iter().map(|h| bounds(h)).collect();
        let block_rings = blocks.iter().map(|b| b.0.iter().map(|p| ccw(p.exterior().0[..p.exterior().0.len() - 1].iter().map(|c| DVec2::new(c.x, c.y)).collect())).collect()).collect();
        lap("obstacles", &mut times);
        let mut space = Space { fit, pieces, edges, space: Vec::new(), node_of: Vec::new(), node_area: Vec::new(), joins: 0, blocks, blocked, block_rings, boxes, closed, grid, shapes, times: Vec::new(), hulls, cutters: Default::default(), fallbacks: Default::default() };
        // Each piece's walking space: itself, less the radius round every
        // closed stretch near it, less the pillars' obstacles.
        let all: Vec<usize> = (0..space.pieces.len()).collect();
        let walking: Vec<Vec<Polygon<f64>>> = par_chunks(&all, |c| c.iter().map(|&i| if space.pieces[i].hole { Vec::new() } else { space.walking(i) }).collect::<Vec<_>>()).into_iter().flatten().collect();
        space.space = walking;
        lap("walking", &mut times);
        space.join();
        lap("join", &mut times);
        space.times = times;
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
        let mut joins = 0;
        let (mut cross, mut sides): (Vec<f64>, Vec<Vec<(f64, f64)>>) = (Vec::new(), Vec::new());
        for e in self.edges.iter().filter(|e| e.open && e.other.is_some_and(|j| j > e.piece)) {
            let j = e.other.unwrap();
            let dir = (e.b - e.a).normalize();
            for (p0, p1) in clear_of(e.a, e.b, &self.closed_near(&[e.a, e.b]), self.fit.radius) {
                if p0.distance(p1) < 1e-3 {
                    continue;
                }
                // (All along it: either side's walking space may be in
                // several fragments along one stretch, and each joins each
                // one across that it meets for `MEET` or more. At its
                // midpoint only, fragments were left apart that a way
                // crossed, found against Detour; every 10 cm, each point
                // looked up in every fragment, it was the join's work.)
                // (Just off it either way: a piece's own side, whichever.)
                let n = dir.perp() * 1e-3;
                let (s0, s1) = ((p0 - e.a).dot(dir), (p1 - e.a).dot(dir));
                let mut side = |i: usize, sides: &mut Vec<Vec<(f64, f64)>>| {
                    sides.clear();
                    for f in &self.space[i] {
                        let mut spans = Vec::new();
                        spans_in(f, e.a + n, dir, s0, s1, 0.0, &mut cross, &mut spans);
                        spans_in(f, e.a - n, dir, s0, s1, 0.0, &mut cross, &mut spans);
                        sides.push(spans);
                    }
                };
                side(e.piece, &mut sides);
                let mine = std::mem::take(&mut sides);
                side(j, &mut sides);
                for (fi, a) in mine.iter().enumerate() {
                    for (fj, b) in sides.iter().enumerate() {
                        if a.iter().any(|x| b.iter().any(|y| x.1.min(y.1) - x.0.max(y.0) >= MEET)) {
                            let (x, y) = (find(&mut parent, id[e.piece][fi]), find(&mut parent, id[j][fj]));
                            if x != y {
                                parent[x] = y;
                                joins += 1;
                            }
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

    /// Piece `i`'s walking space: itself less a band the radius wide round
    /// each closed stretch near it and the pillars' obstacles near it. A
    /// convex piece (most) is first cut by a straight line along each of its
    /// own edges closed all along (the band along such an edge, its round
    /// ends too, is all on that line's side within a convex piece): no
    /// boolean. What is left, the stretches whose band reaches into it and
    /// the pillars' hulls that meet it, made by hand (a capsule each) and
    /// taken out in one boolean, as one shape (the non-zero rule: their
    /// overlaps are not holes in it). (geo's buffer round the stretches cost
    /// 0.2 ms a piece, 3.8 s in the stacks; every stretch and hull near a
    /// piece a boolean, 1.3 s.)
    fn walking(&self, i: usize) -> Vec<Polygon<f64>> {
        let Some((shape, cut, convex)) = self.cutting(i) else {
            return Vec::new();
        };
        self.cutters.fetch_add(cut.len(), std::sync::atomic::Ordering::Relaxed);
        if cut.is_empty() {
            return vec![polygon(&shape)];
        }
        if convex {
            if let Some(made) = convex_less(&shape, &cut) {
                return made;
            }
            self.fallbacks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        polygon(&shape).boolean_op_with_fill_rule(&MultiPolygon::new(cut.iter().map(|c| polygon_exact(c)).collect()), geo::algorithm::bool_ops::OpType::Difference, geo::algorithm::bool_ops::FillRule::NonZero).0
    }

    /// Piece `i` as `walking` cuts it: itself, cut along its own edges
    /// closed all along where that is exact (counter-clockwise), the shapes
    /// to take out of what is left (each convex, counter-clockwise, its
    /// last corner not its first), and whether it is convex; none if
    /// nothing is left.
    pub(crate) fn cutting(&self, i: usize) -> Option<(Vec<DVec2>, Vec<Vec<DVec2>>, bool)> {
        let p = &self.pieces[i];
        let r = self.fit.radius;
        let (lo, hi) = bounds(&p.poly);
        let (lo, hi) = (lo - r, hi + r);
        let mut near: Vec<usize> = cells(lo, hi, 0.0).flat_map(|c| self.closed.get(&c).into_iter().flatten().copied()).collect();
        near.sort();
        near.dedup();
        near.retain(|&k| {
            let e = &self.edges[k];
            e.a.min(e.b).cmple(hi).all() && e.a.max(e.b).cmpge(lo).all()
        });
        let poly = ccw(p.poly.clone());
        let n = poly.len();
        let convex = n >= 3 && (0..n).all(|k| (poly[(k + 1) % n] - poly[k]).perp_dot(poly[(k + 2) % n] - poly[(k + 1) % n]) >= -1e-9);
        let mut shape = poly.clone();
        if convex {
            // Its own edges closed all along: their stretches, merged.
            let mut whole = vec![false; n];
            for k in 0..n {
                let (a, b) = (poly[k], poly[(k + 1) % n]);
                let len = a.distance(b);
                if len < 1e-9 {
                    continue;
                }
                let dir = (b - a) / len;
                let on = |x: DVec2| (x - a).perp_dot(dir).abs() <= ON;
                let mut parts: Vec<(f64, f64)> = near
                    .iter()
                    .map(|&s| &self.edges[s])
                    .filter(|e| e.piece == i && on(e.a) && on(e.b))
                    .map(|e| {
                        let (t0, t1) = ((e.a - a).dot(dir), (e.b - a).dot(dir));
                        (t0.min(t1), t0.max(t1))
                    })
                    .collect();
                parts.sort_by(|x, y| x.0.total_cmp(&y.0));
                let mut t = 0.0;
                for (t0, t1) in parts {
                    if t0 > t + ON {
                        break;
                    }
                    t = t.max(t1);
                }
                whole[k] = t >= len - ON;
            }
            // Cut by its line those whose band, cut by the line, takes no
            // more: past each end, at a corner wider than square, the line's
            // band reaches beyond the edge's (further than the radius from
            // its end) unless the edge on from it is closed all along too,
            // long enough to take that much in its own band.
            let at = |k: usize| poly[k % n];
            let end_ok = |k: usize, here: DVec2, there: DVec2, next: Option<usize>, beyond: DVec2| {
                let len = here.distance(there);
                (beyond - here).dot(there - here) >= 0.0 || next.is_some_and(|m| whole[m] && (beyond - here).perp_dot(there - here).abs() / len >= r) && whole[k]
            };
            let lines: Vec<bool> = (0..n).map(|k| whole[k] && end_ok(k, at(k), at(k + 1), Some((k + n - 1) % n), at(k + n - 1)) && end_ok(k, at(k + 1), at(k), Some((k + 1) % n), at(k + 2))).collect();
            let edge_of = |e: &Stretch| {
                (0..n).find(|&k| {
                    let (a, b) = (at(k), at(k + 1));
                    let len = a.distance(b);
                    len >= 1e-9 && {
                        let dir = (b - a) / len;
                        let on = |x: DVec2| (x - a).perp_dot(dir).abs() <= ON && (-ON..=len + ON).contains(&(x - a).dot(dir));
                        on(e.a) && on(e.b)
                    }
                })
            };
            for k in (0..n).filter(|&k| lines[k]) {
                shape = keep_left(&shape, at(k), at(k + 1), r);
                if shape.len() < 3 {
                    return None;
                }
            }
            // (A stretch along an edge cut by its line: its band within the
            // piece already gone; along an edge closed all along, another's
            // within its own.)
            near.retain(|&s| {
                let e = &self.edges[s];
                match edge_of(e) {
                    Some(k) if lines[k] => false,
                    Some(k) if whole[k] => e.piece == i,
                    _ => true,
                }
            });
            // (Only bands that reach into what is left.)
            near.retain(|&s| within(&shape, self.edges[s].a, self.edges[s].b, r));
        }
        let mut cut: Vec<Vec<DVec2>> = near.iter().map(|&s| capsule(self.edges[s].a, self.edges[s].b, r)).collect();
        let mut ks: Vec<usize> = cells(lo, hi, 0.0).flat_map(|c| self.blocked.get(&c).into_iter().flatten().copied()).collect();
        ks.sort();
        ks.dedup();
        // (Only the pillars' hulls that meet it: a pillar's every hull came
        // with any cell it shares, 60 cutters a piece in the stacks.)
        cut.extend(ks.into_iter().flat_map(|k| self.blocks[k].0.iter()).filter(|b| {
            b.bounding_rect().is_some_and(|q| q.min().x <= hi.x && q.min().y <= hi.y && q.max().x >= lo.x && q.max().y >= lo.y) && (!convex || convex_meet(&b.exterior().0, &shape))
        }).map(|b| {
            let ring = &b.exterior().0;
            ring[..ring.len() - 1].iter().map(|c| DVec2::new(c.x, c.y)).collect()
        }));
        Some((shape, cut, convex))
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
        self.grid.get(&((q.x / BIN).floor() as i32, (q.y / BIN).floor() as i32))?.iter().copied().find(|&i| near(&self.shapes[i]))
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
        let mut found = pieces.iter().copied().filter(|&i| near(&self.shapes[i])).flat_map(|i| self.space[i].iter().enumerate().filter(|(_, f)| near(f)).map(move |(k, _)| (self.node_of[i][k], i, k)));
        let first = found.next()?;
        if prefer.is_none_or(|n| n == first.0) {
            return Some(first);
        }
        Some(found.find(|f| Some(f.0) == prefer).unwrap_or(first))
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
        let pt = Point::new(q.x, q.y);
        let near = |p: &Polygon<f64>| p.intersects(&pt) || geo::Euclidean.distance(p, &pt) <= ROUNDING;
        about.pieces.iter().copied().filter(|&i| self.boxes[i].0.cmple(q + ROUNDING).all() && self.boxes[i].1.cmpge(q - ROUNDING).all()).find(|&i| near(&self.shapes[i]))
    }

    /// Whether `q` is on node `n`'s walking space (within `ROUNDING`, as
    /// `node_at_preferring` with `n` preferred finds it), among `about`'s
    /// pieces.
    pub(crate) fn on_node_in(&self, about: &About, q: DVec2, n: u32) -> bool {
        let pt = Point::new(q.x, q.y);
        let near = |p: &Polygon<f64>| p.intersects(&pt) || geo::Euclidean.distance(p, &pt) <= ROUNDING;
        about.pieces.iter().copied().filter(|&i| self.boxes[i].0.cmple(q + ROUNDING).all() && self.boxes[i].1.cmpge(q - ROUNDING).all()).any(|i| {
            self.space[i].iter().enumerate().any(|(k, f)| self.node_of[i][k] == n && near(f)) && near(&self.shapes[i])
        })
    }

    /// Among `about`'s pieces: where along `a` + `dir` s (`dir` of length 1), s from `s0` to `s1`,
    /// node `n`'s walking space is (within `ROUNDING` of it, as
    /// `node_at_preferring` finds): its spans, merged, in order. The path's
    /// line crossing each of the node's fragments' rings, the inside
    /// between each pair. (Found point by point, nine about each of a
    /// drop's landing spots, the stacks' landings were 0.9 s of work.)
    pub(crate) fn node_along_in(&self, about: &About, n: u32, a: DVec2, dir: DVec2, s0: f64, s1: f64) -> Vec<(f64, f64)> {
        let (p0, p1) = (a + dir * s0, a + dir * s1);
        let (lo, hi) = (p0.min(p1) - ROUNDING, p0.max(p1) + ROUNDING);
        let mut spans: Vec<(f64, f64)> = Vec::new();
        let mut cross: Vec<f64> = Vec::new();
        for i in about.pieces.iter().copied().filter(|&i| self.boxes[i].0.cmple(hi).all() && self.boxes[i].1.cmpge(lo).all()) {
            for (k, f) in self.space[i].iter().enumerate() {
                if self.node_of[i][k] != n {
                    continue;
                }
                spans_in(f, a, dir, s0, s1, ROUNDING, &mut cross, &mut spans);
            }
        }
        spans.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut out: Vec<(f64, f64)> = Vec::new();
        for (u0, u1) in spans {
            match out.last_mut() {
                Some(last) if u0 <= last.1 => last.1 = last.1.max(u1),
                _ => out.push((u0, u1)),
            }
        }
        out
    }

    /// The hops between nodes no more than `far` apart (3D): points every
    /// metre round each fragment's edges, each pair of nodes' nearest in
    /// each `HOP_CELL` of the lower-numbered node's side; keyed by the pair,
    /// the lower first, each hop from it.
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

    /// Points every metre round each fragment's edges: where, how high,
    /// and its node.
    pub(crate) fn edge_points(&self, world: &PlateWorld) -> Vec<(DVec2, f64, u32)> {
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
        // (Each cell's points in it, side by side, and its neighbours found
        // once a cell, not once a point: 3 million lookups in the stacks.)
        let mut cell: HashMap<(i32, i32), Vec<(DVec2, f64, u32)>> = HashMap::new();
        for p in &points {
            cell.entry(((p.0.x / far).floor() as i32, (p.0.y / far).floor() as i32)).or_default().push(*p);
        }
        let keys: Vec<(i32, i32)> = cell.keys().copied().collect();
        type Best = HashMap<((u32, u32), (i32, i32)), Link>;
        let keep = |out: &mut Best, pair: (u32, u32), l: Link| {
            let at = spread.map_or((0, 0), |c| ((l.a.x / c).floor() as i32, (l.a.y / c).floor() as i32));
            // (Ties by where: the same whatever the threads, as chunked.)
            let e = out.entry((pair, at)).or_insert(l);
            if (l.short, l.a.x, l.a.y, l.b.x, l.b.y) < (e.short, e.a.x, e.a.y, e.b.x, e.b.y) {
                *e = l;
            }
        };
        {
            let (cell, short) = (&cell, &short);
            let handles = par_chunks(&keys, |c| {
                        let mut out: Best = HashMap::new();
                        for &(cx, cz) in c {
                            let near: Vec<&Vec<(DVec2, f64, u32)>> = (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (cx + dx, cz + dz))).filter_map(|k| cell.get(&k)).collect();
                            let mut best: Vec<Link> = Vec::new();
                            let mut to: Vec<u32> = Vec::new();
                            for &pa in &cell[&(cx, cz)] {
                                // (The least of each node from this point,
                                // then kept: kept pair by pair, most of it.)
                                best.clear();
                                to.clear();
                                for &pb in near.iter().flat_map(|o| o.iter()) {
                                    if pb.2 == pa.2 {
                                        continue;
                                    }
                                    let gap = pa.0.distance(pb.0);
                                    if gap > far {
                                        continue;
                                    }
                                    // (No move, none kept: most pairs.)
                                    let s = short(gap, pb.1 - pa.1);
                                    if !s.is_finite() {
                                        continue;
                                    }
                                    let l = Link { short: s, a: pa.0, ay: pa.1, b: pb.0, by: pb.1 };
                                    match to.iter().position(|&n| n == pb.2) {
                                        Some(k) => {
                                            if (l.short, l.b.x, l.b.y) < (best[k].short, best[k].b.x, best[k].b.y) {
                                                best[k] = l;
                                            }
                                        }
                                        None => {
                                            to.push(pb.2);
                                            best.push(l);
                                        }
                                    }
                                }
                                for (&n, &l) in to.iter().zip(&best) {
                                    keep(&mut out, (pa.2, n), l);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `convex_less` on pieces it once could not make (seed 3's stacks).
    #[test]
    fn convex_less_hard_cases() {
        LESS_DEBUG.store(true, std::sync::atomic::Ordering::Relaxed);
        let v = |p: &[(f64, f64)]| p.iter().map(|&(x, y)| DVec2::new(x, y)).collect::<Vec<_>>();
        let shape = v(&[(15716.541114303176, 14026.09497634145), (15717.22669393342, 14021.787170677344), (15718.690557827851, 14020.279662931578), (15719.166234969935, 14025.416179117788), (15718.267110427825, 14026.385118840128)]);
        let cut = vec![ccw(v(&[(15718.60600181567, 14020.2876953125), (15719.07670494067, 14020.24375), (15719.36665443433, 14020.24375), (15719.5716796875, 14020.44877525317), (15719.7494140625, 14022.36869712817), (15719.7494140625, 14022.65864662183), (15719.54438880933, 14022.863671875), (15719.07368568433, 14022.9076171875), (15718.78373619067, 14022.9076171875), (15718.5787109375, 14022.70259193433), (15718.4009765625, 14020.78267005933), (15718.4009765625, 14020.49272056567)]))];
        let made = convex_less(&shape, &cut);
        assert!(made.is_some(), "not made");
    }
}
