//! Dressing: conduits, ducts and cables that belong to a structure.
//!
//! In BLAME! and Giger the pipework is part of the object, not a layer on
//! its surface: it rises out of the ground, runs up a wall and plunges into
//! it, bridges the gap between two masses, hangs between towers. This pass
//! reads a built structure's own geometry (the faces of its pieces, the
//! gaps between them, its tall tops) and routes such things through it:
//!
//! - bundles of ducts rising along faces (from the ground or partway up),
//!   ringed by collars, with junction boxes, ending by turning into the
//!   wall or running along it;
//! - bundles spanning a gap to the mass across it;
//! - cables sagging between the tops of tall pieces.
//!
//! Everything is boxes (oriented solids), so it meshes and collides like
//! the rest. The amounts come from a named `dressing` in the data file.

use std::collections::HashMap;

use glam::{Quat, Vec2, Vec3};
use serde::Deserialize;

use crate::forms::{Prism, inward_normals};
use crate::noise::hash01;
use crate::structure::Solid;

fn d_pipes() -> f32 {
    0.3
}
fn d_bundle() -> (u32, u32) {
    (1, 4)
}
fn d_width() -> (f32, f32) {
    (0.3, 0.9)
}
fn d_collars() -> f32 {
    2.5
}
fn d_junctions() -> f32 {
    0.3
}
fn d_spans() -> f32 {
    0.35
}
fn d_cables() -> f32 {
    0.25
}
fn d_max() -> usize {
    6000
}
fn d_tone() -> f32 {
    -0.03
}

/// How much pipework a structure carries.
#[derive(Clone, Debug, Deserialize)]
pub struct Dressing {
    /// Chance that a face (of at least ~30 m²) carries a rising bundle.
    #[serde(default = "d_pipes")]
    pub pipes: f32,
    /// Ducts per bundle.
    #[serde(default = "d_bundle")]
    pub bundle: (u32, u32),
    /// Duct width, metres.
    #[serde(default = "d_width")]
    pub width: (f32, f32),
    /// Collars every so many metres along a duct (0: none).
    #[serde(default = "d_collars")]
    pub collars: f32,
    /// Chance of a junction box on a duct.
    #[serde(default = "d_junctions")]
    pub junctions: f32,
    /// Chance that a face sends a bundle across a gap to the mass facing it.
    #[serde(default = "d_spans")]
    pub spans: f32,
    /// Chance that a tall piece's top hangs a cable to another.
    #[serde(default = "d_cables")]
    pub cables: f32,
    /// Pieces at most.
    #[serde(default = "d_max")]
    pub max: usize,
    /// Tone relative to the structure's own (pipework reads darker).
    #[serde(default = "d_tone")]
    pub tone: f32,
}

/// A vertical face: bottom edge from `a` to `b` at height `y0`, up to `y1`,
/// facing `normal` (horizontal, unit).
struct Face {
    a: Vec2,
    b: Vec2,
    y0: f32,
    y1: f32,
    normal: Vec2,
    piece: usize,
}

/// What occupies space, to find the mass across a gap.
enum Shape {
    Prism { poly: Vec<Vec2>, inward: Vec<Vec2>, y0: f32, y1: f32 },
    Box { center: Vec3, inverse: Quat, half: Vec3 },
}

impl Shape {
    fn contains(&self, p: Vec3) -> bool {
        match self {
            Shape::Prism { poly, inward, y0, y1 } => {
                p.y >= *y0
                    && p.y <= *y1
                    && inward.iter().enumerate().all(|(i, n)| n.dot(Vec2::new(p.x, p.z) - poly[i]) >= 0.0)
            }
            Shape::Box { center, inverse, half } => {
                let l = *inverse * (p - *center);
                l.x.abs() <= half.x && l.y.abs() <= half.y && l.z.abs() <= half.z
            }
        }
    }
}

/// The pipework for a structure made of these solids and prisms.
pub fn dress(d: &Dressing, solids: &[Solid], prisms: &[Prism], tone: f32, seed: u32) -> Vec<Solid> {
    let mut shapes = Vec::new();
    let mut faces = Vec::new();
    let mut tops: Vec<(Vec3, usize)> = Vec::new();
    for p in prisms {
        if p.points.len() < 3 {
            continue;
        }
        let i = shapes.len();
        let inward = inward_normals(&p.points);
        // Straight prisms only carry faces; all of them occupy space.
        if (p.top_scale - 1.0).abs() < 1e-3 && p.lean.length_squared() < 1e-6 {
            for (k, n) in inward.iter().enumerate() {
                let (a, b) = (p.points[k], p.points[(k + 1) % p.points.len()]);
                faces.push(Face { a, b, y0: p.y0.max(0.0), y1: p.y1, normal: -*n, piece: i });
            }
        }
        let c = crate::forms::centroid(&p.points);
        tops.push((Vec3::new(c.x, p.y1, c.y), i));
        shapes.push(Shape::Prism { poly: p.points.clone(), inward, y0: p.y0, y1: p.y1 });
    }
    for s in solids {
        let i = shapes.len();
        shapes.push(Shape::Box { center: s.center, inverse: s.rotation.inverse(), half: s.half });
        if s.wedge {
            continue;
        }
        // Boxes turned about the vertical only (as the box styles make them).
        let corners: Vec<Vec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .iter()
            .map(|&(x, z)| {
                let w = s.center + s.rotation * Vec3::new(x * s.half.x, 0.0, z * s.half.z);
                Vec2::new(w.x, w.z)
            })
            .collect();
        let inward = inward_normals(&corners);
        let (y0, y1) = (s.center.y - s.half.y, s.center.y + s.half.y);
        for (k, n) in inward.iter().enumerate() {
            faces.push(Face { a: corners[k], b: corners[(k + 1) % 4], y0: y0.max(0.0), y1, normal: -*n, piece: i });
        }
        tops.push((s.center + Vec3::Y * s.half.y, i));
    }

    // Space, by 8 m columns, for finding what is across a gap.
    const CELL: f32 = 8.0;
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let key = |p: Vec2| ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32);
    for (i, shape) in shapes.iter().enumerate() {
        let (lo, hi) = match shape {
            Shape::Prism { poly, .. } => poly.iter().fold((Vec2::MAX, Vec2::MIN), |(l, h), q| (l.min(*q), h.max(*q))),
            Shape::Box { center, half, .. } => {
                let r = Vec2::new(half.x, half.z).length();
                (Vec2::new(center.x - r, center.z - r), Vec2::new(center.x + r, center.z + r))
            }
        };
        let (k0, k1) = (key(lo), key(hi));
        for z in k0.1..=k1.1 {
            for x in k0.0..=k1.0 {
                grid.entry((x, z)).or_default().push(i);
            }
        }
    }
    let occupied = |p: Vec3, except: usize| {
        grid.get(&key(Vec2::new(p.x, p.z)))
            .is_some_and(|list| list.iter().any(|&i| i != except && shapes[i].contains(p)))
    };

    let mut out = Out { d, tone: tone + d.tone, pieces: Vec::new() };
    // Biggest faces first, so the budget goes where it shows.
    let mut order: Vec<usize> = (0..faces.len()).collect();
    let area = |f: &Face| (f.b - f.a).length() * (f.y1 - f.y0);
    order.sort_by(|&x, &y| area(&faces[y]).total_cmp(&area(&faces[x])));
    for fi in order {
        let face = &faces[fi];
        if out.full() {
            break;
        }
        let length = (face.b - face.a).length();
        let height = face.y1 - face.y0;
        if length < 3.0 || height < 3.0 || length * height < 30.0 {
            continue;
        }
        let r = |k: i32| hash01(fi as i32, k, seed as i32, 0xd7e5);
        let along = (face.b - face.a) / length;
        // Only faces that look out on open space carry rising ducts (a
        // face pressed against a neighbour would hide them).
        let mid = (face.a + face.b) * 0.5 + face.normal * 1.0;
        let open = [0.25, 0.5, 0.75]
            .iter()
            .all(|&f| !occupied(Vec3::new(mid.x, face.y0 + height * f, mid.y), face.piece));
        if open && r(0) < d.pipes {
            out.rising_bundle(face, along, length, &r);
        }
        if r(1) < d.spans {
            out.span(face, along, length, &r, &occupied);
        }
    }
    // Cables between the tops of tall pieces.
    let tall: Vec<&(Vec3, usize)> = tops.iter().filter(|(t, _)| t.y > 25.0).collect();
    for (ti, (top, _)) in tall.iter().enumerate() {
        if out.full() {
            break;
        }
        let r = |k: i32| hash01(ti as i32, k, seed as i32, 0xca81);
        if r(0) >= d.cables {
            continue;
        }
        // The nearest other tall top 15-90 m away, a little lower or higher.
        let other = tall
            .iter()
            .filter(|(o, _)| {
                let h = Vec2::new(o.x - top.x, o.z - top.z).length();
                (15.0..90.0).contains(&h)
            })
            .min_by(|(x, _), (y, _)| x.distance_squared(*top).total_cmp(&y.distance_squared(*top)));
        if let Some((o, _)) = other {
            let a = *top - Vec3::Y * (1.0 + r(1) * 6.0);
            let b = *o - Vec3::Y * (1.0 + r(2) * 6.0);
            out.cable(a, b, 0.08 + 0.12 * r(3), 0.08 + 0.1 * r(4));
        }
    }
    out.pieces
}

/// How much bigger ducts get on a face of this size (1 on a 15 m face).
fn scale(face: &Face) -> f32 {
    ((face.y1 - face.y0) / 15.0).sqrt().clamp(0.7, 3.0)
}

struct Out<'a> {
    d: &'a Dressing,
    tone: f32,
    pieces: Vec<Solid>,
}

impl Out<'_> {
    fn full(&self) -> bool {
        self.pieces.len() >= self.d.max
    }

    fn push(&mut self, center: Vec3, rotation: Quat, half: Vec3, shade: f32) {
        if self.full() || half.min_element() <= 0.0 {
            return;
        }
        self.pieces.push(Solid { wedge: false, center, rotation, half, albedo: (self.tone + shade).clamp(0.03, 0.4) });
    }

    /// A straight duct from `from` to `to`, `w` wide, with collars.
    fn duct(&mut self, from: Vec3, to: Vec3, w: f32, shade: f32) {
        let v = to - from;
        let len = v.length();
        if len < 0.05 {
            return;
        }
        let dir = v / len;
        // Its own long axis along z.
        let rotation = Quat::from_rotation_arc(Vec3::Z, dir);
        self.push((from + to) * 0.5, rotation, Vec3::new(w * 0.5, w * 0.5, len * 0.5), shade);
        if self.d.collars > 0.0 {
            let n = (len / self.d.collars).floor() as i32;
            for k in 1..n {
                let p = from + dir * (k as f32 * self.d.collars);
                self.push(p, rotation, Vec3::new(w * 0.68, w * 0.68, 0.12), shade + 0.02);
            }
        }
    }

    /// Ducts rising along a face, out of the ground or partway up, ending
    /// by turning into the wall or running along it.
    fn rising_bundle(&mut self, face: &Face, along: Vec2, length: f32, r: &impl Fn(i32) -> f32) {
        let (b0, b1) = self.d.bundle;
        let mut count = b0 + ((b1.saturating_sub(b0) + 1) as f32 * r(2)).floor() as u32;
        // Bigger faces carry bigger ducts.
        let w = (self.d.width.0 + (self.d.width.1 - self.d.width.0) * r(3)) * scale(face);
        let pitch = w * 1.5;
        while count > 1 && count as f32 * pitch > length * 0.8 {
            count -= 1;
        }
        let span = count as f32 * pitch;
        let start = (0.1 + r(4) * (0.8 - span / length).max(0.0)) * length;
        let height = face.y1 - face.y0;
        // From the ground (into it) or from partway up.
        let y_start = if r(5) < 0.6 { face.y0 - 1.0 } else { face.y0 + r(6) * height * 0.5 };
        let y_end = y_start + (0.35 + 0.65 * r(7)) * (face.y1 - y_start);
        let off = w * 0.5 + 0.05;
        let n3 = Vec3::new(face.normal.x, 0.0, face.normal.y);
        let a3 = Vec3::new(along.x, 0.0, along.y);
        let ending = r(8);
        for i in 0..count {
            let s = start + i as f32 * pitch + w * 0.5;
            let base = face.a + along * s + face.normal * off;
            let bottom = Vec3::new(base.x, y_start, base.y);
            // Each duct a little different in height.
            let top = Vec3::new(base.x, y_end - i as f32 * pitch * 0.5, base.y);
            let shade = (i % 2) as f32 * 0.015;
            self.duct(bottom, top, w, shade);
            if r(20 + i as i32) < self.d.junctions {
                let at = bottom.lerp(top, 0.2 + 0.6 * r(40 + i as i32));
                self.push(at + n3 * w * 0.3, Quat::from_rotation_arc(Vec3::X, a3), Vec3::new(w * 1.3, w * 1.8, w * 1.1), 0.03);
            }
            if ending < 0.55 {
                // Turn into the wall.
                let into = top - n3 * (off + 0.6);
                self.duct(top + n3 * 0.0, into, w, shade);
            } else if ending < 0.85 {
                // Run along the face, then into it.
                let run = (length - s - 0.5).min(4.0 + r(60) * length * 0.5).max(0.0);
                let end = top + a3 * run;
                self.duct(top, end, w, shade);
                self.duct(end, end - n3 * (off + 0.6), w, shade);
            }
        }
    }

    /// Ducts crossing a gap from this face to the mass facing it.
    fn span(
        &mut self,
        face: &Face,
        along: Vec2,
        length: f32,
        r: &impl Fn(i32) -> f32,
        occupied: &impl Fn(Vec3, usize) -> bool,
    ) {
        let s = (0.2 + 0.6 * r(10)) * length;
        let y = face.y0 + 2.0 + (face.y1 - face.y0 - 4.0).max(0.0) * r(11);
        let from = face.a + along * s;
        let n3 = Vec3::new(face.normal.x, 0.0, face.normal.y);
        let mut hit = None;
        let mut t = 1.5;
        while t <= 18.0 {
            let p = Vec3::new(from.x, y, from.y) + n3 * t;
            if occupied(p, face.piece) {
                hit = Some(t);
                break;
            }
            t += 0.5;
        }
        let Some(gap) = hit else { return };
        let w = (self.d.width.0 + (self.d.width.1 - self.d.width.0) * r(12)) * scale(face).min(gap / 4.0).max(0.5);
        let count = 1 + (r(13) * 3.0) as u32;
        let a3 = Vec3::new(along.x, 0.0, along.y);
        for i in 0..count {
            // Side by side or stacked.
            let offset = if r(14) < 0.5 { a3 * (i as f32 * w * 1.5) } else { Vec3::Y * (i as f32 * w * 1.5) };
            let start = Vec3::new(from.x, y, from.y) + offset - n3 * 0.4;
            self.duct(start, start + n3 * (gap + 0.8), w, (i % 2) as f32 * 0.015);
        }
    }

    /// A cable sagging from `a` to `b`, in straight segments.
    fn cable(&mut self, a: Vec3, b: Vec3, thickness: f32, sag: f32) {
        const SEGMENTS: usize = 14;
        let dip = a.distance(b) * sag;
        let point = |t: f32| a.lerp(b, t) - Vec3::Y * (4.0 * dip * t * (1.0 - t));
        for k in 0..SEGMENTS {
            let (p, q) = (point(k as f32 / SEGMENTS as f32), point((k + 1) as f32 / SEGMENTS as f32));
            let v = q - p;
            let len = v.length();
            if len < 1e-3 {
                continue;
            }
            let rotation = Quat::from_rotation_arc(Vec3::Z, v / len);
            self.push((p + q) * 0.5, rotation, Vec3::new(thickness, thickness, len * 0.5 + thickness), -0.02);
        }
    }
}
