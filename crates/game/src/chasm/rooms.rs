//! Rooms on a grid (`--opt chasm --opt rooms`): each room is a fine grid of
//! cells, a quarter of a metre on a side, each cell holding one filler. A
//! filler is empty, solid, or a prism: a region of the cell's cross-section
//! across one axis (part of a slope, part of a quarter circle), straight
//! through the cell along that axis. Units (an arch, a column, a stair, a
//! slope) stamp fillers into the grid.
//!
//! The mesh is built from the cells, never by 3D booleans: each face between
//! two cells is solid on each side over a region of its own (its profile),
//! and the surface there is where one side is solid and the other is not;
//! inside a prism, the slant or the curve of its region. So the surface is
//! closed, and two surfaces never lie in one plane facing each other.
//!
//! For now a lab: a few rooms built by hand on the flat ground, to judge the
//! kit before rooms are generated, and before any goes into the chasm.

use std::collections::{HashMap, HashSet};

use bevy::{math::DVec3, prelude::*};
use manifold_csg::{CrossSection, cross_section::FillRule, triangulate_polygons};

use super::{Geometry, Parts};

/// A cell's size (a power of two: its multiples are exact).
pub(super) const CELL: f32 = 0.25;

/// Cells to a metre.
const PER_M: f32 = 1.0 / CELL;

/// A prism's region, in its block of cells (its cross-section; the block is
/// `size` cells): a slope rising `rise` over `run` (a rational slope, so it
/// tiles); a quarter circle of radius `r` cells round the block's corner,
/// solid outside it (an arch's haunch) or inside it (a column's quarter).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Form {
    Ramp { run: u8, rise: u8 },
    Arch { r: u8 },
    Round { r: u8 },
}

impl Form {
    /// The block's size in cells (before it is turned).
    fn size(self) -> (u8, u8) {
        match self {
            Form::Ramp { run, rise } => (run, rise),
            Form::Arch { r } | Form::Round { r } => (r, r),
        }
    }

    /// The region, anticlockwise, in the block: the slope rising to the
    /// block's far corner; the circle centred on its near corner.
    fn region(self) -> Vec<[f64; 2]> {
        let arc = |r: f64, from: f64, to: f64| {
            let n = (r as usize * 2).clamp(6, 32);
            (0..=n).map(move |i| {
                let a = from + (to - from) * i as f64 / n as f64;
                // (The ends exact: on the block's sides.)
                match i {
                    0 => [r * from.cos().round(), r * from.sin().round()],
                    _ if i == n => [r * to.cos().round(), r * to.sin().round()],
                    _ => [r * a.cos(), r * a.sin()],
                }
            })
        };
        let q = std::f64::consts::FRAC_PI_2;
        match self {
            Form::Ramp { run, rise } => vec![[0.0, 0.0], [run as f64, 0.0], [run as f64, rise as f64]],
            Form::Arch { r } => {
                let r = r as f64;
                let mut out = vec![[r, r]];
                out.extend(arc(r, q, 0.0));
                out
            }
            Form::Round { r } => {
                let mut out = vec![[0.0, 0.0]];
                out.extend(arc(r as f64, 0.0, q));
                out
            }
        }
    }
}

/// A turn of a block (any of the square's eight): flipped across its width,
/// its height, then its two axes swapped.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(super) struct Turn {
    flip_x: bool,
    flip_y: bool,
    swap: bool,
}

impl Turn {
    fn apply(self, (w, h): (f64, f64), p: [f64; 2]) -> [f64; 2] {
        let x = if self.flip_x { w - p[0] } else { p[0] };
        let y = if self.flip_y { h - p[1] } else { p[1] };
        if self.swap { [y, x] } else { [x, y] }
    }

    /// Whether it turns the region over (its outline then runs clockwise).
    fn mirrors(self) -> bool {
        self.flip_x ^ self.flip_y ^ self.swap
    }
}

/// A prism: its axis (0, 1, 2: x, y, z), its region, and which cell of the
/// region's block it is. Its cross-section runs across the two other axes in
/// turn (`s` the next, `t` the one after: y and z across x, z and x across
/// y, x and y across z).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct Prism {
    axis: u8,
    form: Form,
    turn: Turn,
    at: (u8, u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum Filler {
    #[default]
    Empty,
    Solid,
    Prism(Prism),
}

/// The solid on one side of a face between cells, in the face's frame
/// (across the face's axis, as a prism's cross-section is): none, all, or a
/// region.
enum Profile {
    None,
    Full,
    Region(Vec<Vec<[f64; 2]>>),
}

/// A room's grid: its cells, x fastest, then z, then y; where its corner
/// stands. Outside it is empty.
pub(super) struct Grid {
    n: [i32; 3],
    cells: Vec<Filler>,
    origin: Vec3,
}

/// A rectangle as a ring, anticlockwise.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// A polygon clipped to the unit square at (i, j), moved to the square's
/// corner (Sutherland-Hodgman: the regions here, where they cross a cell,
/// stay in one piece).
fn clip(poly: &[[f64; 2]], i: f64, j: f64) -> Vec<[f64; 2]> {
    let mut out = poly.to_vec();
    for (axis, bound, keep_below) in [(0, i, false), (0, i + 1.0, true), (1, j, false), (1, j + 1.0, true)] {
        let inside = |p: &[f64; 2]| if keep_below { p[axis] <= bound } else { p[axis] >= bound };
        let input = std::mem::take(&mut out);
        for k in 0..input.len() {
            let (p, q) = (input[k], input[(k + 1) % input.len()]);
            let (pin, qin) = (inside(&p), inside(&q));
            if pin {
                out.push(p);
            }
            if pin != qin {
                let t = (bound - p[axis]) / (q[axis] - p[axis]);
                let mut x = [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                x[axis] = bound;
                out.push(x);
            }
        }
        if out.is_empty() {
            return out;
        }
    }
    // (Points repeated where the outline ran along a side: dropped.)
    out.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12);
    while out.len() > 1 && (out[0][0] - out[out.len() - 1][0]).abs() < 1e-12 && (out[0][1] - out[out.len() - 1][1]).abs() < 1e-12 {
        out.pop();
    }
    out.iter().map(|p| [p[0] - i, p[1] - j]).collect()
}

fn area(poly: &[[f64; 2]]) -> f64 {
    (0..poly.len()).map(|k| {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        p[0] * q[1] - q[0] * p[1]
    }).sum::<f64>() * 0.5
}

/// Where a cell's region meets one side of it (`axis` 0: the side s =
/// `at`, 1: t = `at`): the stretches along that side, merged.
fn side(poly: &[[f64; 2]], axis: usize, at: f64) -> Vec<(f64, f64)> {
    let on = |p: &[f64; 2]| (p[axis] - at).abs() < 1e-9;
    let mut out: Vec<(f64, f64)> = (0..poly.len())
        .filter(|&k| on(&poly[k]) && on(&poly[(k + 1) % poly.len()]))
        .map(|k| {
            let (a, b) = (poly[k][1 - axis], poly[(k + 1) % poly.len()][1 - axis]);
            (a.min(b), a.max(b))
        })
        .filter(|(a, b)| b - a > 1e-9)
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (a, b) in out {
        match merged.last_mut() {
            Some(last) if a <= last.1 + 1e-9 => last.1 = last.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    merged
}

/// What is known of each kind of prism cell: its region (in the cell, its
/// corner at the origin), and where it meets the cell's sides.
pub(super) struct Cache(HashMap<(Form, Turn, (u8, u8)), Vec<[f64; 2]>>);

impl Cache {
    fn region(&mut self, p: &Prism) -> &Vec<[f64; 2]> {
        self.0.entry((p.form, p.turn, p.at)).or_insert_with(|| {
            let (w, h) = p.form.size();
            let mut poly: Vec<[f64; 2]> = p.form.region().into_iter().map(|q| p.turn.apply((w as f64, h as f64), q)).collect();
            if p.turn.mirrors() {
                poly.reverse();
            }
            clip(&poly, p.at.0 as f64, p.at.1 as f64)
        })
    }
}

impl Grid {
    /// A grid of `size` (in metres, rounded to cells), its corner at
    /// `origin`, all solid.
    pub(super) fn solid(origin: Vec3, size: Vec3) -> Grid {
        let n = (size * PER_M).round().as_ivec3().to_array();
        Grid { n, cells: vec![Filler::Solid; (n[0] * n[1] * n[2]) as usize], origin }
    }

    fn index(&self, c: [i32; 3]) -> Option<usize> {
        (0..3).all(|k| (0..self.n[k]).contains(&c[k])).then(|| (c[0] + self.n[0] * (c[2] + self.n[2] * c[1])) as usize)
    }

    fn get(&self, c: [i32; 3]) -> Filler {
        self.index(c).map_or(Filler::Empty, |i| self.cells[i])
    }

    fn set(&mut self, c: [i32; 3], f: Filler) {
        if let Some(i) = self.index(c) {
            self.cells[i] = f;
        }
    }

    /// Fills the cells from `lo` up to `hi` (in metres from the grid's
    /// corner, rounded to cells).
    pub(super) fn fill(&mut self, lo: Vec3, hi: Vec3, f: Filler) {
        let (lo, hi) = (cells(lo), cells(hi));
        for y in lo[1]..hi[1] {
            for z in lo[2]..hi[2] {
                for x in lo[0]..hi[0] {
                    self.set([x, y, z], f);
                }
            }
        }
    }

    /// Stamps a block of prisms along `axis` from `at` (its corner, in
    /// metres from the grid's corner) for `len` metres: each cell of the
    /// block the part of the region in it (none, all, or a prism), in place
    /// of what was there.
    pub(super) fn block(&mut self, axis: usize, form: Form, turn: Turn, at: Vec3, len: f32, cache: &mut Cache) {
        let (s, t) = ((axis + 1) % 3, (axis + 2) % 3);
        let (w, h) = form.size();
        let (w, h) = if turn.swap { (h, w) } else { (w, h) };
        let at = cells(at);
        for i in 0..w {
            for j in 0..h {
                let prism = Prism { axis: axis as u8, form, turn, at: (i, j) };
                let a = area(cache.region(&prism));
                let f = if a < 1e-9 { Filler::Empty } else if a > 1.0 - 1e-9 { Filler::Solid } else { Filler::Prism(prism) };
                for k in 0..(len * PER_M).round() as i32 {
                    let mut c = at;
                    c[axis] += k;
                    c[s] += i as i32;
                    c[t] += j as i32;
                    self.set(c, f);
                }
            }
        }
    }

    /// A cell's profile on its face across `d`, on its far side (`far`) or
    /// its near side, in the face's frame (across `d`: the axes after it in
    /// turn).
    fn profile(&self, c: [i32; 3], d: usize, far: bool, cache: &mut Cache) -> Profile {
        let p = match self.get(c) {
            Filler::Empty => return Profile::None,
            Filler::Solid => return Profile::Full,
            Filler::Prism(p) => p,
        };
        let a = p.axis as usize;
        let poly = cache.region(&p).clone();
        let at = if far { 1.0 } else { 0.0 };
        // (Across its own axis, its region; across `s`, where its region
        // meets the side s = 0 or 1, along t, the face's first axis (its
        // second being the prism's axis); across `t`, where it meets t = 0
        // or 1, along s, the face's second axis.)
        let rings = if d == a {
            vec![poly]
        } else if d == (a + 1) % 3 {
            side(&poly, 0, at).into_iter().map(|(t0, t1)| rect(t0, 0.0, t1, 1.0)).collect()
        } else {
            side(&poly, 1, at).into_iter().map(|(s0, s1)| rect(0.0, s0, 1.0, s1)).collect()
        };
        match rings.len() {
            0 => Profile::None,
            1 if (area(&rings[0]) - 1.0).abs() < 1e-9 => Profile::Full,
            _ => Profile::Region(rings),
        }
    }

    /// The room's surface, into `g`. Returns the volume it encloses and the
    /// volume of its cells' solid (they agree when it is closed).
    pub(super) fn mesh(&self, g: &mut Geometry) -> (f64, f64) {
        let mut cache = Cache(HashMap::new());
        let start = g.indices.len();
        // Whole faces between solid and empty, by plane (axis, where,
        // facing which way), merged into rectangles at the end.
        let mut whole: HashMap<(usize, i32, bool), HashSet<(i32, i32)>> = HashMap::new();
        let mut solid = 0.0;
        for y in -1..self.n[1] {
            for z in -1..self.n[2] {
                for x in -1..self.n[0] {
                    let c = [x, y, z];
                    let here = self.get(c);
                    match here {
                        Filler::Solid => solid += 1.0,
                        Filler::Prism(p) => {
                            solid += area(cache.region(&p));
                            self.slant(c, &p, &mut cache, g);
                        }
                        Filler::Empty => {}
                    }
                    for d in 0..3 {
                        let mut next = c;
                        next[d] += 1;
                        let there = self.get(next);
                        let (u, v) = ((d + 1) % 3, (d + 2) % 3);
                        match (here, there) {
                            (Filler::Empty, Filler::Empty) | (Filler::Solid, Filler::Solid) => continue,
                            (Filler::Solid, Filler::Empty) => {
                                whole.entry((d, c[d] + 1, true)).or_default().insert((c[u], c[v]));
                                continue;
                            }
                            (Filler::Empty, Filler::Solid) => {
                                whole.entry((d, c[d] + 1, false)).or_default().insert((c[u], c[v]));
                                continue;
                            }
                            // (Two cells of one prism along its axis: alike.)
                            (Filler::Prism(a), Filler::Prism(b)) if a == b && a.axis as usize == d => continue,
                            _ => {}
                        }
                        let (a, b) = (self.profile(c, d, true, &mut cache), self.profile(next, d, false, &mut cache));
                        for (from, to, facing) in [(&a, &b, true), (&b, &a, false)] {
                            let rings = match (from, to) {
                                (Profile::None, _) | (_, Profile::Full) => continue,
                                (Profile::Full, Profile::None) => {
                                    whole.entry((d, c[d] + 1, facing)).or_default().insert((c[u], c[v]));
                                    continue;
                                }
                                (Profile::Region(r), Profile::None) => r.clone(),
                                (from, to) => {
                                    let section = |p: &Profile| match p {
                                        Profile::Region(r) => CrossSection::from_polygons_with_fill_rule(r, FillRule::NonZero),
                                        _ => CrossSection::from_polygons(&[rect(0.0, 0.0, 1.0, 1.0)]),
                                    };
                                    section(from).difference(&section(to)).to_polygons()
                                }
                            };
                            self.face(c, d, facing, &rings, g);
                        }
                    }
                }
            }
        }
        for ((d, at, facing), set) in whole {
            for (u0, v0, u1, v1) in rectangles(set) {
                let (u, v) = ((d + 1) % 3, (d + 2) % 3);
                let mut c = [0; 3];
                c[d] = at - 1;
                c[u] = u0;
                c[v] = v0;
                self.face(c, d, facing, &[rect(0.0, 0.0, (u1 - u0) as f64, (v1 - v0) as f64)], g);
            }
        }
        // (The volume enclosed: by the divergence theorem, over the
        // triangles added, from a point in no plane of theirs: a face in a
        // plane through it would count for nothing, whichever way it
        // faced.)
        let from = self.origin.as_dvec3() - DVec3::new(0.317, 0.529, 0.743);
        let enclosed: f64 = g.indices[start..]
            .chunks_exact(3)
            .map(|t| {
                let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(g.positions[t[k] as usize]).as_dvec3() - from);
                a.dot(b.cross(c)) / 6.0
            })
            .sum();
        let cell = (CELL as f64).powi(3);
        (enclosed, solid * cell)
    }

    /// A region on the far face across `d` of cell `c` (in the face's
    /// frame, its corner the cell's), facing on across `d` or back.
    fn face(&self, c: [i32; 3], d: usize, facing: bool, rings: &[Vec<[f64; 2]>], g: &mut Geometry) {
        let rings: Vec<Vec<[f64; 2]>> = rings.iter().filter(|r| r.len() >= 3).cloned().collect();
        if rings.is_empty() {
            return;
        }
        let (u, v) = ((d + 1) % 3, (d + 2) % 3);
        let point = |p: [f64; 2]| {
            let mut q = [0.0; 3];
            q[d] = (c[d] + 1) as f32;
            q[u] = c[u] as f32 + p[0] as f32;
            q[v] = c[v] as f32 + p[1] as f32;
            self.origin + Vec3::from(q) * CELL
        };
        let mut normal = Vec3::ZERO;
        normal[d] = if facing { 1.0 } else { -1.0 };
        let flat: Vec<[f64; 2]> = rings.iter().flatten().copied().collect();
        if let [r] = &rings[..]
            && r.len() == 4
            && r[0][1] == r[1][1]
            && r[1][0] == r[2][0]
            && r[2][1] == r[3][1]
            && r[3][0] == r[0][0]
        {
            // (A rectangle: two triangles.)
            let mut q = [point(r[0]), point(r[1]), point(r[2]), point(r[3])];
            if !facing {
                q.reverse();
            }
            g.quad(q, normal);
            return;
        }
        let Some(tris) = triangulate_polygons(&rings, 1e-12) else {
            warn!("rooms: a face that would not triangulate: {rings:?}");
            return;
        };
        for t in tris {
            let mut k = [t[0], t[1], t[2]];
            if !facing {
                k.swap(1, 2);
            }
            let base = g.positions.len() as u32;
            for i in k {
                g.positions.push(point(flat[i as usize]).to_array());
                g.normals.push(normal.to_array());
            }
            g.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }

    /// A prism's slant or curve: its region's edges inside the cell, drawn
    /// along its axis (outwards: the region lies to an edge's left). A
    /// curve is shaded smooth: its normals from the circle's centre.
    fn slant(&self, c: [i32; 3], p: &Prism, cache: &mut Cache, g: &mut Geometry) {
        let a = p.axis as usize;
        let (s, t) = ((a + 1) % 3, (a + 2) % 3);
        let poly = cache.region(p).clone();
        let point = |q: [f64; 2], along: f32| {
            let mut r = [0.0; 3];
            r[a] = c[a] as f32 + along;
            r[s] = c[s] as f32 + q[0] as f32;
            r[t] = c[t] as f32 + q[1] as f32;
            self.origin + Vec3::from(r) * CELL
        };
        let on_side = |q: &[f64; 2], k: usize| q[k].abs() < 1e-9 || (q[k] - 1.0).abs() < 1e-9;
        // (The centre, in the cell; outwards away from it round a column,
        // towards it under an arch.)
        let (w, h) = p.form.size();
        let centre = p.turn.apply((w as f64, h as f64), [0.0, 0.0]);
        let centre = [centre[0] - p.at.0 as f64, centre[1] - p.at.1 as f64];
        let round = match p.form {
            Form::Ramp { .. } => None,
            Form::Arch { .. } => Some(-1.0),
            Form::Round { .. } => Some(1.0),
        };
        let normal = |q: [f64; 2], sign: f64| {
            let mut n = Vec3::ZERO;
            n[s] = ((q[0] - centre[0]) * sign) as f32;
            n[t] = ((q[1] - centre[1]) * sign) as f32;
            n.normalize_or(Vec3::Y)
        };
        for k in 0..poly.len() {
            let (p0, p1) = (poly[k], poly[(k + 1) % poly.len()]);
            if (0..2).any(|i| on_side(&p0, i) && on_side(&p1, i) && (p0[i] - p1[i]).abs() < 1e-9) {
                continue;
            }
            let corners = [point(p0, 0.0), point(p1, 0.0), point(p1, 1.0), point(p0, 1.0)];
            if let Some(sign) = round {
                let (n0, n1) = (normal(p0, sign), normal(p1, sign));
                let base = g.positions.len() as u32;
                for (c, n) in corners.into_iter().zip([n0, n1, n1, n0]) {
                    g.positions.push(c.to_array());
                    g.normals.push(n.to_array());
                }
                g.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                continue;
            }
            let mut n = Vec3::ZERO;
            n[s] = (p1[1] - p0[1]) as f32;
            n[t] = (p0[0] - p1[0]) as f32;
            g.quad(corners, n.normalize_or(Vec3::Y));
        }
    }
}

/// Metres from a grid's corner, in cells.
fn cells(p: Vec3) -> [i32; 3] {
    (p * PER_M).round().as_ivec3().to_array()
}

/// A set of cells in a plane, as rectangles covering it (each grown as far
/// along u as it goes, then along v while whole rows go): (u0, v0, u1, v1).
fn rectangles(mut set: HashSet<(i32, i32)>) -> Vec<(i32, i32, i32, i32)> {
    let mut order: Vec<(i32, i32)> = set.iter().copied().collect();
    order.sort_by_key(|&(u, v)| (v, u));
    let mut out = Vec::new();
    for (u0, v0) in order {
        if !set.contains(&(u0, v0)) {
            continue;
        }
        let mut u1 = u0 + 1;
        while set.contains(&(u1, v0)) {
            u1 += 1;
        }
        let mut v1 = v0 + 1;
        while (u0..u1).all(|u| set.contains(&(u, v1))) {
            v1 += 1;
        }
        for v in v0..v1 {
            for u in u0..u1 {
                set.remove(&(u, v));
            }
        }
        out.push((u0, v0, u1, v1));
    }
    out
}

/// Units: what rooms are made of, each stamping fillers into the grid. All
/// in metres from the grid's corner; `axis` 0 or 2 (x or z) for what runs
/// along a horizontal axis.
impl Grid {
    /// An arched opening through a wall, the wall across `axis`: `x` its
    /// side's position across the opening (along the wall), `width` wide,
    /// from the floor `y` up to where its arch springs, `spring`, then a
    /// half circle; through the wall from `n` to `n + depth` (along `axis`).
    pub(super) fn arch(&mut self, axis: usize, x: f32, width: f32, y: f32, spring: f32, n: f32, depth: f32, cache: &mut Cache) {
        let r = width * 0.5;
        self.fill(self.frame(axis, x, y, n), self.frame(axis, x + width, y + spring, n + depth), Filler::Empty);
        // (The two haunches: the circle's centre at the inner corner of
        // each. Across x, the cross-section's axes are (y, z): the block's
        // turned so its height is up.)
        let swap = axis == 0;
        for (at, flip) in [(x, true), (x + r, false)] {
            let turn = Turn { flip_x: flip, flip_y: false, swap };
            self.block(axis, Form::Arch { r: (r * PER_M).round() as u8 }, turn, self.frame(axis, at, y + spring, n), depth, cache);
        }
    }

    /// A barrel vault: an arch as long as the room.
    pub(super) fn vault(&mut self, axis: usize, x: f32, width: f32, y: f32, spring: f32, n: f32, len: f32, cache: &mut Cache) {
        self.arch(axis, x, width, y, spring, n, len, cache);
    }

    /// A round column from `y0` to `y1`, centred at (x, z), of radius `r`
    /// (on whole cells).
    pub(super) fn column(&mut self, x: f32, z: f32, r: f32, (y0, y1): (f32, f32), cache: &mut Cache) {
        let form = Form::Round { r: (r * PER_M).round() as u8 };
        // (Across y, the cross-section's axes are (z, x).)
        for (dz, dx) in [(0.0, 0.0), (-r, 0.0), (0.0, -r), (-r, -r)] {
            let turn = Turn { flip_x: dz < 0.0, flip_y: dx < 0.0, swap: false };
            self.block(1, form, turn, Vec3::new(x + dx, y0, z + dz), y1 - y0, cache);
        }
    }

    /// A stair rising along `axis` (`dir` +1 or -1) from (x, y, n): `steps`
    /// steps of `rise` up and `tread` along, `width` wide, solid beneath.
    pub(super) fn stair(&mut self, axis: usize, dir: f32, x: f32, width: f32, y: f32, n: f32, steps: u32, rise: f32, tread: f32) {
        for i in 0..steps {
            let (a, b) = (n + dir * tread * i as f32, n + dir * tread * (i + 1) as f32);
            let top = y + rise * (i + 1) as f32;
            self.fill(self.frame(axis, x, y, a.min(b)), self.frame(axis, x + width, top, a.max(b)), Filler::Solid);
        }
    }

    /// A slope rising `rise` over `run` (cells: 1 over 4, 1 over 2, 1 over
    /// 1) along `axis` (`dir` +1 or -1) from (x, y, n) up `height`, `width`
    /// wide, solid beneath; a run of blocks, each a step up and along.
    pub(super) fn slope(&mut self, axis: usize, dir: f32, x: f32, width: f32, y: f32, n: f32, (run, rise): (u8, u8), height: f32, cache: &mut Cache) {
        let steps = (height * PER_M / rise as f32).round() as i32;
        let (run_m, rise_m) = (run as f32 * CELL, rise as f32 * CELL);
        // (Along x, the cross-section of a prism along z is (x, y): the
        // slope rising along x as it is; along z, across x it is (y, z):
        // turned so its height is up. Falling the other way: flipped.)
        let (along, across) = if axis == 0 { (0, 2) } else { (2, 0) };
        for i in 0..steps {
            let a = if dir > 0.0 { n + run_m * i as f32 } else { n - run_m * (i + 1) as f32 };
            let base = y + rise_m * i as f32;
            self.fill(self.frame(axis, x, y, a), self.frame(axis, x + width, base, a + run_m), Filler::Solid);
            let turn = Turn { flip_x: dir < 0.0, flip_y: false, swap: axis == 2 };
            let mut at = Vec3::ZERO;
            at[along] = a;
            at[across] = x;
            at[1] = base;
            self.block(across, Form::Ramp { run, rise }, turn, at, width, cache);
        }
    }

    /// A point in a unit's frame: `x` across it (along the wall), `y` up,
    /// `n` along `axis`.
    fn frame(&self, axis: usize, x: f32, y: f32, n: f32) -> Vec3 {
        if axis == 0 { Vec3::new(n, y, x) } else { Vec3::new(x, y, n) }
    }
}

/// Where the lab is: by the chasm's centre (the world wraps round).
const LAB: Vec3 = Vec3::new(1200.0, 0.0, 900.0);

/// Where you start in the lab: before the first room's door, looking in.
pub const START: [f32; 5] = [LAB.x + 8.0, 1.7, LAB.z - 10.0, 180.0, -2.0];

/// The lab: rooms in a row on the ground, each built by hand from the units.
pub(super) fn lab() -> Parts {
    let (parts, report) = built();
    for line in report {
        info!("rooms: {line}");
    }
    parts
}

/// `--check rooms`: each room's surface encloses what its cells hold (it is
/// closed, and faces outwards).
pub fn check() -> bool {
    let (_, report) = built();
    let mut ok = true;
    for line in report {
        ok &= !line.contains("OPEN");
        eprintln!("rooms: {line}");
    }
    ok
}

/// The lab's rooms, and a line on each.
fn built() -> (Parts, Vec<String>) {
    let mut parts = Parts::default();
    let mut report = Vec::new();
    let mut cache = Cache(HashMap::new());
    // (Each grid's corner half a metre under the ground, its floor a cell
    // above it.)
    let floor = 0.75;
    let rooms = [nave(&mut cache, floor), stair_hall(&mut cache, floor)];
    let mut x = 0.0;
    for (grid, lights) in rooms {
        let grid = Grid { origin: LAB + Vec3::new(x, -0.5, 0.0), ..grid };
        let before = parts.stone.indices.len() / 3;
        let (enclosed, solid) = grid.mesh(&mut parts.stone);
        let closed = (enclosed - solid).abs() < solid * 1e-5;
        report.push(format!(
            "the room at x {x:.0}: {solid:.2} m3 of cells, {enclosed:.2} m3 enclosed by its surface{}; {} triangles",
            if closed { "" } else { ": OPEN" },
            parts.stone.indices.len() / 3 - before
        ));
        for (p, range, k) in lights {
            parts.lights.push((grid.origin + p, range, k));
        }
        x += grid.n[0] as f32 * CELL + 8.0;
    }
    (parts, report)
}

/// A nave under a barrel vault, an arcade to each side opening onto an
/// aisle, an arched door in its front wall.
fn nave(cache: &mut Cache, f: f32) -> (Grid, Vec<(Vec3, f32, f32)>) {
    let mut g = Grid::solid(Vec3::ZERO, Vec3::new(16.0, 11.0, 24.0));
    // The aisles and the nave, up to the arcades' lintels.
    g.fill(Vec3::new(1.0, f, 1.0), Vec3::new(15.0, f + 5.0, 23.0), Filler::Empty);
    // The arcades: walls a metre thick, an arch 3 m wide every 4 m.
    for x in [4.0, 11.0] {
        g.fill(Vec3::new(x, f, 1.0), Vec3::new(x + 1.0, f + 5.0, 23.0), Filler::Solid);
        for k in 0..5 {
            g.arch(0, 2.5 + 4.0 * k as f32, 3.0, f, 3.0, x, 1.0, cache);
        }
    }
    // The nave's vault: springing at 6 m, 6 m across.
    g.vault(2, 5.0, 6.0, f, 6.0, 1.0, 22.0, cache);
    // The door.
    g.arch(2, 7.0, 2.0, f, 2.5, 0.0, 1.0, cache);
    let mut lights = Vec::new();
    for k in 0..5 {
        let z = 3.0 + 4.5 * k as f32;
        lights.push((Vec3::new(8.0, f + 5.0, z), 12.0, 0.0003));
        lights.push((Vec3::new(2.5, f + 3.5, z), 6.0, 0.00005));
        lights.push((Vec3::new(13.5, f + 3.5, z), 6.0, 0.00005));
    }
    (g, lights)
}

/// A hall with a gallery across its back on round columns, a stair up to it
/// along one wall, a slope along the other, an arched niche under the
/// gallery, an arched door.
fn stair_hall(cache: &mut Cache, f: f32) -> (Grid, Vec<(Vec3, f32, f32)>) {
    let mut g = Grid::solid(Vec3::ZERO, Vec3::new(14.0, 12.0, 16.0));
    g.fill(Vec3::new(1.0, f, 1.0), Vec3::new(13.0, f + 10.0, 15.0), Filler::Empty);
    // The gallery: a slab half a metre thick at 4 m, from z 10 back, with
    // a parapet a metre high along its edge.
    g.fill(Vec3::new(1.0, f + 3.5, 10.0), Vec3::new(13.0, f + 4.0, 15.0), Filler::Solid);
    g.fill(Vec3::new(3.0, f + 4.0, 10.0), Vec3::new(13.0, f + 5.0, 10.25), Filler::Solid);
    for x in [4.0, 7.5, 11.0] {
        g.column(x, 10.5, 0.5, (f, f + 3.5), cache);
    }
    // The stair up to it, along the left wall: 16 steps of a quarter metre
    // up and half a metre along.
    g.stair(2, 1.0, 1.0, 2.0, f, 2.0, 16, 0.25, 0.5);
    // A slope of 1 in 4 up the right wall, 2 m up onto a landing.
    g.slope(2, 1.0, 11.0, 2.0, f, 1.5, (4, 1), 2.0, cache);
    g.fill(Vec3::new(11.0, f, 9.5), Vec3::new(13.0, f + 2.0, 10.0), Filler::Solid);
    // A niche under the gallery, in the back wall: arched, a metre deep.
    g.arch(2, 6.5, 2.0, f, 1.5, 15.0, 0.75, cache);
    // A high arched window in the right wall, onto the outside.
    g.arch(0, 5.0, 2.0, f + 6.0, 2.0, 13.0, 1.0, cache);
    // The door.
    g.arch(2, 6.0, 2.0, f, 2.5, 0.0, 1.0, cache);
    let lights = vec![(Vec3::new(7.0, f + 7.0, 6.0), 16.0, 0.0005), (Vec3::new(7.0, f + 2.5, 12.5), 6.0, 0.00005)];
    (g, lights)
}
