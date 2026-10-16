//! Canals: an alien transport system of huge half-pipes, each running dead
//! straight from one station to another (see `network.rs`: they cross
//! nowhere but at a station). They were endless parallel loops round the
//! torus; ending at junctions, places in the world like stations, the user.
//!
//! The canal floor follows a gentle height field (given by the caller)
//! evaluated at the centreline, so canals cut trenches through high ground
//! and ride embankments over low ground, easing at each end to its
//! station's. The surface is slick (see the player): no friction, so the
//! pipe rides like a skate or snowboard half-pipe.
//!
//! A station, for now, is a placeholder: a flat round court at the level of
//! the pipes' floor, so a pipe opens out onto it.

use glam::{DVec2, Vec3};

use crate::mesh::ColumnMesh;
use crate::network::Network;
use crate::noise::hash01;

/// Radius of the half-pipe, in metres.
pub const PIPE_RADIUS: f64 = 7.0;
/// Half the width of the corridor a canal occupies: the pipe's own (a flat
/// dark lip along each side, 3 m wide, read as slabs laid beside it: gone,
/// the user; the pipe's own outer facets dark instead, see `DARK_FACETS`).
pub const HALF_WIDTH: f64 = PIPE_RADIUS;
/// How far the canal's outer walls reach below its rim.
const OUTER_DEPTH: f64 = 70.0;
/// Facets across the half-pipe.
const FACETS: usize = 24;
/// Target length of a mesh segment at the finest level of detail.
const SEGMENT: f64 = 8.0;
/// A station's court: its radius (to its corners), and its sides.
pub const STATION_RADIUS: f64 = 40.0;
const STATION_SIDES: usize = 32;
const STATION_ALBEDO: f32 = 0.2;
/// How far along a canal its floor eases to its station's (m).
const RAMP: f64 = 200.0;

const PIPE_ALBEDO: f32 = 0.27;
/// The pipe's outermost facets each side, up to its rim, dark.
const DARK_FACETS: usize = 3;
const DARK_ALBEDO: f32 = 0.06;
const WALL_ALBEDO: f32 = 0.12;

/// A canal: from station `a` to station `b`.
#[derive(Clone, Copy, Debug)]
pub struct Canal {
    pub a: usize,
    pub b: usize,
    /// Station `a`, and the direction to `b`.
    start: DVec2,
    dir: DVec2,
    perp: DVec2,
    length: f64,
    /// +1 or -1: which way the flow runs along `dir`.
    pub flow: f64,
}

/// A point's relation to a canal.
#[derive(Clone, Copy, Debug)]
pub struct CanalHit {
    pub canal: usize,
    /// Signed distance from the centreline, across it.
    pub offset: f64,
    /// The nearest point on the centreline.
    pub center: DVec2,
    /// Height of the floor there.
    pub floor: f64,
    /// Direction of the flow.
    pub flow_dir: DVec2,
    /// Unit direction in which `offset` grows.
    pub across: DVec2,
}

/// A point in a canal's frame: across and along it, and where its start
/// lies in the point's own copy of the world.
#[derive(Clone, Copy, Debug)]
struct Local {
    offset: f64,
    along: f64,
    start: DVec2,
}

pub struct Canals {
    size: f64,
    pub lines: Vec<Canal>,
    pub stations: Vec<DVec2>,
}

impl Canals {
    pub fn new(size: f64, seed: u32) -> Self {
        let network = Network::new(size, seed);
        let r = |k: i32| hash01(0, k, 0, seed ^ 0xca7a1) as f64;
        let lines = network
            .links
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let dir = l.offset / l.length();
                Canal {
                    a: l.a,
                    b: l.b,
                    start: network.stations[l.a],
                    dir,
                    perp: DVec2::new(-dir.y, dir.x),
                    length: l.length(),
                    flow: if r(3 + i as i32) < 0.5 { 1.0 } else { -1.0 },
                }
            })
            .collect();
        Self { size, lines, stations: network.stations }
    }

    /// `d` round the wrap the short way.
    fn wrap(&self, d: DVec2) -> DVec2 {
        d - (d / self.size).round() * self.size
    }

    /// `p` in canal `i`'s frame, from the copy of the canal nearest it.
    fn local(&self, i: usize, p: DVec2) -> Local {
        let c = &self.lines[i];
        let mid = c.start + c.dir * (c.length * 0.5);
        let rel = self.wrap(p - mid);
        Local { offset: rel.dot(c.perp), along: rel.dot(c.dir) + c.length * 0.5, start: p - rel - c.dir * (c.length * 0.5) }
    }

    /// Where a pipe meets the court at either end: how far along the canal
    /// from each station (its corners on the court's rim).
    fn end(&self) -> f64 {
        let apothem = STATION_RADIUS * (std::f64::consts::PI / STATION_SIDES as f64).cos();
        (apothem * apothem - HALF_WIDTH * HALF_WIDTH).sqrt()
    }

    /// The floor of canal `i` at `along` (its centreline point `at`): the
    /// field's, easing to each station's near its ends.
    fn floor(&self, i: usize, along: f64, at: DVec2, field: &impl Fn(DVec2) -> f64) -> f64 {
        let c = &self.lines[i];
        let f = field(at);
        let ease = |s: f64| {
            let t = ((s - self.end()) / RAMP).clamp(0.0, 1.0);
            1.0 - t * t * (3.0 - 2.0 * t)
        };
        let (wa, wb) = (ease(along), ease(c.length - along));
        f + (field(self.stations[c.a]) - f) * wa + (field(self.stations[c.b]) - f) * wb
    }

    /// Canal `i`'s stations: where `a` stands, and `b`'s copy nearest it.
    pub fn ends(&self, i: usize) -> (DVec2, DVec2) {
        let c = &self.lines[i];
        (c.start, c.start + c.dir * c.length)
    }

    /// Station `s`'s floor (the level of its canals' floors).
    pub fn station_floor(&self, s: usize, field: &impl Fn(DVec2) -> f64) -> f64 {
        field(self.stations[s])
    }

    /// The station whose court contains `p`, if any.
    pub fn station_at(&self, p: DVec2) -> Option<usize> {
        (0..self.stations.len()).find(|&s| self.station_planes(s, p).iter().all(|f| f(p) <= 0.0))
    }

    /// The edges of station `s`'s court in `near`'s copy of the world: each
    /// how far outside it a point is (positive outside).
    fn station_planes(&self, s: usize, near: DVec2) -> Vec<Box<dyn Fn(DVec2) -> f64>> {
        let center = near - self.wrap(near - self.stations[s]);
        let apothem = STATION_RADIUS * (std::f64::consts::PI / STATION_SIDES as f64).cos();
        (0..STATION_SIDES)
            .map(|k| {
                let a = k as f64 / STATION_SIDES as f64 * std::f64::consts::TAU;
                let n = DVec2::new(a.cos(), a.sin());
                Box::new(move |v: DVec2| (v - center).dot(n) - apothem) as Box<dyn Fn(DVec2) -> f64>
            })
            .collect()
    }

    /// The canal whose pipe corridor (plus `margin` across it) contains
    /// `p`, if any.
    pub fn hit(&self, p: DVec2, margin: f64, field: &impl Fn(DVec2) -> f64) -> Option<CanalHit> {
        let end = self.end();
        (0..self.lines.len())
            .filter_map(|i| {
                let l = self.local(i, p);
                (l.offset.abs() < HALF_WIDTH + margin && l.along >= end && l.along <= self.lines[i].length - end).then(|| self.hit_at(i, l, l.along, field))
            })
            .min_by(|a, b| a.offset.abs().total_cmp(&b.offset.abs()))
    }

    fn hit_at(&self, i: usize, l: Local, along: f64, field: &impl Fn(DVec2) -> f64) -> CanalHit {
        let c = &self.lines[i];
        let center = l.start + c.dir * along;
        CanalHit { canal: i, offset: l.offset, center, floor: self.floor(i, along, center, field), flow_dir: c.dir * c.flow, across: c.perp }
    }

    /// The nearest point on any canal's centreline (a canal runs on to its
    /// stations' centres), however far away.
    pub fn nearest(&self, p: DVec2, field: &impl Fn(DVec2) -> f64) -> Option<CanalHit> {
        (0..self.lines.len())
            .map(|i| {
                let l = self.local(i, p);
                let along = l.along.clamp(0.0, self.lines[i].length);
                let hit = self.hit_at(i, l, along, field);
                (hit, hit.center.distance(p))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(h, _)| h)
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

    /// Surface height in a station's court or a canal's corridor at `p`.
    pub fn surface(&self, p: DVec2, field: &impl Fn(DVec2) -> f64) -> Option<f64> {
        if let Some(s) = self.station_at(p) {
            return Some(self.station_floor(s, field));
        }
        self.hit(p, 0.0, field).map(|h| Self::profile_height(h.offset, h.floor))
    }

    /// The parts of a convex polygon outside every canal corridor and
    /// station court.
    pub fn cut(&self, poly: Vec<DVec2>) -> Vec<Vec<DVec2>> {
        let mut pieces = vec![poly];
        let end = self.end();
        for s in 0..self.stations.len() {
            pieces = pieces
                .into_iter()
                .flat_map(|piece| {
                    let near = centroid(&piece);
                    subtract(piece, &self.station_planes(s, near))
                })
                .collect();
        }
        for i in 0..self.lines.len() {
            let c = self.lines[i];
            pieces = pieces
                .into_iter()
                .flat_map(|piece| {
                    // Each vertex in the frame of the copy nearest the
                    // piece (pieces are far smaller than the world).
                    let near = centroid(&piece);
                    let l = self.local(i, near);
                    let offset = move |v: DVec2| l.offset + (v - near).dot(c.perp);
                    let along = move |v: DVec2| l.along + (v - near).dot(c.dir);
                    let planes: [Box<dyn Fn(DVec2) -> f64>; 4] = [
                        Box::new(move |v| offset(v) - HALF_WIDTH),
                        Box::new(move |v| -offset(v) - HALF_WIDTH),
                        Box::new(move |v| along(v) - (c.length - end)),
                        Box::new(move |v| end - along(v)),
                    ];
                    subtract(piece, &planes)
                })
                .collect();
        }
        pieces
    }

    /// Meshes the canal segments and station courts whose midpoint lies in
    /// the square `[x0, x0 + size) x [z0, z0 + size)`, with segments `scale`
    /// times the finest length. Positions are relative to `(x0, z0)`.
    pub fn mesh_square(&self, mesh: &mut ColumnMesh, (x0, z0): (f64, f64), size: f64, scale: f64, field: &impl Fn(DVec2) -> f64) {
        let inside = |p: DVec2| p.x >= x0 && p.x < x0 + size && p.y >= z0 && p.y < z0 + size;
        let center = DVec2::new(x0 + size * 0.5, z0 + size * 0.5);
        let reach = size * 0.75 + HALF_WIDTH;
        let end = self.end();
        for i in 0..self.lines.len() {
            let c = self.lines[i];
            let l = self.local(i, center);
            if l.offset.abs() > reach || l.along < -reach || l.along > c.length + reach {
                continue;
            }
            let run = c.length - 2.0 * end;
            let n = (run / (SEGMENT * scale)).round().max(1.0) as i64;
            let seg = run / n as f64;
            let first = (((l.along - reach - end) / seg).floor() as i64).max(0);
            let last = (((l.along + reach - end) / seg).ceil() as i64).min(n);
            for k in first..last {
                let (s0, s1) = (end + k as f64 * seg, end + (k + 1) as f64 * seg);
                if !inside(l.start + c.dir * ((s0 + s1) * 0.5)) {
                    continue;
                }
                let (a, b) = (l.start + c.dir * s0, l.start + c.dir * s1);
                let (fa, fb) = (self.floor(i, s0, a, field), self.floor(i, s1, b, field));
                Self::mesh_segment(mesh, c.perp, a, b, fa, fb, (x0, z0));
                // Its ends, where it opens onto a court: capped.
                if k == 0 {
                    Self::mesh_cap(mesh, c.perp, a, fa, -c.dir, (x0, z0));
                }
                if k == n - 1 {
                    Self::mesh_cap(mesh, c.perp, b, fb, c.dir, (x0, z0));
                }
            }
        }
        for s in 0..self.stations.len() {
            let at = center - self.wrap(center - self.stations[s]);
            if inside(at) {
                Self::mesh_station(mesh, at, self.station_floor(s, field), (x0, z0));
            }
        }
    }

    /// A station's court: its floor, and a skirt down past the ground.
    fn mesh_station(mesh: &mut ColumnMesh, at: DVec2, floor: f64, origin: (f64, f64)) {
        let local = |p: DVec2, y: f64| Vec3::new((p.x - origin.0) as f32, y as f32, (p.y - origin.1) as f32);
        // (Corners between the edges `station_planes` gives.)
        let corner = |k: usize| {
            let a = (k as f64 + 0.5) / STATION_SIDES as f64 * std::f64::consts::TAU;
            at + DVec2::new(a.cos(), a.sin()) * STATION_RADIUS
        };
        let middle = local(at, floor);
        for k in 0..STATION_SIDES {
            let (p, q) = (local(corner(k), floor), local(corner(k + 1), floor));
            let base = mesh.positions.len() as u32;
            for v in [middle, p, q] {
                mesh.positions.push(v.to_array());
                mesh.normals.push([0.0, 1.0, 0.0]);
                mesh.albedo.push(STATION_ALBEDO);
                mesh.ao.push(1.0);
            }
            let face = (p - middle).cross(q - middle);
            let up = if face.y >= 0.0 { [base, base + 1, base + 2] } else { [base, base + 2, base + 1] };
            mesh.indices.extend_from_slice(&up);
            let out = (corner(k) + corner(k + 1)) * 0.5 - at;
            let normal = Vec3::new(out.x as f32, 0.0, out.y as f32).normalize();
            let down = Vec3::Y * OUTER_DEPTH as f32;
            push_quad(mesh, [p, q, q - down, p - down], normal, WALL_ALBEDO, [1.0, 1.0, 0.0, 0.0]);
        }
    }

    /// The end of a pipe where it opens onto a court: closed from its
    /// surface down to the court's floor, facing `out`.
    fn mesh_cap(mesh: &mut ColumnMesh, perp: DVec2, at: DVec2, floor: f64, out: DVec2, origin: (f64, f64)) {
        let local = |o: f64, h: f64| {
            let p = at + perp * o;
            Vec3::new((p.x - origin.0) as f32, (floor + h) as f32, (p.y - origin.1) as f32)
        };
        let normal = Vec3::new(out.x as f32, 0.0, out.y as f32);
        for w in Self::profile().windows(2) {
            let ((o0, h0, _), (o1, h1, _)) = (w[0], w[1]);
            if h0 + h1 < 1e-3 {
                continue;
            }
            push_quad(mesh, [local(o0, h0), local(o1, h1), local(o1, 0.0), local(o0, 0.0)], normal, WALL_ALBEDO, [1.0; 4]);
        }
    }

    /// The profile across a canal: (offset, height above floor, albedo).
    /// (Each point's albedo is the facet's that starts at it: the outermost
    /// each side dark.)
    fn profile() -> Vec<(f64, f64, f32)> {
        let mut profile = vec![(-PIPE_RADIUS, PIPE_RADIUS, DARK_ALBEDO)];
        for f in 1..FACETS {
            let theta = std::f64::consts::PI * (1.0 + f as f64 / FACETS as f64);
            let dark = f < DARK_FACETS || f >= FACETS - DARK_FACETS;
            profile.push((PIPE_RADIUS * theta.cos(), PIPE_RADIUS * (1.0 + theta.sin()), if dark { DARK_ALBEDO } else { PIPE_ALBEDO }));
        }
        profile.push((PIPE_RADIUS, PIPE_RADIUS, DARK_ALBEDO));
        profile
    }

    fn mesh_segment(mesh: &mut ColumnMesh, perp: DVec2, a: DVec2, b: DVec2, floor_a: f64, floor_b: f64, origin: (f64, f64)) {
        let world = |center: DVec2, f: f64, (o, h, _): (f64, f64, f32)| {
            let p = center + perp * o;
            Vec3::new((p.x - origin.0) as f32, (f + h) as f32, (p.y - origin.1) as f32)
        };
        // Smooth shading: each profile point gets the curved surface's
        // normal there (towards the pipe's axis).
        let profile_normal = |(o, h, _): (f64, f64, f32)| {
            let n = DVec2::new(-o, PIPE_RADIUS - h).normalize_or_zero();
            Vec3::new((perp.x * n.x) as f32, n.y as f32, (perp.y * n.x) as f32).normalize_or(Vec3::Y)
        };
        for w in Self::profile().windows(2) {
            let (p0, p1) = (w[0], w[1]);
            let quad = [world(a, floor_a, p0), world(a, floor_a, p1), world(b, floor_b, p1), world(b, floor_b, p0)];
            // Open side is up (and towards the pipe's axis inside it).
            let along = quad[3] - quad[0];
            let across = quad[1] - quad[0];
            let mut normal = across.cross(along).normalize_or_zero();
            if normal.y < 0.0 {
                normal = -normal;
            }
            let bottom_ness = 1.0 - (p0.1 + p1.1) as f32 * 0.5 / PIPE_RADIUS as f32;
            let ao = 1.0 - 0.35 * bottom_ness.max(0.0);
            let (n0, n1) = (profile_normal(p0), profile_normal(p1));
            push_quad_smooth(mesh, quad, [n0, n1, n1, n0], normal, p0.2, [ao; 4]);
        }

        // Outer walls, down past the ground on both sides.
        for side in [-1.0, 1.0] {
            let top_a = world(a, floor_a, (side * HALF_WIDTH, PIPE_RADIUS, 0.0));
            let top_b = world(b, floor_b, (side * HALF_WIDTH, PIPE_RADIUS, 0.0));
            let down = Vec3::Y * OUTER_DEPTH as f32;
            let normal = Vec3::new((perp.x * side) as f32, 0.0, (perp.y * side) as f32);
            push_quad(mesh, [top_a, top_b, top_b - down, top_a - down], normal, WALL_ALBEDO, [1.0, 1.0, 0.0, 0.0]);
        }
    }
}

fn centroid(poly: &[DVec2]) -> DVec2 {
    poly.iter().copied().sum::<DVec2>() / poly.len() as f64
}

/// The parts of a convex polygon outside a convex region, the region given
/// by its edges (each how far outside it a point is: positive outside).
fn subtract(poly: Vec<DVec2>, planes: &[Box<dyn Fn(DVec2) -> f64>]) -> Vec<Vec<DVec2>> {
    if poly.len() < 3 {
        return Vec::new();
    }
    // Wholly outside one edge: untouched.
    if planes.iter().any(|f| poly.iter().all(|&v| f(v) >= 0.0)) {
        return vec![poly];
    }
    let mut out = Vec::new();
    let mut rest = poly;
    for f in planes {
        let outside = clip_half_plane(&rest, f);
        if outside.len() >= 3 {
            out.push(outside);
        }
        rest = clip_half_plane(&rest, |v| -f(v));
        if rest.len() < 3 {
            break;
        }
    }
    out
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

/// Appends a quad as two triangles facing `facing`, with its own normal at
/// each corner for smooth shading.
fn push_quad_smooth(
    mesh: &mut ColumnMesh,
    quad: [Vec3; 4],
    normals: [Vec3; 4],
    facing: Vec3,
    albedo: f32,
    ao: [f32; 4],
) {
    let base = mesh.positions.len() as u32;
    for ((p, n), a) in quad.iter().zip(normals).zip(ao) {
        mesh.positions.push(p.to_array());
        mesh.normals.push(n.to_array());
        mesh.albedo.push(albedo);
        mesh.ao.push(a);
    }
    let face = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
    if face.dot(facing) >= 0.0 {
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        mesh.indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
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

    const SIZE: f64 = 16384.0;

    fn field(p: DVec2) -> f64 {
        (p.x * 0.001).sin() * 20.0 + (p.y * 0.0013).cos() * 15.0
    }

    /// A canal's floor meets its stations' at its ends: no step out of the
    /// pipe onto the court.
    #[test]
    fn floors_meet_the_stations() {
        let canals = Canals::new(SIZE, 1);
        let end = canals.end();
        for (i, c) in canals.lines.iter().enumerate() {
            for (along, s) in [(end, c.a), (c.length - end, c.b)] {
                let at = c.start + c.dir * along;
                let f = canals.floor(i, along, at, &field);
                assert!((f - canals.station_floor(s, &field)).abs() < 1e-6, "canal {i} at {along}");
            }
        }
    }

    /// The cut removes exactly the corridors and the courts: no piece left
    /// inside one, and the pieces cover the rest.
    #[test]
    fn cut_removes_corridors_and_courts() {
        let canals = Canals::new(SIZE, 1);
        let c = canals.lines[0];
        let area = |p: &[DVec2]| (0..p.len()).map(|k| p[k].perp_dot(p[(k + 1) % p.len()])).sum::<f64>().abs() * 0.5;
        for along in [c.length * 0.5, 10.0, c.length - 25.0] {
            let at = c.start + c.dir * along;
            let square = vec![at + DVec2::new(-30.0, -30.0), at + DVec2::new(30.0, -30.0), at + DVec2::new(30.0, 30.0), at + DVec2::new(-30.0, 30.0)];
            let pieces = canals.cut(square.clone());
            let kept: f64 = pieces.iter().map(|p| area(p)).sum();
            // The square's area outside, sampled a metre apart.
            let mut outside = 0;
            for x in 0..60 {
                for z in 0..60 {
                    let p = square[0] + DVec2::new(x as f64 + 0.5, z as f64 + 0.5);
                    if canals.station_at(p).is_none() && canals.hit(p, 0.0, &field).is_none() {
                        outside += 1;
                    }
                }
            }
            assert!((kept - outside as f64).abs() < 60.0, "along {along}: kept {kept:.0} m2, sampled {outside}");
            for piece in &pieces {
                let m = centroid(piece);
                assert!(canals.station_at(m).is_none() && canals.hit(m, 0.0, &field).is_none(), "a piece inside, along {along}");
            }
        }
    }

    /// Round the wrap, a canal is the same canal.
    #[test]
    fn canals_wrap() {
        let canals = Canals::new(SIZE, 5);
        for i in 0..canals.lines.len() {
            for k in 0..20 {
                let p = DVec2::new(k as f64 * 173.0, k as f64 * 61.0);
                let (l1, l2) = (canals.local(i, p), canals.local(i, p + DVec2::new(SIZE, -SIZE)));
                assert!((l1.offset - l2.offset).abs() < 1e-6 && (l1.along - l2.along).abs() < 1e-6);
            }
        }
    }
}
