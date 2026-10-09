//! The chasm (`--opt chasm`): a standalone place, apart from the torus world,
//! to explore the depths: two vast walls rising out of the haze with a gap
//! between them, crossed by bridges and a web of cables, the walls' detail
//! being their own geometry (bands, bays, frames within frames, rows of
//! openings, packed conduits, ledges, stairs), never dressing on a surface.
//! You start on the rim of one wall, looking across. The flat lab ground is
//! the floor, far below, lost in the haze.
//!
//! The chasm runs along z, centred on `CENTRE`; each wall is a chain of
//! straight faces at angles of their own (see `plan`), so the gap widens
//! and narrows (30-230 m) and the whole snakes. Everything here is
//! in metres.

use avian3d::prelude::*;
use bevy::{asset::RenderAssetUsages, mesh::{Indices, PrimitiveTopology}, prelude::*};
use manifold_csg::{
    CrossSection, Manifold,
    cross_section::{FillRule, JoinType},
};
use worldgen::noise::hash01;

use crate::{Args, rope::tubes};

/// A taut cable from `a` to `b`, sagging `sag` in the middle.
fn rope_static(a: Vec3, b: Vec3, sag: f32) -> Vec<Vec3> {
    let n = ((a.distance(b) / 1.5).ceil() as usize).clamp(8, 64);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            a.lerp(b, t) - Vec3::Y * sag * 4.0 * t * (1.0 - t)
        })
        .collect()
}

/// Where the chasm is (the default camera start in the world is nearby).
pub const CENTRE: Vec3 = Vec3::new(1200.0, 0.0, 900.0);
/// The walls' height, and the segment's length.
pub const HEIGHT: f32 = 550.0;
pub const LENGTH: f32 = 1000.0;
/// How far the walls' masses reach back from their faces (the plateau on
/// top).
const BACK: f32 = 120.0;

pub struct ChasmPlugin;

impl Plugin for ChasmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build);
    }
}

/// The height of the walkable top at (x, z) if it is on one of the walls.
pub fn rim(seed: u32, x: f32, z: f32) -> Option<f32> {
    let seed = seed as i32;
    let walls = [shape(-1.0, seed, &plan(1.0, seed + 7919)), shape(1.0, seed + 7919, &plan(-1.0, seed))];
    let p = Vec3::new(x, HEIGHT, z);
    walls
        .iter()
        .any(|w| {
            locate(w, z).is_some_and(|(m, _)| {
                let n = (p - m.wall.origin).dot(m.wall.out);
                n < m.face(HEIGHT) && n > m.face(HEIGHT) - BACK * 0.9
            })
        })
        .then_some(HEIGHT)
}

/// Where you start: on the rim of the near wall behind the first flight,
/// looking along it and down.
pub fn start(seed: u32) -> [f32; 5] {
    layout(seed as i32).2.start
}

/// A stretch of a wall's plan: from `z.0` to `z.1` (absolute), its face
/// running from x `x.0` to `x.1`; a shaft (a deep narrow slot) or a massif.
#[derive(Clone, Copy)]
struct Stretch {
    z: (f32, f32),
    x: (f32, f32),
    shaft: bool,
}

/// A wall's plan, along the chasm: stretches 70-260 m long (shafts 4-10 m),
/// each wall standing 15-115 m from a centre line that wanders, so
/// the faces meet at angles, the gap widens and narrows and the walls are
/// rarely parallel. `side` -1 for the near wall (low x), +1 for the far.
fn plan(side: f32, seed: i32) -> Vec<Stretch> {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d2);
    // The centre line wanders the same way for both walls.
    let drift = |z: f32| 36.0 * (z * 0.0067 + 1.3).sin() + 16.0 * (z * 0.017 + 0.4).sin();
    let half = LENGTH * 0.5;
    // (The joints go near, far, anywhere in turn, out of step between the
    // walls, so the gap surely narrows and widens.)
    let phase = if side < 0.0 { 0 } else { 1 };
    let at = |z: f32, k: i32| {
        let t = match (k + phase) % 3 {
            0 => 0.25 * r(k, 9),
            1 => 0.7 + 0.3 * r(k, 9),
            _ => r(k, 9),
        };
        CENTRE.x + drift(z) + side * (15.0 + 100.0 * t)
    };
    let mut out = Vec::new();
    let mut z = CENTRE.z - half;
    let mut k = 0;
    let mut x = at(z, 0);
    while z < CENTRE.z + half {
        // (No shaft where you start.)
        let shaft = r(k, 0) < 0.18 && !(z - 12.0..z + 12.0).contains(&CENTRE.z);
        let length = if shaft { 4.0 + 6.0 * r(k, 1) } else { 70.0 + 190.0 * r(k, 1) }.min(CENTRE.z + half - z);
        let z1 = z + length;
        let x1 = if shaft { x } else { at(z1, k + 1) };
        out.push(Stretch { z: (z, z1), x: (x, x1), shaft });
        z = z1;
        x = x1;
        k += 1;
    }
    out
}

/// Where a wall's face is (its x) at `z`, from its plan.
fn face_x(plan: &[Stretch], z: f32) -> f32 {
    plan.iter()
        .find(|s| z >= s.z.0 && z <= s.z.1)
        .map_or(CENTRE.x, |s| s.x.0 + (s.x.1 - s.x.0) * ((z - s.z.0) / (s.z.1 - s.z.0).max(1e-3)))
}

/// Triangles, flat-shaded, for one material.
#[derive(Default)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl Geometry {
    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3) {
        let base = self.positions.len() as u32;
        for c in corners {
            self.positions.push(c.to_array());
            self.normals.push(normal.to_array());
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A convex polygon face, wound to face away from `inside`.
    fn face(&mut self, pts: &[Vec3], inside: Vec3) {
        let mut n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or(Vec3::Y);
        let centre = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
        let mut order: Vec<Vec3> = pts.to_vec();
        if n.dot(centre - inside) < 0.0 {
            n = -n;
            order.reverse();
        }
        let base = self.positions.len() as u32;
        for p in &order {
            self.positions.push(p.to_array());
            self.normals.push(n.to_array());
        }
        for i in 1..order.len() as u32 - 1 {
            self.indices.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }

    /// A convex polygon swept along `d`: a prism of any cross-section.
    fn sweep(&mut self, section: &[Vec3], d: Vec3) {
        let k = section.len();
        let inside = section.iter().copied().sum::<Vec3>() / k as f32 + d * 0.5;
        let far: Vec<Vec3> = section.iter().map(|&p| p + d).collect();
        self.face(section, inside);
        self.face(&far, inside);
        for i in 0..k {
            let j = (i + 1) % k;
            self.face(&[section[i], section[j], far[j], far[i]], inside);
        }
    }

    /// A horizontal slab from `y0` to `y1` over the convex hull of points in
    /// plan (their heights ignored).
    fn plate(&mut self, pts: &[Vec3], y0: f32, y1: f32) {
        let mut p: Vec<Vec2> = pts.iter().map(|q| Vec2::new(q.x, q.z)).collect();
        p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        p.dedup_by(|a, b| a.distance(*b) < 1e-3);
        if p.len() < 3 {
            return;
        }
        let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
        let mut hull: Vec<Vec2> = Vec::new();
        for pass in 0..2 {
            let start = hull.len();
            let it: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 { Box::new(p.iter()) } else { Box::new(p.iter().rev()) };
            for &q in it {
                while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 0.0 {
                    hull.pop();
                }
                hull.push(q);
            }
            hull.pop();
        }
        if hull.len() < 3 {
            return;
        }
        let section: Vec<Vec3> = hull.iter().map(|q| Vec3::new(q.x, y0, q.y)).collect();
        self.sweep(&section, Vec3::Y * (y1 - y0));
    }

    /// A box from its centre and three half-axes (any orientation).
    fn oriented(&mut self, c: Vec3, a: Vec3, b: Vec3, n: Vec3) {
        // (Kept right-handed so the faces wind outwards.)
        let a = if a.cross(b).dot(n) < 0.0 { -a } else { a };
        let p = |sa: f32, sb: f32, sn: f32| c + a * sa + b * sb + n * sn;
        let (na, nb, nn) = (a.normalize_or(Vec3::X), b.normalize_or(Vec3::Y), n.normalize_or(Vec3::Z));
        self.quad([p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.), p(1., -1., 1.)], na);
        self.quad([p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.), p(-1., -1., -1.)], -na);
        self.quad([p(-1., 1., -1.), p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.)], nb);
        self.quad([p(-1., -1., 1.), p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.)], -nb);
        self.quad([p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)], nn);
        self.quad([p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)], -nn);
    }

    /// A box between two points (its long axis), `w` wide and `t` thick,
    /// `up` fixing which way is thick.
    fn beam(&mut self, a: Vec3, b: Vec3, w: f32, t: f32, up: Vec3) {
        let d = b - a;
        let along = d.normalize_or(Vec3::X);
        let side = along.cross(up).normalize_or(along.any_orthonormal_vector()) * (w * 0.5);
        let lift = side.cross(along).normalize() * (t * 0.5);
        let corner = |s: f32, l: f32, end: Vec3| end + side * s + lift * l;
        let faces = [
            ([corner(-1., 1., a), corner(-1., 1., b), corner(1., 1., b), corner(1., 1., a)], lift),
            ([corner(1., -1., a), corner(1., -1., b), corner(-1., -1., b), corner(-1., -1., a)], -lift),
            ([corner(1., 1., a), corner(1., 1., b), corner(1., -1., b), corner(1., -1., a)], side),
            ([corner(-1., -1., a), corner(-1., -1., b), corner(-1., 1., b), corner(-1., 1., a)], -side),
            ([corner(-1., -1., a), corner(-1., 1., a), corner(1., 1., a), corner(1., -1., a)], -along),
            ([corner(1., -1., b), corner(1., 1., b), corner(-1., 1., b), corner(-1., -1., b)], along),
        ];
        // (The corners above run clockwise seen from outside: reversed, so
        // the faces wind outwards and the near ones are the ones drawn.)
        for (mut c, n) in faces {
            c.reverse();
            self.quad(c, n.normalize());
        }
    }

    /// A cylinder between two points, faceted.
    fn cylinder(&mut self, a: Vec3, b: Vec3, r: f32, sides: usize) {
        let along = (b - a).normalize_or(Vec3::Y);
        let (u, v) = along.any_orthonormal_pair();
        for k in 0..sides {
            let (a0, a1) = (k as f32 / sides as f32 * std::f32::consts::TAU, (k + 1) as f32 / sides as f32 * std::f32::consts::TAU);
            let (o0, o1) = (u * a0.cos() + v * a0.sin(), u * a1.cos() + v * a1.sin());
            let n = (o0 + o1).normalize();
            self.quad([a + o0 * r, a + o1 * r, b + o1 * r, b + o0 * r], n);
        }
    }

    /// Where two faces lie in the same plane, facing the same way, and
    /// overlap (they flicker as the depth test picks one or the other): the
    /// middle of each overlapping pair.
    fn coincident(&self) -> Vec<(usize, usize, Vec3)> {
        use std::collections::HashMap;
        let p = |i: u32| Vec3::from(self.positions[i as usize]);
        let tris: Vec<[Vec3; 3]> = self.indices.chunks_exact(3).map(|t| [p(t[0]), p(t[1]), p(t[2])]).collect();
        // Grouped by plane (normal to about a degree, offset to 2 cm) and
        // by a 16 m cell in that plane.
        let mut groups: HashMap<(i32, i32, i32, i32, i32, i32), Vec<usize>> = HashMap::new();
        for (k, t) in tris.iter().enumerate() {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            if n.length() < 1e-4 {
                continue;
            }
            let n = n.normalize();
            let (e1, e2) = n.any_orthonormal_pair();
            let c = (t[0] + t[1] + t[2]) / 3.0;
            let key = (
                (n.x * 60.0).round() as i32,
                (n.y * 60.0).round() as i32,
                (n.z * 60.0).round() as i32,
                (n.dot(c) * 50.0).round() as i32,
                (e1.dot(c) / 16.0).floor() as i32,
                (e2.dot(c) / 16.0).floor() as i32,
            );
            groups.entry(key).or_default().push(k);
        }
        // Overlap of two triangles in their plane (separating axes), by more
        // than a sliver.
        let overlap = |a: &[Vec3; 3], b: &[Vec3; 3]| {
            let n = (a[1] - a[0]).cross(a[2] - a[0]).normalize();
            let (e1, e2) = n.any_orthonormal_pair();
            let flat = |t: &[Vec3; 3]| t.map(|q| Vec2::new(e1.dot(q), e2.dot(q)));
            let (a, b) = (flat(a), flat(b));
            for tri in [&a, &b] {
                for i in 0..3 {
                    let d = tri[(i + 1) % 3] - tri[i];
                    let axis = Vec2::new(-d.y, d.x).normalize_or_zero();
                    let span = |t: &[Vec2; 3]| t.iter().map(|q| axis.dot(*q)).fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
                    let ((a0, a1), (b0, b1)) = (span(&a), span(&b));
                    if a1.min(b1) - a0.max(b0) < 0.02 {
                        return false;
                    }
                }
            }
            true
        };
        let mut found = Vec::new();
        for list in groups.values() {
            for (i, &x) in list.iter().enumerate() {
                for &y in &list[i + 1..] {
                    // (The two halves of one quad share an edge, not area.)
                    if overlap(&tris[x], &tris[y]) {
                        found.push((x, y, (tris[x][0] + tris[x][1] + tris[x][2] + tris[y][0] + tris[y][1] + tris[y][2]) / 6.0));
                    }
                }
            }
        }
        found
    }

    /// Where lines (named) pass through any of its triangles: the first
    /// crossing on each, if any.
    fn crossings<'a>(&self, lines: &'a [(String, Vec<Vec3>)]) -> Vec<(&'a str, Vec3, usize)> {
        use std::collections::HashMap;
        const CELL: f32 = 6.0;
        let p = |i: u32| Vec3::from(self.positions[i as usize]);
        let tris: Vec<[Vec3; 3]> = self.indices.chunks_exact(3).map(|t| [p(t[0]), p(t[1]), p(t[2])]).collect();
        let cell = |q: Vec3| ((q.x / CELL).floor() as i32, (q.y / CELL).floor() as i32, (q.z / CELL).floor() as i32);
        let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for (k, t) in tris.iter().enumerate() {
            let (lo, hi) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
            let (a, b) = (cell(lo), cell(hi));
            if (b.0 - a.0 + 1) * (b.1 - a.1 + 1) * (b.2 - a.2 + 1) > 4096 {
                continue;
            }
            for x in a.0..=b.0 {
                for y in a.1..=b.1 {
                    for z in a.2..=b.2 {
                        grid.entry((x, y, z)).or_default().push(k);
                    }
                }
            }
        }
        // (Moller-Trumbore: whether segment a-b passes through triangle t.
        // Not through slivers with no area, which block nothing: in f32, far
        // from the origin, their determinant is noise, not zero; a real skin
        // has triangles with area.)
        let hit = |a: Vec3, b: Vec3, t: &[Vec3; 3]| {
            let d = b - a;
            let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
            let area = e1.cross(e2).length();
            if area < 1e-3 * (e1.length() + e2.length()).max(1e-3) {
                return None;
            }
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-9 {
                return None;
            }
            let f = 1.0 / det;
            let s = a - t[0];
            let u = f * s.dot(h);
            let q = s.cross(e1);
            let v = f * d.dot(q);
            let w = f * e2.dot(q);
            (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && (0.0..=1.0).contains(&w)).then(|| a + d * w)
        };
        let mut out = Vec::new();
        'line: for (what, pts) in lines {
            for seg in pts.windows(2) {
                let (a, b) = (seg[0], seg[1]);
                let steps = (a.distance(b) / (CELL * 0.5)).ceil().max(1.0) as i32;
                let mut seen = std::collections::HashSet::new();
                for i in 0..=steps {
                    let c = cell(a.lerp(b, i as f32 / steps as f32));
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                for &k in grid.get(&(c.0 + dx, c.1 + dy, c.2 + dz)).map(|v| &v[..]).unwrap_or(&[]) {
                                    if seen.insert(k)
                                        && let Some(at) = hit(a, b, &tris[k])
                                    {
                                        out.push((what.as_str(), at, k));
                                        continue 'line;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// A solid from the boolean library, flat-shaded.
    fn solid(&mut self, m: &Manifold) {
        let (verts, props, tris) = m.to_mesh_f32();
        let p = |i: u32| Vec3::new(verts[i as usize * props], verts[i as usize * props + 1], verts[i as usize * props + 2]);
        for t in tris.chunks_exact(3) {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let n = (b - a).cross(c - a).normalize_or(Vec3::Y);
            let base = self.positions.len() as u32;
            for q in [a, b, c] {
                self.positions.push(q.to_array());
                self.normals.push(n.to_array());
            }
            self.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }

    fn mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_indices(Indices::U32(self.indices))
    }

    fn collider(&self) -> Option<Collider> {
        let vertices = self.positions.iter().map(|&p| Vec3::from(p)).collect();
        let indices = self.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        Collider::try_trimesh(vertices, indices).ok()
    }
}

/// One face's frame: `at(u, v, n)` is the point `u` along the face from its
/// start, `v` up and `n` out from it into the gap, the face standing
/// `offset` out from its line (set back where negative).
#[derive(Clone, Copy)]
struct Wall {
    origin: Vec3,
    along: Vec3,
    out: Vec3,
    offset: f32,
}

impl Wall {
    fn at(&self, u: f32, v: f32, n: f32) -> Vec3 {
        self.origin + self.along * u + Vec3::Y * v + self.out * (n + self.offset)
    }

    /// The same face standing `offset` out.
    fn moved(&self, offset: f32) -> Wall {
        Wall { offset, ..*self }
    }

    /// A prism along the face from `u.0` to `u.1` whose cross-section is the
    /// convex polygon `section` of (v, n) points.
    fn section(&self, g: &mut Geometry, u: (f32, f32), section: &[(f32, f32)]) {
        let pts: Vec<Vec3> = section.iter().map(|&(v, n)| self.at(u.0, v, n)).collect();
        g.sweep(&pts, self.along * (u.1 - u.0));
    }

    /// A box on the face from (u0, v0) to (u1, v1), standing out from `n0`
    /// to `n1`.
    fn block(&self, g: &mut Geometry, u: (f32, f32), v: (f32, f32), n: (f32, f32)) {
        let centre = self.at((u.0 + u.1) * 0.5, (v.0 + v.1) * 0.5, (n.0 + n.1) * 0.5);
        g.oriented(centre, self.along * ((u.1 - u.0).abs() * 0.5), Vec3::Y * ((v.1 - v.0).abs() * 0.5), self.out * ((n.1 - n.0).abs() * 0.5));
    }
}

/// A stretch of wall with its own face (its `wall`, `len` long): set back
/// or bulging, its upper part often overhanging the lower; or a shaft, a
/// deep slot.
struct Massif {
    wall: Wall,
    len: f32,
    z: (f32, f32),
    split: f32,
    low: f32,
    high: f32,
    /// How tall the sloped face is, below the split, that carries the face
    /// from `low` out (or back) to `high`.
    slope: f32,
    /// How far it may reach out into the gap (half the gap here, less a
    /// margin).
    room: f32,
    shaft: bool,
}

impl Massif {
    /// How far its face stands out at height `v`.
    fn face(&self, v: f32) -> f32 {
        if self.shaft {
            SHAFT_DEPTH
        } else if v < self.split - self.slope {
            self.low
        } else if v < self.split {
            self.low + (self.high - self.low) * (v - (self.split - self.slope)) / self.slope.max(1e-3)
        } else {
            self.high
        }
    }
}

/// The massif at `z` on a wall, and how far along its face `z` is.
fn locate(massifs: &[Massif], z: f32) -> Option<(&Massif, f32)> {
    let m = massifs.iter().find(|m| z >= m.z.0 && z <= m.z.1)?;
    Some((m, (z - m.z.0) / (m.z.1 - m.z.0).max(1e-3) * m.len))
}

/// How deep a shaft is cut into the wall.
const SHAFT_DEPTH: f32 = -18.0;

/// Everything the chasm is made of, by material.
#[derive(Default)]
struct Parts {
    stone: Geometry,
    dark: Geometry,
    /// Light built in: lines along bands, strips deep in shafts, glowing
    /// grooves and the edges of overhangs; `dim`, slits lit faintly within.
    glow: Geometry,
    dim: Geometry,
    /// Cables: points, thickness.
    cables: Vec<(Vec<Vec3>, f32)>,
    /// Lights cast by the built-in light (the glowing strips light only
    /// themselves): where, how far they reach, and how bright (times the
    /// chasm's light).
    lights: Vec<(Vec3, f32, f32)>,
    /// The chambers cut into the walls (for what joins them).
    chambers: Vec<Chamber>,
    /// The places on the walkways (halls with terraces).
    terraces: Vec<Terrace>,
    /// Flights' steps (seen, not walked on), and the slopes beneath them
    /// (walked on, not seen: real steps make you vault).
    steps: Geometry,
    ramps: Geometry,
    /// (`--opt seams`: the walls' solids, and the space a walker needs over
    /// each route, by what it is, to check one against the other.)
    walls: Vec<Manifold>,
    clearance: Vec<(String, Manifold)>,
    /// (`--opt seams`: lines a walker follows along each route, at chest
    /// height: no surface may cross them.)
    paths: Vec<(String, Vec<Vec3>)>,
    /// Where piers stand (one to a spot).
    piers: Vec<Vec3>,
    /// (`--opt seams`: which builder made the stone from which triangle on.)
    marks: Vec<(usize, &'static str)>,
    /// Where on the face being built routes run (walkways, stairs), as (u0,
    /// u1, v0, v1) in its frame: relief keeps flush there, so they have room.
    clear: Vec<(f32, f32, f32, f32)>,
    /// The space every route needs, as boxes in the world (both walls'):
    /// nothing on a face may stand into them. And the face being built (its
    /// frame), to place what is built on it in the world.
    keep: Vec<(Vec3, Vec3)>,
    frame: Option<Wall>,
}

impl Parts {
    fn mark(&mut self, what: &'static str) {
        self.marks.push((self.stone.indices.len() / 3, what));
    }

    fn maker(&self, triangle: usize) -> &'static str {
        self.marks.iter().rev().find(|m| m.0 <= triangle).map_or("?", |m| m.1)
    }

    /// Whether a piece of the face being built overlaps where a route runs:
    /// along the face, or in the world, as far out as anything on the face
    /// stands (whatever the faces' angles, either wall's routes).
    /// Whether a box in the world overlaps any route's space.
    fn keeps(&self, lo: Vec3, hi: Vec3) -> bool {
        self.keep.iter().any(|(a, b)| lo.x < b.x && hi.x > a.x && lo.y < b.y && hi.y > a.y && lo.z < b.z && hi.z > a.z)
    }

    fn blocked(&self, u: (f32, f32), v: (f32, f32)) -> bool {
        self.clear.iter().any(|&(a, b, c, d)| u.0 < b && u.1 > a && v.0 < d && v.1 > c)
            || self.frame.is_some_and(|w| {
                let corners = [u.0, u.1].into_iter().flat_map(|a| [v.0, v.1].into_iter().flat_map(move |b| [-1.0, STANDS_OUT].map(move |c| w.at(a, b, c))));
                let (lo, hi) = corners.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
                self.keeps(lo, hi)
            })
    }
}

/// A chamber cut into a wall: which wall, where its mouth is along the
/// chasm, its floor and ceiling heights, the face it opens in (its
/// `Wall`), and how deep it goes.
#[derive(Clone, Copy)]
struct Chamber {
    side: f32,
    wall: Wall,
    u: (f32, f32),
    floor: f32,
    ceiling: f32,
    depth: f32,
}

/// A place on a walkway: a low hall cut back into the wall, wider than
/// tall, the walkway passing through it between gateways; its floor running
/// on out past the face as a terrace with chamfered corners, looking along
/// the chasm both ways and across. In its massif's frame: which walkway and
/// massif, from `u.0` to `u.1` along the face (`z` along the chasm), the
/// floor and the hall's ceiling, the face's `n`, how far back the hall goes
/// (`back`) and how far out the terrace reaches (`front`).
#[derive(Clone, Copy)]
struct Terrace {
    way: usize,
    massif: usize,
    u: (f32, f32),
    z: (f32, f32),
    floor: f32,
    ceiling: f32,
    face: f32,
    back: f32,
    front: f32,
}

/// How far anything built on a face stands out from it, at most.
const STANDS_OUT: f32 = 8.0;

/// How bright the chasm's own lights are (lumens; `--set chasm_light`).
const LIGHT: f32 = 2.0e8;

fn build(
    mut commands: Commands,
    args: Res<Args>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !args.opt("chasm") {
        return;
    }
    let seed = args.seed as i32;
    let mut parts = Parts::default();
    // (`--opt chambers`: chambers cut into the walls, set aside for now.)
    let chambers = args.opt("chambers");
    // (The walls' shapes and the routes are planned first: the walls keep
    // clear where the routes run.)
    let (near, far, routing, solids) = layout(seed);
    let bottom = routing.ways.last().map_or(HEIGHT, |w| w.v0);
    info!(
        "the chasm: the way down: {} flights, {} tunnels, {} walkways, {} bridges, down to {bottom:.0} m; {:.1} km, {:.0} min at a walk",
        routing.flights.len(),
        routing.tunnels.len(),
        routing.ways.len(),
        routing.bridges.len(),
        routing.length / 1000.0,
        routing.length / 4.0 / 60.0
    );
    // (What the walls' detail must keep clear of: every route's space.)
    let bounds = |m: &Manifold, pad: f32| {
        m.bounding_box().map(|b| {
            let (lo, hi) = (b.min(), b.max());
            (Vec3::new(lo[0] as f32, lo[1] as f32, lo[2] as f32) - pad, Vec3::new(hi[0] as f32, hi[1] as f32, hi[2] as f32) + pad)
        })
    };
    let walls_of = |side: f32| if side < 0.0 { &near } else { &far };
    for w in &routing.ways {
        parts.keep.extend(bounds(&walk_space(w, walls_of(w.side)), 0.5));
    }
    for f in &routing.flights {
        let m = &walls_of(f.side)[f.m];
        let n0 = inner(m, f.v.0);
        parts.keep.extend(bounds(&flight_space(m, f.u, f.v, (n0, n0 + f.width)), 0.5));
    }
    for t in &routing.tunnels {
        let m = &walls_of(t.side)[t.m];
        for (u, v) in t.doors {
            parts.keep.extend(bounds(&wbox(&m.wall, (u - t.width, u + t.width), (v - 1.0, v + 4.0), (inner(m, v) - 1.0, inner(m, v) + 4.0)), 0.5));
        }
    }
    for s in &routing.spans {
        let (lo, hi) = (s.0.min(s.1), s.0.max(s.1));
        parts.keep.push((lo - Vec3::new(4.0, s.2 + 1.0, 4.0), hi + Vec3::new(4.0, 4.0, 4.0)));
    }
    wall(&mut parts, -1.0, seed, &near, &solids[0], &routing, chambers);
    wall(&mut parts, 1.0, seed + 7919, &far, &solids[1], &routing, chambers);
    crossings(&mut parts);
    routes(&mut parts, &routing, &near, &far);
    parts.mark("web");
    web(&mut parts, seed, &near, &far, &routing);
    // `--opt seams`: where faces coincide (they flicker), and where a route
    // runs into a wall (no way on), logged.
    if args.opt("seams") {
        let walls = Manifold::batch_union(&parts.walls);
        let blocked: Vec<_> = parts.clearance.iter().filter_map(|(what, c)| {
            let x = c.intersection(&walls);
            (x.volume() > 0.05).then(|| (what, x.volume(), x.bounding_box()))
        }).collect();
        info!("the chasm: {} of {} routes' spaces run into a wall", blocked.len(), parts.clearance.len());
        for (what, vol, bb) in blocked.iter().take(20) {
            info!("the chasm: blocked: {what} ({vol:.1} m3, {bb:?})");
        }
        let crossed = parts.stone.crossings(&parts.paths);
        info!("the chasm: {} of {} routes' ways crossed by a surface", crossed.len(), parts.paths.len());
        for (what, at, k) in crossed.iter().take(20) {
            let t = [0, 1, 2].map(|i| Vec3::from(parts.stone.positions[parts.stone.indices[k * 3 + i] as usize]));
            info!("the chasm: crossed: {what} at {at:?}, by {}: {t:?}", parts.maker(*k));
        }
        let found = parts.stone.coincident();
        info!("the chasm: {} coinciding faces", found.len());
        let mut kinds: std::collections::BTreeMap<(&str, &str), (usize, Vec3)> = default();
        for &(a, b, at) in &found {
            let (x, y) = (parts.maker(a), parts.maker(b));
            let e = kinds.entry((x.min(y), x.max(y))).or_insert((0, at));
            e.0 += 1;
        }
        for ((x, y), (n, at)) in kinds {
            info!("the chasm: coinciding {x} / {y}: {n}, e.g. at {at:?}");
        }
    }

    let stone = materials.add(StandardMaterial { base_color: Color::srgb(0.62, 0.62, 0.62), perceptual_roughness: 0.92, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.02, 0.02, 0.02), perceptual_roughness: 0.9, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(60.0), ..default() });
    let dim = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(8.0), ..default() });
    let cable = materials.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.05), perceptual_roughness: 0.6, ..default() });

    if let Some(collider) = parts.stone.collider() {
        commands.spawn((RigidBody::Static, collider, Transform::IDENTITY));
    }
    if let Some(collider) = parts.ramps.collider() {
        commands.spawn((RigidBody::Static, collider, Transform::IDENTITY));
    }
    let steps_mesh = meshes.add(std::mem::take(&mut parts.steps).mesh());
    commands.spawn((Mesh3d(steps_mesh), MeshMaterial3d(stone.clone()), Transform::IDENTITY));
    let stone_mesh = meshes.add(std::mem::take(&mut parts.stone).mesh());
    commands.spawn((Mesh3d(stone_mesh), MeshMaterial3d(stone), Transform::IDENTITY));
    let dark_mesh = meshes.add(std::mem::take(&mut parts.dark).mesh());
    commands.spawn((Mesh3d(dark_mesh), MeshMaterial3d(dark), Transform::IDENTITY));
    let glow_mesh = meshes.add(std::mem::take(&mut parts.glow).mesh());
    commands.spawn((Mesh3d(glow_mesh), MeshMaterial3d(glow), Transform::IDENTITY, bevy::light::NotShadowCaster));
    let dim_mesh = meshes.add(std::mem::take(&mut parts.dim).mesh());
    commands.spawn((Mesh3d(dim_mesh), MeshMaterial3d(dim), Transform::IDENTITY, bevy::light::NotShadowCaster));
    let mut cable_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    tubes(&mut cable_mesh, parts.cables.iter().map(|(p, w)| (&p[..], (*w, *w), 0.05)));
    commands.spawn((Mesh3d(meshes.add(cable_mesh)), MeshMaterial3d(cable), Transform::IDENTITY));
    let power = args.num("chasm_light", LIGHT);
    for &(at, range, k) in &parts.lights {
        commands.spawn((PointLight { intensity: power * k, range, shadow_maps_enabled: false, ..default() }, Transform::from_translation(at)));
    }
    for t in &parts.terraces {
        let m = if routing.ways[t.way].side < 0.0 { &near } else { &far };
        let w = &m[t.massif].wall;
        let c = (t.u.0 + t.u.1) * 0.5;
        info!("the chasm: a place at {:?} (along {:?}, out {:?}, {:.0} m wide, back {:.0} m, out {:.0} m)", w.at(c, t.floor, t.face), w.along, w.out, t.u.1 - t.u.0, t.face - t.back, t.front - t.face);
    }
    info!("the chasm: built ({} lights, {} places)", parts.lights.len(), parts.terraces.len());
}

/// A wall: massifs along it, each with its own face (set back, bulging, its
/// upper part often overhanging), some cut by deep shafts between them; each
/// part of a massif in strata of its own (so the bands do not line up across
/// the wall), or a colossal bare slab finely speckled; a few giant columns;
/// heavy cables hanging down it.
/// A wall's shape, before anything is built: along its plan, a massif to
/// each stretch, its own face set back or standing out, its upper part
/// often overhanging the lower (or a shaft, a deep slot). (Its geometry comes
/// with `wall`.)
fn shape(side: f32, seed: i32, other: &[Stretch]) -> Vec<Massif> {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c1);
    let mut massifs = Vec::new();
    for (m, stretch) in plan(side, seed).into_iter().enumerate() {
        let m = m as i32;
        // The face's frame: along it, and out of it into the gap.
        let (start, end) = (Vec3::new(stretch.x.0, 0.0, stretch.z.0), Vec3::new(stretch.x.1, 0.0, stretch.z.1));
        let along = (end - start).normalize_or(Vec3::Z);
        let mut out = Vec3::new(along.z, 0.0, -along.x);
        if out.x * side > 0.0 {
            out = -out;
        }
        let nominal = Wall { origin: start, along, out, offset: 0.0 };
        let len = start.distance(end);
        if stretch.shaft {
            massifs.push(Massif { wall: nominal, len, z: stretch.z, split: HEIGHT, low: SHAFT_DEPTH, high: SHAFT_DEPTH, slope: 0.0, room: 0.0, shaft: true });
            continue;
        }
        let split = HEIGHT * (0.3 + 0.5 * r(m, 2));
        let low = -10.0 + 18.0 * r(m, 3);
        // How much room there is: half the gap here, less a margin, so the
        // walls never close below about 50 m.
        let zmid = (stretch.z.0 + stretch.z.1) * 0.5;
        let room = ((face_x(other, zmid) - (stretch.x.0 + stretch.x.1) * 0.5).abs() * 0.5 - 25.0).max(0.0);
        // The upper part leans out over the chasm (mostly) or stands back,
        // a sloped face carrying it there. (Never set back far: you start
        // on the rim.)
        let delta = if r(m, 4) < 0.65 { 8.0 + 27.0 * r(m, 6) } else { -(4.0 + 10.0 * r(m, 6)) };
        let high = (low + delta).clamp(-4.0, (low + room.max(2.0)).max(-4.0));
        let slope = ((high - low).abs() * (0.5 + 1.5 * r(m, 7))).min(split - 30.0).max(0.0);
        massifs.push(Massif { wall: nominal, len, z: stretch.z, split, low, high, slope, room, shaft: false });
    }
    massifs
}

/// A wall's mass as one solid, from its shape: each massif's (its lower
/// part, its sloped stretch, its upper part; a shaft's slot), and between
/// them what fills their corners: band by band in height (in each, both
/// faces plain or evenly sloped), the hull of the one's end and the other's
/// start, so the fill runs from face to face and never stands out of either.
fn wall_solid(massifs: &[Massif]) -> Manifold {
    let mut mass = Vec::new();
    for ms in massifs {
        let (w, len) = (&ms.wall, ms.len);
        if ms.shaft {
            mass.push(wbox(w, (0.0, len), (0.0, HEIGHT), (-BACK, SHAFT_DEPTH)));
            continue;
        }
        // (One piece: its whole profile, lower part, sloped stretch and upper
        // part, swept along the face, so there are no seams inside it: pieces
        // that only touch leave a skin of rock between them, which a tunnel
        // cut through would meet.)
        let (a, b) = (ms.split - ms.slope, ms.split);
        let profile = [[-BACK, 0.0], [ms.low, 0.0], [ms.low, a], [ms.high, b], [ms.high, HEIGHT], [-BACK, HEIGHT]].map(|[n, v]| [n as f64, v as f64]);
        // (Section in (n, v), swept along u, set with n out and v up; swept
        // along the face, or back along it from its far end, whichever keeps
        // the frame right-handed.)
        let swept = CrossSection::from_polygons_with_fill_rule(&[profile.to_vec()], FillRule::NonZero).extrude(len as f64);
        let (al, out) = (w.along, w.out);
        let (z, o) = if out.dot(Vec3::Y.cross(al)) > 0.0 { (al, w.origin) } else { (-al, w.origin + al * len) };
        mass.push(swept.transform(&[out.x as f64, out.y as f64, out.z as f64, 0.0, 1.0, 0.0, z.x as f64, z.y as f64, z.z as f64, o.x as f64, o.y as f64, o.z as f64]));
    }
    for pair in massifs.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let mut cuts = vec![0.0, HEIGHT];
        for x in [a, b].into_iter().filter(|x| !x.shaft) {
            cuts.extend([x.split - x.slope, x.split]);
        }
        cuts.retain(|v| (0.0..=HEIGHT).contains(v));
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|x, y| (*x - *y).abs() < 0.01);
        // (Each band overlaps the massifs' ends, and the bands above and
        // below it, by `E`, never in front of a face: pieces that only touch
        // leave a skin of rock between them.)
        const E: f32 = 0.05;
        for band in cuts.windows(2) {
            let (v0, v1) = (band[0], band[1]);
            let mut pts = Vec::new();
            for (m, u, inward) in [(a, a.len, -E), (b, 0.0, E)] {
                let (f0, f1) = (m.face(v0 + 1e-3), m.face(v1 - 1e-3));
                let (w0, w1) = ((v0 - E).max(0.0), (v1 + E).min(HEIGHT));
                let mut at = vec![(u, v0, f0), (u, v1, f1), (u + inward, v0, f0 - E), (u + inward, v1, f1 - E)];
                for (v, f) in [(w0, f0.min(m.face(w0))), (w1, f1.min(m.face(w1)))] {
                    at.extend([(u, v, f - E), (u + inward, v, f - E)]);
                }
                for &(u, v, _) in &at.clone() {
                    at.push((u, v, -BACK));
                }
                for (u, v, n) in at {
                    let p = m.wall.at(u, v, n);
                    pts.push([p.x as f64, p.y as f64, p.z as f64]);
                }
            }
            mass.push(Manifold::hull_pts(&pts));
        }
    }
    Manifold::batch_union(&mass)
}

/// Both walls' shapes and solids, and the routes planned on them (checked
/// against the solids: nothing placed where a walker would run into rock).
fn layout(seed: i32) -> (Vec<Massif>, Vec<Massif>, Routing, [Manifold; 2]) {
    let near = shape(-1.0, seed, &plan(1.0, seed + 7919));
    let far = shape(1.0, seed + 7919, &plan(-1.0, seed));
    let solids = [wall_solid(&near), wall_solid(&far)];
    let routing = Routing::plan(seed, &near, &far, &solids);
    (near, far, routing, solids)
}

fn wall(parts: &mut Parts, side: f32, seed: i32, massifs: &[Massif], solid: &Manifold, routing: &Routing, chambers: bool) {
    parts.mark("wall");
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c1);
    let mut zones = routing.zones(side);
    zones.extend(routing.door_zones(side, massifs));

    // (Its mass is its solid, less what is cut into it: tunnels, halls.)
    let mut cuts: Vec<Manifold> = routing.tunnels.iter().filter(|t| t.side == side).map(|t| tunnel_cut(&massifs[t.m], t)).collect();
    for (m, ms) in massifs.iter().enumerate() {
        let m = m as i32;
        let (nominal, len) = (ms.wall, ms.len);
        let (u0, u1) = (0.0, len);
        // Where routes run along this face. (And what is built on it keeps
        // clear of every route's space in the world: see `Parts::blocked`.)
        let to_u = |z: f32| (z - ms.z.0) / (ms.z.1 - ms.z.0).max(1e-3) * len;
        parts.clear = zones
            .iter()
            .filter(|z| z.z.1 > ms.z.0 && z.z.0 < ms.z.1)
            .map(|z| (to_u(z.z.0.max(ms.z.0)), to_u(z.z.1.min(ms.z.1)), z.v.0, z.v.1))
            .collect();
        if ms.shaft {
            let w = nominal.moved(SHAFT_DEPTH);
            // Black at the back, a strip of light running up it.
            w.block(&mut parts.dark, (u0, u1), (0.0, HEIGHT), (0.0, 0.05));
            let c = (u0 + u1) * 0.5;
            w.block(&mut parts.glow, (c - 0.15, c + 0.15), (0.0, HEIGHT), (0.05, 0.1));
            for k in 0..3 {
                parts.lights.push((w.at(c, HEIGHT * (0.2 + 0.3 * k as f32), 3.0), 100.0, 1.0));
            }
            continue;
        }
        let (split, low, high, slope, room) = (ms.split, ms.low, ms.high, ms.slope, ms.room);
        // The sloped face: an underside leaning out over the chasm, or a
        // battered stretch standing back; a rib now and then across it.
        if slope > 1.0 {
            let (a, b) = (split - slope, split);
            parts.mark("ribs");
            parts.frame = Some(nominal.moved(low.max(high)));
            let ribs = (len / (6.0 + 10.0 * r(m, 8))).floor() as i32;
            for i in 1..ribs {
                let u = u0 + (u1 - u0) * i as f32 / ribs as f32;
                if !parts.blocked((u - 0.6, u + 0.6), (a, b)) {
                    nominal.section(&mut parts.stone, (u - 0.6, u + 0.6), &[(a, low), (a, low + 1.2), (b, high + 1.2), (b, high)]);
                }
            }
        }
        for (part, (v0, v1, n)) in [(0.0, split - slope, low), (split, HEIGHT, high)].into_iter().enumerate() {
            let w = nominal.moved(n);
            let k = seed + m * 31 + part as i32 * 7;
            let p = m * 2 + part as i32;
            // A place on a walkway passing it, if one fits.
            let reach = low + room - n;
            if let Some(t) = site(parts, routing, side, m as usize, ms, (v0, v1), n, reach, k) {
                parts.mark("hall");
                hall(parts, &nominal, &t, (u0, u1), (v0, v1), k);
                cuts.push(wbox(&nominal, t.u, (t.floor, t.ceiling), (t.back, t.face + 2.0)));
                parts.mark("wall");
                zones.push(Zone { z: (t.z.0 - 10.0, t.z.1 + 10.0), v: (t.floor - 25.0, t.ceiling + 10.0) });
                parts.terraces.push(t);
                continue;
            }
            // (`--opt chambers`: now and then a chamber cut into it.)
            let (cw, ch) = (15.0 + 45.0 * r(p, 20), 12.0 + 28.0 * r(p, 21));
            if chambers && r(p, 22) < 0.45 && u1 - u0 > cw + 12.0 && v1 - v0 > ch + 30.0 {
                let cu0 = u0 + 6.0 + (u1 - u0 - cw - 12.0) * r(p, 23);
                let cv0 = v0 + 15.0 + (v1 - v0 - ch - 30.0) * r(p, 24);
                let c = Chamber { side, wall: w, u: (cu0, cu0 + cw), floor: cv0, ceiling: cv0 + ch, depth: 20.0 + 30.0 * r(p, 25) };
                chamber(parts, &c, (u0, u1), (v0, v1), k);
                parts.chambers.push(c);
                continue;
            }
            parts.mark("detail");
            parts.frame = Some(w);
            if r(p, 5) < 0.2 {
                slab(parts, &w, (u0, u1), (v0, v1), k);
            } else {
                strata(parts, &w, (u0, u1), (v0, v1), k);
            }
        }
        // An overhang: a line of light along its edge, underneath.
        if high > low + 1.0 {
            let w = nominal.moved(high);
            w.block(&mut parts.glow, (u0, u1), (split - 0.3, split - 0.1), (-0.6, -0.4));
            parts.lights.push((w.at((u0 + u1) * 0.5, split - 3.0, 1.0), 90.0, 1.0));
        }
    }
    parts.clear.clear();
    parts.frame = None;
    parts.mark("wall mass");
    let solid = solid.difference(&Manifold::batch_union(&cuts));
    parts.stone.solid(&solid);
    parts.walls.push(solid);
    let half = LENGTH * 0.5;
    parts.mark("columns");
    // Giant half-sunk columns, spanning much of the height, standing on
    // whatever face is there.
    for k in 0..8 {
        let z = CENTRE.z - half + LENGTH * (k as f32 + 0.3 + 0.4 * r(k, 40)) / 8.0;
        let Some((m, u)) = locate(massifs, z) else { continue };
        let radius = 3.0 + 4.0 * r(k, 41);
        let (v0, v1) = (HEIGHT * 0.2 * r(k, 42), HEIGHT * (0.5 + 0.5 * r(k, 43)));
        if crosses(&zones, (z - radius, z + radius), (v0, v1)) {
            continue;
        }
        let n = m.face(v0).max(m.face(v1)) + radius * 0.4;
        let (a, b) = (m.wall.at(u, v0, n), m.wall.at(u, v1, n));
        if parts.keeps(a.min(b) - radius, a.max(b) + radius) {
            continue;
        }
        parts.stone.cylinder(a, b, radius, 14);
    }
    // Heavy cables hanging down the face in twisted pairs.
    for k in 0..24 {
        let z = CENTRE.z - half + LENGTH * r(k, 50);
        let Some((m, u)) = locate(massifs, z) else { continue };
        let v0 = HEIGHT * (0.3 + 0.7 * r(k, 51));
        let drop = 40.0 + 200.0 * r(k, 52);
        let thick = 0.25 + 0.35 * r(k, 53);
        if crosses(&zones, (z - 2.0, z + 2.0), (v0 - drop, v0)) {
            continue;
        }
        let n = m.face(v0).max(m.face(v0 - drop)) + 1.5 + thick;
        let (a, b) = (m.wall.at(u, v0, n), m.wall.at(u, v0 - drop, n));
        if parts.keeps(a.min(b) - 2.0 * thick, a.max(b) + 2.0 * thick) {
            continue;
        }
        for strand in 0..2 {
            let phase = strand as f32 * std::f32::consts::PI;
            let points: Vec<Vec3> = (0..32)
                .map(|i| {
                    let t = i as f32 / 31.0;
                    let a = phase + t * drop / 6.0;
                    m.wall.at(u + a.cos() * thick * 1.1, v0 - drop * t, n + a.sin() * thick * 1.1)
                })
                .collect();
            parts.cables.push((points, thick));
        }
    }
}

/// A part of a massif in tall strata (40-140 m), a walkable ledge under
/// each (some carrying a line of light), each stratum's face in fractal
/// relief.
fn strata(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c9);
    let mut top = v1;
    let mut stratum = 0;
    while top > v0 {
        let h = (40.0 + 100.0 * r(stratum, 0)).min(top - v0);
        let bottom = top - h;
        let (bh, bn) = (1.2, 4.0 + 2.0 * r(stratum, 2));
        if bottom > v0 {
            // Ledges stop short of the massif's ends now and then (and are
            // left out where a route runs).
            let (a, b) = if r(stratum, 5) < 0.3 { (u0 + (u1 - u0) * 0.3 * r(stratum, 6), u1 - (u1 - u0) * 0.3 * r(stratum, 7)) } else { (u0, u1) };
            if parts.blocked((a, b), (bottom - bh - 2.0, bottom + 3.0)) {
                top = bottom - bh;
                stratum += 1;
                continue;
            }
            w.block(&mut parts.stone, (a, b), (bottom - bh, bottom), (0.0, bn));
            if r(stratum, 4) < 0.5 {
                w.block(&mut parts.glow, (a, b), (bottom - bh * 0.6, bottom - bh * 0.4), (bn, bn + 0.05));
                if r(stratum, 8) < 0.5 {
                    parts.lights.push((w.at((a + b) * 0.5, bottom - bh, bn + 2.0), 80.0, 1.0));
                }
            }
        }
        // Each stratum its own boldness: bold steps, medium, or fine.
        let bold = [1.0, 0.5, 0.25][(r(stratum, 9) * 3.0) as usize % 3];
        relief(parts, w, (u0, u1), (bottom, top), 0.0, 0, bold, seed.wrapping_mul(97).wrapping_add(stratum));
        top = bottom - bh;
        stratum += 1;
    }
}

/// A massif part with a chamber cut into it: the mass built round the
/// opening (below, above and to either side, and a back wall deep in), the
/// face round it in strata; inside, a floor, rows of columns, a gallery round
/// the back and sides with a lit edge, relief on the back wall, lights; a
/// stepped portal round the mouth.
fn chamber(parts: &mut Parts, c: &Chamber, (u0, u1): (f32, f32), (v0, v1): (f32, f32), seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d1);
    let w = &c.wall;
    let back = -BACK - w.offset;
    let (cu0, cu1) = c.u;
    let (f, top) = (c.floor, c.ceiling);
    // The mass round the opening, and behind the chamber.
    w.block(&mut parts.stone, (u0, u1), (v0, f), (back, 0.0));
    w.block(&mut parts.stone, (u0, u1), (top, v1), (back, 0.0));
    w.block(&mut parts.stone, (u0, cu0), (f, top), (back, 0.0));
    w.block(&mut parts.stone, (cu1, u1), (f, top), (back, 0.0));
    w.block(&mut parts.stone, (cu0, cu1), (f, top), (back, -c.depth));
    // The face round it.
    strata(parts, w, (u0, u1), (v0, f), seed);
    strata(parts, w, (u0, u1), (top, v1), seed + 1);
    relief(parts, w, (u0, cu0), (f, top), 0.0, 1, 0.5, seed + 2);
    relief(parts, w, (cu1, u1), (f, top), 0.0, 1, 0.5, seed + 3);
    // A stepped portal: frames round the mouth, each standing further out.
    for step in 0..3 {
        let t = 1.2 + step as f32 * 1.0;
        let out = (0.6, 0.6 + 0.8 * (3 - step) as f32);
        w.block(&mut parts.stone, (cu0 - t, cu0), (f, top + t), out);
        w.block(&mut parts.stone, (cu1, cu1 + t), (f, top + t), out);
        w.block(&mut parts.stone, (cu0 - t, cu1 + t), (top, top + t), out);
    }
    // A threshold, and a line of light along the floor's edge.
    w.block(&mut parts.stone, (cu0, cu1), (f - 0.5, f + 0.3), (-0.5, 1.5));
    w.block(&mut parts.glow, (cu0, cu1), (f + 0.25, f + 0.35), (1.45, 1.5));
    // Inside: the back wall in relief.
    let inner = w.moved(w.offset - c.depth);
    relief(parts, &inner, (cu0, cu1), (f, top), 0.0, 1, 0.4, seed + 4);
    // Rows of columns, floor to ceiling.
    let rows = 1 + (r(0, 0) * 2.0) as i32;
    let cols = ((cu1 - cu0) / (6.0 + 6.0 * r(0, 1))).floor().max(2.0) as i32;
    let radius = 0.5 + 0.8 * r(0, 2);
    for j in 0..rows {
        let n = -c.depth * (j as f32 + 1.0) / (rows as f32 + 1.0);
        for i in 0..cols {
            let u = cu0 + (cu1 - cu0) * (i as f32 + 0.5) / cols as f32;
            parts.stone.cylinder(w.at(u, f, n), w.at(u, top, n), radius, 10);
        }
    }
    // A gallery round the back and sides at mid-height, a lit edge on it.
    let g = f + (top - f) * (0.45 + 0.15 * r(0, 3));
    let deep = 3.0 + 2.0 * r(0, 4);
    if top - f > 14.0 {
        w.block(&mut parts.stone, (cu0, cu1), (g - 0.6, g), (-c.depth, -c.depth + deep));
        w.block(&mut parts.stone, (cu0, cu0 + deep), (g - 0.6, g), (-c.depth, -2.0));
        w.block(&mut parts.stone, (cu1 - deep, cu1), (g - 0.6, g), (-c.depth, -2.0));
        w.block(&mut parts.glow, (cu0 + deep, cu1 - deep), (g - 0.35, g - 0.25), (-c.depth + deep, -c.depth + deep + 0.05));
    }
    // Ribs up the side walls and beams across the ceiling, repeating along
    // the depth and the width.
    let pitch = 2.5 + 2.5 * r(0, 5);
    let mut n = -c.depth + 1.0;
    while n < -1.0 {
        w.block(&mut parts.stone, (cu0, cu0 + 0.7), (f, top), (n, n + 0.6));
        w.block(&mut parts.stone, (cu1 - 0.7, cu1), (f, top), (n, n + 0.6));
        n += pitch;
    }
    let pitch = 3.0 + 3.0 * r(0, 6);
    let mut u = cu0 + pitch * 0.5;
    while u < cu1 {
        w.block(&mut parts.stone, (u - 0.4, u + 0.4), (top - 1.4, top), (-c.depth, 0.0));
        u += pitch;
    }
    // Lit from within: a light or two deep inside.
    let lights = if cu1 - cu0 > 35.0 { 2 } else { 1 };
    for i in 0..lights {
        let u = cu0 + (cu1 - cu0) * (i as f32 + 0.5) / lights as f32;
        parts.lights.push((w.at(u, f + (top - f) * 0.7, -c.depth * 0.5), 70.0, 0.25));
    }
}

/// Where a place fits on a massif part (from `v.0` to `v.1`, its face at
/// `face`, free to reach `reach` beyond it): on a walkway passing it (now
/// and then; one a walkway), long enough there, clear of where flights and
/// bridges join it, with the part's mass above and below it.
#[allow(clippy::too_many_arguments)]
fn site(parts: &Parts, routing: &Routing, side: f32, massif: usize, ms: &Massif, (v0, v1): (f32, f32), face: f32, reach: f32, seed: i32) -> Option<Terrace> {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d5);
    let (z, len) = (ms.z, ms.len);
    let span = (z.1 - z.0).max(1e-3);
    let (to_u, to_z) = (|q: f32| (q - z.0) / span * len, |u: f32| z.0 + u / len * span);
    for (k, w) in routing.ways.iter().enumerate().filter(|(_, w)| w.side == side) {
        let i = k as i32;
        if r(i, 0) > 0.35 || parts.terraces.iter().any(|t| t.way == k) {
            continue;
        }
        let (us, ue) = (to_u(z.0.max(w.z.0)), to_u(z.1.min(w.z.1)));
        let height = 10.0 + 5.0 * r(i, 1);
        let width = (44.0 + 20.0 * r(i, 2)).max(height * 3.0);
        if ue - us < width + 30.0 {
            continue;
        }
        let a = us + 15.0 + (ue - us - 30.0 - width) * r(i, 3);
        let (z0, z1) = (to_z(a), to_z(a + width));
        let near = |z: f32| z > z0 - 20.0 && z < z1 + 20.0;
        if routing.flights.iter().any(|f| f.side == side && (near(f.z.0) || near(f.z.1)) && (f.v.0 - w.v0).abs().min((f.v.1 - w.v0).abs()) < 15.0) || routing.bridges.iter().any(|b| (b.1 == k || b.2 == k) && near(b.0)) {
            continue;
        }
        // A step or few down into it from the walkway at either end.
        let (ga, gb) = (w.v(z0), w.v(z1));
        let floor = ga.min(gb) - 0.6;
        let ceiling = floor + height.max(w.width * 0.5 + 8.0 + (ga - gb).abs());
        if floor - 15.0 < v0 || ceiling + 15.0 > v1 || crosses(&routing.flight_zones(side), (z0 - 10.0, z1 + 10.0), (floor - 20.0, ceiling + 10.0)) {
            continue;
        }
        let edge = face - 0.5 + w.width;
        let extra = (8.0 + 12.0 * r(i, 4)).min(face + reach - edge);
        if extra < 6.0 || width < 2.0 * extra + 10.0 {
            continue;
        }
        return Some(Terrace { way: k, massif, u: (a, a + width), z: (z0, z1), floor, ceiling, face, back: face - 14.0 - 10.0 * r(i, 5), front: edge + extra });
    }
    None
}

/// A massif part with a hall cut into it (its place; the wall's solid has
/// the hall taken out): the face round it in strata; inside, an arcade along its open front, the back wall in relief either side of a dark
/// doorway in a stepped frame, a lit line along the front of the ceiling,
/// a light within. (The walkway's part of it, gateways, steps and terrace,
/// comes with the walkway.)
fn hall(parts: &mut Parts, wall: &Wall, t: &Terrace, (u0, u1): (f32, f32), (v0, v1): (f32, f32), seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d6);
    let (a, b) = t.u;
    let (f, top) = (t.floor, t.ceiling);
    let w = wall.moved(t.face);
    strata(parts, &w, (u0, u1), (v0, f), seed);
    strata(parts, &w, (u0, u1), (top, v1), seed + 1);
    relief(parts, &w, (u0, a), (f, top), 0.0, 1, 0.5, seed + 2);
    relief(parts, &w, (b, u1), (f, top), 0.0, 1, 0.5, seed + 3);
    // Along the open front: piers, or an arcade.
    let bays = ((b - a) / (10.0 + 4.0 * r(0, 0))).round().max(2.0) as i32;
    let bay = (b - a) / bays as f32;
    for i in 0..bays {
        let (b0, b1) = (a + bay * i as f32, a + bay * (i + 1) as f32);
        let rad = (bay - 3.2) * 0.5;
        arch(&mut parts.stone, wall, (b0, b1), f, top - rad - 1.0, top, (t.face - 3.0, t.face), 1.6);
    }
    wall.block(&mut parts.glow, (a, b), (top - 0.15, top), (t.face - 3.6, t.face - 3.4));
    // The back wall: a dark doorway in a stepped frame, relief either side.
    let inner = wall.moved(t.back);
    let c = (a + b) * 0.5;
    let (dw, dh) = (2.0 + r(0, 1), 6.0 + 3.0 * r(0, 2));
    inner.block(&mut parts.dark, (c - dw, c + dw), (f, f + dh), (0.0, 0.05));
    for s in 0..3 {
        let (o, d) = (0.7 * s as f32, 0.3 + 0.35 * (3 - s) as f32);
        inner.block(&mut parts.stone, (c - dw - o - 0.7, c - dw - o), (f, f + dh + o), (0.0, d));
        inner.block(&mut parts.stone, (c + dw + o, c + dw + o + 0.7), (f, f + dh + o), (0.0, d));
        inner.block(&mut parts.stone, (c - dw - o - 0.7, c + dw + o + 0.7), (f + dh + o, f + dh + o + 0.7), (0.0, d));
    }
    relief(parts, &inner, (a, c - dw - 3.0), (f, top), 0.0, 1, 0.4, seed + 4);
    relief(parts, &inner, (c + dw + 3.0, b), (f, top), 0.0, 1, 0.4, seed + 5);
    parts.lights.push((wall.at(c, top - 2.0, (t.back + t.face) * 0.5), 45.0, 0.15));
}

/// Bridges between chambers facing each other across the gap at similar
/// heights: from floor to floor, sloping if they differ.
fn crossings(parts: &mut Parts) {
    let chambers = parts.chambers.clone();
    let mouth = |c: &Chamber| c.wall.at((c.u.0 + c.u.1) * 0.5, c.floor - 0.4, 0.0);
    for a in chambers.iter().filter(|c| c.side < 0.0) {
        for b in chambers.iter().filter(|c| c.side > 0.0) {
            let (from, to) = (mouth(a), mouth(b));
            if (from.z - to.z).abs() > 10.0 || (a.floor - b.floor).abs() > 25.0 {
                continue;
            }
            parts.stone.beam(from, to, 4.0, 0.8, Vec3::Y);
            // A lit line along each edge.
            let across = (to - from).cross(Vec3::Y).normalize_or(Vec3::Z);
            for s in [-1.0, 1.0] {
                let off = across * s * 1.9 + Vec3::Y * 0.42;
                parts.glow.beam(from + off, to + off, 0.08, 0.04, Vec3::Y);
            }
        }
    }
}

/// A colossal slab: a vast bare face, a few broad shallow panels standing
/// out from it carrying a little coarse relief; a deep groove or two running
/// down it.
fn slab(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7ca);
    // Broad panels, a little proud.
    let panels = 1 + (r(0, 0) * 3.0) as i32;
    for p in 0..panels {
        let (a, b) = (u0 + (u1 - u0) * (0.05 + 0.4 * r(p, 1)), u1 - (u1 - u0) * (0.05 + 0.4 * r(p, 2)));
        let (c, d) = (v0 + (v1 - v0) * (0.05 + 0.4 * r(p, 3)), v1 - (v1 - v0) * (0.05 + 0.4 * r(p, 4)));
        if b - a > 4.0 && d - c > 4.0 && !parts.blocked((a, b), (c, d)) {
            let proud = 0.6 + 1.5 * r(p, 5);
            w.block(&mut parts.stone, (a, b), (c, d), (0.0, proud));
            // Coarse relief only (three levels): the slab stays vast.
            relief(parts, w, (a, b), (c, d), proud, 3, 0.5, seed.wrapping_add(p * 13));
        }
    }
    // A deep groove or two, top to bottom.
    for g in 0..(r(0, 7) * 2.5) as i32 {
        let u = u0 + (u1 - u0) * (0.15 + 0.7 * r(g, 8));
        w.block(&mut parts.dark, (u - 0.6, u + 0.6), (v0, v1), (0.0, 0.08));
        // Lit down its middle, mostly.
        if r(g, 9) < 0.7 {
            w.block(&mut parts.glow, (u - 0.12, u + 0.12), (v0, v1), (0.08, 0.12));
        }
    }
}

/// Fractal relief: the face split again and again along its longer side,
/// at proportions (a half, a third, the golden section) so it has rhythm,
/// each piece standing out from its parent by an amount in proportion to its
/// size, the same grammar from massifs down to a few metres. Now and then a
/// piece becomes a run of fins (rhythm along an axis), a seam carries a line
/// of light, or the splitting stops early (bare at every scale); the
/// smallest end in frames within frames, packed conduits, louvres or slits.
/// `bold` scales how far pieces stand out (1 bold, 0.25 fine). Now and then
/// a piece near the top becomes a run of bays instead, breaking the pattern.
#[allow(clippy::too_many_arguments)]
fn relief(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), n: f32, depth: i32, bold: f32, seed: i32) {
    let (du, dv) = (u1 - u0, v1 - v0);
    let size = du.min(dv);
    let h = |a: i32, b: i32| hash01(seed, depth, a * 31 + b, 0x7cb);
    // (Flush where a route runs: no detail standing out there.)
    let blocked = parts.blocked((u0, u1), (v0, v1));
    if size < 1.5 || depth >= 8 || (depth >= 3 && h(0, 0) < 0.1) {
        if !blocked {
            leaf(parts, w, (u0, u1), (v0, v1), n, seed);
        }
        return;
    }
    // A run of bays between pilasters, now and then.
    if (1..=2).contains(&depth) && size > 10.0 && h(8, 0) < 0.15 && !blocked {
        bays(parts, &w.moved(w.offset + n), (u0, u1), (v0, v1), seed);
        return;
    }
    // A run of fins along the longer side, alternate ones standing out.
    if depth >= 2 && size > 4.0 && h(1, 0) < 0.15 && !blocked {
        let along_u = du > dv;
        let len = if along_u { du } else { dv };
        let count = (len / (1.2 + 3.0 * h(2, 0))).round().clamp(3.0, 24.0) as i32;
        let step = len / count as f32;
        let fin = (0.04 * size + 0.3 * h(3, 0)).min(3.0);
        for i in (0..count).step_by(2) {
            let (a, b) = (i as f32 * step, (i as f32 + 1.0) * step);
            let (cu, cv) = if along_u { ((u0 + a, u0 + b), (v0, v1)) } else { ((u0, u1), (v0 + a, v0 + b)) };
            w.block(&mut parts.stone, cu, cv, (n, n + fin));
        }
        return;
    }
    const RATIOS: [f32; 7] = [0.5, 0.3333, 0.6667, 0.25, 0.75, 0.382, 0.618];
    let t = RATIOS[(h(4, 0) * RATIOS.len() as f32) as usize % RATIOS.len()];
    let split_u = if du > dv * 1.3 { true } else if dv > du * 1.3 { false } else { h(5, 0) < 0.5 };
    let children = if split_u {
        let m = u0 + du * t;
        [((u0, m), (v0, v1)), ((m, u1), (v0, v1))]
    } else {
        let m = v0 + dv * t;
        [((u0, u1), (v0, m)), ((u0, u1), (m, v1))]
    };
    // A seam of light along the split, near the top of the hierarchy.
    if depth <= 2 && h(6, 0) < 0.2 {
        let (cu, cv) = children[0];
        let line = if split_u { ((cu.1 - 0.12, cu.1 + 0.12), (v0, v1)) } else { ((u0, u1), (cv.1 - 0.12, cv.1 + 0.12)) };
        w.block(&mut parts.glow, line.0, line.1, (n, n + 0.06));
        // The big seams light the wall about them.
        if depth == 0 {
            let mid = w.at((line.0 .0 + line.0 .1) * 0.5, (line.1 .0 + line.1 .1) * 0.5, n + 3.0);
            parts.lights.push((mid, 90.0, 1.0));
        }
    }
    const STANDS: [f32; 4] = [0.0, 0.03, 0.06, 0.12];
    for (k, (cu, cv)) in children.into_iter().enumerate() {
        let csize = (cu.1 - cu.0).min(cv.1 - cv.0);
        let dn = if parts.blocked(cu, cv) { 0.0 } else { (csize * STANDS[(h(7, k as i32) * 4.0) as usize % 4] * bold).min(6.0 * bold) };
        if dn > 0.05 {
            w.block(&mut parts.stone, cu, cv, (n, n + dn));
        }
        relief(parts, w, cu, cv, n + dn, depth + 1, bold, seed.wrapping_mul(31).wrapping_add(k as i32 + 1));
    }
}

/// A run of bays: pilasters standing out, between them frames within frames,
/// packed conduits, louvres or slits, or bare.
fn bays(parts: &mut Parts, w: &Wall, (start, end): (f32, f32), (v0, v1): (f32, f32), seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c2);
    let mut u = start;
    let mut k = 0;
    while u < end {
        let width = (4.0 + 20.0 * r(k, 0)).min(end - u);
        let pw = 1.0 + 2.0 * r(k, 1);
        w.block(&mut parts.stone, (u, u + pw), (v0, v1), (0.0, 1.0 + 1.5 * r(k, 2)));
        let (b0, b1) = (u + pw, u + width);
        if b1 - b0 > 1.5 {
            match (r(k, 3) * 5.0) as u32 {
                0 => frames(parts, w, (b0, b1), (v0, v1), 0.0, 4, r(k, 5)),
                1 => conduits(parts, w, (b0, b1), (v0, v1), seed, k),
                2 => louvres(parts, w, (b0, b1), (v0, v1), r(k, 6)),
                3 => openings(parts, w, (b0, b1), (v0, v1), r(k, 4)),
                _ => {}
            }
        }
        u += width;
        k += 1;
    }
}

/// The end of the relief's splitting: bare mostly, or frames within frames,
/// packed conduits, louvres or slits.
fn leaf(parts: &mut Parts, w: &Wall, u: (f32, f32), v: (f32, f32), n: f32, seed: i32) {
    let w = w.moved(w.offset + n);
    let r = hash01(seed, 99, 0, 0x7cc);
    match (r * 12.0) as u32 {
        0 | 1 => frames(parts, &w, u, v, 0.0, 3, hash01(seed, 99, 1, 0x7cc)),
        2 => conduits(parts, &w, u, v, seed, 7),
        3 => louvres(parts, &w, u, v, hash01(seed, 99, 2, 0x7cc)),
        4 => openings(parts, &w, u, v, 0.1),
        _ => {}
    }
}

/// Slits: long dark horizontal slots across a piece, a few lit faintly
/// from within.
fn openings(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), r: f32) {
    let seed = (r * 1.0e6) as i32;
    let h = |a: i32, b: i32| hash01(seed, a, b, 0x7c8);
    let pitch = 1.5 + 3.0 * h(0, 0);
    let mut v = v0 + pitch * 0.5;
    let mut j = 0;
    while v < v1 - 0.3 {
        let (a, b) = (u0 + (u1 - u0) * 0.15 * h(j, 1), u1 - (u1 - u0) * 0.15 * h(j, 2));
        let lit = h(j, 4) < 0.25;
        w.block(if lit { &mut parts.dim } else { &mut parts.dark }, (a, b), (v, v + 0.12 + 0.2 * h(j, 3)), (0.0, 0.06));
        v += pitch;
        j += 1;
    }
}

/// Frames within frames: each frame a border standing out, the next inside
/// it standing out further (or less), `levels` deep.
fn frames(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), n: f32, levels: u32, r: f32) {
    if levels == 0 || u1 - u0 < 1.0 || v1 - v0 < 1.0 {
        return;
    }
    let t = (0.08 + 0.06 * r) * (u1 - u0).min(v1 - v0);
    let depth = 0.3 + 0.5 * r;
    let n1 = n + depth;
    w.block(&mut parts.stone, (u0, u1), (v1 - t, v1), (n, n1));
    w.block(&mut parts.stone, (u0, u1), (v0, v0 + t), (n, n1));
    w.block(&mut parts.stone, (u0, u0 + t), (v0 + t, v1 - t), (n, n1));
    w.block(&mut parts.stone, (u1 - t, u1), (v0 + t, v1 - t), (n, n1));
    let next = if levels % 2 == 0 { n } else { n + depth * 0.5 };
    frames(parts, w, (u0 + t * 1.6, u1 - t * 1.6), (v0 + t * 1.6, v1 - t * 1.6), next, levels - 1, (r * 7.3).fract());
}

/// Conduits packed into a bay: vertical tubes of mixed thickness, half sunk,
/// side by side, some running the whole height and some stopping short.
fn conduits(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), seed: i32, k: i32) {
    let mut u = u0;
    let mut i = 0;
    while u < u1 {
        let radius = 0.2 + 0.9 * hash01(seed, k, i, 0x7c3).powi(2);
        if u + radius * 2.0 > u1 {
            break;
        }
        let c = u + radius;
        let top = if hash01(seed, k, i, 0x7c4) < 0.75 { v1 } else { v0 + (v1 - v0) * (0.3 + 0.5 * hash01(seed, k, i, 0x7c5)) };
        parts.stone.cylinder(w.at(c, v0, radius * 0.5), w.at(c, top, radius * 0.5), radius, 8);
        u += radius * 2.0 + 0.05;
        i += 1;
    }
}

/// Horizontal slabs stacked up the bay, each standing out a different
/// amount: a rhythm of ledges and shadows.
fn louvres(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), r: f32) {
    let pitch = 0.8 + 1.4 * r;
    let mut v = v0 + pitch * 0.5;
    let mut i = 0;
    while v < v1 - 0.2 {
        let n = 0.3 + 0.9 * (((i as f32) * 0.37 + r).fract());
        w.block(&mut parts.stone, (u0, u1), (v, v + 0.25), (0.0, n));
        v += pitch;
        i += 1;
    }
}

/// A walkway along a wall: which wall, from `z.0` to `z.1` along the
/// chasm, its height at `z.0` and how it rises (grade, metres a metre),
/// how wide.
#[derive(Clone, Copy)]
struct Walkway {
    side: f32,
    z: (f32, f32),
    v0: f32,
    grade: f32,
    width: f32,
}

impl Walkway {
    fn v(&self, z: f32) -> f32 {
        self.v0 + self.grade * (z - self.z.0)
    }
}

/// How thick a walkway's shelf is at its outer edge, and how high a parapet
/// stands.
const EDGE: f32 = 1.8;
const PARAPET: f32 = 1.0;

/// Where a walkway's deck starts out from a massif's face at height `v`
/// (across a shaft, from the wall's line).
fn inner(m: &Massif, v: f32) -> f32 {
    if m.shaft { 0.0 } else { m.face(v) - 0.5 }
}

/// Where a route runs along a wall: along the chasm and in height.
struct Zone {
    z: (f32, f32),
    v: (f32, f32),
}

/// Whether anything spanning `z` and `v` would cross a route.
fn crosses(zones: &[Zone], z: (f32, f32), v: (f32, f32)) -> bool {
    zones.iter().any(|q| z.0 < q.z.1 && z.1 > q.z.0 && v.0 < q.v.1 && v.1 > q.v.0)
}

/// The routes' lattice: every walking surface on a level a whole number of
/// risers up; every step a riser up and a tread along; along a face, every
/// end on a whole metre; widths whole metres. (So whatever meets meets on
/// the same numbers.)
const RISER: f32 = 0.2;
const TREAD: f32 = 0.25;

/// A height put on the lattice's levels.
fn level(v: f32) -> f32 {
    (v / RISER).round() * RISER
}

/// A bare flight of stairs down along a wall's face (massif `m` on wall
/// `side`): from `u.0` along it at height `v.0` to `u.1` at `v.1`, `width`
/// wide; `z`: where it runs along the chasm.
#[derive(Clone, Copy)]
struct Flight {
    side: f32,
    m: usize,
    u: (f32, f32),
    v: (f32, f32),
    z: (f32, f32),
    width: f32,
}

/// A way through a wall (massif `m` on wall `side`), all on the lattice: a
/// door (`doors.0`: where along the face, how high) and a corridor straight
/// in; a stair down inside the rock in runs, parallel to the face, each
/// along a lane `n` from the wall's line (one run; or, back and forth,
/// alternate runs in two lanes, one deeper, with a landing across both at
/// every turn); a corridor back out to a door (`doors.1`), onto a walkway
/// going on from there. `width` wide throughout.
#[derive(Clone)]
struct Tunnel {
    side: f32,
    m: usize,
    doors: [(f32, f32); 2],
    runs: Vec<Run>,
    width: f32,
}

/// A run of a tunnel's stair: from `u.0` at height `v.0` to `u.1` at `v.1`,
/// along the lane `n`.
#[derive(Clone, Copy)]
struct Run {
    u: (f32, f32),
    v: (f32, f32),
    n: f32,
}

impl Tunnel {
    /// The two corridors: where along the face, how high, the lane they
    /// reach.
    fn corridors(&self) -> [(f32, f32, f32); 2] {
        let (a, b) = (self.runs[0], self.runs[self.runs.len() - 1]);
        [(self.doors[0].0, self.doors[0].1, a.n), (self.doors[1].0, self.doors[1].1, b.n)]
    }

    /// The landings at the turns: centred where along the face, how high,
    /// across the lanes from `n.0` to `n.1`.
    fn landings(&self) -> Vec<(f32, f32, (f32, f32))> {
        let w = self.width * 0.5;
        self.runs
            .windows(2)
            .map(|p| {
                let d = (p[0].u.1 - p[0].u.0).signum();
                (p[0].u.1 + d * w, p[0].v.1, (p[0].n.min(p[1].n) - w, p[0].n.max(p[1].n) + w))
            })
            .collect()
    }

    /// The line a walker follows through it, 1.2 m up.
    fn path(&self, m: &Massif) -> Vec<Vec3> {
        let p = |u: f32, v: f32, n: f32| m.wall.at(u, v + 1.2, n);
        let [(ua, va, na), (ux, vx, nx)] = self.corridors();
        let mut out = vec![p(ua, va, inner(m, va) + 0.3), p(ua, va, na)];
        let landings = self.landings();
        for (k, r) in self.runs.iter().enumerate() {
            out.extend([p(r.u.0, r.v.0, r.n), p(r.u.1, r.v.1, r.n)]);
            if let (Some(&(lu, lv, _)), Some(next)) = (landings.get(k), self.runs.get(k + 1)) {
                out.extend([p(lu, lv, r.n), p(lu, lv, next.n)]);
            }
        }
        out.extend([p(ux, vx, nx), p(ux, vx, inner(m, vx) + 0.3)]);
        out
    }

    /// From where to where along the face it reaches, and from what height
    /// to what (for what else may go there).
    fn extent(&self) -> ((f32, f32), (f32, f32)) {
        let w = self.width;
        let us = self.runs.iter().flat_map(|r| [r.u.0, r.u.1]).chain([self.doors[0].0, self.doors[1].0]);
        let (lo, hi) = us.fold((f32::MAX, f32::MIN), |(a, b), u| (a.min(u), b.max(u)));
        ((lo - w, hi + w), (self.doors[1].1, self.doors[0].1))
    }
}

/// A box in a face's frame, as a solid.
fn wbox(w: &Wall, u: (f32, f32), v: (f32, f32), n: (f32, f32)) -> Manifold {
    let mut pts = Vec::new();
    for a in [u.0, u.1] {
        for b in [v.0, v.1] {
            for c in [n.0, n.1] {
                let p = w.at(a, b, c);
                pts.push([p.x as f64, p.y as f64, p.z as f64]);
            }
        }
    }
    Manifold::hull_pts(&pts)
}

/// What a tunnel takes out of the rock: the two corridors (from just out
/// beyond the face, so their doors open in it), the sloping runs, the
/// landings, each 3 m high above its floor (whose top the routes lay, 0.3 m
/// above the cut). (Each run reaches on into what is at both its ends:
/// pieces cut out together must overlap, not just touch, or a skin of rock
/// is left between them.)
fn tunnel_cut(m: &Massif, t: &Tunnel) -> Manifold {
    let (w, h) = (t.width * 0.5, 3.3);
    let mut cut = Vec::new();
    for (u, v, n) in t.corridors() {
        cut.push(wbox(&m.wall, (u - w, u + w), (v - 0.3, v + h - 0.3), (n - w, m.face(v) + 2.0)));
    }
    for r in &t.runs {
        let d = (r.u.1 - r.u.0).signum();
        let mut pts = Vec::new();
        for (u, v) in [(r.u.0 - d * w, r.v.0), (r.u.0, r.v.0), (r.u.1, r.v.1), (r.u.1 + d * w, r.v.1)] {
            for y in [v - 0.3, v + h - 0.3] {
                for n in [r.n - w, r.n + w] {
                    let p = m.wall.at(u, y, n);
                    pts.push([p.x as f64, p.y as f64, p.z as f64]);
                }
            }
        }
        cut.push(Manifold::hull_pts(&pts));
    }
    for (u, v, n) in t.landings() {
        cut.push(wbox(&m.wall, (u - w - 0.3, u + w + 0.3), (v - 0.3, v + h - 0.3), n));
    }
    Manifold::batch_union(&cut)
}

/// The routes, planned before anything is built (on the walls' shapes), on
/// the lattice: a way down from the rim where you start, as a chain of
/// human-scale steps. A short flight (12-24 steps) on down along the wall
/// from the end of the walkway it is on, onto a walkway going on from its
/// foot; now and then a door into the wall, a stair down inside it and a
/// door out lower down (the way past the walls' overhangs); now and then a
/// bridge across to the other wall, onto a walkway there; and so on down, each step clear of the walls (their overhangs,
/// headroom) and of every other; taking steps back when nothing fits. And
/// where you start.
struct Routing {
    ways: Vec<Walkway>,
    flights: Vec<Flight>,
    tunnels: Vec<Tunnel>,
    /// Bridges: where along the chasm, and the walkways they join (near,
    /// far).
    bridges: Vec<(f32, usize, usize)>,
    /// What crosses the void: from, to (on the walkways' edges, at deck
    /// height), and how deep beneath.
    spans: Vec<(Vec3, Vec3, f32)>,
    start: [f32; 5],
    /// How long the way down is (m), walking it.
    length: f32,
}

/// The nearest distance between two segments.
fn segment_gap(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> f32 {
    let mut best = f32::MAX;
    for i in 0..=24 {
        let p = a0.lerp(a1, i as f32 / 24.0);
        let d = b1 - b0;
        let t = ((p - b0).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
        best = best.min(p.distance(b0 + d * t));
    }
    best
}

/// Whether a massif's face is the same all the way from `v.0` to `v.1`
/// (clear of its sloped stretch, with room), so what runs along it there
/// can keep to one line.
fn plain(m: &Massif, v: (f32, f32)) -> bool {
    m.shaft || v.1 < m.split - m.slope - 1.0 || v.0 > m.split + 0.5
}

impl Routing {
    fn plan(seed: i32, near: &[Massif], far: &[Massif], solids: &[Manifold; 2]) -> Routing {
        let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d3);
        let walls = |side: f32| if side < 0.0 { near } else { far };
        // Whether a walker's space is clear of rock (either wall's: where the
        // gap narrows, one wall's overhang can reach the other's routes).
        let open = |_side: f32, space: &Manifold| solids.iter().all(|s| space.intersection(s).volume() <= 0.002);
        let (lo, hi) = (CENTRE.z - LENGTH * 0.5 + 10.0, CENTRE.z + LENGTH * 0.5 - 10.0);
        let to_z = |m: &Massif, u: f32| m.z.0 + u / m.len * (m.z.1 - m.z.0);
        // A point along the chasm put on its face's lattice (a whole metre
        // along it).
        let snap = |side: f32, z: f32| locate(walls(side), z).map(|(m, u)| to_z(m, u.round()));
        // A walkway's outer edge, at deck height.
        let edge = |side: f32, z: f32, v: f32, width: f32| {
            let (m, u) = locate(walls(side), z)?;
            Some(m.wall.at(u, v, inner(m, v) + width - 0.5))
        };
        // What is taken along the walls: (side, z, v) boxes, with headroom,
        // each with the walkway it is (if one).
        type Taken = Vec<(Option<usize>, (f32, (f32, f32), (f32, f32)))>;
        let mut taken: Taken = Vec::new();
        let free = |taken: &Taken, except: Option<usize>, b: (f32, (f32, f32), (f32, f32))| {
            !taken.iter().any(|(o, t)| (except.is_none() || *o != except) && t.0 == b.0 && b.1.0 < t.1.1 && b.1.1 > t.1.0 && b.2.0 < t.2.1 && b.2.1 > t.2.0)
        };
        // (Exactly as long as they are: where a flight meets a walkway end
        // to end they touch, not overlap.)
        let way_box = |w: &Walkway| (w.side, w.z, (w.v0 - shelf_depth(w.width) - 1.0, w.v0 + 3.5));
        let flight_box = |f: &Flight| (f.side, (f.z.0.min(f.z.1) + 0.2, f.z.0.max(f.z.1) - 0.2), (f.v.1 - 4.5, f.v.0 + 3.5));
        // Whether a line through the void keeps clear of both walls (with
        // headroom above, `depth` beneath) and of what else crosses.
        let clear = |spans: &[(Vec3, Vec3, f32)], a: Vec3, b: Vec3, depth: f32| {
            for i in 1..40 {
                let p = a.lerp(b, i as f32 / 40.0);
                for side in [-1.0, 1.0] {
                    let Some((m, _)) = locate(walls(side), p.z) else { return false };
                    let n = (p - m.wall.origin).dot(m.wall.out);
                    if n < m.face(p.y + 3.0).max(m.face(p.y - depth)) + 1.0 {
                        return false;
                    }
                }
            }
            spans.iter().all(|s| segment_gap(a, b, s.0, s.1) > 8.0 + depth.max(s.2))
        };
        // A walkway on `side` at height `v` from `z0` on along the chasm
        // `dir`-wards about `length`, its far end on the lattice: if it fits
        // (within the chasm, on faces plain at its height, clear).
        let walkway_at = |taken: &Taken, side: f32, z0: f32, v: f32, dir: f32, length: f32, width: f32| {
            let far = snap(side, (z0 + dir * length).clamp(lo, hi))?;
            let (a, b) = (z0.min(far), z0.max(far));
            if b - a < 4.0 {
                return None;
            }
            let w = Walkway { side, z: (a, b), v0: v, grade: 0.0, width };
            let depth = shelf_depth(width);
            let plain_all = (a..=b).step_by_f32(2.0).all(|z| locate(walls(side), z).is_some_and(|(m, _)| plain(m, (v - depth - 1.0, v + 3.5))));
            // (Where it passes from one face to the next, they stand out
            // within its width of each other, so there is a way on round the
            // corner, not into the next face.)
            let faces: Vec<f32> = (a..=b).step_by_f32(1.0).filter_map(|z| locate(walls(side), z).map(|(m, _)| inner(m, v))).collect();
            let passable = faces.windows(2).all(|p| (p[1] - p[0]).abs() <= width - 1.5);
            (plain_all && passable && v > 30.0 && free(taken, None, way_box(&w)) && open(side, &walk_space(&w, walls(side)))).then_some(w)
        };
        let mut ways: Vec<Walkway> = Vec::new();
        let mut flights: Vec<Flight> = Vec::new();
        let mut tunnels: Vec<Tunnel> = Vec::new();
        // What tunnels take inside the walls: (side, z, v).
        let mut inside: Vec<(f32, (f32, f32), (f32, f32))> = Vec::new();
        let mut bridges: Vec<(f32, usize, usize)> = Vec::new();
        let mut spans: Vec<(Vec3, Vec3, f32)> = Vec::new();
        // Where the way down has got to: which wall, where along the chasm
        // (on the lattice), how high, which way it goes on, how wide its
        // walkways are, the walkway it is on, and how many steps since it
        // last crossed.
        let first = snap(-1.0, CENTRE.z).unwrap_or(CENTRE.z);
        let (mut side, mut z, mut v) = (-1.0f32, first, level(HEIGHT));
        let mut dir = if r(0, 0) < 0.5 { 1.0f32 } else { -1.0 };
        let mut width = 3.0f32;
        let mut on: Option<usize> = None;
        let mut since = 0;
        let mut history = Vec::new();
        let mut salt = 0;
        let mut stuck = 0;
        while v > 40.0 && flights.len() < 400 {
            let i = flights.len() as i32 + bridges.len() as i32 * 1000;
            enum Step {
                Down(Flight, Walkway),
                Through(Tunnel, Walkway),
                Across(f32, Walkway, (Vec3, Vec3, f32)),
            }
            let mut found: Option<Step> = None;
            for t in 0..24 {
                let k = i * 50 + t + salt * 7919;
                let length = 8.0 + 40.0 * r(k, 3) * r(k, 4);
                // Across now and then (more likely the longer since the
                // last time, or where the way on is blocked); the only way
                // to turn back.
                let cross = on.is_some() && since >= 6 && r(k, 1) < 0.08 + 0.02 * (since as f32 - 6.0) + if t >= 12 { 0.5 } else { 0.0 };
                // Where nothing else will do: through the wall, a stair back
                // and forth inside the rock, down as far as it takes to a
                // level where a walkway outside fits. (It can always go on
                // down: the way down never ends against the wall.)
                if on.is_some() && t >= 18 {
                    let here = ways[on.unwrap()];
                    let w = here.width.min(3.0);
                    let zd = here.z.0 + w + (here.z.1 - here.z.0 - 2.0 * w) * r(k, 14);
                    let Some((m, ud)) = locate(walls(side), zd) else { continue };
                    let mi = walls(side).iter().position(|x| std::ptr::eq(x, m)).unwrap();
                    if m.shaft || !plain(m, (v - 0.5, v + 3.5)) {
                        continue;
                    }
                    let ua = ud.round();
                    let d0 = if r(k, 13) < 0.5 { 1.0f32 } else { -1.0 };
                    let n = 4 * (12 + (r(k, 15) * 10.0) as i32);
                    let (run, drop) = (n as f32 * TREAD, n as f32 * RISER);
                    // (The lanes deep enough behind the face anywhere down to
                    // the bottom; the one deeper than the other.)
                    let face = (35.0..=v + 4.0).step_by_f32(2.0).map(|x| m.face(x)).fold(f32::MAX, f32::min);
                    let lanes = [face - 5.0 - w * 0.5, face - 5.0 - w * 1.5 - 1.5];
                    let (lo, hi) = (w + 2.0, m.len - w - 2.0);
                    let us = ua + d0 * w * 0.5;
                    if [ua, us + d0 * (run + w)].iter().any(|u| *u < lo || *u > hi) || lanes[1] - w < -BACK + 6.0 {
                        continue;
                    }
                    let mut runs = Vec::new();
                    let (mut u, mut vv, mut d) = (us, v, d0);
                    let mut exit = None;
                    for i in 0..40 {
                        let lane = lanes[i % 2];
                        runs.push(Run { u: (u, u + d * run), v: (vv, vv - drop), n: lane });
                        u += d * run;
                        vv -= drop;
                        if vv < 35.0 {
                            break;
                        }
                        // Out here, if a walkway fits below the door.
                        let ux = u + d * w * 0.5;
                        if plain(m, (vv - 0.5, vv + 3.5))
                            && let Some(start) = snap(side, to_z(m, ux - d * (w * 0.5 + 0.5)))
                            && let Some(next) = walkway_at(&taken, side, start, vv, d, length.max(8.0), w)
                        {
                            exit = Some((ux, next));
                            break;
                        }
                        d = -d;
                    }
                    let Some((ux, next)) = exit else { continue };
                    let t = Tunnel { side, m: mi, doors: [(ua, v), (ux, runs[runs.len() - 1].v.1)], runs, width: w };
                    let ((e0, e1), (v0, v1)) = t.extent();
                    let zs = (to_z(m, e0) - 1.0, to_z(m, e1) + 1.0);
                    if inside.iter().any(|b| b.0 == side && zs.0 < b.1.1 && zs.1 > b.1.0 && v0 - 1.0 < b.2.1 && v1 + 4.0 > b.2.0) {
                        continue;
                    }
                    found = Some(Step::Through(t, next));
                    break;
                }
                // Through the wall now and then (more often where the way
                // down the face is blocked).
                let through = on.is_some() && !cross && r(k, 9) < if t >= 6 { 0.6 } else { 0.12 };
                if through {
                    let here = ways[on.unwrap()];
                    let w = here.width.min(3.0);
                    // The door anywhere along the walkway (what lies beyond
                    // it along the walkway is left a dead end).
                    let zd = here.z.0 + w + (here.z.1 - here.z.0 - 2.0 * w) * r(k, 14);
                    let Some((m, ud)) = locate(walls(side), zd) else { continue };
                    let mi = walls(side).iter().position(|x| std::ptr::eq(x, m)).unwrap();
                    let ua = ud.round();
                    // (Down as far as it takes, on the way it was going, or
                    // back the way it came, under it, inside the rock.)
                    let n = 4 * (5 + (r(k, 10) * r(k, 12) * 95.0) as i32);
                    let t_dir = if r(k, 13) < 0.5 { dir } else { -dir };
                    let us = ua + t_dir * w * 0.5;
                    let ue = us + t_dir * n as f32 * TREAD;
                    let ux = ue + t_dir * w * 0.5;
                    let vb = v - n as f32 * RISER;
                    let face = (vb - 1.0..=v + 4.0).step_by_f32(1.0).map(|x| m.face(x)).fold(f32::MAX, f32::min);
                    let lane = face - 5.0 - 5.0 * r(k, 11) - w * 0.5;
                    if m.shaft || ux.min(ua) < w + 2.0 || ux.max(ua) > m.len - w - 2.0 || vb < 35.0 || !plain(m, (v - 0.5, v + 3.5)) {
                        continue;
                    }
                    let t = Tunnel { side, m: mi, doors: [(ua, v), (ux, vb)], runs: vec![Run { u: (us, ue), v: (v, vb), n: lane }], width: w };
                    let zs = (to_z(m, ua.min(ux)) - 1.0, to_z(m, ua.max(ux)) + 1.0);
                    if inside.iter().any(|b| b.0 == side && zs.0 < b.1.1 && zs.1 > b.1.0 && vb - 1.0 < b.2.1 && v + 4.0 > b.2.0) {
                        continue;
                    }
                    let Some(start) = snap(side, to_z(m, ux - t_dir * (w * 0.5 + 0.5))) else { continue };
                    if let Some(next) = walkway_at(&taken, side, start, vb, t_dir, length.max(8.0), w) {
                        found = Some(Step::Through(t, next));
                        break;
                    }
                    continue;
                }
                if !cross {
                    // Down: a short flight on along the wall, onto a walkway
                    // going on from its foot.
                    let try_dir = dir;
                    let Some((m, ua)) = locate(walls(side), z + try_dir * 0.3) else { continue };
                    let mi = walls(side).iter().position(|x| std::ptr::eq(x, m)).unwrap();
                    let ua = ua.round();
                    let n = 4 * (3 + (r(k, 5) * 4.0) as i32);
                    let ub = ua + try_dir * n as f32 * TREAD;
                    let vb = v - n as f32 * RISER;
                    if m.shaft || ub < 0.5 || ub > m.len - 0.5 || !plain(m, (vb - 3.5, v + 3.5)) {
                        continue;
                    }
                    let f = Flight { side, m: mi, u: (ua, ub), v: (v, vb), z: (to_z(m, ua), to_z(m, ub)), width };
                    let n0 = inner(m, v);
                    if !free(&taken, None, flight_box(&f)) || !open(side, &flight_space(m, f.u, f.v, (n0, n0 + width))) {
                        continue;
                    }
                    if let Some(w) = walkway_at(&taken, side, f.z.1, vb, try_dir, length, width) {
                        found = Some(Step::Down(f, w));
                        break;
                    }
                } else {
                    // Across: a level bridge from near the end of this
                    // walkway straight over to the other wall, onto a
                    // walkway there going on either way.
                    let here = ways[on.unwrap()];
                    let zb = here.z.0 + 2.0 + (here.z.1 - here.z.0 - 4.0) * r(k, 6);
                    let other = -side;
                    let to_width = (2.0 + (r(k, 7) * 3.0).floor()).min(4.0);
                    let (Some(a), Some(b)) = (edge(side, zb, v, width), edge(other, zb, v, to_width)) else { continue };
                    let depth = (a.distance(b) / 18.0).clamp(2.5, 7.0);
                    if !clear(&spans, a, b, depth) {
                        continue;
                    }
                    let to_dir = if r(k, 8) < 0.5 { 1.0 } else { -1.0 };
                    let Some(start) = snap(other, zb - to_dir * 4.0) else { continue };
                    if let Some(w) = walkway_at(&taken, other, start, v, to_dir, length.max(12.0), to_width) {
                        found = Some(Step::Across(zb, w, (a, b, depth)));
                        break;
                    }
                }
            }
            let Some(step) = found else {
                stuck += 1;
                if salt >= 2000 {
                    info!("the chasm: the way down gives up at {v:.0} m (side {side}, z {z:.0}, walkway {:?})", on.map(|k| (ways[k].z, ways[k].width)));
                    break;
                }
                // (Nothing fits from the rim: start somewhere else along it.)
                if history.is_empty() {
                    salt += 1;
                    z = snap(-1.0, CENTRE.z + (r(salt, 20) - 0.5) * 300.0).unwrap_or(CENTRE.z);
                    dir = -dir;
                    continue;
                }
                // Back a step; stuck again and again, further back at once
                // (where the way went wrong may be well behind).
                for _ in 0..(1 + stuck / 6).min(history.len()) {
                    let (t, sp, nf, nb, (nt, ni), state) = history.pop().unwrap();
                    taken.truncate(t);
                    spans.truncate(sp);
                    flights.truncate(nf);
                    bridges.truncate(nb);
                    tunnels.truncate(nt);
                    inside.truncate(ni);
                    ways.pop();
                    (side, z, v, dir, width, on, since) = state;
                }
                salt += 1;
                // (Back at the rim: start from somewhere else along it.)
                if history.is_empty() {
                    z = snap(-1.0, CENTRE.z + (r(salt, 20) - 0.5) * 300.0).unwrap_or(CENTRE.z);
                    dir = if r(salt, 21) < 0.5 { 1.0 } else { -1.0 };
                }
                continue;
            };
            history.push((taken.len(), spans.len(), flights.len(), bridges.len(), (tunnels.len(), inside.len()), (side, z, v, dir, width, on, since)));
            stuck = stuck.saturating_sub(1);
            let w = match step {
                Step::Down(f, w) => {
                    taken.push((None, flight_box(&f)));
                    dir = (f.z.1 - f.z.0).signum();
                    flights.push(f);
                    since += 1;
                    w
                }
                Step::Through(t, w) => {
                    let m = &walls(t.side)[t.m];
                    let to_z = |u: f32| m.z.0 + u / m.len * (m.z.1 - m.z.0);
                    let ((e0, e1), (v0, v1)) = t.extent();
                    inside.push((t.side, (to_z(e0) - 1.0, to_z(e1) + 1.0), (v0 - 1.0, v1 + 4.0)));
                    let last = t.runs[t.runs.len() - 1];
                    dir = (last.u.1 - last.u.0).signum();
                    tunnels.push(t);
                    width = w.width;
                    since += 1;
                    w
                }
                Step::Across(zb, w, span) => {
                    for sd in [side, w.side] {
                        taken.push((None, (sd, (zb - 6.0, zb + 6.0), (v - span.2 - 2.0, v + 3.5))));
                    }
                    let (a, b) = (on.unwrap(), ways.len());
                    bridges.push(if side < 0.0 { (zb, a, b) } else { (zb, b, a) });
                    spans.push(span);
                    dir = if (w.z.1 - zb).abs() > (w.z.0 - zb).abs() { 1.0 } else { -1.0 };
                    width = w.width;
                    since = 0;
                    w
                }
            };
            taken.push((Some(ways.len()), way_box(&w)));
            side = w.side;
            z = if dir > 0.0 { w.z.1 } else { w.z.0 };
            v = w.v0;
            on = Some(ways.len());
            ways.push(w);
        }
        // How long it is to walk.
        let length = ways.iter().map(|w| w.z.1 - w.z.0).sum::<f32>()
            + flights.iter().map(|f| Vec2::new(f.u.1 - f.u.0, f.v.1 - f.v.0).length()).sum::<f32>()
            + spans.iter().map(|s| s.0.distance(s.1)).sum::<f32>()
            + tunnels.iter().flat_map(|t| t.runs.iter().map(|r| Vec2::new(r.u.1 - r.u.0, r.v.1 - r.v.0).length() + 2.0 * t.width)).sum::<f32>();
        // Where you start: on the rim behind the first flight, looking along
        // it and down.
        let start = flights
            .first()
            .map(|f| {
                let m = &near[f.m];
                let d = (f.u.1 - f.u.0).signum();
                let top = m.face(HEIGHT);
                let eye = m.wall.at(f.u.0 - d * 5.0, HEIGHT + 1.7, top - 1.5);
                let look = m.wall.at(f.u.0 + d * 12.0, HEIGHT - 6.0, top + 2.0) - eye;
                [eye.x, HEIGHT + 1.7, eye.z, (-look.x).atan2(-look.z).to_degrees(), look.y.atan2(Vec2::new(look.x, look.z).length()).to_degrees()]
            })
            .unwrap_or([face_x(&plan(-1.0, seed), CENTRE.z) - 5.0, HEIGHT + 1.7, CENTRE.z, -130.0, -38.0]);
        Routing { ways, flights, tunnels, bridges, spans, start, length }
    }

    /// Where the routes run along a wall (with headroom): its walkways, its
    /// flights.
    fn zones(&self, side: f32) -> Vec<Zone> {
        let mut out = Vec::new();
        for w in self.ways.iter().filter(|w| w.side == side) {
            out.push(Zone { z: w.z, v: (w.v0 - shelf_depth(w.width) - 1.0, w.v0 + 4.0) });
        }
        out.extend(self.flight_zones(side));
        out
    }

    /// Where tunnels' doors open in a wall.
    fn door_zones(&self, side: f32, walls: &[Massif]) -> Vec<Zone> {
        let mut out = Vec::new();
        for t in self.tunnels.iter().filter(|t| t.side == side) {
            let m = &walls[t.m];
            let to_z = |u: f32| m.z.0 + u / m.len * (m.z.1 - m.z.0);
            for (u, v) in t.doors {
                out.push(Zone { z: (to_z(u - t.width), to_z(u + t.width)), v: (v - 1.0, v + 4.0) });
            }
        }
        out
    }

    /// Where flights run along a wall.
    fn flight_zones(&self, side: f32) -> Vec<Zone> {
        self.flights
            .iter()
            .filter(|f| f.side == side)
            .map(|f| Zone { z: (f.z.0.min(f.z.1) - 1.0, f.z.0.max(f.z.1) + 1.0), v: (f.v.1 - 4.0, f.v.0 + 4.0) })
            .collect()
    }
}

/// Routes: the walkways (open where a bridge joins), bridges across, united
/// in one solid; the flights, bare, their steps to be seen and a slope
/// beneath them to walk on.
fn routes(parts: &mut Parts, routing: &Routing, near: &[Massif], far: &[Massif]) {
    let massifs = |side: f32| if side < 0.0 { near } else { far };
    let ways = &routing.ways;
    let mut solids = Vec::new();
    for (k, w) in ways.iter().enumerate() {
        let openings: Vec<(f32, f32)> = routing.bridges.iter().filter(|b| b.1 == k || b.2 == k).map(|b| (b.0 - 3.0, b.0 + 3.0)).collect();
        let terraces: Vec<Terrace> = parts.terraces.iter().filter(|t| t.way == k).copied().collect();
        let built = walkway(parts, w, massifs(w.side), &openings, &terraces);
        solids.push(built);
    }
    for &(z, i, j) in &routing.bridges {
        let (a, b) = (&ways[i], &ways[j]);
        let (Some((ma, ua)), Some((mb, ub))) = (locate(near, z), locate(far, z)) else { continue };
        let pa = ma.wall.at(ua, a.v0, inner(ma, a.v0) + a.width - 0.5);
        let pb = mb.wall.at(ub, b.v0, inner(mb, b.v0) + b.width - 0.5);
        solids.push(span(parts, pa, pb, 3.0, (a.width, b.width)));
        info!("the chasm: a bridge from {:?} to {:?}", pa, pb);
    }
    for t in &routing.tunnels {
        let m = &massifs(t.side)[t.m];
        let w = t.width * 0.5;
        let line = |a: (f32, f32, f32), b: (f32, f32, f32)| {
            let mut pts = Vec::new();
            for (u, v, n) in [a, b] {
                for (dv, dn) in [(0.0, -0.08), (0.0, 0.08), (-0.06, -0.08), (-0.06, 0.08)] {
                    let p = m.wall.at(u, v + dv, n + dn);
                    pts.push([p.x as f64, p.y as f64, p.z as f64]);
                }
            }
            Manifold::hull_pts(&pts)
        };
        let top = 3.0 - 0.31;
        let mut lines = Vec::new();
        // The corridors: their floors, the space over them, a line along
        // the ceiling.
        for (u, v, n) in t.corridors() {
            solids.push(wbox(&m.wall, (u - w, u + w), (v - 0.3, v), (n - w, inner(m, v) + 0.6)));
            parts.clearance.push((format!("tunnel door at {v:.0} m"), wbox(&m.wall, (u - w + 0.2, u + w - 0.2), (v + 0.1, v + 2.4), (n, inner(m, v) + 0.5))));
            lines.push(line((u, v + top, inner(m, v) + 0.3), (u, v + top, n)));
        }
        // The runs: steps, the slope to walk on, the space over them; lit
        // every 12 m and along the ceiling.
        for r in &t.runs {
            let (steps, slope, clear) = stair(m, r.u, r.v, (r.n - w + 0.2, r.n + w), None);
            parts.steps.solid(&steps);
            parts.ramps.solid(&slope);
            parts.clearance.push((format!("tunnel stair at {:.0} m", r.v.0), clear));
            lines.push(line((r.u.0, r.v.0 + top, r.n), (r.u.1, r.v.1 + top, r.n)));
            let lamps = ((r.u.1 - r.u.0).abs() / 12.0).ceil().max(1.0) as i32;
            for i in 0..=lamps {
                let f = i as f32 / lamps as f32;
                parts.lights.push((m.wall.at(r.u.0 + (r.u.1 - r.u.0) * f, r.v.0 + (r.v.1 - r.v.0) * f + 2.6, r.n), 10.0, 0.01));
            }
        }
        // The landings: their floors, the space over them.
        for (u, v, n) in t.landings() {
            solids.push(wbox(&m.wall, (u - w - 0.3, u + w + 0.3), (v - 0.3, v), n));
            parts.clearance.push((format!("tunnel landing at {v:.0} m"), wbox(&m.wall, (u - w + 0.2, u + w - 0.2), (v + 0.1, v + 2.4), (n.0 + 0.2, n.1 - 0.2))));
        }
        parts.dim.solid(&Manifold::batch_union(&lines));
        parts.paths.push((format!("tunnel at {:.0} m", t.doors[0].1), t.path(m)));
        let [(ua, va, na), (ux, vx, _)] = t.corridors();
        info!(
            "the chasm: a tunnel from {:?} to {:?} (out {:?}, inside {:?}, {} runs)",
            m.wall.at(ua, va, inner(m, va)),
            m.wall.at(ux, vx, inner(m, vx)),
            m.wall.out,
            m.wall.at(t.runs[0].u.0, va, na),
            t.runs.len()
        );
    }
    parts.mark("routes");
    parts.stone.solid(&Manifold::batch_union(&solids));
    for f in &routing.flights {
        let m = &massifs(f.side)[f.m];
        let n0 = inner(m, f.v.0);
        let (steps, slope, clear) = stair(m, f.u, f.v, (n0, n0 + f.width), Some(m.face(f.v.0) - 1.5));
        parts.clearance.push((format!("flight at {:.0} m", f.v.0), clear));
        let n = n0 + f.width * 0.5 + 0.25;
        let d = (f.u.1 - f.u.0).signum() * 0.3;
        parts.paths.push((format!("flight at {:.0} m", f.v.0), vec![m.wall.at(f.u.0 + d, f.v.0 + 1.2, n), m.wall.at(f.u.1 - d, f.v.1 + 1.2, n)]));
        info!("the chasm: a flight from {:?} to {:?} (out {:?})", m.wall.at(f.u.0, f.v.0, n0 + f.width), m.wall.at(f.u.1, f.v.1, n0 + f.width), m.wall.out);
        parts.steps.solid(&steps);
        parts.ramps.solid(&slope);
    }
}

/// A solid parapet wall along a walking edge, from `a` to `b` (points on the
/// walking surface under the wall's middle): a pier at each end, the wall
/// between them (so parapets meeting at a corner meet in a pier, never
/// overlapping).
fn parapet(parts: &mut Parts, a: Vec3, b: Vec3) {
    let along = (b - a).normalize_or(Vec3::X);
    let flat = Vec3::new(along.x, 0.0, along.z).normalize_or(Vec3::X);
    pier(parts, a, flat);
    pier(parts, b, flat);
    if a.distance(b) > 0.9 {
        let (p, q) = (a + along * 0.3, b - along * 0.3);
        parts.stone.beam(p + Vec3::Y * (PARAPET * 0.5), q + Vec3::Y * (PARAPET * 0.5), 0.4, PARAPET, Vec3::Y);
    }
}

/// A pier at `at` (on the walking surface), unless one stands there.
fn pier(parts: &mut Parts, at: Vec3, along: Vec3) {
    if parts.piers.iter().any(|p| p.distance(at) < 0.5) {
        return;
    }
    parts.piers.push(at);
    parts.stone.beam(at - Vec3::Y * 0.2, at + Vec3::Y * (PARAPET + 0.3), 0.7, 0.7, along);
}

/// How far a walkway's shelf goes down beneath it, back into the wall.
fn shelf_depth(width: f32) -> f32 {
    (width * 0.9).clamp(2.5, 10.0)
}

/// A plan point (x, z) as a section's point (x, -z): so a section extruded
/// upwards and stood up by `upright` keeps its faces wound outwards.
fn sec(p: Vec2) -> [f64; 2] {
    [p.x as f64, -p.y as f64]
}

/// The convex region in plan around `pts`.
fn region(pts: &[Vec2]) -> CrossSection {
    CrossSection::hull_polygons(&[pts.iter().map(|&p| sec(p)).collect()])
}

/// The convex solid around points given in plan and height above the deck.
fn hull3(pts: &[(Vec2, f32)]) -> Manifold {
    Manifold::hull_pts(&pts.iter().map(|&(p, h)| [p.x as f64, -p.y as f64, h as f64]).collect::<Vec<_>>())
}

/// A region in plan raised from `h0` to `h1` above the deck.
fn raised(s: &CrossSection, h0: f32, h1: f32) -> Manifold {
    s.extrude((h1 - h0) as f64).translate(0.0, 0.0, h0 as f64)
}

/// A solid built in a walkway's frame (plan, and height above its deck)
/// set in place: the deck's plane is its slope along the chasm.
fn upright(m: &Manifold, w: &Walkway) -> Manifold {
    let g = w.grade as f64;
    m.transform(&[1.0, 0.0, 0.0, 0.0, -g, -1.0, 0.0, 1.0, 0.0, 0.0, (w.v0 - w.grade * w.z.0) as f64, 0.0])
}

/// The faces a walkway passes: the massif, from where to where along it,
/// its heights there, the deck's inner and outer edge.
fn walkway_faces(w: &Walkway, massifs: &[Massif]) -> Vec<(usize, f32, f32, (f32, f32), f32, f32)> {
    let mut faces = Vec::new();
    for (k, m) in massifs.iter().enumerate() {
        let (zs, ze) = (m.z.0.max(w.z.0), m.z.1.min(w.z.1));
        if ze - zs < 0.5 {
            continue;
        }
        let to_u = |z: f32| (z - m.z.0) / (m.z.1 - m.z.0).max(1e-3) * m.len;
        let (vs, ve) = (w.v(zs), w.v(ze));
        let n0 = inner(m, (vs + ve) * 0.5);
        faces.push((k, to_u(zs), to_u(ze), (vs, ve), n0, n0 + w.width));
    }
    faces
}

/// The space a walker needs over a walkway: over what is walkable on it
/// (from just out from the face to the edge, round its corners), 2.4 m up.
fn walk_space(w: &Walkway, massifs: &[Massif]) -> Manifold {
    let faces = walkway_faces(w, massifs);
    let plan = |k: usize, u: f32, n: f32| {
        let p = massifs[k].wall.at(u, 0.0, n);
        Vec2::new(p.x, p.z)
    };
    let mut walk = Vec::new();
    for (i, &(k, us, ue, _, n0, n1)) in faces.iter().enumerate() {
        walk.push(region(&[plan(k, us, n1 - 0.1), plan(k, ue, n1 - 0.1), plan(k, ue, n0 + 0.6), plan(k, us, n0 + 0.6)]));
        if let Some(&(k2, us2, _, _, n02, n12)) = faces.get(i + 1) {
            walk.push(region(&[plan(k, ue, n1 - 0.1), plan(k, ue, n0 + 0.6), plan(k2, us2, n12 - 0.1), plan(k2, us2, n02 + 0.6)]));
        }
    }
    upright(&raised(&CrossSection::batch_union(&walk), 0.1, 2.4), w)
}

/// The space a walker needs over a stair along a face (see `stair`): over
/// its slope, 2.3 m up, from just out from its inner side to its edge.
fn flight_space(m: &Massif, (ua, ub): (f32, f32), (va, vb): (f32, f32), n: (f32, f32)) -> Manifold {
    let s = if m.wall.along.dot(Vec3::Y.cross(m.wall.out)) > 0.0 { 1.0 } else { -1.0 };
    let over = CrossSection::from_polygons_with_fill_rule(&[vec![[ua as f64, (va + 0.1) as f64], [ub as f64, (vb + 0.1) as f64], [ub as f64, (vb + 2.4) as f64], [ua as f64, (va + 2.4) as f64]]], FillRule::NonZero);
    placed(&across(&over, (n.0 + 0.7) * s, (n.1 - 0.1) * s), m.wall.origin, m.wall.along, m.wall.out * s)
}

/// A walkway, built as one solid from its plan. The plan: along each face
/// it passes, a strip from its outer edge back into the wall; where two
/// faces meet, the hull of their ends (a bevel at an outer corner; at an
/// inner one, inside the strips' overlap). Taken as one
/// region, so however short a face or sharp a turn, the outline is simply
/// that region's edge. On it: the deck (cut away for its places' floors),
/// its underside sloping back into the wall (clipped to the plan); bare,
/// nothing at its edge; a lit line along the edge (broken at its ends,
/// places and where bridges join); lights below. All of it in the deck's
/// frame, united, then set on the walkway's slope: returned, to be united
/// with the rest of the routes.
fn walkway(parts: &mut Parts, w: &Walkway, massifs: &[Massif], openings: &[(f32, f32)], terraces: &[Terrace]) -> Manifold {
    let depth = shelf_depth(w.width);
    let flat = |p: Vec3| Vec2::new(p.x, p.z);
    // The faces it passes; and how far back the wall lies behind the edge,
    // at most (the deck reaches that far in on every face).
    let faces = walkway_faces(w, massifs);
    let mut reach = w.width + 1.0;
    for &(k, _, _, (vs, ve), n0, n1) in &faces {
        let m = &massifs[k];
        let back = (vs.min(ve) - depth..=vs.max(ve)).step_by_f32(2.0).map(|v| inner(m, v)).fold(n0, f32::min) - 1.0;
        reach = reach.max(n1 - back);
    }
    if faces.is_empty() {
        return Manifold::empty();
    }
    let plan = |k: usize, u: f32, n: f32| flat(massifs[k].wall.at(u, 0.0, n));
    // A band across a face's strip, from `a` to `b` along it.
    let across = |k: usize, a: f32, b: f32, n1: f32| region(&[plan(k, a, n1 + 2.0), plan(k, b, n1 + 2.0), plan(k, b, n1 - reach - 2.0), plan(k, a, n1 - reach - 2.0)]);
    // The plan.
    let mut areas = Vec::new();
    let mut under = Vec::new();
    let profile = |k: usize, u: f32, n1: f32| [(plan(k, u, n1), 0.0), (plan(k, u, n1), -EDGE), (plan(k, u, n1 - reach), 0.0), (plan(k, u, n1 - reach), -depth)];
    for (i, &(k, us, ue, _, _, n1)) in faces.iter().enumerate() {
        areas.push(region(&[plan(k, us, n1), plan(k, ue, n1), plan(k, ue, n1 - reach), plan(k, us, n1 - reach)]));
        under.push(hull3(&[profile(k, us, n1), profile(k, ue, n1)].concat()));
        if let Some(&(k2, us2, _, _, _, n12)) = faces.get(i + 1) {
            areas.push(region(&[plan(k, ue, n1), plan(k, ue, n1 - reach), plan(k2, us2, n12), plan(k2, us2, n12 - reach)]));
            under.push(hull3(&[profile(k, ue, n1), profile(k2, us2, n12)].concat()));
        }
    }
    let deck = CrossSection::batch_union(&areas);
    parts.clearance.push((format!("walkway at {:.0} m", w.v0), walk_space(w, massifs)));
    let mut line = Vec::new();
    for &(k, us, ue, _, n0, n1) in &faces {
        let m = &massifs[k];
        let (a, b) = (us.min(ue) + 0.4, us.max(ue) - 0.4);
        if b > a {
            line.extend([m.wall.at(a, w.v0 + 1.2, (n0 + n1) * 0.5 + 0.25), m.wall.at(b, w.v0 + 1.2, (n0 + n1) * 0.5 + 0.25)]);
        }
    }
    parts.paths.push((format!("walkway at {:.1} m (side {}, z {:.0}-{:.0})", w.v0, w.side, w.z.0, w.z.1), line));
    // Its places, cut out of the deck; the parapet's openings: the places,
    // where bridges join, and the walkway's two ends.
    let mut gaps: Vec<(usize, f32, f32)> = Vec::new();
    for &(k, ..) in &faces {
        gaps.extend(terraces.iter().filter(|t| t.massif == k).map(|t| (k, t.u.0, t.u.1)));
    }
    let n1_of = |k: usize| faces.iter().find(|f| f.0 == k).map_or(0.0, |f| f.5);
    let cut = |list: &[(usize, f32, f32)]| CrossSection::batch_union(&list.iter().map(|&(k, a, b)| across(k, a, b, n1_of(k))).collect::<Vec<_>>());
    let places = cut(&gaps);
    let floor = deck.difference(&places);
    for &(z0, z1) in openings {
        let z = (z0 + z1) * 0.5;
        if let Some(&(k, ..)) = faces.iter().find(|f| (massifs[f.0].z.0..=massifs[f.0].z.1).contains(&z)) {
            let m = &massifs[k];
            let to_u = |z: f32| (z - m.z.0) / (m.z.1 - m.z.0).max(1e-3) * m.len;
            gaps.push((k, to_u(z0), to_u(z1)));
        }
    }
    let ((k0, us0, ..), (k1, _, ue1, ..)) = (faces[0], faces[faces.len() - 1]);
    gaps.push((k0, us0 - 3.0, us0 + 0.5));
    gaps.push((k1, ue1 - 0.5, ue1 + 3.0));
    let open = cut(&gaps);
    // The solid: bare, nothing at its edge.
    let solid = [raised(&floor, -EDGE, 0.0), Manifold::batch_union(&under).intersection(&raised(&floor, -60.0, 0.0))];
    let built = upright(&Manifold::batch_union(&solid), w);
    // A lit line along the edge.
    let line = deck.offset(0.04, JoinType::Miter, 4.0, 0).difference(&deck).difference(&open);
    parts.glow.solid(&upright(&raised(&line, -1.0, -0.9), w));
    // (Its outline's corners on the edge side, logged: where it turns,
    // from and on, and which way is in, for close looks.)
    for poly in deck.to_polygons() {
        let pts: Vec<Vec2> = poly.iter().map(|p| Vec2::new(p[0] as f32, -p[1] as f32)).collect();
        for j in 0..pts.len() {
            let (a, p, b) = (pts[(j + pts.len() - 1) % pts.len()], pts[j], pts[(j + 1) % pts.len()]);
            let Some(&(k, ..)) = faces.iter().min_by(|f, g| {
                let d = |f: &(usize, f32, f32, (f32, f32), f32, f32)| plan(f.0, (f.1 + f.2) * 0.5, f.5).distance(p);
                d(f).total_cmp(&d(g))
            }) else { continue };
            let m = &massifs[k];
            let n = (Vec3::new(p.x, 0.0, p.y) - m.wall.origin).dot(m.wall.out) - m.wall.offset;
            if n < n1_of(k) - 1.5 {
                continue;
            }
            let g = |v: Vec2| Vec3::new(v.x, 0.0, v.y);
            let height = w.v(p.y);
            info!("the chasm: a walkway corner at {:?} (from {:?}, on {:?}, in {:?})", Vec3::new(p.x, height, p.y), g((p - a).normalize_or_zero()), g((b - p).normalize_or_zero()), -m.wall.out);
        }
    }
    // Per face: lights below, and its places.
    for &(k, us, ue, (vs, ve), n0, n1) in &faces {
        let m = &massifs[k];
        let lights = ((ue - us) / 70.0).round() as i32;
        for i in 0..lights {
            let u = us + (ue - us) * (i as f32 + 0.5) / lights as f32;
            parts.lights.push((m.wall.at(u, vs - depth - 4.0, (n0 + n1) * 0.5), 60.0, 0.35));
        }
        let v_at = |u: f32| vs + (ve - vs) * (u - us) / (ue - us).max(1e-3);
        for t in terraces.iter().filter(|t| t.massif == k) {
            parts.mark("place");
            place(parts, &m.wall, t, &v_at, (n0, n1), depth);
            parts.mark("walkway");
        }
    }
    built
}

/// A walkway's part of a place: at each end a buttress across the walkway,
/// an arch cut through it, corbelled beneath like the shelf, steps down from
/// it to the floor inside; the floor running on out past the face, its
/// corners chamfered, a stepped corbel beneath it back into the wall, a
/// parapet round its edge, a line of light under it and a light below.
fn place(parts: &mut Parts, wall: &Wall, t: &Terrace, v_at: &dyn Fn(f32) -> f32, (n0, n1): (f32, f32), depth: f32) {
    let (a, b) = t.u;
    let f = t.floor;
    let at = |u: f32, v: f32, n: f32| wall.at(u, v, n);
    // The floor, and beneath it a stepped corbel back to the face.
    let mut s = 0;
    loop {
        let front = t.front - 2.0 * s as f32;
        if front <= t.face {
            break;
        }
        let e = (front - n1).max(0.0);
        let inside = t.face;
        let mut pts = vec![at(a, 0.0, inside), at(b, 0.0, inside)];
        if front > n1 {
            pts.extend([at(b, 0.0, n1), at(b - e, 0.0, front), at(a + e, 0.0, front), at(a, 0.0, n1)]);
        } else {
            pts.extend([at(b, 0.0, front), at(a, 0.0, front)]);
        }
        let (bottom, top) = if s == 0 { (f - EDGE, f) } else { (f - EDGE - 2.4 * s as f32, f - EDGE - 2.4 * (s - 1) as f32) };
        parts.stone.plate(&pts, bottom, top);
        s += 1;
    }
    // Round its edge.
    let e = t.front - n1;
    let edge = [(a + 0.2, n1), (a + e + 0.1, t.front - 0.2), (b - e - 0.1, t.front - 0.2), (b - 0.2, n1)];
    for pair in edge.windows(2) {
        let ((ua, na), (ub, nb)) = (pair[0], pair[1]);
        let (p, q) = (at(ua, f, na), at(ub, f, nb));
        parapet(parts, p, q);
        parts.glow.beam(p - Vec3::Y * (EDGE + 0.05), q - Vec3::Y * (EDGE + 0.05), 0.15, 0.05, Vec3::Y);
    }
    parts.lights.push((at((a + b) * 0.5, f - 12.0, (n1 + t.front) * 0.5), 80.0, 0.35));
    // At each end a buttress across the walkway, an arch through it, and
    // steps down inside.
    for (u, dir) in [(a, 1.0f32), (b, -1.0)] {
        let gv = v_at(u);
        let (g0, g1) = if dir > 0.0 { (u - 3.0, u) } else { (u, u + 3.0) };
        let (o0, o1) = (t.face - 1.0, n1 + 1.5);
        // (Its top a touch below the deck it crosses.)
        wall.section(&mut parts.stone, (g0, g1), &[(gv - depth - 3.0, o0), (gv - 0.03, o0), (gv - 0.03, o1), (gv - EDGE - 1.5, o1)]);
        let gate = Wall { origin: wall.origin + wall.along * g0, along: wall.out, out: wall.along, offset: 0.0 };
        arch(&mut parts.stone, &gate, (o0, o1), gv - 0.03, gv + 4.0, t.ceiling + 1.5, (0.0, 3.0), 1.5);
        let rise = gv - f;
        let steps = (rise / 0.3).ceil() as i32;
        for i in 0..steps - 1 {
            let s = u + dir * 0.45 * i as f32;
            let top = gv - rise * (i + 1) as f32 / steps as f32;
            wall.block(&mut parts.stone, (s.min(s + dir * 0.45), s.max(s + dir * 0.45)), (f - 0.1, top), (n0, n1));
        }
    }
}

/// A flight's treads in side view (x, y): from (x0, y0) down to (x1, y1),
/// step by step on the lattice (a riser down, then the rest of the tread
/// along); the first step a riser below y0, the last at y1.
fn treads(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<[f64; 2]> {
    let n = ((y0 - y1) / RISER).round().max(1.0) as i32;
    let (riser, tread) = ((y0 - y1) / n as f32, (x1 - x0) / n as f32);
    let mut pts = Vec::new();
    for i in 0..n {
        let y = (y0 - riser * (i + 1) as f32) as f64;
        pts.push([(x0 + tread * i as f32) as f64, y]);
        pts.push([(x0 + tread * (i + 1) as f32) as f64, y]);
    }
    pts
}

/// A flight's mass in side view: under its treads (`top`), down to a line
/// parallel to the flight `under` below.
fn flight_mass(top: &[[f64; 2]], under: f32) -> CrossSection {
    let (a, b) = (top[0], top[top.len() - 1]);
    let mut pts = top.to_vec();
    pts.push([b[0], b[1] - under as f64]);
    pts.push([a[0], a[1] - under as f64]);
    CrossSection::from_polygons_with_fill_rule(&[pts], FillRule::NonZero)
}

/// A section extruded across from `a` to `b` (either way round).
fn across(s: &CrossSection, a: f32, b: f32) -> Manifold {
    s.extrude((a - b).abs() as f64).translate(0.0, 0.0, a.min(b) as f64)
}

/// A solid built in a frame (x along `x`, y up, z along `z`) set in place
/// at `origin` (the frame kept right-handed by whoever builds in it).
fn placed(m: &Manifold, origin: Vec3, x: Vec3, z: Vec3) -> Manifold {
    m.transform(&[x.x as f64, x.y as f64, x.z as f64, 0.0, 1.0, 0.0, z.x as f64, z.y as f64, z.z as f64, origin.x as f64, origin.y as f64, origin.z as f64])
}

/// A stair on the lattice along a face (massif `m`): its steps from `u.0`
/// at height `v.0` (the end of the walkway it leaves, or a corridor's
/// floor) to `u.1` at `v.1` (the start of the next), `n` out from the
/// wall's line (as wide as what it joins and as far out, so they meet end to
/// end on the same line); nothing at its edges; beneath, a mass parallel to
/// it, and (`back`) deepening back into the wall to there. Returned as what
/// is seen, the slope to walk on (from the one floor to the other), and the
/// space a walker needs over it.
fn stair(m: &Massif, u: (f32, f32), v: (f32, f32), n: (f32, f32), back: Option<f32>) -> (Manifold, Manifold, Manifold) {
    let ((ua, ub), (va, vb)) = (u, v);
    // (Right-handed: across the face out, or in, whichever keeps it so.)
    let s = if m.wall.along.dot(Vec3::Y.cross(m.wall.out)) > 0.0 { 1.0 } else { -1.0 };
    let top = treads(ua, va, ub, vb);
    let mut solid = vec![across(&flight_mass(&top, 1.2), back.unwrap_or(n.0) * s, n.1 * s)];
    if let Some(back) = back {
        let mut pts = Vec::new();
        for (u, v) in [(ua, va), (ub, vb)] {
            for (y, x) in [(v - 1.0, back), (v - 1.0, n.0 + 1.0), (v - 4.0, back)] {
                pts.push([u as f64, y as f64, (x * s) as f64]);
            }
        }
        solid.push(Manifold::hull_pts(&pts));
    }
    let slope = CrossSection::from_polygons_with_fill_rule(&[vec![[ua as f64, va as f64], [ub as f64, vb as f64], [ub as f64, (vb - 1.0) as f64], [ua as f64, (va - 1.0) as f64]]], FillRule::NonZero);
    let frame = |x: &Manifold| placed(x, m.wall.origin, m.wall.along, m.wall.out * s);
    (frame(&Manifold::batch_union(&solid)), frame(&across(&slope, n.0 * s, n.1 * s)), flight_space(m, u, v, n))
}

/// A bar from `a` to `b` (its long axis), `w` wide and `t` thick, `up`
/// fixing which way is thick: as a solid.
fn bar(a: Vec3, b: Vec3, w: f32, t: f32, up: Vec3) -> Manifold {
    let along = (b - a).normalize_or(Vec3::X);
    let side = along.cross(up).normalize_or(along.any_orthonormal_vector()) * (w * 0.5);
    let lift = side.cross(along).normalize() * (t * 0.5);
    let mut pts = Vec::new();
    for end in [a, b] {
        for s in [-1.0, 1.0] {
            for l in [-1.0, 1.0] {
                let p = end + side * s + lift * l;
                pts.push([p.x as f64, p.y as f64, p.z as f64]);
            }
        }
    }
    Manifold::hull_pts(&pts)
}

/// A bridge between two points on walkways' outer edges, reaching back
/// `reach` (a, b) to the faces behind them: a deep girder, its top the deck,
/// running on back under the walkways into the walls; bare, nothing at its
/// edges; lines of light under them. Its solid returned, to be united
/// with the rest of the routes.
fn span(parts: &mut Parts, a: Vec3, b: Vec3, width: f32, reach: (f32, f32)) -> Manifold {
    let along = (b - a).normalize_or(Vec3::X);
    let across = along.cross(Vec3::Y).normalize_or(Vec3::Z);
    let depth = (a.distance(b) / 18.0).clamp(3.0, 7.0);
    let mut solid = vec![bar(a - Vec3::Y * (depth * 0.5), b - Vec3::Y * (depth * 0.5), width, depth, Vec3::Y)];
    // On back under the walkways (below their decks).
    let low = (depth - EDGE) * 0.5 + EDGE;
    for (end, dir, r) in [(a, -along, reach.0), (b, along, reach.1)] {
        solid.push(bar(end - Vec3::Y * low, end + dir * (r + 1.0) - Vec3::Y * low, width, depth - EDGE, Vec3::Y));
    }
    for s in [-1.0, 1.0] {
        let off = across * s * (width * 0.5 - 0.2);
        parts.glow.beam(a + off - Vec3::Y * (depth + 0.05), b + off - Vec3::Y * (depth + 0.05), 0.15, 0.05, Vec3::Y);
    }
    Manifold::batch_union(&solid)
}

/// An arched opening in a solid wall on a face: piers at both ends from `v0`
/// up to `vt`, the opening between them round-arched from the springing
/// `vs`, filled solid above the arch to `vt`. `n`: the wall's depth range.
#[allow(clippy::too_many_arguments)]
fn arch(g: &mut Geometry, w: &Wall, (u0, u1): (f32, f32), v0: f32, vs: f32, vt: f32, (n0, n1): (f32, f32), pier: f32) {
    w.block(g, (u0, u0 + pier), (v0, vt), (n0, n1));
    w.block(g, (u1 - pier, u1), (v0, vt), (n0, n1));
    let (ua, ub) = (u0 + pier, u1 - pier);
    let r = (ub - ua) * 0.5;
    let c = (ua + ub) * 0.5;
    const SEGS: usize = 10;
    let arc = |i: usize| {
        let a = std::f32::consts::PI * (1.0 - i as f32 / SEGS as f32);
        (c + a.cos() * r, vs + a.sin() * r)
    };
    let top = vt.max(vs + r + 0.3);
    let depth = w.out * (n1 - n0);
    for i in 0..SEGS {
        let ((ux, vx), (uy, vy)) = (arc(i), arc(i + 1));
        // Solid above this stretch of the arch, up to the top.
        let quad = [w.at(ux, vx, n0), w.at(uy, vy, n0), w.at(uy, top, n0), w.at(ux, top, n0)];
        g.sweep(&quad, depth);
    }
}

/// Steps through a range of heights.
trait StepBy {
    fn step_by_f32(self, step: f32) -> impl Iterator<Item = f32>;
}

impl StepBy for std::ops::RangeInclusive<f32> {
    fn step_by_f32(self, step: f32) -> impl Iterator<Item = f32> {
        let (a, b) = (*self.start(), *self.end());
        let n = ((b - a) / step).ceil().max(0.0) as i32;
        (0..=n).map(move |i| (a + step * i as f32).min(b))
    }
}

/// A web of taut cables across the void at every angle, wall to wall, clear
/// of the routes.
fn web(parts: &mut Parts, seed: i32, near_m: &[Massif], far_m: &[Massif], routing: &Routing) {
    let zones = [routing.zones(-1.0), routing.zones(1.0)];
    let blocked = |p: Vec3| {
        routing.spans.iter().any(|s| segment_gap(p, p, s.0, s.1) < s.2 + 6.0)
            || [near_m, far_m].iter().zip(&zones).any(|(w, zs)| {
                locate(w, p.z).is_some_and(|(m, _)| (p - m.wall.origin).dot(m.wall.out) < m.face(p.y) + 16.0 && crosses(zs, (p.z - 2.0, p.z + 2.0), (p.y - 2.0, p.y + 2.0)))
            })
    };
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c7);
    for k in 0..150 {
        let z0 = CENTRE.z - LENGTH * 0.45 + LENGTH * 0.9 * r(k, 0);
        let z1 = (z0 + (r(k, 1) - 0.5) * 200.0).clamp(CENTRE.z - LENGTH * 0.5, CENTRE.z + LENGTH * 0.5);
        let v0 = HEIGHT * (0.1 + 0.85 * r(k, 2));
        let v1 = (v0 + (r(k, 3) - 0.5) * 140.0).clamp(10.0, HEIGHT - 5.0);
        let (Some((nm, nu)), Some((fm, fu))) = (locate(near_m, z0), locate(far_m, z1)) else { continue };
        let a = nm.wall.at(nu, v0, nm.face(v0) + 0.5);
        let b = fm.wall.at(fu, v1, fm.face(v1) + 0.5);
        let thick = if r(k, 4) < 0.2 { 0.25 + 0.3 * r(k, 5) } else { 0.05 + 0.1 * r(k, 5) };
        // Taut: a little sag, more for the long ones.
        let sag = a.distance(b) * (0.01 + 0.03 * r(k, 6));
        let rope = rope_static(a, b, sag);
        if rope.iter().any(|&p| blocked(p)) {
            continue;
        }
        parts.cables.push((rope, thick));
    }
}
