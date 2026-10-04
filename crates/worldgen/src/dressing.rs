//! Dressing: pipework that belongs to a structure.
//!
//! In BLAME! and Giger the pipework is part of the object, not a layer on
//! its surface. This pass reads a built structure's own geometry and routes
//! pipes through it:
//!
//! - along its members: pipes laid beside long, thin pieces (a truss's
//!   bars), clamped to them;
//! - up its open faces: bundles rising from the ground or partway up, held
//!   off the wall by brackets, flanged at their joints, ending in a plate
//!   where they enter the wall or running along it first;
//! - across its gaps: bundles spanning to the mass facing them;
//! - between its tops: cables sagging from one tall piece to another,
//!   clamped at both ends.
//!
//! Pipes are octagonal tubes (see `Tube`); brackets, clamps, plates and
//! junction boxes are boxes. The amounts come from a named `dressing` in the
//! data file. It reads as structure on lattices and stepped masses, and as
//! something stuck on on smooth ones (lab batch 2), so it is used sparingly.

use std::collections::HashMap;

use glam::{Quat, Vec2, Vec3};
use serde::Deserialize;

use crate::forms::{Prism, inward_normals};
use crate::mesh::ColumnMesh;
use crate::noise::hash01;
use crate::structure::Solid;

fn d_pipes() -> f32 {
    0.35
}
fn d_bundle() -> (u32, u32) {
    (1, 3)
}
fn d_width() -> (f32, f32) {
    (0.4, 1.2)
}
fn d_collars() -> f32 {
    6.0
}
fn d_brackets() -> f32 {
    3.5
}
fn d_junctions() -> f32 {
    0.25
}
fn d_spans() -> f32 {
    0.35
}
fn d_cables() -> f32 {
    0.3
}
fn d_members() -> f32 {
    0.35
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
    /// Chance that an open face (of at least ~30 m²) carries a rising bundle.
    #[serde(default = "d_pipes")]
    pub pipes: f32,
    /// Pipes per bundle.
    #[serde(default = "d_bundle")]
    pub bundle: (u32, u32),
    /// Pipe diameter, metres (scaled up on big faces).
    #[serde(default = "d_width")]
    pub width: (f32, f32),
    /// A flange every so many metres along a pipe (0: none).
    #[serde(default = "d_collars")]
    pub collars: f32,
    /// A bracket (or clamp) every so many metres, holding a pipe to what it
    /// runs along.
    #[serde(default = "d_brackets")]
    pub brackets: f32,
    /// Chance of a junction box on a pipe.
    #[serde(default = "d_junctions")]
    pub junctions: f32,
    /// Chance that a face sends a bundle across a gap to the mass facing it.
    #[serde(default = "d_spans")]
    pub spans: f32,
    /// Chance that a tall piece's top hangs a cable to another.
    #[serde(default = "d_cables")]
    pub cables: f32,
    /// Chance that a long, thin piece (a member) carries pipes along it.
    #[serde(default = "d_members")]
    pub members: f32,
    /// Pieces at most.
    #[serde(default = "d_max")]
    pub max: usize,
    /// Tone relative to the structure's own (pipework reads darker).
    #[serde(default = "d_tone")]
    pub tone: f32,
}

/// An octagonal tube from `from` to `to`.
#[derive(Clone, Copy, Debug)]
pub struct Tube {
    pub from: Vec3,
    pub to: Vec3,
    pub radius: f32,
    pub albedo: f32,
    /// How brightly it glows (0: not at all).
    pub glow: f32,
}

const SIDES: usize = 8;

impl Tube {
    /// The rings of corners at each end.
    fn rings(&self) -> Option<([Vec3; SIDES], [Vec3; SIDES])> {
        let axis = (self.to - self.from).try_normalize()?;
        let side = axis.any_orthonormal_vector();
        let up = axis.cross(side);
        let corner = |c: Vec3, k: usize| {
            let a = k as f32 / SIDES as f32 * std::f32::consts::TAU + std::f32::consts::PI / SIDES as f32;
            c + (side * a.cos() + up * a.sin()) * self.radius
        };
        Some((std::array::from_fn(|k| corner(self.from, k)), std::array::from_fn(|k| corner(self.to, k))))
    }

    /// Its corners, for a convex hull collider.
    pub fn hull_points(&self) -> Vec<Vec3> {
        self.rings().map_or(Vec::new(), |(a, b)| a.iter().chain(&b).copied().collect())
    }
}

/// Adds tubes to a mesh, flat shaded like the structures.
pub fn mesh_tubes(mesh: &mut ColumnMesh, tubes: &[Tube]) {
    for tube in tubes {
        let Some((a, b)) = tube.rings() else { continue };
        let mut face = |points: &[Vec3], normal: Vec3, ao: f32| {
            let base = mesh.positions.len() as u32;
            for p in points {
                mesh.positions.push(p.to_array());
                mesh.normals.push(normal.to_array());
                mesh.albedo.push(tube.albedo);
                mesh.ao.push(ao);
            }
            mesh.face_size(points.len(), crate::mesh::polygon_width(points));
            if tube.glow > 0.0 {
                mesh.glow_of(points.len(), tube.glow);
            }
            for k in 1..points.len() as u32 - 1 {
                // Wind each triangle to face `normal`.
                let (p0, p1, p2) = (points[0], points[k as usize], points[k as usize + 1]);
                if (p1 - p0).cross(p2 - p0).dot(normal) >= 0.0 {
                    mesh.indices.extend_from_slice(&[base, base + k, base + k + 1]);
                } else {
                    mesh.indices.extend_from_slice(&[base, base + k + 1, base + k]);
                }
            }
        };
        let axis = (tube.to - tube.from).normalize();
        for k in 0..SIDES {
            let j = (k + 1) % SIDES;
            let mid = (a[k] + a[j]) * 0.5 - tube.from;
            let normal = (mid - axis * mid.dot(axis)).normalize_or(Vec3::Y);
            let ao = 0.75 + 0.25 * normal.y.max(0.0);
            face(&[a[k], a[j], b[j], b[k]], normal, ao);
        }
        face(&a, -axis, 0.7);
        face(&b, axis, 0.7);
    }
}

/// The pipework for a structure.
#[derive(Default)]
pub struct Pipework {
    pub solids: Vec<Solid>,
    pub tubes: Vec<Tube>,
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
pub fn dress(d: &Dressing, solids: &[Solid], prisms: &[Prism], tone: f32, seed: u32) -> Pipework {
    let mut shapes = Vec::new();
    let mut faces = Vec::new();
    let mut tops: Vec<Vec3> = Vec::new();
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
        tops.push(Vec3::new(c.x, p.y1, c.y));
        shapes.push(Shape::Prism { poly: p.points.clone(), inward, y0: p.y0, y1: p.y1 });
    }
    let mut members = Vec::new();
    for (si, s) in solids.iter().enumerate() {
        let i = shapes.len();
        shapes.push(Shape::Box { center: s.center, inverse: s.rotation.inverse(), half: s.half });
        if s.wedge {
            continue;
        }
        // Long and thin: a member, which pipes can run along.
        let mut h = s.half.to_array();
        h.sort_by(f32::total_cmp);
        if h[2] > 4.0 && h[2] > 4.0 * h[1] {
            members.push(si);
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
        tops.push(s.center + Vec3::Y * s.half.y);
    }

    // Space, by 8 m columns, for finding what is across a gap.
    const CELL: f32 = 8.0;
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let key = |p: Vec2| ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32);
    for (i, shape) in shapes.iter().enumerate() {
        let (lo, hi) = match shape {
            Shape::Prism { poly, .. } => poly.iter().fold((Vec2::MAX, Vec2::MIN), |(l, h), q| (l.min(*q), h.max(*q))),
            Shape::Box { center, half, .. } => {
                let r = half.length();
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

    let mut out = Out { d, tone: tone + d.tone, work: Pipework::default() };
    // Along members first: on a lattice this is what reads as structure.
    for (mi, &si) in members.iter().enumerate() {
        // Half the budget at most, so faces and cables get their share.
        if out.work.solids.len() + out.work.tubes.len() >= d.max / 2 {
            break;
        }
        let r = |k: i32| hash01(mi as i32, k, seed as i32, 0x3e3b);
        if r(0) < d.members {
            out.along_member(&solids[si], &r);
        }
    }
    // Then faces, biggest first, so the budget goes where it shows.
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
        // Only faces that look out on open space carry rising pipes.
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
    let tall: Vec<Vec3> = tops.into_iter().filter(|t| t.y > 25.0).collect();
    for (ti, top) in tall.iter().enumerate() {
        if out.full() {
            break;
        }
        let r = |k: i32| hash01(ti as i32, k, seed as i32, 0xca81);
        if r(0) >= d.cables {
            continue;
        }
        let other = tall
            .iter()
            .filter(|o| (15.0..90.0).contains(&Vec2::new(o.x - top.x, o.z - top.z).length()))
            .min_by(|x, y| x.distance_squared(*top).total_cmp(&y.distance_squared(*top)));
        if let Some(o) = other {
            let a = *top - Vec3::Y * (1.0 + r(1) * 6.0);
            let b = *o - Vec3::Y * (1.0 + r(2) * 6.0);
            out.cable(a, b, 0.07 + 0.08 * r(3), 0.08 + 0.1 * r(4));
        }
    }
    out.work
}

/// How much bigger pipes get on a face of this size (1 on a 15 m face).
fn scale(face: &Face) -> f32 {
    ((face.y1 - face.y0) / 15.0).sqrt().clamp(0.7, 3.0)
}

struct Out<'a> {
    d: &'a Dressing,
    tone: f32,
    work: Pipework,
}

impl Out<'_> {
    fn full(&self) -> bool {
        self.work.solids.len() + self.work.tubes.len() >= self.d.max
    }

    fn albedo(&self, shade: f32) -> f32 {
        (self.tone + shade).clamp(0.03, 0.4)
    }

    fn block(&mut self, center: Vec3, rotation: Quat, half: Vec3, shade: f32) {
        if self.full() || half.min_element() <= 0.0 {
            return;
        }
        let albedo = self.albedo(shade);
        self.work.solids.push(Solid { wedge: false, round: false, center, rotation, half, albedo });
    }

    fn tube(&mut self, from: Vec3, to: Vec3, radius: f32, shade: f32) {
        if self.full() || from.distance_squared(to) < 1e-4 {
            return;
        }
        let albedo = self.albedo(shade);
        self.work.tubes.push(Tube { from, to, radius, albedo, glow: 0.0 });
    }

    /// A pipe from `from` to `to`, `w` across, flanged at its ends and every
    /// so often along it.
    fn pipe(&mut self, from: Vec3, to: Vec3, w: f32, shade: f32) {
        let v = to - from;
        let len = v.length();
        if len < 0.05 {
            return;
        }
        let dir = v / len;
        let r = w * 0.5;
        self.tube(from, to, r, shade);
        let flange = |o: &mut Self, at: Vec3| o.tube(at - dir * 0.12, at + dir * 0.12, r * 1.35, shade + 0.02);
        flange(self, from + dir * 0.2);
        flange(self, to - dir * 0.2);
        if self.d.collars > 0.0 {
            let n = (len / self.d.collars).floor() as i32;
            for k in 1..n {
                flange(self, from + dir * (len * k as f32 / n as f32));
            }
        }
    }

    /// Brackets holding a pipe (axis `from`-`to`) to a surface `away` behind
    /// it at distance `off` from its axis.
    fn brackets(&mut self, from: Vec3, to: Vec3, w: f32, away: Vec3, off: f32) {
        if self.d.brackets <= 0.0 {
            return;
        }
        let v = to - from;
        let len = v.length();
        let n = (len / self.d.brackets).floor().max(1.0) as i32;
        let rotation = Quat::from_rotation_arc(Vec3::Z, away);
        for k in 0..=n {
            let at = from + v * (k as f32 / n as f32);
            self.block(at + away * (off * 0.5), rotation, Vec3::new(w * 0.35, w * 0.2, off * 0.5 + 0.05), 0.01);
        }
    }

    /// Pipes laid along a member, clamped to it.
    fn along_member(&mut self, s: &Solid, r: &impl Fn(i32) -> f32) {
        let half = s.half;
        // Its long axis, and the side the pipes lie against (its widest).
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        let long = (0..3).max_by(|&a, &b| half[a].total_cmp(&half[b])).unwrap();
        let others: Vec<usize> = (0..3).filter(|&a| a != long).collect();
        let (side, flat) = if r(1) < 0.5 { (others[0], others[1]) } else { (others[1], others[0]) };
        let dir = s.rotation * axes[long];
        let out = s.rotation * axes[side] * if r(2) < 0.5 { 1.0 } else { -1.0 };
        let across = s.rotation * axes[flat];
        let w = (half[flat] * (0.5 + 0.6 * r(3))).clamp(0.15, 1.2);
        let count = 1 + (r(4) * 2.5) as i32;
        let off = half[side] + w * 0.5 + 0.06;
        for i in 0..count {
            let lateral = across * ((i as f32 - (count - 1) as f32 * 0.5) * w * 1.3);
            let from = s.center - dir * half[long] * 0.95 + out * off + lateral;
            let to = s.center + dir * half[long] * 0.95 + out * off + lateral;
            self.pipe(from, to, w, (i % 2) as f32 * 0.015);
        }
        // Clamps around the member and its pipes.
        let n = ((2.0 * half[long]) / self.d.brackets.max(1.0)).floor().max(1.0) as i32;
        let rotation = Quat::from_mat3(&glam::Mat3::from_cols(across, out, dir));
        let clamp_half = Vec3::new(
            half[flat].max(count as f32 * w * 0.65) + 0.08,
            (half[side] + w + 0.1) * 0.5 + 0.04,
            0.1,
        );
        for k in 0..=n {
            let t = -0.9 + 1.8 * k as f32 / n as f32;
            let at = s.center + dir * half[long] * t + out * (w + 0.1) * 0.5;
            self.block(at, rotation, clamp_half, 0.02);
        }
    }

    /// Pipes rising along a face, out of the ground or partway up, held off
    /// it by brackets, ending in a plate where they enter the wall or after
    /// running along it.
    fn rising_bundle(&mut self, face: &Face, along: Vec2, length: f32, r: &impl Fn(i32) -> f32) {
        let (b0, b1) = self.d.bundle;
        let mut count = b0 + ((b1.saturating_sub(b0) + 1) as f32 * r(2)).floor() as u32;
        let w = (self.d.width.0 + (self.d.width.1 - self.d.width.0) * r(3)) * scale(face);
        let pitch = w * 1.6;
        while count > 1 && count as f32 * pitch > length * 0.8 {
            count -= 1;
        }
        let span = count as f32 * pitch;
        let start = (0.1 + r(4) * (0.8 - span / length).max(0.0)) * length;
        let height = face.y1 - face.y0;
        let y_start = if r(5) < 0.6 { face.y0 - 1.0 } else { face.y0 + r(6) * height * 0.5 };
        let y_end = y_start + (0.35 + 0.65 * r(7)) * (face.y1 - y_start);
        let off = w * 0.5 + 0.35;
        let n3 = Vec3::new(face.normal.x, 0.0, face.normal.y);
        let a3 = Vec3::new(along.x, 0.0, along.y);
        let ending = r(8);
        for i in 0..count {
            let s = start + i as f32 * pitch + w * 0.5;
            let base = face.a + along * s + face.normal * off;
            let bottom = Vec3::new(base.x, y_start, base.y);
            let top = Vec3::new(base.x, y_end - i as f32 * pitch * 0.5, base.y);
            let shade = (i % 2) as f32 * 0.015;
            self.pipe(bottom, top, w, shade);
            self.brackets(bottom.max(Vec3::new(bottom.x, face.y0 + 1.0, bottom.z)), top, w, -n3, off);
            if r(20 + i as i32) < self.d.junctions {
                let at = bottom.lerp(top, 0.2 + 0.6 * r(40 + i as i32));
                self.block(at, Quat::from_rotation_arc(Vec3::Z, n3), Vec3::new(w * 1.2, w * 1.6, w * 0.9), 0.03);
            }
            let end = if ending < 0.6 {
                top
            } else {
                // Run along the face first.
                let run = (length - s - 0.5).min(4.0 + r(60) * length * 0.5).max(0.0);
                let end = top + a3 * run;
                // An elbow: a short sleeve where it turns.
                self.tube(top - Vec3::Y * w * 0.6, top + a3 * w * 0.6, w * 0.62, shade + 0.02);
                self.pipe(top, end, w, shade);
                self.brackets(top, end, w, -n3, off);
                end
            };
            // Into the wall, through a plate.
            let wall = end - n3 * off;
            self.pipe(end, wall - n3 * 0.3, w, shade);
            self.block(wall + n3 * 0.08, Quat::from_rotation_arc(Vec3::Z, n3), Vec3::new(w * 0.9, w * 0.9, 0.08), 0.03);
        }
    }

    /// Pipes crossing a gap from this face to the mass facing it, with a
    /// plate at each end.
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
            if occupied(Vec3::new(from.x, y, from.y) + n3 * t, face.piece) {
                hit = Some(t);
                break;
            }
            t += 0.5;
        }
        let Some(gap) = hit else { return };
        let w = (self.d.width.0 + (self.d.width.1 - self.d.width.0) * r(12)) * scale(face).min(gap / 4.0).max(0.5);
        let count = 1 + (r(13) * 3.0) as u32;
        let a3 = Vec3::new(along.x, 0.0, along.y);
        let plate = Quat::from_rotation_arc(Vec3::Z, n3);
        for i in 0..count {
            let offset = if r(14) < 0.5 { a3 * (i as f32 * w * 1.6) } else { Vec3::Y * (i as f32 * w * 1.6) };
            let start = Vec3::new(from.x, y, from.y) + offset;
            self.pipe(start - n3 * 0.3, start + n3 * (gap + 0.3), w, (i % 2) as f32 * 0.015);
            self.block(start + n3 * 0.08, plate, Vec3::new(w * 0.9, w * 0.9, 0.08), 0.03);
            self.block(start + n3 * (gap - 0.08), plate, Vec3::new(w * 0.9, w * 0.9, 0.08), 0.03);
        }
    }

    /// A cable sagging from `a` to `b`, clamped at both ends.
    fn cable(&mut self, a: Vec3, b: Vec3, radius: f32, sag: f32) {
        const SEGMENTS: usize = 16;
        let dip = a.distance(b) * sag;
        let point = |t: f32| a.lerp(b, t) - Vec3::Y * (4.0 * dip * t * (1.0 - t));
        for k in 0..SEGMENTS {
            let (p, q) = (point(k as f32 / SEGMENTS as f32), point((k + 1) as f32 / SEGMENTS as f32));
            // A little overlap, so the joints do not show.
            let v = (q - p).normalize_or_zero() * radius;
            self.tube(p - v, q + v, radius, -0.02);
        }
        for end in [a, b] {
            self.block(end, Quat::IDENTITY, Vec3::splat(radius * 3.0), 0.02);
        }
    }
}
