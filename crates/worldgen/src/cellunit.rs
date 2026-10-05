//! What stands on a lattice cell's deck (see `lattice.rs`), composed with
//! the cell as its frame: the rim kept as a walk round the edge, and inside
//! it a few pieces from a varied vocabulary rather than one kind of
//! building: halls with arcades below and a giant order above, slender
//! towers with open belvederes and domes, stepped or spired caps,
//! colonnades and stoas on round columns, rotundas, pergolas, and terraces
//! climbed by a broad stair on the axis. Each kind of cell composes them
//! differently, with heights and choices varied cell by cell.
//!
//! On the composer's grid (6 m bays, 4.5 m storeys, 0.45 m steps), in the
//! cell's own frame: the deck's top at y = 0, x and z centred, x along the
//! cell.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Quat, Vec2, Vec3};

use crate::noise::hash01;
use crate::structure::Solid;

const STOREY: f32 = 4.5;
/// The walk round the rim (metres).
const RIM: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Plaza,
    Court,
    Terraces,
    Gallery,
    Tower,
}

impl Kind {
    pub fn from_name(name: &str) -> Option<Kind> {
        Some(match name {
            "plaza" => Kind::Plaza,
            "court" => Kind::Court,
            "terraces" => Kind::Terraces,
            "gallery" => Kind::Gallery,
            "tower" => Kind::Tower,
            _ => return None,
        })
    }
}

/// The pieces' drawing kit.
struct Kit {
    out: Vec<Solid>,
    tone: f32,
    seed: u32,
    salt: i32,
}

impl Kit {
    /// The next random number of this cell's sequence.
    fn r(&mut self) -> f32 {
        self.salt += 1;
        hash01(self.salt, 3, 0xce13, self.seed)
    }

    fn pick<T: Copy>(&mut self, options: &[T]) -> T {
        options[((self.r() * options.len() as f32) as usize).min(options.len() - 1)]
    }

    fn solid(&mut self, center: Vec3, half: Vec3, rotation: Quat, shade: f32, detail: bool, round: bool) {
        if half.min_element() < 0.005 {
            return;
        }
        self.out.push(Solid { detail, wedge: false, round, center, rotation, half, albedo: self.tone + shade });
    }

    /// A box from `lo` to `hi`.
    fn span(&mut self, lo: Vec3, hi: Vec3, shade: f32) {
        let (a, b) = (lo.min(hi), lo.max(hi));
        self.solid((a + b) * 0.5, (b - a) * 0.5, Quat::IDENTITY, shade, false, false);
    }

    /// A fine piece (left out from afar).
    fn fine(&mut self, lo: Vec3, hi: Vec3, shade: f32) {
        let (a, b) = (lo.min(hi), lo.max(hi));
        self.solid((a + b) * 0.5, (b - a) * 0.5, Quat::IDENTITY, shade, true, false);
    }

    /// An upright cylinder standing on `base`.
    fn cylinder(&mut self, base: Vec3, height: f32, radius: f32, shade: f32) {
        self.solid(base + Vec3::Y * (height * 0.5), Vec3::new(height * 0.5, radius, radius), Quat::from_rotation_z(FRAC_PI_2), shade, false, true);
    }

    /// A round column: a square plinth, the shaft, a square capital.
    fn column(&mut self, base: Vec3, height: f32, radius: f32, shade: f32) {
        let p = radius * 1.45;
        self.span(base + Vec3::new(-p, 0.0, -p), base + Vec3::new(p, 0.5, p), shade);
        self.cylinder(base + Vec3::Y * 0.5, height - 1.0, radius, shade + 0.02);
        self.span(base + Vec3::new(-p, height - 0.5, -p), base + Vec3::new(p, height, p), shade);
    }

    /// A low solid parapet along `a`..`b` (on their height).
    fn parapet(&mut self, a: Vec3, b: Vec3, shade: f32) {
        let pad = if (b - a).x.abs() > (b - a).z.abs() { Vec3::new(0.0, 0.0, 0.22) } else { Vec3::new(0.22, 0.0, 0.0) };
        self.span(a.min(b) - pad, a.max(b) + pad + Vec3::Y * 0.95, shade);
        // A coping, a little wider.
        self.fine(a.min(b) - pad * 1.5 + Vec3::Y * 0.95, a.max(b) + pad * 1.5 + Vec3::Y * 1.1, shade + 0.03);
    }

    /// A box in a face's frame: `u0..u1` along `a`→`b`, `y0..y1`, `d0..d1`
    /// in from the face (towards -`out`).
    #[allow(clippy::too_many_arguments)]
    fn face_box(&mut self, a: Vec3, along: Vec3, out: Vec3, (u0, u1): (f32, f32), (y0, y1): (f32, f32), (d0, d1): (f32, f32), shade: f32, fine: bool) {
        let p = a + along * u0 - out * d0;
        let q = a + along * u1 - out * d1;
        let (lo, hi) = (Vec3::new(p.x.min(q.x), y0, p.z.min(q.z)), Vec3::new(p.x.max(q.x), y1, p.z.max(q.z)));
        if fine { self.fine(lo, hi, shade) } else { self.span(lo, hi, shade) }
    }

    /// An arcade along the face `a`→`b` (facing `out`): `n` round arches on
    /// piers from `y0`, `height` tall, `depth` deep, a band over them.
    #[allow(clippy::too_many_arguments)]
    fn arcade(&mut self, a: Vec3, b: Vec3, out: Vec3, n: usize, y0: f32, height: f32, depth: f32, shade: f32) {
        let len = (b - a).length();
        if len < 1.0 || n == 0 {
            return;
        }
        let along = (b - a) / len;
        let bay = len / n as f32;
        let pier = (bay * 0.2).clamp(0.6, 2.5);
        let band = (height * 0.1).clamp(0.5, 1.5);
        let rad = (bay - pier) * 0.5;
        let spring = (y0 + height - band - rad * 1.05).max(y0 + 1.0);
        for k in 0..=n {
            let u = k as f32 * bay;
            let (u0, u1) = ((u - pier * 0.5).max(0.0), (u + pier * 0.5).min(len));
            self.face_box(a, along, out, (u0, u1), (y0, y0 + height - band), (0.0, depth), shade, false);
        }
        self.face_box(a, along, out, (0.0, len), (y0 + height - band, y0 + height), (-0.15, depth), shade + 0.02, false);
        for k in 0..n {
            let c = (k as f32 + 0.5) * bay;
            let strips = 8;
            for j in 0..strips {
                let v0 = -rad + 2.0 * rad * j as f32 / strips as f32;
                let v1 = v0 + 2.0 * rad / strips as f32;
                let vi = if v0.abs() < v1.abs() { v0 } else { v1 };
                let arc = (spring + (rad * rad - vi * vi).max(0.0).sqrt()).min(y0 + height - band);
                self.face_box(a, along, out, (c + v0 - 0.01, c + v1 + 0.01), (arc, y0 + height - band + 0.01), (0.0, depth), shade, true);
            }
        }
    }

    /// A cap over a square top centred on `top` with half size `half`.
    fn cap(&mut self, top: Vec3, half: f32, style: u32, shade: f32) {
        match style {
            // Stepped: three setbacks and a lantern.
            0 => {
                let mut y = top.y;
                let mut h = half;
                for k in 0..3 {
                    let tall = half * (0.35 - 0.06 * k as f32);
                    self.span(Vec3::new(top.x - h, y, top.z - h), Vec3::new(top.x + h, y + tall, top.z + h), shade + 0.01 * k as f32);
                    y += tall;
                    h *= 0.68;
                }
                self.span(Vec3::new(top.x - h * 0.4, y, top.z - h * 0.4), Vec3::new(top.x + h * 0.4, y + half * 0.8, top.z + h * 0.4), shade + 0.04);
            }
            // A dome on a drum.
            1 => {
                let r = half * 0.85;
                self.cylinder(top, half * 0.25, r, shade);
                let mut y = top.y + half * 0.25;
                let tiers = 6;
                for k in 0..tiers {
                    let t0 = k as f32 / tiers as f32;
                    let t1 = (k + 1) as f32 / tiers as f32;
                    let rr = r * (1.0 - t0 * t0).sqrt();
                    let dh = r * (t1 - t0);
                    self.cylinder(Vec3::new(top.x, y, top.z), dh, rr, shade + 0.02);
                    y += dh;
                }
                self.cylinder(Vec3::new(top.x, y, top.z), half * 0.4, r * 0.08, shade + 0.04);
            }
            // A slender spire of fine setbacks.
            _ => {
                let mut y = top.y;
                let tiers = 10;
                for k in 0..tiers {
                    let h = half * 0.7 * (1.0 - k as f32 / tiers as f32);
                    let tall = half * 0.35;
                    self.span(Vec3::new(top.x - h, y, top.z - h), Vec3::new(top.x + h, y + tall, top.z + h), shade + 0.01 * (k % 3) as f32);
                    y += tall;
                }
            }
        }
    }

    /// A flight from `foot` going `dir`, `width` wide, up `rise` (steps
    /// 0.45 high, 0.6 deep, solid to the foot's height).
    fn stair(&mut self, foot: Vec3, dir: Vec3, width: f32, rise: f32, shade: f32) {
        let steps = (rise / 0.45).round().max(1.0) as i32;
        let across = Vec3::new(-dir.z, 0.0, dir.x) * (width * 0.5);
        for k in 0..steps {
            let a = foot + dir * (0.6 * k as f32) - across;
            let b = foot + dir * (0.6 * (k + 1) as f32 + 0.02) + across;
            self.fine(Vec3::new(a.x, foot.y, a.z), Vec3::new(b.x, foot.y + 0.45 * (k + 1) as f32, b.z), shade);
        }
    }
}

/// A rect on the deck (metres): min and max corners.
#[derive(Clone, Copy, Debug)]
struct Rect {
    lo: Vec2,
    hi: Vec2,
}

impl Rect {
    fn new(x0: f32, z0: f32, x1: f32, z1: f32) -> Self {
        Rect { lo: Vec2::new(x0.min(x1), z0.min(z1)), hi: Vec2::new(x0.max(x1), z0.max(z1)) }
    }
    fn size(&self) -> Vec2 {
        self.hi - self.lo
    }
    fn center(&self) -> Vec2 {
        (self.lo + self.hi) * 0.5
    }
    /// Its four sides as (start, end, outward), at height `y`.
    fn sides(&self, y: f32) -> [(Vec3, Vec3, Vec3); 4] {
        let (a, b) = (self.lo, self.hi);
        [
            (Vec3::new(a.x, y, a.y), Vec3::new(b.x, y, a.y), Vec3::NEG_Z),
            (Vec3::new(b.x, y, a.y), Vec3::new(b.x, y, b.y), Vec3::X),
            (Vec3::new(b.x, y, b.y), Vec3::new(a.x, y, b.y), Vec3::Z),
            (Vec3::new(a.x, y, b.y), Vec3::new(a.x, y, a.y), Vec3::NEG_X),
        ]
    }
}

/// Arches that fit a length at about `bay` metres each.
fn arches_for(len: f32, bay: f32) -> usize {
    ((len / bay).round() as usize).max(1)
}

/// A hall: an arcade below (double height when tall enough) with a gallery
/// behind it, a giant order of pilasters above, a cornice, a parapet and
/// sometimes a pergola on the roof.
fn hall(k: &mut Kit, r: Rect, storeys: i32) {
    let h = storeys.max(1) as f32 * STOREY;
    let ground = if storeys >= 3 { 2.0 * STOREY } else { STOREY };
    let deep = 3.0;
    let bay = k.pick(&[5.0, 6.0, 7.5]);
    // The arcade's gallery: a core set back behind the arches.
    let core = Rect::new(r.lo.x + deep, r.lo.y + deep, r.hi.x - deep, r.hi.y - deep);
    if core.size().min_element() > 1.0 {
        k.span(Vec3::new(core.lo.x, 0.0, core.lo.y), Vec3::new(core.hi.x, ground, core.hi.y), 0.0);
    }
    for (a, b, out) in r.sides(0.0) {
        let n = arches_for((b - a).length(), bay);
        k.arcade(a, b, out, n, 0.0, ground, 1.2, 0.03);
    }
    // The gallery's ceiling.
    k.span(Vec3::new(r.lo.x, ground - 0.6, r.lo.y), Vec3::new(r.hi.x, ground, r.hi.y), 0.01);
    if h > ground + 0.1 {
        // The upper mass, a giant order of pilasters, a cornice.
        k.span(Vec3::new(r.lo.x + 0.4, ground, r.lo.y + 0.4), Vec3::new(r.hi.x - 0.4, h - 1.0, r.hi.y - 0.4), 0.0);
        for (a, b, out) in r.sides(0.0) {
            let len = (b - a).length();
            let along = (b - a) / len;
            let n = arches_for(len, bay);
            for i in 0..=n {
                let u = i as f32 * len / n as f32;
                k.face_box(a, along, out, ((u - 0.5).max(0.0), (u + 0.5).min(len)), (ground, h - 1.0), (-0.1, 0.4), 0.04, false);
            }
            // Tall slots between them: dark recesses.
            for i in 0..n {
                let (u0, u1) = (i as f32 * len / n as f32, (i + 1) as f32 * len / n as f32);
                let w = (u1 - u0) * 0.22;
                let c = (u0 + u1) * 0.5;
                k.face_box(a, along, out, (c - w, c + w), (ground + 1.2, h - 2.2), (0.35, 0.45), -0.06, true);
            }
        }
        let c = 0.8;
        k.span(Vec3::new(r.lo.x - c * 0.5, h - 1.0, r.lo.y - c * 0.5), Vec3::new(r.hi.x + c * 0.5, h, r.hi.y + c * 0.5), 0.05);
    }
    for (a, b, _) in r.sides(h) {
        k.parapet(a, b, 0.04);
    }
    if k.r() < 0.45 {
        let s = r.size();
        let p = Rect::new(r.lo.x + 2.0, r.lo.y + 2.0, r.lo.x + 2.0 + s.x * 0.5, r.hi.y - 2.0);
        pergola(k, p, h);
    }
}

/// A slender tower: a shaft with corner piers and bands, an open belvedere
/// of arches at the top, and a cap (stepped, domed or spired).
fn tower(k: &mut Kit, c: Vec2, half: f32, h: f32) {
    let open = (half * 1.2).clamp(4.0, 9.0);
    let shaft = (h - open).max(STOREY);
    let (lo, hi) = (Vec3::new(c.x - half, 0.0, c.y - half), Vec3::new(c.x + half, shaft, c.y + half));
    k.span(lo + Vec3::new(0.5, 0.0, 0.5), hi - Vec3::new(0.5, 0.0, 0.5), 0.0);
    let pier = (half * 0.3).clamp(0.8, 2.5);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let p = Vec3::new(c.x + sx * (half - pier * 0.5), 0.0, c.y + sz * (half - pier * 0.5));
        k.span(p + Vec3::new(-pier * 0.5, 0.0, -pier * 0.5), p + Vec3::new(pier * 0.5, shaft, pier * 0.5), 0.03);
    }
    // Bands every other storey.
    let mut y = 2.0 * STOREY;
    while y < shaft - 1.0 {
        k.fine(Vec3::new(c.x - half - 0.25, y - 0.35, c.y - half - 0.25), Vec3::new(c.x + half + 0.25, y, c.y + half + 0.25), 0.05);
        y += 2.0 * STOREY;
    }
    // The belvedere: arches all round, a floor and a roof.
    let r = Rect::new(c.x - half, c.y - half, c.x + half, c.y + half);
    let n = if half > 5.0 { 2 } else { 1 };
    for (a, b, out) in r.sides(shaft) {
        k.arcade(a, b, out, n, shaft, open, 0.9, 0.04);
    }
    k.span(Vec3::new(c.x - half - 0.4, shaft + open - 0.2, c.y - half - 0.4), Vec3::new(c.x + half + 0.4, shaft + open + 0.6, c.y + half + 0.4), 0.05);
    let style = k.pick(&[0, 1, 2, 1]);
    k.cap(Vec3::new(c.x, shaft + open + 0.6, c.y), half, style, 0.03);
}

/// A colonnade along `a`→`b`: round columns, an entablature; with `stoa`,
/// a second row `depth` behind (towards -`out`) and a roof over both.
#[allow(clippy::too_many_arguments)]
fn colonnade(k: &mut Kit, a: Vec3, b: Vec3, out: Vec3, height: f32, stoa: bool, depth: f32) {
    let len = (b - a).length();
    let along = (b - a) / len;
    let n = ((len / 4.0).round() as i32).max(1);
    let radius = (height * 0.055).clamp(0.3, 0.8);
    let rows: &[f32] = if stoa { &[0.0, 1.0] } else { &[0.0] };
    for &row in rows {
        for i in 0..=n {
            let p = a + along * (len * i as f32 / n as f32) - out * (row * depth);
            k.column(p, height, radius, 0.05);
        }
    }
    let w = radius * 1.6;
    let back = if stoa { depth } else { 0.0 };
    k.face_box(a, along, out, (-w, len + w), (height, height + 1.1), (-w, back + w), 0.04, false);
    if stoa {
        k.face_box(a, along, out, (-w, len + w), (height + 1.1, height + 1.5), (-w - 0.3, back + w + 0.3), 0.06, false);
    }
}

/// A rotunda: a drum ringed by columns, an entablature ring, a dome.
fn rotunda(k: &mut Kit, c: Vec2, radius: f32, height: f32) {
    let base = Vec3::new(c.x, 0.0, c.y);
    // A plinth with steps all round.
    k.cylinder(base, 1.0, radius + 2.0, 0.02);
    k.cylinder(base, 0.55, radius + 2.6, 0.01);
    let base = base + Vec3::Y * 1.0;
    k.cylinder(base, height, radius * 0.62, 0.0);
    let n = ((TAU * radius / 3.5).round() as i32).clamp(8, 20);
    let col = (height * 0.05).clamp(0.3, 0.7);
    for i in 0..n {
        let a = TAU * i as f32 / n as f32;
        k.column(base + Vec3::new(a.cos(), 0.0, a.sin()) * radius, height, col, 0.05);
    }
    // The entablature: segments round the ring.
    for i in 0..n {
        let a = TAU * (i as f32 + 0.5) / n as f32;
        let seg = TAU * radius / n as f32 * 0.5 + 0.3;
        let p = base + Vec3::new(a.cos(), 0.0, a.sin()) * radius + Vec3::Y * (height + 0.5);
        k.solid(p, Vec3::new(1.0, 0.5, seg), Quat::from_rotation_y(-a), 0.04, false, false);
    }
    k.cap(base + Vec3::Y * (height + 1.0), radius * 0.75, 1, 0.03);
}

/// A pergola over `r` from `y`: posts round the edge, beams across.
fn pergola(k: &mut Kit, r: Rect, y: f32) {
    let h = 3.6;
    let s = r.size();
    if s.min_element() < 3.0 {
        return;
    }
    let (nx, nz) = (((s.x / 4.0).round() as i32).max(1), ((s.y / 4.0).round() as i32).max(1));
    for i in 0..=nx {
        for j in [0, nz] {
            let p = Vec3::new(r.lo.x + s.x * i as f32 / nx as f32, y, r.lo.y + s.y * j as f32 / nz as f32);
            k.fine(p + Vec3::new(-0.2, 0.0, -0.2), p + Vec3::new(0.2, h, 0.2), 0.06);
        }
    }
    for j in 0..=nz {
        let z = r.lo.y + s.y * j as f32 / nz as f32;
        k.fine(Vec3::new(r.lo.x - 0.3, y + h, z - 0.15), Vec3::new(r.hi.x + 0.3, y + h + 0.35, z + 0.15), 0.07);
    }
    let beams = ((s.x / 1.2) as i32).max(2);
    for i in 0..=beams {
        let x = r.lo.x + s.x * i as f32 / beams as f32;
        k.fine(Vec3::new(x - 0.1, y + h + 0.35, r.lo.y - 0.5), Vec3::new(x + 0.1, y + h + 0.6, r.hi.y + 0.5), 0.07);
    }
}

/// Terraces stepping up along +z from the front of `r`, `levels` high, a
/// broad stair on the axis between each, the risers arcaded.
fn terraces(k: &mut Kit, r: Rect, levels: i32, cx: f32) {
    let levels = levels.max(1);
    let s = r.size();
    let deep = s.y / (levels + 1) as f32;
    let wide = (s.x * 0.22).clamp(6.0, 14.0);
    for i in 1..=levels {
        let z0 = r.lo.y + deep * i as f32;
        let y = i as f32 * STOREY;
        k.span(Vec3::new(r.lo.x, 0.0, z0), Vec3::new(r.hi.x, y, r.hi.y), 0.0);
        // Arched niches in the riser, either side of the stair.
        let face = Vec3::new(r.lo.x, y - STOREY, z0);
        let left = cx - wide * 0.5 - r.lo.x;
        let right = r.hi.x - (cx + wide * 0.5);
        if left > 3.0 {
            k.arcade(face, face + Vec3::X * left, Vec3::NEG_Z, arches_for(left, 4.5), y - STOREY, STOREY, -0.8, 0.03);
        }
        if right > 3.0 {
            let f = Vec3::new(cx + wide * 0.5, y - STOREY, z0);
            k.arcade(f, f + Vec3::X * right, Vec3::NEG_Z, arches_for(right, 4.5), y - STOREY, STOREY, -0.8, 0.03);
        }
        // Its sides above the terrace below: arched niches too.
        for (x, out) in [(r.lo.x, Vec3::NEG_X), (r.hi.x, Vec3::X)] {
            let (a, b) = (Vec3::new(x, y - STOREY, z0), Vec3::new(x, y - STOREY, r.hi.y));
            let (a, b) = if out.x < 0.0 { (b, a) } else { (a, b) };
            k.arcade(a, b, out, arches_for(r.hi.y - z0, 4.5), y - STOREY, STOREY, -0.8, 0.03);
        }
        // The stair up to it, in front of the riser.
        let steps_run = STOREY / 0.45 * 0.6;
        k.stair(Vec3::new(cx, y - STOREY, z0 - steps_run), Vec3::Z, wide, STOREY, 0.05);
        // Parapets along its front edge either side of the stair.
        k.parapet(Vec3::new(r.lo.x, y, z0), Vec3::new(cx - wide * 0.5, y, z0), 0.04);
        k.parapet(Vec3::new(cx + wide * 0.5, y, z0), Vec3::new(r.hi.x, y, z0), 0.04);
    }
}

/// The kinds a cell may be, by its length in slots.
pub fn pick(len: usize, h: f32) -> Kind {
    let kinds: &[(Kind, f32)] = if len >= 2 {
        &[(Kind::Gallery, 3.0), (Kind::Court, 2.0), (Kind::Terraces, 1.5), (Kind::Plaza, 0.6)]
    } else {
        &[(Kind::Court, 2.0), (Kind::Terraces, 2.0), (Kind::Tower, 1.5), (Kind::Plaza, 1.0), (Kind::Gallery, 1.0)]
    };
    let total: f32 = kinds.iter().map(|k| k.1).sum();
    let mut x = h * total;
    for &(kind, w) in kinds {
        if x < w {
            return kind;
        }
        x -= w;
    }
    kinds[0].0
}

/// What stands on a deck `half` metres across (x along the cell), up to
/// `storeys` high.
pub fn unit(kind: Kind, half: (f32, f32), storeys: i32, tone: f32, seed: u32) -> Vec<Solid> {
    let mut k = Kit { out: Vec::new(), tone, seed, salt: 0 };
    let storeys = storeys.max(2);
    let top = storeys as f32 * STOREY;
    // The buildable area inside the walk round the rim.
    let b = Rect::new(-half.0 + RIM, -half.1 + RIM, half.0 - RIM, half.1 - RIM);
    let s = b.size();
    if s.min_element() < 12.0 {
        return k.out;
    }
    let c = b.center();
    match kind {
        Kind::Plaza => {
            // Stoas framing the plaza on one or two sides, a rotunda or a
            // slender tower in the middle, pergolas at the open sides.
            let h = (top * 0.35).clamp(5.0, 9.0);
            colonnade(&mut k, Vec3::new(b.lo.x + 4.0, 0.0, b.hi.y - 2.0), Vec3::new(b.hi.x - 4.0, 0.0, b.hi.y - 2.0), Vec3::Z, h, true, 5.0);
            if k.r() < 0.5 {
                colonnade(&mut k, Vec3::new(b.hi.x - 4.0, 0.0, b.lo.y + 2.0), Vec3::new(b.lo.x + 4.0, 0.0, b.lo.y + 2.0), Vec3::NEG_Z, h, true, 5.0);
            } else {
                pergola(&mut k, Rect::new(b.lo.x + 4.0, b.lo.y + 2.0, b.hi.x - 4.0, b.lo.y + 8.0), 0.0);
            }
            if k.r() < 0.6 {
                let radius = (s.min_element() * 0.12).clamp(4.5, 8.0);
                rotunda(&mut k, c, radius, (top * 0.4).clamp(6.0, 10.0));
            } else {
                tower(&mut k, c, 3.5, top + STOREY);
            }
            pergola(&mut k, Rect::new(b.lo.x + 2.0, c.y - 6.0, b.lo.x + 12.0, c.y + 6.0), 0.0);
            pergola(&mut k, Rect::new(b.hi.x - 12.0, c.y - 6.0, b.hi.x - 2.0, c.y + 6.0), 0.0);
        }
        Kind::Court => {
            // Halls round a court (cloistered inside), a tower at a corner,
            // a rotunda or pergola in the middle.
            let d = (s.min_element() * 0.22).clamp(9.0, 16.0);
            let h1 = k.pick(&[storeys - 1, storeys - 2, storeys]).max(2);
            let h2 = k.pick(&[storeys - 2, storeys - 1]).max(2);
            hall(&mut k, Rect::new(b.lo.x, b.lo.y, b.hi.x, b.lo.y + d), h1);
            hall(&mut k, Rect::new(b.lo.x, b.hi.y - d, b.hi.x, b.hi.y), h2);
            let side = k.pick(&[1, 2]).min(h1);
            hall(&mut k, Rect::new(b.lo.x, b.lo.y + d, b.lo.x + d * 0.8, b.hi.y - d), side);
            hall(&mut k, Rect::new(b.hi.x - d * 0.8, b.lo.y + d, b.hi.x, b.hi.y - d), side);
            let corner = Vec2::new(if k.r() < 0.5 { b.lo.x + 4.5 } else { b.hi.x - 4.5 }, b.hi.y - 4.5);
            let h = top + k.r() * STOREY * 2.0;
            tower(&mut k, corner, 4.5, h);
            let inner = Rect::new(b.lo.x + d * 0.8, b.lo.y + d, b.hi.x - d * 0.8, b.hi.y - d);
            if inner.size().min_element() > 16.0 && k.r() < 0.6 {
                rotunda(&mut k, inner.center(), (inner.size().min_element() * 0.18).clamp(3.5, 7.0), 6.0);
            } else {
                pergola(&mut k, Rect::new(inner.lo.x + 3.0, inner.lo.y + 3.0, inner.hi.x - 3.0, inner.hi.y - 3.0), 0.0);
            }
        }
        Kind::Terraces => {
            // Terraces climbed on the axis, a portico or rotunda at the top.
            let levels = (storeys - 1).clamp(1, 3);
            let back = Rect::new(b.lo.x, b.lo.y, b.hi.x, b.hi.y);
            terraces(&mut k, back, levels, c.x);
            let y = levels as f32 * STOREY;
            let z = b.hi.y - (s.y / (levels + 1) as f32) * 0.5;
            if k.r() < 0.5 {
                let width = (s.x * 0.5).clamp(12.0, 36.0);
                colonnade(&mut k, Vec3::new(c.x - width * 0.5, y, z - 3.0), Vec3::new(c.x + width * 0.5, y, z - 3.0), Vec3::NEG_Z, 7.5, true, 5.0);
            } else {
                // A rotunda, lifted onto the top terrace.
                let before = k.out.len();
                rotunda(&mut k, Vec2::new(c.x, z), 5.0, 6.0);
                for solid in &mut k.out[before..] {
                    solid.center.y += y;
                }
            }
            // Towers either side at the back, sometimes.
            if k.r() < 0.6 {
                let h = top + k.r() * STOREY * 2.0;
                tower(&mut k, Vec2::new(b.lo.x + 4.0, b.hi.y - 4.0), 4.0, h);
                tower(&mut k, Vec2::new(b.hi.x - 4.0, b.hi.y - 4.0), 4.0, h);
            }
        }
        Kind::Gallery => {
            // A hall along the back, towers at its ends, a colonnade in
            // front, a pergola.
            let d = (s.y * 0.32).clamp(10.0, 18.0);
            let hall_storeys = k.pick(&[storeys - 1, storeys - 2]).max(2);
            hall(&mut k, Rect::new(b.lo.x + 9.0, b.hi.y - d, b.hi.x - 9.0, b.hi.y), hall_storeys);
            let h = top + k.r() * STOREY * 2.5;
            tower(&mut k, Vec2::new(b.lo.x + 4.5, b.hi.y - 4.5), 4.5, h);
            let second = h * k.pick(&[1.0, 0.75]);
            tower(&mut k, Vec2::new(b.hi.x - 4.5, b.hi.y - 4.5), 4.5, second);
            let stoa = k.r() < 0.5;
            colonnade(&mut k, Vec3::new(b.lo.x + 3.0, 0.0, b.lo.y + 3.0), Vec3::new(b.hi.x - 3.0, 0.0, b.lo.y + 3.0), Vec3::NEG_Z, (top * 0.3).clamp(5.0, 8.0), stoa, 4.5);
            pergola(&mut k, Rect::new(c.x - 8.0, c.y - 5.0, c.x + 8.0, c.y + 3.0), 0.0);
        }
        Kind::Tower => {
            // Two or three towers of different heights on a podium, a
            // bridge between two of them at height.
            let podium = Rect::new(b.lo.x + 4.0, b.lo.y + 4.0, b.hi.x - 4.0, b.hi.y - 4.0);
            k.span(Vec3::new(podium.lo.x, 0.0, podium.lo.y), Vec3::new(podium.hi.x, STOREY, podium.hi.y), 0.0);
            for (a, bb, out) in podium.sides(0.0) {
                k.arcade(a, bb, out, arches_for((bb - a).length(), 6.0), 0.0, STOREY, -0.8, 0.03);
            }
            for (a, bb, _) in podium.sides(STOREY) {
                k.parapet(a, bb, 0.04);
            }
            k.stair(Vec3::new(c.x, 0.0, podium.lo.y - 6.0), Vec3::Z, 10.0, STOREY, 0.05);
            let n = k.pick(&[2, 3, 3]);
            let spots = [Vec2::new(c.x - s.x * 0.22, c.y + s.y * 0.15), Vec2::new(c.x + s.x * 0.2, c.y - s.y * 0.1), Vec2::new(c.x + s.x * 0.05, c.y + s.y * 0.28)];
            let mut tops = Vec::new();
            for (i, &p) in spots.iter().take(n).enumerate() {
                let half = k.pick(&[3.5, 4.5, 5.5]);
                let h = top * (0.7 + 0.5 * k.r()) + if i == 0 { STOREY * 2.0 } else { 0.0 };
                let before = k.out.len();
                tower(&mut k, p, half, h - STOREY);
                for solid in &mut k.out[before..] {
                    solid.center.y += STOREY;
                }
                tops.push((p, half, h));
            }
            // A bridge between the first two, two thirds up.
            if tops.len() >= 2 {
                let (p, q) = (tops[0].0, tops[1].0);
                let y = tops[0].2.min(tops[1].2) * 0.6;
                let along = (q - p).normalize();
                let start = p + along * tops[0].1;
                let end = q - along * tops[1].1;
                let mid = (start + end) * 0.5;
                let len = (end - start).length();
                let yaw = (-along.y).atan2(along.x);
                k.solid(Vec3::new(mid.x, y, mid.y), Vec3::new(len * 0.5, 0.6, 2.0), Quat::from_rotation_y(yaw), 0.04, false, false);
                for side in [-1.85, 1.85] {
                    let off = Vec2::new(-along.y, along.x) * side;
                    k.solid(Vec3::new(mid.x + off.x, y + 1.1, mid.y + off.y), Vec3::new(len * 0.5, 0.5, 0.15), Quat::from_rotation_y(yaw), 0.06, true, false);
                }
            }
        }
    }
    k.out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_stay_on_the_deck() {
        for kind in [Kind::Plaza, Kind::Court, Kind::Terraces, Kind::Gallery, Kind::Tower] {
            for seed in 0..6 {
                let half = (42.0, 42.0);
                let solids = unit(kind, half, 5, 0.15, seed);
                assert!(!solids.is_empty(), "{kind:?}");
                for s in &solids {
                    // Plain boxes by their extents; round and turned ones
                    // by a radius across.
                    let reach = if s.round { Vec3::new(s.half.y, s.half.x, s.half.z) } else if s.rotation == Quat::IDENTITY { s.half } else { let a = s.half.x.max(s.half.z); Vec3::new(a, s.half.y, a) };
                    let (lo, hi) = (s.center - reach, s.center + reach);
                    assert!(lo.y >= -0.6, "{kind:?} below the deck: {s:?}");
                    assert!(lo.x >= -half.0 - 1.0 && hi.x <= half.0 + 1.0 && lo.z >= -half.1 - 1.0 && hi.z <= half.1 + 1.0, "{kind:?} off the deck: {s:?}");
                }
            }
        }
    }
}
