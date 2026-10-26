//! Cuts into a round wall (the void's cylinders, for the scale lab): each a
//! recess of its own depth, a run of legs up the wall (straight, leaning or
//! jogging: a seam's piece), or a band all round it (a course's joint). The wall is meshed round them, row by row, never bored out of a
//! solid: each row between heights where something starts, ends or turns a
//! corner, in it each stretch at its depth, walls between stretches, and
//! where one row meets the next, ledges where their depths differ.
//!
//! Along the wall, `s`: metres round its outline from its first corner
//! (counter-clockwise, as `void::ring` draws it), 0 to its perimeter.

use glam::{DVec2, DVec3};

use super::Builder;

/// A polygonal wall: `sides` corners round `c`, `radius` out.
pub struct Wall {
    pub c: DVec2,
    pub radius: f64,
    pub sides: usize,
}

/// A cut's leg: from `y.0` up to `y.1`, its left edge at `s.0` along the
/// wall at the foot and `s.1` at the top.
#[derive(Clone, Copy)]
pub struct Leg {
    pub y: (f64, f64),
    pub s: (f64, f64),
}

/// A cut: its legs (one above the next, meeting edge to edge), so wide, so
/// deep.
pub struct Cut {
    pub legs: Vec<Leg>,
    pub wide: f64,
    pub deep: f64,
}

/// A band round the whole wall, from `y.0` to `y.1`, so deep.
#[derive(Clone, Copy)]
pub struct Band {
    pub y: (f64, f64),
    pub deep: f64,
}

/// A stretch of a row: its left and right edges (along the wall at the
/// row's foot and top), how deep.
#[derive(Clone, Copy)]
struct Stretch {
    l: (f64, f64),
    r: (f64, f64),
    deep: f64,
}

const EPS: f64 = 1e-6;

impl Wall {
    /// The perimeter's sides' length.
    pub fn side(&self) -> f64 {
        2.0 * self.radius * (std::f64::consts::PI / self.sides as f64).sin()
    }

    pub fn perimeter(&self) -> f64 {
        self.side() * self.sides as f64
    }

    fn corner(&self, k: usize) -> DVec2 {
        DVec2::from_angle((k % self.sides) as f64 * std::f64::consts::TAU / self.sides as f64) * self.radius
    }

    /// The side `s` is on, and how far along it (0..1).
    fn on(&self, s: f64) -> (usize, f64) {
        let x = s / self.side();
        let k = (x.floor().max(0.0) as usize).min(self.sides - 1);
        (k, x - k as f64)
    }

    /// The point `s` along the wall, `deep` in from it, at `y`.
    fn at(&self, s: f64, deep: f64, y: f64) -> DVec3 {
        let (k, t) = self.on(s);
        let p = self.c + self.corner(k).lerp(self.corner(k + 1), t) * ((self.radius - deep) / self.radius);
        DVec3::new(p.x, y, p.y)
    }

    /// The way along the wall at `s` (its side's).
    fn tangent(&self, s: f64) -> DVec3 {
        let (k, _) = self.on(s);
        let d = (self.corner(k + 1) - self.corner(k)).normalize();
        DVec3::new(d.x, 0.0, d.y)
    }

    /// The corners strictly between `a` and `b` along the wall.
    fn corners(&self, a: f64, b: f64) -> impl Iterator<Item = f64> + '_ {
        let side = self.side();
        (1..self.sides).map(move |k| k as f64 * side).filter(move |&s| s > a + EPS && s < b - EPS)
    }

    /// The wall from `y.0` up to `y.1`, with `cuts` and `bands` cut into
    /// it (none overlapping another, all within its height).
    pub fn build(&self, b: &mut Builder, y: (f64, f64), cuts: &[Cut], bands: &[Band]) {
        let edge = |l: &Leg, at: f64, off: f64| l.s.0 + (l.s.1 - l.s.0) * (at - l.y.0) / (l.y.1 - l.y.0) + off;
        // The rows' heights: the wall's ends; where each cut's legs and each
        // band start and end; where a leaning leg's edges pass a corner.
        let mut ys = vec![y.0, y.1];
        for band in bands {
            ys.extend([band.y.0, band.y.1]);
        }
        for cut in cuts {
            for l in &cut.legs {
                ys.extend([l.y.0, l.y.1]);
                for off in [0.0, cut.wide] {
                    let (a, z) = (l.s.0 + off, l.s.1 + off);
                    if (z - a).abs() > EPS {
                        for s in self.corners(a.min(z), a.max(z)) {
                            ys.push(l.y.0 + (l.y.1 - l.y.0) * (s - a) / (z - a));
                        }
                    }
                }
            }
        }
        ys.retain(|&h| h >= y.0 - EPS && h <= y.1 + EPS);
        ys.sort_by(f64::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
        // Each row's stretches, at its foot and top.
        let rows: Vec<(f64, f64, Vec<Stretch>)> = ys
            .windows(2)
            .map(|w| {
                let (lo, hi) = (w[0], w[1]);
                let mid = (lo + hi) * 0.5;
                let base = bands.iter().find(|band| band.y.0 <= mid && band.y.1 >= mid).map_or(0.0, |band| band.deep);
                let mut holes: Vec<Stretch> = cuts
                    .iter()
                    .flat_map(|cut| cut.legs.iter().map(move |l| (cut, l)))
                    .filter(|(_, l)| l.y.0 <= mid && l.y.1 >= mid)
                    .map(|(cut, l)| Stretch { l: (edge(l, lo, 0.0), edge(l, hi, 0.0)), r: (edge(l, lo, cut.wide), edge(l, hi, cut.wide)), deep: cut.deep })
                    .collect();
                holes.sort_by(|a, z| a.l.0.total_cmp(&z.l.0));
                let mut stretches = Vec::new();
                let mut from = (0.0, 0.0);
                for h in holes {
                    stretches.push(Stretch { l: from, r: h.l, deep: base });
                    from = h.r;
                    stretches.push(h);
                }
                let end = self.perimeter();
                stretches.push(Stretch { l: from, r: (end, end), deep: base });
                (lo, hi, stretches)
            })
            .collect();
        for (lo, hi, stretches) in &rows {
            for (n, st) in stretches.iter().enumerate() {
                // Its face, in pieces between the corners.
                let mut cols = vec![(st.l.0, st.l.1)];
                cols.extend(self.corners(st.l.0.max(st.l.1), st.r.0.min(st.r.1)).map(|s| (s, s)));
                cols.push((st.r.0, st.r.1));
                for w in cols.windows(2) {
                    let (a, z) = (w[0], w[1]);
                    if (z.0 - a.0).max(z.1 - a.1) < EPS {
                        continue;
                    }
                    let q = [self.at(a.0, st.deep, *lo), self.at(z.0, st.deep, *lo), self.at(z.1, st.deep, *hi), self.at(a.1, st.deep, *hi)];
                    let mid = (q[0] + q[1] + q[2] + q[3]) * 0.25;
                    let c = self.c;
                    let out = move |p: DVec3| DVec3::new(p.x - c.x, 0.0, p.z - c.y);
                    b.smooth(DVec2::new(q[0].x, q[0].z), out, |b| b.quad(q, out(mid)));
                }
                // The wall from it to the next, if their depths differ,
                // facing the deeper.
                if let Some(next) = stretches.get(n + 1).filter(|next| (next.deep - st.deep).abs() > EPS) {
                    let (a, z) = (st.deep.min(next.deep), st.deep.max(next.deep));
                    let e = st.r;
                    let q = [self.at(e.0, a, *lo), self.at(e.0, z, *lo), self.at(e.1, z, *hi), self.at(e.1, a, *hi)];
                    let towards = if next.deep > st.deep { 1.0 } else { -1.0 };
                    b.quad(q, self.tangent((e.0 + e.1) * 0.5) * towards);
                }
            }
        }
        // Where one row meets the next: a ledge wherever their depths
        // differ, facing down under the shallower above, up over it below.
        for pair in rows.windows(2) {
            let (below, above) = (&pair[0].2, &pair[1].2);
            let h = pair[0].1;
            let mut ss: Vec<f64> = below.iter().flat_map(|st| [st.l.1, st.r.1]).chain(above.iter().flat_map(|st| [st.l.0, st.r.0])).collect();
            ss.extend(self.corners(0.0, self.perimeter()));
            ss.sort_by(f64::total_cmp);
            ss.dedup_by(|a, b| (*a - *b).abs() < EPS);
            let deep = |row: &[Stretch], s: f64, top: bool| row.iter().find(|st| if top { st.l.1 <= s && st.r.1 >= s } else { st.l.0 <= s && st.r.0 >= s }).map_or(0.0, |st| st.deep);
            for w in ss.windows(2) {
                let (a, z) = (w[0], w[1]);
                let mid = (a + z) * 0.5;
                let (d0, d1) = (deep(below, mid, true), deep(above, mid, false));
                if (d0 - d1).abs() < EPS {
                    continue;
                }
                let (near, far) = (d0.min(d1), d0.max(d1));
                let q = [self.at(a, near, h), self.at(z, near, h), self.at(z, far, h), self.at(a, far, h)];
                b.quad(q, if d0 > d1 { -DVec3::Y } else { DVec3::Y });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_stay_within_the_wall() {
        let c = DVec2::new(100.0, 200.0);
        let wall = Wall { c, radius: 22.0, sides: 32 };
        let mut b = Builder::new(c);
        let cut = |s: f64, shift: f64| Cut { legs: vec![Leg { y: (10.0, 40.0), s: (s + shift, s) }], wide: 1.4, deep: 1.6 };
        wall.build(&mut b, (0.0, 50.0), &[cut(83.8, 10.8), cut(86.4, 10.8)], &[Band { y: (20.0, 20.3), deep: 0.25 }]);
        let patch = b.finish();
        // Every way out from the middle meets the wall (no holes in it).
        let tris: Vec<[DVec3; 3]> = patch.pieces.iter().flat_map(|m| m.indices.chunks(3).map(|t| [0, 1, 2].map(|k| { let p = m.positions[t[k] as usize]; DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) })).collect::<Vec<_>>()).collect();
        let hit = |o: DVec3, d: DVec3| tris.iter().any(|t| {
            let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
            let p = d.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-12 { return false; }
            let s = o - t[0];
            let u = s.dot(p) / det;
            let q = s.cross(e1);
            let v = d.dot(q) / det;
            u >= -1e-9 && v >= -1e-9 && u + v <= 1.0 + 1e-9 && e2.dot(q) / det > 0.0
        });
        let mut holes = Vec::new();
        for a in 0..720 {
            for h in 0..100 {
                let (a, y) = (a as f64 * 0.5_f64.to_radians() + 0.0013, 0.37 + h as f64 * 0.5);
                if !hit(DVec3::new(c.x, y, c.y), DVec3::new(a.cos(), 0.0, a.sin())) {
                    holes.push((a.to_degrees(), y));
                }
            }
        }
        assert!(holes.is_empty(), "{} holes, e.g. {:?}", holes.len(), &holes[..holes.len().min(10)]);
        for m in &patch.pieces {
            for p in &m.positions {
                let r = DVec2::new(p[0] as f64, p[2] as f64).distance(c);
                assert!((22.0 - 1.6 - 0.1 - 1e-3..=22.0 + 1e-3).contains(&r), "{p:?} at {r}");
            }
        }
    }
}
