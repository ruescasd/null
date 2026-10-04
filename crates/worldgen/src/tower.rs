//! Ordered towers, transcribed from a Manifold Garden tower: symmetric,
//! proportioned, every element architectural, nothing jittered. Bottom to
//! top: a stepped base; a plinth ribbed with pilasters; a platform carried
//! by a colonnade (each column with base and capital); the core shaft with
//! corner pilasters, a deep slot down each face and panels of fretwork (a
//! Hilbert curve: a maze line, a fractal); slender piers flanking the core,
//! tied to it at regular heights; a corbelled cornice; a crown stepping in.
//! Stairs are part of the design: a flight up two faces of the plinth to
//! the platform, and flights zigzagging up the core between balconies.
//! Optionally the same tower repeats on a lattice.

use glam::{Quat, Vec2, Vec3};

use crate::structure::Solid;

/// Step rise and run (metres).
const RISE: f32 = 0.42;
const RUN: f32 = 0.5;

/// Builds a tower `width` across and `height` tall, its base centred at
/// `origin` (world), its local x along `dir` (unit, horizontal).
pub fn build(origin: Vec3, dir: Vec2, width: f32, height: f32, tone: f32) -> Vec<Solid> {
    let mut t = Tower { out: Vec::new(), origin, ex: Vec3::new(dir.x, 0.0, dir.y), ez: Vec3::new(-dir.y, 0.0, dir.x), yaw: -dir.y.atan2(dir.x) };
    t.build(width, height, tone);
    t.out
}

/// Towers on a lattice `spacing` apart, filling a `half` box (local x and
/// z half sizes) about `origin`; heights vary by row and column in a
/// regular pattern.
pub fn lattice(origin: Vec3, dir: Vec2, half: Vec2, width: f32, height: f32, spacing: f32, tone: f32) -> Vec<Solid> {
    let mut out = Vec::new();
    let nx = ((half.x * 2.0 - width) / spacing).floor().max(0.0) as i32;
    let nz = ((half.y * 2.0 - width) / spacing).floor().max(0.0) as i32;
    let (ex, ez) = (Vec2::new(dir.x, dir.y), Vec2::new(-dir.y, dir.x));
    for i in 0..=nx {
        for j in 0..=nz {
            let u = (i as f32 - nx as f32 * 0.5) * spacing;
            let v = (j as f32 - nz as f32 * 0.5) * spacing;
            let p = Vec2::new(origin.x, origin.z) + ex * u + ez * v;
            // Taller towards the middle, in steps.
            let ring = (i - nx / 2).abs().max((j - nz / 2).abs());
            let h = height * (1.0 - 0.18 * ring as f32).max(0.4);
            out.extend(build(Vec3::new(p.x, origin.y, p.y), dir, width, h, tone));
        }
    }
    out
}

struct Tower {
    out: Vec<Solid>,
    origin: Vec3,
    ex: Vec3,
    ez: Vec3,
    yaw: f32,
}

/// The four faces: outward normal and the tangent along the face, both in
/// local (x, z).
const FACES: [(Vec2, Vec2); 4] = [
    (Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)),
    (Vec2::new(-1.0, 0.0), Vec2::new(0.0, -1.0)),
    (Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0)),
    (Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0)),
];

impl Tower {
    /// An axis-aligned box in the local frame, from `min` to `max`.
    fn b(&mut self, min: Vec3, max: Vec3, albedo: f32) {
        let (lo, hi) = (min.min(max), min.max(max));
        if (hi - lo).min_element() < 0.005 {
            return;
        }
        let c = (lo + hi) * 0.5;
        self.out.push(Solid {
            wedge: false,
            round: false,
            center: self.origin + self.ex * c.x + self.ez * c.z + Vec3::Y * c.y,
            rotation: Quat::from_rotation_y(self.yaw),
            half: (hi - lo) * 0.5,
            albedo,
        });
    }

    /// A box against a face: `dist` from the centre to the face, `n` its
    /// normal and `t` its tangent; `u` along the face, `y` up, `d` out from
    /// the face.
    #[allow(clippy::too_many_arguments)]
    fn on(&mut self, n: Vec2, t: Vec2, dist: f32, u: (f32, f32), y: (f32, f32), d: (f32, f32), albedo: f32) {
        let p = |uu: f32, yy: f32, dd: f32| {
            let q = n * (dist + dd) + t * uu;
            Vec3::new(q.x, yy, q.y)
        };
        self.b(p(u.0, y.0, d.0), p(u.1, y.1, d.1), albedo);
    }

    /// A square block centred on the axis.
    fn block(&mut self, width: f32, y0: f32, y1: f32, albedo: f32) {
        let h = width * 0.5;
        self.b(Vec3::new(-h, y0, -h), Vec3::new(h, y1, h), albedo);
    }

    /// A column standing at local (x, z): base, shaft, capital.
    fn column(&mut self, at: Vec2, w: f32, y0: f32, y1: f32, albedo: f32) {
        let h = y1 - y0;
        let base = (h * 0.07).max(0.4);
        let cap = (h * 0.06).max(0.4);
        let sq = |s: f32, a: f32, b: f32, this: &mut Self| {
            this.b(Vec3::new(at.x - s, a, at.y - s), Vec3::new(at.x + s, b, at.y + s), albedo);
        };
        sq(w * 0.75, y0, y0 + base * 0.5, self);
        sq(w * 0.62, y0 + base * 0.5, y0 + base, self);
        sq(w * 0.5, y0 + base, y1 - cap, self);
        sq(w * 0.62, y1 - cap, y1 - cap * 0.5, self);
        sq(w * 0.75, y1 - cap * 0.5, y1, self);
    }

    /// A flight of steps from `a` (local x, z, height) up to height `top`,
    /// running in direction `run` (unit, local), `width` wide, a thin
    /// stepped slab.
    fn flight(&mut self, a: Vec3, run: Vec2, top: f32, width: f32, albedo: f32) {
        let steps = ((top - a.y) / RISE).ceil().max(1.0) as i32;
        let rise = (top - a.y) / steps as f32;
        let side = Vec2::new(-run.y, run.x) * (width * 0.5);
        for s in 0..steps {
            let p0 = Vec2::new(a.x, a.z) + run * (s as f32 * RUN);
            let p1 = p0 + run * RUN;
            let y1 = a.y + rise * (s + 1) as f32;
            let lo = (p0 - side).min(p1 + side).min(p0 + side).min(p1 - side);
            let hi = (p0 - side).max(p1 + side).max(p0 + side).max(p1 - side);
            self.b(Vec3::new(lo.x, y1 - 0.9, lo.y), Vec3::new(hi.x, y1, hi.y), albedo);
        }
    }

    /// Fretwork: a Hilbert curve of the given order drawn as bars across a
    /// rectangle of a face (`u`, `y`), standing `d` proud of it, in a frame.
    #[allow(clippy::too_many_arguments)]
    fn fretwork(&mut self, n: Vec2, t: Vec2, dist: f32, u: (f32, f32), y: (f32, f32), order: u32, albedo: f32) {
        let side = 1u32 << order;
        let (w, h) = (u.1 - u.0, y.1 - y.0);
        let cell = Vec2::new(w / side as f32, h / side as f32);
        let bar = cell.min_element() * 0.32;
        let pt = |d: u32| {
            let (x, yy) = hilbert(side, d);
            Vec2::new(u.0 + (x as f32 + 0.5) * cell.x, y.0 + (yy as f32 + 0.5) * cell.y)
        };
        for d in 0..side * side - 1 {
            let (a, b) = (pt(d), pt(d + 1));
            let (lo, hi) = (a.min(b) - Vec2::splat(bar * 0.5), a.max(b) + Vec2::splat(bar * 0.5));
            self.on(n, t, dist, (lo.x, hi.x), (lo.y, hi.y), (0.0, 0.35), albedo);
        }
        // The frame.
        let f = bar * 1.6;
        self.on(n, t, dist, (u.0 - f, u.1 + f), (y.0 - f, y.0), (0.0, 0.5), albedo);
        self.on(n, t, dist, (u.0 - f, u.1 + f), (y.1, y.1 + f), (0.0, 0.5), albedo);
        self.on(n, t, dist, (u.0 - f, u.0), (y.0, y.1), (0.0, 0.5), albedo);
        self.on(n, t, dist, (u.1, u.1 + f), (y.0, y.1), (0.0, 0.5), albedo);
    }

    fn build(&mut self, w: f32, h: f32, tone: f32) {
        let light = tone + 0.05;
        let dark = (tone * 0.3).max(0.02);
        // Stepped base.
        let mut y = 0.0;
        let step = h * 0.012;
        for k in 0..3 {
            self.block(w * (1.0 - 0.05 * k as f32), y - if k == 0 { 3.0 } else { 0.0 }, y + step, tone);
            y += step;
        }
        // Plinth, ribbed with pilasters at a regular rhythm.
        let p = w * 0.8;
        let plinth_top = y + h * 0.17;
        self.block(p, y, plinth_top, tone);
        let ribs = 7;
        for (n, t) in FACES {
            for k in 0..ribs {
                let u = -p * 0.5 + p * (k as f32 + 0.5) / ribs as f32;
                self.on(n, t, p * 0.5, (u - p * 0.02, u + p * 0.02), (y, plinth_top - h * 0.01), (0.0, 0.5), tone);
            }
            // A band under the platform.
            self.on(n, t, p * 0.5, (-p * 0.5, p * 0.5), (plinth_top - h * 0.012, plinth_top), (0.0, 0.7), light);
        }
        // Stairs up two faces of the plinth: from one corner, rising along
        // the face to a landing at the other (opposite faces mirrored).
        let sw = 2.6;
        for (n, t) in [FACES[0], FACES[1]] {
            let start = n * (p * 0.5 + 0.7 + sw * 0.5) + t * (-p * 0.5);
            let length = (plinth_top - y) / RISE * RUN;
            let run_len = length.min(p + 2.0);
            let top = y + run_len / RUN * RISE;
            self.flight(Vec3::new(start.x, y, start.y), t, top, sw, light);
            // The landing, and a narrower flight on up if the face was too
            // short, back the other way, further out.
            let end = start + t * run_len;
            self.b(Vec3::new(end.x - 2.0, top - 0.9, end.y - 2.0), Vec3::new(end.x + 2.0, top, end.y + 2.0), light);
            if top < plinth_top - 0.5 {
                let back = end + n * sw;
                self.flight(Vec3::new(back.x, top, back.y), -t, plinth_top, sw, light);
            }
        }
        // The platform.
        let platform = plinth_top + h * 0.015;
        self.block(w * 0.92, plinth_top, platform, light);
        // The colonnade: columns round the platform's edge carrying the
        // entablature; the core rises through the middle.
        let colonnade_top = platform + h * 0.12;
        let cols = 5;
        let span = w * 0.8;
        let cw = w * 0.045;
        for i in 0..cols {
            for j in 0..cols {
                if i != 0 && i != cols - 1 && j != 0 && j != cols - 1 {
                    continue;
                }
                let at = Vec2::new(-span * 0.5 + span * i as f32 / (cols - 1) as f32, -span * 0.5 + span * j as f32 / (cols - 1) as f32);
                self.column(at, cw, platform, colonnade_top, tone);
            }
        }
        let entablature = colonnade_top + h * 0.02;
        self.block(w * 0.88, colonnade_top, entablature, light);
        self.block(w * 0.84, entablature, entablature + h * 0.006, tone);

        // The core shaft.
        let c = w * 0.4;
        let shaft_top = h * 0.82;
        let slot = c * 0.16;
        for (fi, (n, t)) in FACES.into_iter().enumerate() {
            // Two halves with a deep slot between, a dark back wall.
            self.on(n, t, c * 0.5 - 2.5, (-c * 0.5, -slot * 0.5), (platform, shaft_top), (0.0, 2.5), tone);
            self.on(n, t, c * 0.5 - 2.5, (slot * 0.5, c * 0.5), (platform, shaft_top), (0.0, 2.5), tone);
            self.on(n, t, c * 0.5 - 4.5, (-slot * 0.5, slot * 0.5), (platform, shaft_top), (0.0, 2.0), dark);
            // Corner pilasters.
            for s in [-1.0, 1.0] {
                let u0 = s * c * 0.5;
                let u1 = s * (c * 0.5 - c * 0.08);
                self.on(n, t, c * 0.5, (u0.min(u1), u0.max(u1)), (entablature, shaft_top), (0.0, 0.6), light);
            }
            // Fretwork panels either side of the slot, high up, on the faces
            // without stairs.
            if fi < 2 {
                let (y0, y1) = (h * 0.6, h * 0.76);
                let inner = slot * 0.5 + c * 0.05;
                let outer = c * 0.5 - c * 0.13;
                self.fretwork(n, t, c * 0.5, (-outer, -inner), (y0, y1), 3, light);
                self.fretwork(n, t, c * 0.5, (inner, outer), (y0, y1), 3, light);
            }
            // Slender piers flanking the core, tied to it.
            let pw = w * 0.045;
            for s in [-1.0, 1.0] {
                let u = s * (c * 0.5 + w * 0.07);
                let at = n * (c * 0.25) + t * u;
                self.column(at, pw, entablature, h * 0.78, tone);
                let mut yy = entablature + h * 0.1;
                while yy < h * 0.76 {
                    let (a, b) = (c * 0.5, c * 0.5 + w * 0.07);
                    let (ua, ub) = if s > 0.0 { (a, b) } else { (-b, -a) };
                    self.on(n, t, c * 0.25 - pw * 0.5, (ua, ub), (yy, yy + h * 0.008), (0.0, pw), tone);
                    yy += h * 0.1;
                }
            }
        }
        // Body of the core behind the faces.
        self.block(c - 5.0, platform, shaft_top, dark);

        // Stairs zigzagging up the two faces without fretwork, outside the
        // piers, between balconies at each end.
        let pw = w * 0.045;
        let clear = w * 0.07 + pw * 0.5 + 1.6;
        for (n, t) in [FACES[2], FACES[3]] {
            let mut y = entablature;
            let mut dir = 1.0;
            let reach = c * 0.5 + w * 0.07;
            for k in 0..4 {
                let out = c * 0.5 + clear + if k % 2 == 1 { 2.8 } else { 0.0 };
                let start = n * out + t * (-reach * dir);
                let rise = (reach * 2.0) / RUN * RISE;
                let top = (y + rise).min(shaft_top - 3.0);
                if top - y < 2.0 {
                    break;
                }
                self.flight(Vec3::new(start.x, y, start.y), t * dir, top, 2.4, light);
                // A balcony at the end, reaching back to the face.
                let end = start + t * (reach * 2.0 * dir);
                let (u0, u1) = (end.dot(t) - 2.5, end.dot(t) + 2.5);
                self.on(n, t, c * 0.5, (u0, u1), (top - 0.6, top), (0.0, clear + 2.8 + 1.3), light);
                y = top;
                dir = -dir;
            }
        }

        // Corbelled cornice, then the crown stepping in.
        let mut y = shaft_top;
        for k in 0..4 {
            let cw = c + w * 0.04 * (k + 1) as f32;
            self.block(cw, y, y + h * 0.008, if k % 2 == 0 { light } else { tone });
            y += h * 0.008;
        }
        for k in 0..3 {
            let cw = c * (0.9 - 0.2 * k as f32);
            let top = y + h * 0.035;
            self.block(cw, y, top, tone);
            // Its own small corbel.
            self.block(cw + w * 0.02, top - h * 0.004, top, light);
            y = top;
        }
        self.block(c * 0.18, y, y + h * 0.05, light);
    }
}

/// The point at distance `d` along a Hilbert curve filling an `n` by `n`
/// grid (`n` a power of two).
fn hilbert(n: u32, d: u32) -> (u32, u32) {
    let (mut x, mut y, mut t) = (0u32, 0u32, d);
    let mut s = 1;
    while s < n {
        let rx = 1 & (t / 2);
        let ry = 1 & (t ^ rx);
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x += s * rx;
        y += s * ry;
        t /= 4;
        s *= 2;
    }
    (x, y)
}
