//! The plates as one planar map (the user, 2026-11-16: correct by
//! construction, no case by case fixes). Every piece's corner is known by
//! what it is, not where it is: where three sites' plates meet (`Key::Meet`),
//! or where two lines cross (`Key::Cross`), each line known by what it is
//! (`Line`: two sites' bisector, a canal's side, a stair's cut). Its place
//! is worked out from that alone (`Geometry::corner`), so the same corner
//! comes out the same, to the bit, in every piece it is a corner of; and
//! every edge knows its line, so a corner on it from a piece beside it is
//! found by what it is (`stitch`), never by how near. Two pieces beside one
//! another then share their edge corner for corner: rounded to any grid,
//! the same points. (Each plate clipped on its own, a corner three plates
//! share came out three ways, a millimetre's rounding apart now and then,
//! and a long edge did not pass through the corners of the small plates
//! along it: cracks between pieces walked across, found by an exact
//! overlay of the walking space.)
//!
//! Assumed: no four sites of a level on one circle, no corner of one kind on
//! a line of another but by what it is (true but for a set of no measure).

use glam::DVec2;

/// A line plates are cut along, by what it is: the same however found.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Line {
    /// Between two sites of a level (the lesser first).
    Bisector { level: u8, a: (i32, i32), b: (i32, i32) },
    /// Canal `i`'s sides (0, 1) and ends (2, 3), its copy round the wrap.
    Canal { i: u32, part: u8, copy: (i32, i32) },
    /// Station `s`'s court's sides, its copy round the wrap.
    Station { s: u32, side: u8, copy: (i32, i32) },
    /// The stair in big plate `site`: its strip's inner side (0), the edge
    /// between its steps `k - 1` and `k` (k from 1).
    Stair { site: (i32, i32), k: u16 },
    /// The square a level's cell is cut from, round its site (none left in
    /// a cell: its neighbours bound it).
    Bound { level: u8, site: (i32, i32), k: u8 },
    /// A line through points of the grid of `GRID` a metre (the edge of a
    /// region a body keeps out of: the game's nodes): `a x + b y = c` in the
    /// grid's units, `a` and `b` with no common factor, `a > 0` or `a == 0 <
    /// b`. The same line, however found, from whichever two of its points:
    /// two such regions' edges on one line are on one line by what it is.
    Grid { a: i32, b: i32, c: i64 },
}

/// The grid `Line::Grid`s are on (per m): exact in binary.
pub const GRID: f64 = 1024.0;

impl Line {
    /// The line through grid points `p` and `q` (whole numbers of `GRID`),
    /// and whether its normal (`Geometry::line`'s, `(a, b)`) points to the
    /// left of `p` to `q`.
    pub fn grid(p: (i64, i64), q: (i64, i64)) -> (Line, bool) {
        let (mut a, mut b) = (p.1 - q.1, q.0 - p.0);
        assert!(a != 0 || b != 0, "a grid line through two points, not one: {p:?}");
        let (mut x, mut y) = (a.abs(), b.abs());
        while y != 0 {
            (x, y) = (y, x % y);
        }
        (a, b) = (a / x, b / x);
        let left = a > 0 || (a == 0 && b > 0);
        if !left {
            (a, b) = (-a, -b);
        }
        let small = |v: i64| i32::try_from(v).expect("a grid line's normal within i32");
        (Line::Grid { a: small(a), b: small(b), c: a * p.0 + b * p.1 }, left)
    }
}

/// A `Line::Grid`'s point and normal (m; see `Geometry::line`): from what it
/// is alone.
pub fn grid_line(a: i32, b: i32, c: i64) -> (DVec2, DVec2) {
    let n = DVec2::new(a as f64, b as f64);
    (n * (c as f64 / n.length_squared() / GRID), n)
}

impl Line {
    /// The bisector of sites `g` and `h` of `level`.
    pub fn bisector(level: usize, g: (i32, i32), h: (i32, i32)) -> Line {
        let (a, b) = if g < h { (g, h) } else { (h, g) };
        Line::Bisector { level: level as u8, a, b }
    }
}

/// A corner, by what it is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Key {
    /// Where three sites' cells of a level meet (in order).
    Meet { level: u8, sites: [(i32, i32); 3] },
    /// Where two lines of different kinds cross (in order).
    Cross(Line, Line),
    /// Where grid lines cross (`Line::Grid`): the point itself, exactly,
    /// `(x, y) / d` in the grid's units, `d > 0`, and which side of a grid
    /// line it is on decided in whole numbers (`side`). On the grid, the
    /// grid point (`d` 1): one name however many grid lines pass through it
    /// (there three lines through a point are no accident: two bands round
    /// one corner share corners). Between, as its two lines give it: the
    /// same from the same two; through a third too only by accident, named
    /// apart from it there (two names for one point: a corner whole edges
    /// are not found across, parts there left unmerged, never wrong).
    Point { x: i128, y: i128, d: i128 },
}

impl Key {
    /// Where lines `a` and `b` meet: two bisectors of a level with a site
    /// in common, where those three sites' cells meet; else their crossing.
    pub fn of(a: Line, b: Line) -> Key {
        if let (Line::Bisector { level: la, a: a1, b: b1 }, Line::Bisector { level: lb, a: a2, b: b2 }) = (a, b)
            && la == lb
        {
            let mut s = [a1, b1, a2, b2];
            s.sort_unstable();
            let mut sites = [s[0]; 3];
            let mut n = 1;
            for &x in &s[1..] {
                if x != sites[n - 1] {
                    if n == 3 {
                        n = 4;
                        break;
                    }
                    sites[n] = x;
                    n += 1;
                }
            }
            if n == 3 {
                return Key::Meet { level: la, sites };
            }
        }
        if let (Line::Grid { a: a1, b: b1, c: c1 }, Line::Grid { a: a2, b: b2, c: c2 }) = (a, b) {
            let (a1, b1, c1, a2, b2, c2) = (a1 as i128, b1 as i128, c1 as i128, a2 as i128, b2 as i128, c2 as i128);
            let d = a1 * b2 - a2 * b1;
            assert!(d != 0, "grid lines that cross: {a:?} and {b:?} are parallel");
            let (x, y) = (c1 * b2 - c2 * b1, a1 * c2 - a2 * c1);
            let s = d.signum();
            let (x, y, d) = (x * s, y * s, d * s);
            // On the grid (most: a region's own corners), the grid point:
            // the quotient's nearest whole number, exact while under 2^51,
            // tried by multiplying back. (Reduced by division and greatest
            // common divisors, 128-bit, half a cut's cost.)
            let (qx, qy) = ((x as f64 / d as f64).round() as i128, (y as f64 / d as f64).round() as i128);
            if qx * d == x && qy * d == y {
                return Key::Point { x: qx, y: qy, d: 1 };
            }
            return Key::Point { x, y, d };
        }
        if a < b { Key::Cross(a, b) } else { Key::Cross(b, a) }
    }

    /// Which side of grid line `l` it is on, exactly, if that can be said
    /// in whole numbers (a `Point`, and `l` a grid line): the sign of `a x +
    /// b y - c` there.
    pub fn side(&self, l: Line) -> Option<i32> {
        match (*self, l) {
            (Key::Point { x, y, d }, Line::Grid { a, b, c }) => Some((a as i128 * x + b as i128 * y - c as i128 * d).signum() as i32),
            _ => None,
        }
    }

    /// Whether it lies on `l`, by what it is.
    pub fn on(&self, l: Line) -> bool {
        match (*self, l) {
            (Key::Meet { level, sites }, Line::Bisector { level: lv, a, b }) => level == lv && sites.contains(&a) && sites.contains(&b),
            (Key::Meet { .. }, _) => false,
            (Key::Cross(x, y), _) => x == l || y == l,
            (Key::Point { .. }, Line::Grid { .. }) => self.side(l) == Some(0),
            (Key::Point { .. }, _) => false,
        }
    }

    /// The lines it lies on, by what it is (a `Point`: those of the edges
    /// it is a corner of, see `stitch`).
    pub fn lines(&self) -> impl Iterator<Item = Line> {
        let list: [Option<Line>; 3] = match *self {
            Key::Meet { level, sites: [a, b, c] } => {
                let l = |x, y| Some(Line::bisector(level as usize, x, y));
                [l(a, b), l(a, c), l(b, c)]
            }
            Key::Cross(x, y) => [Some(x), Some(y), None],
            Key::Point { .. } => [None, None, None],
        };
        list.into_iter().flatten()
    }
}

/// A piece's corner: where, what it is, the line of its edge on to the
/// next corner, and a number its user may give it (kept by its copies, a
/// corner made `UNNUMBERED`: what it is, known by four bytes, not 64).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Corner {
    pub at: DVec2,
    pub key: Key,
    pub next: Line,
    pub number: u32,
}

impl Corner {
    /// Not yet numbered by its user.
    pub const UNNUMBERED: u32 = u32::MAX;

    pub fn new(at: DVec2, key: Key, next: Line) -> Corner {
        Corner { at, key, next, number: Corner::UNNUMBERED }
    }
}

/// Where lines and corners are, from what they are alone.
pub trait Geometry {
    /// Site `g` of `level`.
    fn site(&self, level: usize, g: (i32, i32)) -> DVec2;
    /// A point on `l` and a normal to it (not of length 1).
    fn line(&self, l: Line) -> (DVec2, DVec2);

    /// A bisector's point and normal: its sites' middle, from the lesser to
    /// the greater (the lesser's side below).
    fn bisector(&self, level: u8, a: (i32, i32), b: (i32, i32)) -> (DVec2, DVec2) {
        let (sa, sb) = (self.site(level as usize, a), self.site(level as usize, b));
        ((sa + sb) * 0.5, sb - sa)
    }

    /// Where corner `k` is.
    fn corner(&self, k: Key) -> DVec2 {
        match k {
            Key::Meet { level, sites } => {
                let [a, b, c] = sites.map(|g| self.site(level as usize, g));
                let (b, c) = (b - a, c - a);
                let d = 2.0 * b.perp_dot(c);
                let (bb, cc) = (b.length_squared(), c.length_squared());
                a + DVec2::new(c.y * bb - b.y * cc, b.x * cc - c.x * bb) / d
            }
            Key::Cross(x, y) => {
                let ((p, n), (q, m)) = (self.line(x), self.line(y));
                let d = n.perp();
                p + d * (m.dot(q - p) / m.dot(d))
            }
            Key::Point { x, y, d } => DVec2::new(x as f64, y as f64) / (d as f64 * GRID),
        }
    }
}

/// `poly` (counter-clockwise) cut by line `l`, the side kept where `(v - p)
/// · n` is at most 0 if `below` (`(p, n)` its `Geometry::line`), else at
/// least: into `out`. A corner on `l` by what it is is on it exactly; a new
/// corner, where an edge crosses `l`, is where its line and `l` meet.
pub fn clip<G: Geometry + ?Sized>(poly: &[Corner], l: Line, below: bool, geometry: &G, out: &mut Vec<Corner>) {
    clip_by(poly, &Cut::new(l, geometry), below, geometry, out);
}

/// A line to cut by, and which side of it corners are on (`side`).
#[derive(Clone, Copy, Debug)]
pub struct Cut {
    pub line: Line,
    p: DVec2,
    n: DVec2,
    /// A grid line's: its normal's |a| + |b| by the rounding's bound, and
    /// its point's largest coordinate.
    err: f64,
    reach: f64,
}

impl Cut {
    pub fn new<G: Geometry + ?Sized>(line: Line, geometry: &G) -> Cut {
        let (p, n) = geometry.line(line);
        let err = match line {
            Line::Grid { a, b, .. } => (a.unsigned_abs() + b.unsigned_abs()) as f64 * f64::EPSILON * 8.0,
            _ => 0.0,
        };
        Cut { line, p, n, err, reach: p.abs().max_element() }
    }

    /// Which side of its line corner `c` is on: above 0 its normal's,
    /// below the other, 0 on it. A point where grid lines cross against a
    /// grid line, exactly: in floating point where that is further from 0
    /// than its rounding can reach, else in whole numbers (`Key::side`).
    /// (Its rounding, in roundings of 2^-53: `c.at` as `Geometry::corner`
    /// places it, within 3 of each coordinate, the line's point within 2;
    /// their difference, by the normal, at most 6.03 by (|a| + |b|) by
    /// their largest coordinates; taken as 16. Every one in whole numbers,
    /// 128-bit, a cut's most work.) On it by what it is, 0; else in
    /// floating point.
    #[inline]
    pub fn side(&self, c: &Corner) -> f64 {
        self.side_at(c.at, &c.key)
    }

    /// `side` of a corner by its place and key.
    #[inline]
    pub fn side_at(&self, at: DVec2, key: &Key) -> f64 {
        let s = (at - self.p).dot(self.n);
        if let Key::Point { .. } = key
            && self.err > 0.0
        {
            if s.abs() > self.err * (at.abs().max_element() + self.reach) {
                return s;
            }
            return key.side(self.line).expect("a point where grid lines cross, and a grid line") as f64;
        }
        if key.on(self.line) { 0.0 } else { s }
    }
}

/// `clip` by a `Cut`, made once for many.
pub fn clip_by<G: Geometry + ?Sized>(poly: &[Corner], cut: &Cut, below: bool, geometry: &G, out: &mut Vec<Corner>) {
    out.clear();
    let l = cut.line;
    let side = |c: &Corner| {
        let s = cut.side(c);
        if below { s } else { -s }
    };
    let m = poly.len();
    let first = side(&poly[0]);
    let mut sb = first;
    for i in 0..m {
        let (a, b) = (poly[i], poly[(i + 1) % m]);
        let sa = sb;
        sb = if i + 1 < m { side(&b) } else { first };
        if sa <= 0.0 {
            out.push(a);
            if sb > 0.0 {
                // Out of it: on along `l`.
                if sa == 0.0 {
                    out.last_mut().unwrap().next = l;
                } else {
                    let key = Key::of(a.next, l);
                    out.push(Corner::new(geometry.corner(key), key, l));
                }
            }
        } else if sb < 0.0 {
            // Back in, crossing: on along the edge.
            let key = Key::of(a.next, l);
            out.push(Corner::new(geometry.corner(key), key, a.next));
        }
    }
}

/// `poly` (counter-clockwise) cut by `cut` both ways at once: what lies
/// below its line (at most 0, `Cut::side`) into `below`, above into
/// `above`, each as `clip` makes it; `sides` its corners' sides, as
/// `Cut::side` gives them. Each crossing named and placed once for both.
pub fn split<G: Geometry + ?Sized>(poly: &[Corner], cut: &Cut, sides: &[f64], geometry: &G, below: &mut Vec<Corner>, above: &mut Vec<Corner>) {
    below.clear();
    above.clear();
    let l = cut.line;
    let m = poly.len();
    for i in 0..m {
        let (a, j) = (poly[i], (i + 1) % m);
        let (sa, sb) = (sides[i], sides[j]);
        let crossing = || {
            let key = Key::of(a.next, l);
            (key, geometry.corner(key))
        };
        if sa < 0.0 {
            below.push(a);
            if sb > 0.0 {
                let (key, at) = crossing();
                below.push(Corner::new(at, key, l));
                above.push(Corner::new(at, key, a.next));
            }
        } else if sa > 0.0 {
            above.push(a);
            if sb < 0.0 {
                let (key, at) = crossing();
                above.push(Corner::new(at, key, l));
                below.push(Corner::new(at, key, a.next));
            }
        } else {
            // On the line: a corner of both, going on along it from
            // whichever leaves it.
            below.push(Corner { next: if sb > 0.0 { l } else { a.next }, ..a });
            above.push(Corner { next: if sb < 0.0 { l } else { a.next }, ..a });
        }
    }
}

/// A polygon's area (counter-clockwise, positive).
pub fn area(poly: &[Corner]) -> f64 {
    let o = poly.first().map_or(DVec2::ZERO, |c| c.at);
    (1..poly.len().saturating_sub(1)).map(|k| (poly[k].at - o).perp_dot(poly[k + 1].at - o)).sum::<f64>() / 2.0
}

/// Its corners' places.
pub fn points(poly: &[Corner]) -> Vec<DVec2> {
    poly.iter().map(|c| c.at).collect()
}

/// Each piece's edges given every corner of the others that lies on its
/// line, between its ends, by what it is (on a line its key names, or one
/// of its own edges is on): pieces beside one another then
/// share their edges corner for corner. (Along a line, corners in order of
/// how far along it they are, as `Geometry::line`'s normal turned.) (Every
/// corner on every line it is on, sorted by line and how far along, once;
/// each edge's found by halving. A map of lists, each edge looking through
/// its line's whole list: 47 ms for 54 000 corners in the stacks.)
pub fn stitch<G: Geometry + ?Sized>(pieces: &mut [Vec<Corner>], geometry: &G) {
    let mut on: Vec<(Line, f64, Key, DVec2)> = Vec::with_capacity(pieces.iter().map(|p| p.len()).sum::<usize>() * 3);
    for piece in pieces.iter() {
        let n = piece.len();
        for (k, c) in piece.iter().enumerate() {
            on.extend(c.key.lines().chain([piece[(k + n - 1) % n].next, c.next]).map(|l| (l, 0.0, c.key, c.at)));
        }
    }
    on.sort_unstable_by(|x, y| x.0.cmp(&y.0).then(x.2.cmp(&y.2)));
    on.dedup_by(|x, y| x.0 == y.0 && x.2 == y.2);
    // Each line's run, its place and direction, and its corners in order.
    let mut lines: Vec<(Line, usize, usize, DVec2, DVec2)> = Vec::new();
    let mut start = 0;
    while start < on.len() {
        let line = on[start].0;
        let mut end = start + 1;
        while end < on.len() && on[end].0 == line {
            end += 1;
        }
        let (p, n) = geometry.line(line);
        let d = n.perp();
        for x in &mut on[start..end] {
            x.1 = (x.3 - p).dot(d);
        }
        on[start..end].sort_unstable_by(|x, y| x.1.total_cmp(&y.1).then(x.2.cmp(&y.2)));
        lines.push((line, start, end, p, d));
        start = end;
    }
    let mut out: Vec<Corner> = Vec::new();
    for piece in pieces.iter_mut() {
        out.clear();
        let n = piece.len();
        for i in 0..n {
            let (a, b) = (piece[i], piece[(i + 1) % n]);
            out.push(a);
            let Ok(k) = lines.binary_search_by(|x| x.0.cmp(&a.next)) else { continue };
            let (_, s0, s1, p, d) = lines[k];
            let run = &on[s0..s1];
            let (ta, tb) = ((a.at - p).dot(d), (b.at - p).dot(d));
            let (lo, hi) = (ta.min(tb), ta.max(tb));
            let from = run.partition_point(|x| x.1 <= lo);
            let to = run.partition_point(|x| x.1 < hi);
            let between = run[from..to.max(from)].iter().filter(|x| x.2 != a.key && x.2 != b.key).map(|x| Corner::new(x.3, x.2, a.next));
            if ta <= tb {
                out.extend(between);
            } else {
                out.extend(between.rev());
            }
        }
        if out.len() != n {
            piece.clone_from(&out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Grid;

    impl Geometry for Grid {
        fn site(&self, _: usize, _: (i32, i32)) -> DVec2 {
            unreachable!("grid lines only")
        }

        fn line(&self, l: Line) -> (DVec2, DVec2) {
            match l {
                Line::Grid { a, b, c } => grid_line(a, b, c),
                _ => unreachable!("grid lines only"),
            }
        }
    }

    /// Which side of a grid line a point where grid lines cross is on: by
    /// `Cut::side` (in floating point where it can say) as in whole numbers
    /// (`Key::side`), over the world's span, lines up to 512 m between
    /// their points; on its own two lines and a third through the same
    /// grid point, 0; and floating point's error within the bound it
    /// trusts it by (its largest share of it printed).
    #[test]
    fn sides_exact() {
        let mut seed = 11u64;
        let mut rand = |n: i64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 11) % (2 * n as u64 + 1)) as i64 - n
        };
        let span = 1i64 << 26;
        let (mut worst, mut on, mut tried) = (0.0f64, 0, 0);
        for _ in 0..200_000 {
            let p = (rand(span), rand(span));
            let mut near = |p: (i64, i64)| (p.0 + rand(1 << 19), p.1 + rand(1 << 19));
            let (q, r) = (near(p), near(p));
            let (s, t) = (near(p), near(p));
            if q == p || r == p || s == t {
                continue;
            }
            // Through one grid point, and a pair through anywhere.
            for (l1, l2) in [(Line::grid(p, q).0, Line::grid(p, r).0), (Line::grid(p, q).0, Line::grid(s, t).0)] {
                let (Line::Grid { a: a1, b: b1, .. }, Line::Grid { a: a2, b: b2, .. }) = (l1, l2) else { unreachable!() };
                if a1 as i64 * b2 as i64 == a2 as i64 * b1 as i64 {
                    continue;
                }
                let key = Key::of(l1, l2);
                let c = Corner::new(Grid.corner(key), key, l1);
                let third = Line::grid(p, s).0;
                for l in [l1, l2, third, Line::grid(s, t).0, Line::grid(r, t).0] {
                    let cut = Cut::new(l, &Grid);
                    let exact = key.side(l).unwrap();
                    let got = cut.side(&c);
                    assert_eq!(got.partial_cmp(&0.0).unwrap() as i32, exact, "{key:?} against {l:?}: {got}");
                    on += (exact == 0) as usize;
                    tried += 1;
                    if let (Key::Point { x, y, d }, Line::Grid { a, b, c: cc }) = (key, l) {
                        let num = a as i128 * x + b as i128 * y - cc as i128 * d;
                        let truth = num as f64 / (d as f64 * GRID);
                        let float = (c.at - cut.p).dot(cut.n);
                        worst = worst.max((float - truth).abs() / (cut.err * (c.at.abs().max_element() + cut.reach)));
                    }
                }
            }
        }
        println!("{tried} sides, {on} on the line; floating point's error at most {worst:.3} of its bound");
        assert!(on > 100_000 && worst < 0.5);
    }
}
