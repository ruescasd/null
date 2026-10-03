//! Canals: an alien transport system of huge half-pipes running dead straight
//! across the planet. Each canal is a line whose direction (a, b) is a pair
//! of whole numbers, so on the torus it closes on itself after wrapping a
//! times one way and b times the other. Wrapped copies of a line are
//! parallel and `spacing` apart.
//!
//! The canal floor follows a gentle height field evaluated at the
//! centreline, so canals cut trenches through high ground and ride
//! embankments over low ground, and two canals meet at the same height where
//! they cross. Inside, an unexplained flow pushes along the canal.

use glam::{DVec2, Vec3};

use crate::mesh::ColumnMesh;
use crate::noise::hash01;

/// Radius of the half-pipe, in metres.
pub const PIPE_RADIUS: f64 = 7.0;
/// Flat lip on each side of the pipe.
const LIP: f64 = 3.0;
/// Half the width of the corridor a canal occupies.
pub const HALF_WIDTH: f64 = PIPE_RADIUS + LIP;
/// How far the canal's outer walls reach below its rim.
const OUTER_DEPTH: f64 = 70.0;
/// Facets across the half-pipe.
const FACETS: usize = 10;
/// Target length of a mesh segment at the finest level of detail.
const SEGMENT: f64 = 8.0;

const PIPE_ALBEDO: f32 = 0.27;
const LIP_ALBEDO: f32 = 0.06;
const WALL_ALBEDO: f32 = 0.12;

#[derive(Clone, Copy, Debug)]
pub struct Canal {
    origin: DVec2,
    dir: DVec2,
    perp: DVec2,
    /// Distance between wrapped copies of the line.
    spacing: f64,
    /// Every way of writing a point's position along the line differs by a
    /// multiple of this, so segment boundaries that divide it agree across
    /// wraps.
    period: f64,
    /// +1 or -1: which way the flow runs along `dir`.
    pub flow: f64,
}

/// A point's relation to the nearest canal.
#[derive(Clone, Copy, Debug)]
pub struct CanalHit {
    pub canal: usize,
    /// Signed distance from the centreline, across it.
    pub offset: f64,
    /// The nearest point on the centreline.
    pub center: DVec2,
    /// Direction of the flow.
    pub flow_dir: DVec2,
}

pub struct Canals {
    pub lines: Vec<Canal>,
}

impl Canals {
    pub fn new(size: f64, seed: u32) -> Self {
        // Directions (a, b): one axis-aligned and two slanted loops.
        let dirs = [(1, 0), (1, 2), (-2, 1)];
        let lines = dirs
            .iter()
            .enumerate()
            .map(|(i, &(a, b))| {
                let r = |k: i32| hash01(i as i32, k, 0, seed ^ 0xca7a1) as f64;
                let n = ((a * a + b * b) as f64).sqrt();
                let dir = DVec2::new(a as f64, b as f64) / n;
                Canal {
                    origin: DVec2::new(r(1) * size, r(2) * size),
                    dir,
                    perp: DVec2::new(-dir.y, dir.x),
                    spacing: size / n,
                    period: size / n,
                    flow: if r(3) < 0.5 { 1.0 } else { -1.0 },
                }
            })
            .collect();
        Self { lines }
    }

    /// Offset across canal `i` (to its nearest wrapped copy) and position
    /// along it.
    fn local(&self, i: usize, p: DVec2) -> (f64, f64, f64) {
        let c = &self.lines[i];
        let rel = p - c.origin;
        let across = rel.dot(c.perp);
        let copy = (across / c.spacing).round();
        (across - copy * c.spacing, rel.dot(c.dir), copy)
    }

    fn point(&self, i: usize, copy: f64, along: f64) -> DVec2 {
        let c = &self.lines[i];
        c.origin + c.perp * (copy * c.spacing) + c.dir * along
    }

    /// The canal whose corridor (plus `margin`) contains `p`, if any.
    pub fn hit(&self, p: DVec2, margin: f64) -> Option<CanalHit> {
        (0..self.lines.len())
            .filter_map(|i| {
                let (offset, along, copy) = self.local(i, p);
                (offset.abs() < HALF_WIDTH + margin).then(|| CanalHit {
                    canal: i,
                    offset,
                    center: self.point(i, copy, along),
                    flow_dir: self.lines[i].dir * self.lines[i].flow,
                })
            })
            .min_by(|a, b| a.offset.abs().total_cmp(&b.offset.abs()))
    }

    /// The nearest canal, however far away.
    pub fn nearest(&self, p: DVec2) -> Option<CanalHit> {
        self.hit(p, f64::MAX / 4.0)
    }

    /// Height of the canal's surface across its profile, given the floor
    /// height at the centreline.
    pub fn profile_height(offset: f64, floor: f64) -> f64 {
        let o = offset.abs();
        if o >= PIPE_RADIUS {
            floor + PIPE_RADIUS
        } else {
            floor + PIPE_RADIUS - (PIPE_RADIUS * PIPE_RADIUS - o * o).sqrt()
        }
    }

    /// Surface height inside any canal corridor at `p`.
    pub fn surface(&self, p: DVec2, floor: &impl Fn(DVec2) -> f64) -> Option<f64> {
        // Where corridors cross, the lower surface wins (the union of troughs).
        (0..self.lines.len())
            .filter_map(|i| {
                let (offset, along, copy) = self.local(i, p);
                (offset.abs() < HALF_WIDTH)
                    .then(|| Self::profile_height(offset, floor(self.point(i, copy, along))))
            })
            .reduce(f64::min)
    }

    /// The parts of a convex polygon outside every canal corridor.
    pub fn cut(&self, poly: Vec<DVec2>) -> Vec<Vec<DVec2>> {
        let mut pieces = vec![poly];
        for i in 0..self.lines.len() {
            let c = self.lines[i];
            let mut next = Vec::new();
            for piece in pieces {
                if piece.len() < 3 {
                    continue;
                }
                let centroid = piece.iter().copied().sum::<DVec2>() / piece.len() as f64;
                let (center_offset, _, _) = self.local(i, centroid);
                // Offsets of each vertex relative to the copy nearest the
                // centroid (polygons are far smaller than `spacing`).
                let offset = |v: DVec2| center_offset + (v - centroid).dot(c.perp);
                let (lo, hi) = piece
                    .iter()
                    .map(|&v| offset(v))
                    .fold((f64::MAX, f64::MIN), |(lo, hi), o| (lo.min(o), hi.max(o)));
                if lo >= HALF_WIDTH || hi <= -HALF_WIDTH {
                    next.push(piece);
                    continue;
                }
                for side in [1.0, -1.0] {
                    let keep = clip_half_plane(&piece, |v| side * offset(v) - HALF_WIDTH);
                    if keep.len() >= 3 {
                        next.push(keep);
                    }
                }
            }
            pieces = next;
        }
        pieces
    }

    /// Meshes the canal segments whose midpoint lies in the square
    /// `[x0, x0 + size) x [z0, z0 + size)`, with segments `scale` times the
    /// finest length. Positions are relative to `(x0, z0)`.
    pub fn mesh_square(
        &self,
        mesh: &mut ColumnMesh,
        (x0, z0): (f64, f64),
        size: f64,
        scale: f64,
        floor: &impl Fn(DVec2) -> f64,
    ) {
        let center = DVec2::new(x0 + size * 0.5, z0 + size * 0.5);
        let reach = size * 0.75 + HALF_WIDTH;
        for i in 0..self.lines.len() {
            let c = self.lines[i];
            let (offset, along, copy) = self.local(i, center);
            if offset.abs() > reach {
                continue;
            }
            let seg = c.period / (c.period / (SEGMENT * scale)).round().max(1.0);
            let first = ((along - reach) / seg).floor() as i64;
            let last = ((along + reach) / seg).ceil() as i64;
            for k in first..last {
                let (s0, s1) = (k as f64 * seg, (k + 1) as f64 * seg);
                let mid = self.point(i, copy, (s0 + s1) * 0.5);
                if mid.x < x0 || mid.x >= x0 + size || mid.y < z0 || mid.y >= z0 + size {
                    continue;
                }
                let (a, b) = (self.point(i, copy, s0), self.point(i, copy, s1));
                self.mesh_segment(mesh, i, a, b, floor(a), floor(b), (x0, z0), floor);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mesh_segment(
        &self,
        mesh: &mut ColumnMesh,
        i: usize,
        a: DVec2,
        b: DVec2,
        floor_a: f64,
        floor_b: f64,
        origin: (f64, f64),
        floor: &impl Fn(DVec2) -> f64,
    ) {
        let perp = self.lines[i].perp;
        // The profile across the canal: (offset, height above floor, albedo).
        let mut profile = vec![(-HALF_WIDTH, PIPE_RADIUS, LIP_ALBEDO), (-PIPE_RADIUS, PIPE_RADIUS, LIP_ALBEDO)];
        for f in 1..FACETS {
            let theta = std::f64::consts::PI * (1.0 + f as f64 / FACETS as f64);
            profile.push((PIPE_RADIUS * theta.cos(), PIPE_RADIUS * (1.0 + theta.sin()), PIPE_ALBEDO));
        }
        profile.push((PIPE_RADIUS, PIPE_RADIUS, LIP_ALBEDO));
        profile.push((HALF_WIDTH, PIPE_RADIUS, LIP_ALBEDO));

        let world = |center: DVec2, f: f64, (o, h, _): (f64, f64, f32)| {
            let p = center + perp * o;
            Vec3::new((p.x - origin.0) as f32, (f + h) as f32, (p.y - origin.1) as f32)
        };
        // Where another canal's trough is lower, this one's surface would
        // block it: drop facets that rise above another canal's surface.
        let others_below = |q: DVec2, h: f64| {
            (0..self.lines.len()).any(|j| {
                if j == i {
                    return false;
                }
                let (offset, along, copy) = self.local(j, q);
                offset.abs() < HALF_WIDTH
                    && Self::profile_height(offset, floor(self.point(j, copy, along))) < h - 0.05
            })
        };

        for w in profile.windows(2) {
            let (p0, p1) = (w[0], w[1]);
            let quad = [world(a, floor_a, p0), world(a, floor_a, p1), world(b, floor_b, p1), world(b, floor_b, p0)];
            let mid_offset = (p0.0 + p1.0) * 0.5;
            let mid = (a + b) * 0.5 + perp * mid_offset;
            let mid_height = (floor_a + floor_b) * 0.5 + (p0.1 + p1.1) * 0.5;
            if others_below(mid, mid_height) {
                continue;
            }
            // Open side is up (and towards the pipe's axis inside it).
            let along = quad[3] - quad[0];
            let across = quad[1] - quad[0];
            let mut normal = across.cross(along).normalize_or_zero();
            if normal.y < 0.0 {
                normal = -normal;
            }
            let bottom_ness = 1.0 - (p0.1 + p1.1) as f32 * 0.5 / PIPE_RADIUS as f32;
            let ao = 1.0 - 0.35 * bottom_ness.max(0.0);
            push_quad(mesh, quad, normal, p0.2, [ao; 4]);
        }

        // Outer walls, down past the ground on both sides.
        for side in [-1.0, 1.0] {
            let top_a = world(a, floor_a, (side * HALF_WIDTH, PIPE_RADIUS, 0.0));
            let top_b = world(b, floor_b, (side * HALF_WIDTH, PIPE_RADIUS, 0.0));
            let mid = (a + b) * 0.5 + perp * side * HALF_WIDTH;
            if others_below(mid, (floor_a + floor_b) * 0.5 + PIPE_RADIUS) {
                continue;
            }
            let down = Vec3::Y * OUTER_DEPTH as f32;
            let normal = Vec3::new((perp.x * side) as f32, 0.0, (perp.y * side) as f32);
            push_quad(mesh, [top_a, top_b, top_b - down, top_a - down], normal, WALL_ALBEDO, [1.0, 1.0, 0.0, 0.0]);
        }
    }
}

/// Keeps the part of a convex polygon where `side(v) >= 0`.
pub fn clip_half_plane(poly: &[DVec2], side: impl Fn(DVec2) -> f64) -> Vec<DVec2> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for k in 0..poly.len() {
        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    out
}

/// Appends a quad as two triangles facing `normal`.
fn push_quad(mesh: &mut ColumnMesh, quad: [Vec3; 4], normal: Vec3, albedo: f32, ao: [f32; 4]) {
    let base = mesh.positions.len() as u32;
    for (p, a) in quad.iter().zip(ao) {
        mesh.positions.push(p.to_array());
        mesh.normals.push(normal.to_array());
        mesh.albedo.push(albedo);
        mesh.ao.push(a);
    }
    let face = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
    if face.dot(normal) >= 0.0 {
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        mesh.indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canal_loops_close_across_the_wrap() {
        let size = 4096.0;
        let canals = Canals::new(size, 5);
        for i in 0..canals.lines.len() {
            for k in 0..20 {
                let p = DVec2::new(k as f64 * 173.0, k as f64 * 61.0);
                let (o1, _, _) = canals.local(i, p);
                let (o2, _, _) = canals.local(i, p + DVec2::new(size, -size));
                assert!((o1 - o2).abs() < 1e-6, "offset must tile");
            }
        }
    }

    #[test]
    fn cut_removes_the_corridor() {
        let canals = Canals::new(4096.0, 5);
        let hit_point = canals.point(0, 0.0, 100.0);
        let square = vec![
            hit_point + DVec2::new(-30.0, -30.0),
            hit_point + DVec2::new(30.0, -30.0),
            hit_point + DVec2::new(30.0, 30.0),
            hit_point + DVec2::new(-30.0, 30.0),
        ];
        let pieces = canals.cut(square);
        for piece in &pieces {
            for &v in piece {
                let (o, _, _) = canals.local(0, v);
                assert!(o.abs() >= HALF_WIDTH - 1e-6);
            }
        }
        assert!(!pieces.is_empty());
    }
}
