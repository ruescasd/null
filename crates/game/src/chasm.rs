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

mod claims;
use claims::{Claim, ROCK, Table, Use};

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
    // (On any face's top: within its length, behind its edge. Not by where
    // the point is along the chasm, which behind a face standing well out
    // can be the next face's stretch.)
    walls
        .iter()
        .flatten()
        .any(|m| {
            let u = (p - m.wall.origin).dot(m.wall.along);
            let n = (p - m.wall.origin).dot(m.wall.out) - m.wall.offset;
            (0.0..=m.len).contains(&u) && n < m.face(HEIGHT) && n > m.face(HEIGHT) - BACK * 0.9
        })
        .then_some(HEIGHT)
}

/// Where you start: on the rim of the near wall behind the first flight,
/// looking along it and down.
pub fn start(seed: u32, junctions: bool) -> [f32; 5] {
    if junctions { catalogue().2.start } else { layout(seed as i32).2.start }
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
    // (A straight centre line: each stretch holds its distance from it all
    // along, so one that wandered would bring the walls across each other.
    // The chasm's larger shape is to come from a grid of its own.)
    let half = LENGTH * 0.5;
    // (The joints go near, far, anywhere in turn, out of step between the
    // walls, so the gap surely narrows and widens.)
    let phase = if side < 0.0 { 0 } else { 1 };
    let at = |k: i32| {
        let t = match (k + phase) % 3 {
            0 => 0.25 * r(k, 9),
            1 => 0.7 + 0.3 * r(k, 9),
            _ => r(k, 9),
        };
        CENTRE.x + side * (15.0 + 100.0 * t)
    };
    // (On the grid: each stretch's face straight along the chasm, a whole
    // number of modules out, its ends on the modules; stretches meeting
    // square, stepped by whole modules.)
    let snap = |x: f32| (x / GRID).round() * GRID;
    let mut out = Vec::new();
    let mut z = snap(CENTRE.z - half);
    let mut k = 0;
    let mut x = snap(at(0));
    while z < CENTRE.z + half - GRID * 0.5 {
        // (No shaft where you start.)
        let shaft = r(k, 0) < 0.18 && !(z - 12.0..z + 12.0).contains(&CENTRE.z);
        let length = snap(if shaft { 4.0 + 6.0 * r(k, 1) } else { 70.0 + 190.0 * r(k, 1) }).max(GRID).min(snap(CENTRE.z + half) - z);
        let z1 = z + length;
        out.push(Stretch { z: (z, z1), x: (x, x), shaft });
        // (The next a whole step out or back: never the same, so a joint
        // is a step, never a seam in a plain face.)
        let next = snap(at(k + 1));
        x = if shaft || (r(k + 1, 0) < 0.18) { x } else if next == x { x + GRID * side } else { next };
        z = z1;
        k += 1;
    }
    out
}

/// The module the walls' plan is laid out on (m): faces a whole number of
/// them out from the centre, their ends on them.
const GRID: f32 = 4.0;

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
    /// middle of each overlapping pair. (`opposite`: facing opposite ways
    /// instead; within one solid, a skin with nothing inside it.)
    fn coincident(&self, opposite: bool) -> Vec<(usize, usize, Vec3)> {
        use std::collections::HashMap;
        let p = |i: u32| Vec3::from(self.positions[i as usize]);
        let tris: Vec<[Vec3; 3]> = self.indices.chunks_exact(3).map(|t| [p(t[0]), p(t[1]), p(t[2])]).collect();
        // Grouped by plane (normal to a few degrees, offset to 10 cm; near
        // the edge of a group, in the next too, as a face and one on it can
        // round apart) and by a 16 m cell in that plane; each pair then
        // tested exactly.
        let mut groups: HashMap<(i32, i32, i32, i32, i32, i32), Vec<usize>> = HashMap::new();
        let bins = |q: f32| {
            let f = q.floor();
            let mut out = vec![f as i32];
            if q - f < 0.15 {
                out.push(f as i32 - 1);
            } else if q - f > 0.85 {
                out.push(f as i32 + 1);
            }
            out
        };
        for (k, t) in tris.iter().enumerate() {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            if n.length() < 1e-4 {
                continue;
            }
            // (Facing either way, grouped by the plane alone.)
            let n = n.normalize();
            let n = if (n.x, n.y, n.z) < (0.0, 0.0, 0.0) { -n } else { n };
            let (e1, e2) = n.any_orthonormal_pair();
            let c = (t[0] + t[1] + t[2]) / 3.0;
            // (In every cell it covers: a big face and a small one on it can
            // lie cells apart by their middles.)
            let span = |e: Vec3| t.iter().map(|q| (e.dot(*q) / 16.0).floor() as i32).fold((i32::MAX, i32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            let ((a0, a1), (b0, b1)) = (span(e1), span(e2));
            for &x in &bins(n.x * 20.0) {
                for &y in &bins(n.y * 20.0) {
                    for &z in &bins(n.z * 20.0) {
                        for &o in &bins(n.dot(c) * 10.0) {
                            for a in a0..=a1 {
                                for b in b0..=b1 {
                                    groups.entry((x, y, z, o, a, b)).or_default().push(k);
                                }
                            }
                        }
                    }
                }
            }
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
        let mut seen = std::collections::HashSet::new();
        for list in groups.values() {
            for (i, &x) in list.iter().enumerate() {
                for &y in &list[i + 1..] {
                    // (The two halves of one quad share an edge, not area.)
                    let facing = |t: &[Vec3; 3]| (t[1] - t[0]).cross(t[2] - t[0]);
                    let (fx, fy) = (facing(&tris[x]).normalize(), facing(&tris[y]).normalize());
                    let same_plane = fx.dot(fy).abs() > 0.9998 && fx.dot(tris[y][0] - tris[x][0]).abs() < 0.02;
                    if (fx.dot(fy) < 0.0) == opposite && same_plane && seen.insert((x.min(y), x.max(y))) && overlap(&tris[x], &tris[y]) {
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

    /// Its collider: duplicate vertices merged and triangles with no area
    /// left out (in the physics they make contacts out of nothing); as it is,
    /// if that fails.
    fn collider(&self) -> Option<Collider> {
        self.collider_with(TrimeshFlags::MERGE_DUPLICATE_VERTICES | TrimeshFlags::DELETE_DEGENERATE_TRIANGLES)
    }

    /// Its collider with `flags`; as it is, if that fails.
    fn collider_with(&self, flags: TrimeshFlags) -> Option<Collider> {
        let vertices: Vec<Vec3> = self.positions.iter().map(|&p| Vec3::from(p)).collect();
        let indices: Vec<[u32; 3]> = self.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        Collider::try_trimesh_with_config(vertices.clone(), indices.clone(), flags).ok().or_else(|| {
            warn!("the chasm: a collider without its flags");
            Collider::try_trimesh(vertices, indices).ok()
        })
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
        } else if v < self.split {
            self.low
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
    /// Flights' steps (seen, not walked on), and the slopes beneath them
    /// (walked on, not seen: real steps make you vault).
    steps: Geometry,
    ramps: Geometry,
    /// Each slope's corners, for its own convex collider (see `build`).
    slopes: Vec<Vec<Vec3>>,
    /// (`--opt seams`: the walls' solids, and the space a walker needs over
    /// each route, by what it is, to check one against the other.)
    walls: Vec<Manifold>,
    clearance: Vec<(String, Manifold)>,
    /// (`--opt seams`: lines a walker follows along each route, at chest
    /// height: no surface may cross them.)
    paths: Vec<(String, Vec<Vec3>)>,
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
    /// What is cut into the wall being built (tunnels, carved ways, halls),
    /// each with its bounds in the world: nothing on a face may stand into
    /// it (a face's detail can lie inside the next massif's rock, behind a
    /// corner, hidden but for what is cut there).
    cuts: Vec<(Vec3, Vec3, Manifold)>,
    /// The routes' solid, as built (for checking what runs into it).
    routes: Option<Manifold>,
}

impl Parts {
    fn mark(&mut self, what: &'static str) {
        self.marks.push((self.stone.indices.len() / 3, what));
    }

    fn maker(&self, triangle: usize) -> &'static str {
        self.marks.iter().rev().find(|m| m.0 <= triangle).map_or("?", |m| m.1)
    }

    /// A slope walked on: seen by the checks with the rest, and kept as its
    /// own convex solid for its collider.
    fn slope(&mut self, slope: &Manifold) {
        self.ramps.solid(slope);
        let (verts, props, _) = slope.to_mesh_f32();
        self.slopes.push(verts.chunks(props).map(|p| Vec3::new(p[0], p[1], p[2])).collect());
    }

    /// Whether a box in the world overlaps any route's space.
    fn keeps(&self, lo: Vec3, hi: Vec3) -> bool {
        self.keep.iter().any(|(a, b)| lo.x < b.x && hi.x > a.x && lo.y < b.y && hi.y > a.y && lo.z < b.z && hi.z > a.z)
    }

    /// Whether a piece of the face being built overlaps where a route runs:
    /// along the face, or in the world, as far out as anything on the face
    /// stands (whatever the faces' angles, either wall's routes); or what is
    /// cut into the wall.
    fn blocked(&self, u: (f32, f32), v: (f32, f32)) -> bool {
        self.blocked_to(u, v, STANDS_OUT)
    }

    /// The same for a piece standing out from the face as far as `out`
    /// (detail standing on detail can stand further out than most).
    fn blocked_to(&self, u: (f32, f32), v: (f32, f32), out: f32) -> bool {
        self.clear.iter().any(|&(a, b, c, d)| u.0 < b && u.1 > a && v.0 < d && v.1 > c)
            || self.frame.is_some_and(|w| {
                let corners = [u.0, u.1].into_iter().flat_map(|a| [v.0, v.1].into_iter().flat_map(move |b| [-1.0, out].map(move |c| w.at(a, b, c))));
                let (lo, hi) = corners.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
                self.keeps(lo, hi)
                    || self.cuts.iter().any(|(a, b, c)| {
                        lo.x < b.x && hi.x > a.x && lo.y < b.y && hi.y > a.y && lo.z < b.z && hi.z > a.z && c.intersection(&wbox(&w, u, v, (-1.0, out))).volume() > 1e-3
                    })
            })
    }
}

/// How far most of what is built on a face stands out from it, at most
/// (relief on relief tests how far it actually stands).
const STANDS_OUT: f32 = 8.0;

/// How bright the chasm's own lights are (lumens; `--set chasm_light`).
const LIGHT: f32 = 2.0e8;

/// The chasm as built for a seed, before anything is put in the world: its
/// parts (by material, with what the checks need), the routing, the walls'
/// shapes.
pub struct Made {
    parts: Parts,
    routing: Routing,
    near: Vec<Massif>,
    far: Vec<Massif>,
    /// A way all the way down (not the junction catalogue's entries).
    whole: bool,
}

/// Builds the chasm for a seed: nothing in the world yet (see `build`, and
/// `--check`).
fn make(seed: i32) -> Made {
    // (The walls' shapes and the routes are planned first: the walls keep
    // clear where the routes run.)
    let (near, far, routing, solids) = layout(seed);
    made_from(seed, near, far, routing, solids, true)
}

/// The junction catalogue (see `catalogue`) as built.
fn junctions() -> Made {
    let (near, far, routing, _) = catalogue();
    let solids = [wall_solid(&near), wall_solid(&far)];
    made_from(1, near, far, routing, solids, false)
}

/// Builds a chasm from its walls and its way (`whole`: a way all the way
/// down, not the catalogue's entries).
fn made_from(seed: i32, near: Vec<Massif>, far: Vec<Massif>, routing: Routing, solids: [Manifold; 2], whole: bool) -> Made {
    let mut parts = Parts::default();
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
        parts.keep.extend(bounds(&flight_space(m, f.u, f.v, deck_n(m, f.v.0, f.width, f.recess)), 0.5));
    }
    for t in &routing.tunnels {
        let m = &walls_of(t.side)[t.m];
        for (u, v) in t.doors {
            parts.keep.extend(bounds(&wbox(&m.wall, (u - t.width, u + t.width), (v - 1.0, v + 4.0), (inner(m, v) - 1.0, inner(m, v) + 4.0)), 0.5));
        }
    }
    for p in &routing.places {
        let m = &walls_of(p.side)[p.m];
        parts.keep.extend(bounds(&place_space(m, p), 0.5));
    }
    for h in &routing.halls {
        let m = &walls_of(h.side)[h.m];
        for u in [h.doors.0, h.doors.1] {
            parts.keep.extend(bounds(&wbox(&m.wall, (u - h.width, u + h.width), (h.v - 1.0, h.v + 4.0), (inner(m, h.v) - 1.0, inner(m, h.v) + 4.0)), 0.5));
        }
    }
    for s in &routing.spans {
        let (lo, hi) = (s.0.min(s.1), s.0.max(s.1));
        parts.keep.push((lo - Vec3::new(4.0, s.2 + 1.0, 4.0), hi + Vec3::new(4.0, 4.0, 4.0)));
    }
    wall(&mut parts, -1.0, seed, &near, &solids[0], &routing, !whole);
    wall(&mut parts, 1.0, seed + 7919, &far, &solids[1], &routing, !whole);
    routes(&mut parts, &routing, &near, &far);
    parts.mark("web");
    if whole {
        web(&mut parts, seed, &near, &far, &routing);
    }
    Made { parts, routing, near, far, whole }
}

impl Made {
    /// The way down in a line: its pieces, how far down it gets, how long.
    fn summary(&self) -> String {
        let r = &self.routing;
        format!(
            "the way down: {} flights ({} carved), {} tunnels, {} walkways ({} carved), {} bridges, {} places, {} halls, down to {:.0} m; {:.1} km, {:.0} min at a walk",
            r.flights.len(),
            r.flights.iter().filter(|f| f.recess > 0.0).count(),
            r.tunnels.len(),
            r.ways.len(),
            r.ways.iter().filter(|w| w.recess > 0.0).count(),
            r.bridges.len(),
            r.places.len(),
            r.halls.len(),
            self.bottom(),
            r.length / 1000.0,
            r.length / 4.0 / 60.0
        )
    }

    /// How far down the way gets.
    fn bottom(&self) -> f32 {
        self.routing.ways.last().map_or(HEIGHT, |w| w.v0)
    }

    /// What it is made of to collide with: the stone (walls, routes,
    /// detail) as one mesh, and each slope walked on as its own convex solid
    /// (as a mesh, a slope's top is two long triangles, and a body going
    /// down a long run straddles the seam between them, near its length, for
    /// tens of metres; the physics can catch on such a seam as on an edge,
    /// and held there the body stops dead, still running).
    fn colliders(&self) -> Vec<Collider> {
        self.parts.stone.collider().into_iter().chain(self.parts.slopes.iter().filter_map(|pts| Collider::convex_hull(pts.clone()))).collect()
    }

    /// The checks (`--opt seams`, `--check static`): the way reaches the
    /// bottom; every route's space clear of the walls and of the routes'
    /// own solid; every route's way crossed by no surface (stone, steps,
    /// slopes); floor all across where flights meet walkways. And faces
    /// that coincide (they flicker), counted.
    fn check(&self) -> Check {
        let (parts, routing) = (&self.parts, &self.routing);
        let walls_of = |side: f32| if side < 0.0 { &self.near } else { &self.far };
        let mut problems = Vec::new();
        // (The walls, and the routes' own solid: a piece's floor only
        // touches the space over it, so anything more is in the way.)
        let walls = Manifold::batch_union(&parts.walls);
        let routes = parts.routes.clone().unwrap_or_else(Manifold::empty);
        let mut blocked = 0;
        for (what, c) in &parts.clearance {
            let x = c.intersection(&walls);
            let y = c.intersection(&routes);
            let (x, by) = if x.volume() >= y.volume() { (x, "a wall") } else { (y, "a route") };
            if x.volume() > 0.05 {
                blocked += 1;
                problems.push(format!("blocked: {what}, by {by} ({:.1} m3, {:?})", x.volume(), x.bounding_box()));
            }
        }
        // (Stone, and stairs: their steps, and the slopes walked on.)
        let mut crossed: Vec<&str> = Vec::new();
        for (g, kind) in [(&parts.stone, "stone"), (&parts.steps, "steps"), (&parts.ramps, "a stair's slope")] {
            for (what, at, k) in g.crossings(&parts.paths) {
                if !crossed.contains(&what) {
                    crossed.push(what);
                    let t = [0, 1, 2].map(|i| Vec3::from(g.positions[g.indices[k * 3 + i] as usize]));
                    problems.push(format!("crossed: {what} at {at:?}, by {kind} ({}): {t:?}", if kind == "stone" { parts.maker(k) } else { "" }));
                }
            }
        }
        // Where flights meet walkways, floor all across (probed straight
        // down, just off each end, across its width).
        let mut probes = Vec::new();
        for f in routing.flights.iter().filter(|f| f.v.0 < HEIGHT - 1.0) {
            let m = &walls_of(f.side)[f.m];
            let (n0, n1) = deck_n(m, f.v.0, f.width, f.recess);
            let d = (f.u.1 - f.u.0).signum();
            for (u, v, out) in [(f.u.0, f.v.0, -d), (f.u.1, f.v.1, d)] {
                for i in 0..=4 {
                    let p = m.wall.at(u + out * 0.3, v, n0 + 0.6 + (n1 - n0 - 0.9) * i as f32 / 4.0);
                    probes.push((format!("{} end of the flight at {:.0} m", if out == -d { "top" } else { "foot" }, f.v.0), vec![p + Vec3::Y * 0.5, p - Vec3::Y * 0.5]));
                }
            }
        }
        let floored = parts.stone.crossings(&probes);
        let mut holes = 0;
        for (what, pts) in probes.iter().filter(|(what, _)| !floored.iter().any(|c| std::ptr::eq(c.0, what.as_str()))) {
            holes += 1;
            problems.push(format!("no floor: {what} at {:?}", pts[0] - Vec3::Y * 0.5));
        }
        let coinciding = parts.stone.coincident(false).len();
        // (The occupancy table, being validated against the checks above:
        // reported, not yet failing.)
        let misfits = routing.misfits(&self.near, &self.far);
        for m in misfits.iter().take(4) {
            problems.push(format!("claims: {m}"));
        }
        // (Skins: within the walls' mass or the routes' solid, faces back to
        // back with nothing between, a sheet of rock no thicker than paper,
        // there to walk into; no volume, so the spaces' test misses them.)
        // (Where the two faces are of pieces pressed together, stone on both
        // sides, they are a seam inside the stone, unseen; where there is
        // air on both sides, a skin.)
        let air = |p: Vec3, solid: &Manifold| {
            let corners: Vec<[f64; 3]> = (0..8).map(|i| [0, 1, 2].map(|k| (p[k] + if i >> k & 1 == 1 { 0.005 } else { -0.005 }) as f64)).collect();
            let cube = Manifold::hull_pts(&corners);
            cube.intersection(solid).volume() < 1e-7
        };
        let mut skins = 0;
        for (x, y, at) in parts.stone.coincident(true) {
            let (a, b) = (parts.maker(x), parts.maker(y));
            let t = [0, 1, 2].map(|i| Vec3::from(parts.stone.positions[parts.stone.indices[x * 3 + i] as usize]));
            let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
            let solid = if a == "wall mass" { &walls } else { &routes };
            if a == b && (a == "wall mass" || a == "routes") && air(at + n * 0.03, solid) && air(at - n * 0.03, solid) {
                skins += 1;
                if skins <= 8 {
                    problems.push(format!("a skin of rock ({a}) at {at:?}"));
                }
            }
        }
        let down = !self.whole || self.bottom() < 50.0;
        if !down {
            problems.insert(0, format!("the way down stops at {:.0} m", self.bottom()));
        }
        let counts = format!(
            "{blocked} of {} routes' spaces run into something, {} of {} ways crossed by a surface, {holes} of {} points where flights meet walkways with no floor, {skins} skins of rock; {coinciding} coinciding faces; {} claim misfits",
            parts.clearance.len(),
            crossed.len(),
            parts.paths.len(),
            probes.len(),
            misfits.len()
        );
        Check { ok: down && blocked == 0 && crossed.is_empty() && holes == 0 && skins == 0 && misfits.is_empty(), summary: format!("{}; {counts}", self.summary()), problems }
    }
}

/// What the checks found: whether all is well, a line of counts, and each
/// problem.
pub struct Check {
    pub ok: bool,
    pub summary: String,
    pub problems: Vec<String>,
}

/// The checks for a seed (`--check static`), or for the junction catalogue
/// (`--check catalogue`).
pub fn check(seed: u32, catalogue: bool) -> Check {
    if catalogue { junctions().check() } else { make(seed as i32).check() }
}

/// For walking it with nothing rendered (`--check walk`, `--check wall`):
/// the chasm's colliders put in the world, and the way down and the walls to
/// walk into.
pub fn headless(app: &mut App, seed: u32, catalogue: bool) {
    let made = if catalogue { junctions() } else { make(seed as i32) };
    for collider in made.colliders() {
        app.world_mut().spawn((RigidBody::Static, collider, Transform::IDENTITY));
    }
    app.insert_resource(Way(made.routing.route(&made.near, &made.far)));
    app.insert_resource(WallTests(wall_tests(&made.routing, &made.near, &made.far)));
}

fn build(
    mut commands: Commands,
    args: Res<Args>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !args.opt("chasm") {
        return;
    }
    let mut made = if args.opt("junctions") {
        for line in catalogue().3 {
            info!("the junctions: {line}");
        }
        junctions()
    } else {
        make(args.seed as i32)
    };
    info!("the chasm: {}", made.summary());
    if args.opt("walkbot") {
        commands.insert_resource(Way(made.routing.route(&made.near, &made.far)));
    }
    if args.opt("wallbot") {
        commands.insert_resource(WallTests(wall_tests(&made.routing, &made.near, &made.far)));
    }
    // `--opt seams`: the checks, logged.
    if args.opt("seams") {
        let found = made.check();
        info!("the chasm: {}: {}", if found.ok { "checked" } else { "PROBLEMS" }, found.summary);
        for p in found.problems.iter().take(30) {
            info!("the chasm: {p}");
        }
    }
    for collider in made.colliders() {
        commands.spawn((RigidBody::Static, collider, Transform::IDENTITY));
    }
    let parts = &mut made.parts;
    let stone = materials.add(StandardMaterial { base_color: Color::srgb(0.62, 0.62, 0.62), perceptual_roughness: 0.92, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.02, 0.02, 0.02), perceptual_roughness: 0.9, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(60.0), ..default() });
    let dim = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(8.0), ..default() });
    let cable = materials.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.05), perceptual_roughness: 0.6, ..default() });

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
    info!("the chasm: built ({} lights)", parts.lights.len());
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
            massifs.push(Massif { wall: nominal, len, z: stretch.z, split: HEIGHT, low: SHAFT_DEPTH, high: SHAFT_DEPTH, room: 0.0, shaft: true });
            continue;
        }
        // (Depths on a 2 m module, the split on 4 m; overhangs a square
        // step, no slope.)
        let depth = |x: f32| (x / 2.0).round() * 2.0;
        let split = (HEIGHT * (0.3 + 0.5 * r(m, 2)) / 4.0).round() * 4.0;
        let low = depth(-10.0 + 18.0 * r(m, 3));
        // How much room there is: half the gap here, less a margin, so the
        // walls never close below about 50 m.
        // (The narrowest gap anywhere along it: the other wall steps in and
        // out.)
        let gap = other.iter().filter(|o| o.z.1 > stretch.z.0 && o.z.0 < stretch.z.1).map(|o| (o.x.0 - stretch.x.0).abs()).fold(f32::MAX, f32::min);
        let room = (gap.min(1000.0) * 0.5 - 25.0).max(0.0);
        // The upper part leans out over the chasm (mostly) or stands back,
        // a sloped face carrying it there. (Never set back far: you start
        // on the rim.)
        let delta = if r(m, 4) < 0.65 { 8.0 + 27.0 * r(m, 6) } else { -(4.0 + 10.0 * r(m, 6)) };
        let high = depth((low + delta).clamp(-4.0, (low + room.max(2.0)).max(-4.0)));
        massifs.push(Massif { wall: nominal, len, z: stretch.z, split, low, high, room, shaft: false });
    }
    massifs
}

/// A wall's mass as one solid, from its shape: each massif's (its lower
/// part, its upper part standing out or back from it; a shaft's slot), and
/// across each joint between them, band by band in height (each face plain
/// in it), a box behind the face set further back, overlapping both.
fn wall_solid(massifs: &[Massif]) -> Manifold {
    let mut mass = Vec::new();
    for ms in massifs {
        let (w, len) = (&ms.wall, ms.len);
        if ms.shaft {
            mass.push(wbox(w, (0.0, len), (0.0, HEIGHT), (-BACK, SHAFT_DEPTH)));
            continue;
        }
        // (One piece: its whole profile, lower part and upper, swept along
        // the face, so there are no seams inside it: pieces that only touch
        // leave a skin of rock between them, which a tunnel cut through
        // would meet.)
        let profile = [[-BACK, 0.0], [ms.low, 0.0], [ms.low, ms.split], [ms.high, ms.split], [ms.high, HEIGHT], [-BACK, HEIGHT]].map(|[n, v]| [n as f64, v as f64]);
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
        cuts.extend([a, b].into_iter().filter(|x| !x.shaft).map(|x| x.split));
        cuts.retain(|v| (0.0..=HEIGHT).contains(v));
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|x, y| (*x - *y).abs() < 0.01);
        // (Each box overlaps the massifs' ends, and the boxes above and
        // below it, by `E`, never in front of a face: pieces that only touch
        // leave a skin of rock between them.)
        const E: f32 = 0.05;
        for band in cuts.windows(2) {
            let (v0, v1) = (band[0], band[1]);
            let mid = (v0 + v1) * 0.5;
            // (Each face's depth from its own line: the other's in this
            // one's terms.)
            let front = a.face(mid).min(b.face(mid) + (b.wall.origin - a.wall.origin).dot(a.wall.out));
            mass.push(wbox(&a.wall, (a.len - E, a.len + E), ((v0 - E).max(0.0), (v1 + E).min(HEIGHT)), (-BACK, front)));
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
    let routing = Routing::plan(seed, &near, &far);
    (near, far, routing, solids)
}

/// The junction catalogue (`--opt junctions`): a straight chasm 48 m wide,
/// its near wall in slots, each a designed join of the way's pieces in a
/// setting (built out or carved, under an overhang, by a step, across a
/// joint), a short way through it: a walkway, the piece, a walkway on. Its
/// walls, the way (each entry's first walkway a start), and what each slot
/// shows.
fn catalogue() -> (Vec<Massif>, Vec<Massif>, Routing, Vec<String>) {
    const SLOT: f32 = 64.0;
    const GAP: f32 = 48.0;
    let v = level(300.0);
    let first = CENTRE.z - LENGTH * 0.5;
    // A massif on `side` from `z.0` to `z.1`: its line `shift` out into the
    // gap, its face `low` out from it below `split`, `high` above.
    let massif = |side: f32, z: (f32, f32), shift: f32, low: f32, split: f32, high: f32| {
        let origin = Vec3::new(CENTRE.x + side * (GAP * 0.5 - shift), 0.0, z.0);
        Massif { wall: Wall { origin, along: Vec3::Z, out: Vec3::new(-side, 0.0, 0.0), offset: 0.0 }, len: z.1 - z.0, z, split, low, high, room: 20.0, shaft: false }
    };
    let (mut near, mut far) = (Vec::new(), Vec::new());
    let mut r = Routing { ways: Vec::new(), legs: Vec::new(), places: Vec::new(), halls: Vec::new(), flights: Vec::new(), tunnels: Vec::new(), bridges: Vec::new(), spans: Vec::new(), start: [0.0; 5], length: 0.0 };
    let mut legend = Vec::new();
    let way = |side: f32, z: (f32, f32), v0: f32, width: f32, recess: f32| Walkway { side, z, v0, grade: 0.0, width, recess };
    let carved = |width: f32, yes: bool| if yes { width - 0.5 } else { 0.0 };
    // (Each entry starts afresh on a walkway: the walker jumps there.)
    let start = |r: &mut Routing, w: Walkway| {
        r.legs.push(Leg::Start(r.ways.len()));
        r.ways.push(w);
    };
    // A flight down from a walkway's end onto a walkway going on, on massif
    // `mi`, from `z` along the chasm, `width` wide.
    let flight = |r: &mut Routing, near: &[Massif], mi: usize, z: f32, width: f32, recess: f32| {
        let m = &near[mi];
        start(r, way(-1.0, (z - 16.0, z), v, width, recess));
        let (u, vb) = (z - m.z.0, v - 16.0 * RISER);
        r.legs.push(Leg::Down(r.flights.len(), r.ways.len()));
        r.flights.push(Flight { side: -1.0, m: mi, u: (u, u + 4.0), v: (v, vb), z: (z, z + 4.0), width, recess });
        r.ways.push(way(-1.0, (z + 4.0, z + 20.0), vb, width, recess));
    };
    for slot in 0..15 {
        let z0 = first + SLOT * slot as f32;
        far.push(massif(1.0, (z0, z0 + SLOT), 0.0, 0.0, HEIGHT, 0.0));
        let plain = |near: &mut Vec<Massif>| {
            near.push(massif(-1.0, (z0, z0 + SLOT), 0.0, 0.0, HEIGHT, 0.0));
            near.len() - 1
        };
        let what = match slot {
            0..=3 => {
                let (width, yes) = [(3.0, false), (3.0, true), (2.0, false), (4.0, true)][slot];
                let mi = plain(&mut near);
                flight(&mut r, &near, mi, z0 + 20.0, width, carved(width, yes));
                format!("a flight between walkways, {width:.0} m wide, {}", if yes { "carved" } else { "built out" })
            }
            4 | 5 => {
                let yes = slot == 5;
                let mi = plain(&mut near);
                let m = &near[mi];
                let (w, rec) = (3.0, carved(3.0, yes));
                start(&mut r, way(-1.0, (z0 + 4.0, z0 + 20.0), v, w, rec));
                // (A door in the walkway's middle; one run down, on the way
                // it goes, 40 steps; out onto a walkway going on.)
                let (ua, n) = (12.0, 40);
                let (us, vb) = (ua + w * 0.5, v - n as f32 * RISER);
                let ue = us + n as f32 * TREAD;
                let ux = ue + w * 0.5;
                let lane = m.face(v) - 5.0 - w * 0.5;
                r.legs.push(Leg::Through(r.tunnels.len(), r.ways.len()));
                r.tunnels.push(Tunnel { side: -1.0, m: mi, doors: [(ua, v), (ux, vb)], runs: vec![Run { u: (us, ue), v: (v, vb), n: lane }], width: w, entry: None });
                r.ways.push(way(-1.0, (z0 + ux - w * 0.5 - 0.5, z0 + 40.0), vb, w, rec));
                format!("a door into the rock, a stair down inside it, a door out; {}", if yes { "carved" } else { "built out" })
            }
            6 => {
                let mi = plain(&mut near);
                let m = &near[mi];
                let w = 3.0;
                start(&mut r, way(-1.0, (z0 + 4.0, z0 + 20.0), v, w, 0.0));
                // (Back and forth: two runs of 48 steps, in two lanes, the
                // second deeper; out where it began, below.)
                let (ua, n) = (12.0, 48);
                let (run, drop) = (n as f32 * TREAD, n as f32 * RISER);
                let face = m.face(v) - 5.0;
                let lanes = [face - w * 0.5, face - w * 1.5 - 1.5];
                let us = ua + w * 0.5;
                let runs = vec![Run { u: (us, us + run), v: (v, v - drop), n: lanes[0] }, Run { u: (us + run, us), v: (v - drop, v - 2.0 * drop), n: lanes[1] }];
                let (ux, vb) = (us - w * 0.5, v - 2.0 * drop);
                r.legs.push(Leg::Through(r.tunnels.len(), r.ways.len()));
                r.tunnels.push(Tunnel { side: -1.0, m: mi, doors: [(ua, v), (ux, vb)], runs, width: w, entry: None });
                r.ways.push(way(-1.0, (z0 + 2.0, z0 + ux + w * 0.5 + 0.5), vb, w, 0.0));
                "a door into the rock, a stair back and forth inside it, a door out below".into()
            }
            7 => {
                let mi = plain(&mut near);
                let m = &near[mi];
                let w = 3.0;
                start(&mut r, way(-1.0, (z0 + 4.0, z0 + 20.0), v, w, 0.0));
                let face = m.face(v);
                let (u0, u1) = (20.0, 46.0);
                let hall = (u0 + 3.0, u1 - 3.0);
                let p = Place { side: -1.0, m: mi, u: (u0, u1), z: (z0 + u0 - 1.0, z0 + u1 + 1.0), join: u0, v, out: face + 10.0, hall, back: face - 14.0, front: face - 1.8, height: 7.0, bays: 3 };
                // (Its stair from the door at the back of the hall, behind
                // it, one run on along; out onto a walkway going on.)
                let lane = (p.back - 3.0) - w * 0.5;
                let (ua, n) = (p.door(), 48);
                let us = ua + w * 0.5;
                let (ue, vb) = (us + n as f32 * TREAD, v - n as f32 * RISER);
                let ux = ue + w * 0.5;
                r.legs.push(Leg::Place(r.places.len(), r.tunnels.len(), r.ways.len()));
                r.places.push(p);
                r.tunnels.push(Tunnel { side: -1.0, m: mi, doors: [(ua, v), (ux, vb)], runs: vec![Run { u: (us, ue), v: (v, vb), n: lane }], width: w, entry: Some(p.back) });
                r.ways.push(way(-1.0, (z0 + ux - w * 0.5 - 0.5, z0 + SLOT - 4.0), vb, w, 0.0));
                "a place: a terrace, a hall behind its arcade, a stair down from the door at its back".into()
            }
            8 | 9 => {
                let yes = slot == 9;
                plain(&mut near);
                let (w, rec) = (3.0, carved(3.0, yes));
                start(&mut r, way(-1.0, (z0 + 4.0, z0 + 20.0), v, w, rec));
                let zb = z0 + 12.0;
                let edge = |ms: &[Massif]| {
                    let (m, u) = locate(ms, zb).unwrap();
                    m.wall.at(u, v, deck_n(m, v, w, rec).1 - 0.5)
                };
                let (a, b) = (edge(&near), edge(&far));
                let depth = (a.distance(b) / 18.0).clamp(2.5, 7.0);
                let (from, to) = (r.ways.len() - 1, r.ways.len());
                r.legs.push(Leg::Across(r.bridges.len(), to));
                r.bridges.push((zb, from, to));
                r.spans.push((a, b, depth));
                r.ways.push(way(1.0, (z0 + 8.0, z0 + 24.0), v, w, rec));
                format!("a bridge across to the far wall; {}", if yes { "carved" } else { "built out" })
            }
            10 => {
                // (The face stepping out 6 m, 8 m over the walkway.)
                near.push(massif(-1.0, (z0, z0 + SLOT), 0.0, 0.0, v + 8.0, 6.0));
                flight(&mut r, &near, near.len() - 1, z0 + 20.0, 3.0, 0.0);
                "a flight between walkways under an overhang".into()
            }
            11 => {
                // (The wall stepping out 4 m just past the walkway's end.)
                near.push(massif(-1.0, (z0, z0 + 40.0), 0.0, 0.0, HEIGHT, 0.0));
                near.push(massif(-1.0, (z0 + 40.0, z0 + SLOT), 4.0, 0.0, HEIGHT, 0.0));
                flight(&mut r, &near, near.len() - 2, z0 + 18.0, 3.0, 0.0);
                "a flight between walkways, the wall stepping out just past the walkway".into()
            }
            13 | 14 => {
                // (Entered at the walkway's end; four bays along, two deep.)
                let yes = slot == 14;
                let mi = plain(&mut near);
                let m = &near[mi];
                let (w, rec) = (3.0, carved(3.0, yes));
                start(&mut r, way(-1.0, (z0 + 4.0, z0 + 20.0), v, w, rec));
                let ua = 20.0 - (w * 0.5 + 1.0);
                let near_end = ua - BAY * 0.5;
                let far_end = near_end + BAY * 4.0;
                let front = (m.face(v) - 3.0).min(deck_n(m, v, w, rec).0 - 2.0);
                let h = Hall { side: -1.0, m: mi, u: (near_end, far_end), v, front, back: front - BAY * 2.0, height: 6.0, doors: (ua, far_end - BAY * 0.5), width: w };
                r.legs.push(Leg::Hall(r.halls.len(), r.ways.len()));
                r.halls.push(h);
                r.ways.push(way(-1.0, (z0 + h.doors.1 - w * 0.5 - 0.5, z0 + far_end + 12.0), v, w, rec));
                format!("a hall in the rock on a grid of pillars, entered from a walkway's end; {}", if yes { "carved" } else { "built out" })
            }
            _ => {
                // (Two massifs on one face line: their lines 2 m apart, their
                // faces out from them by as much.)
                near.push(massif(-1.0, (z0, z0 + 30.0), 0.0, 0.0, HEIGHT, 0.0));
                near.push(massif(-1.0, (z0 + 30.0, z0 + SLOT), -2.0, 2.0, HEIGHT, 2.0));
                flight(&mut r, &near, near.len() - 1, z0 + 36.0, 3.0, 0.0);
                "a walkway across a joint between two massifs on one face line".into()
            }
        };
        legend.push(format!("slot {slot} (z {z0:.0}-{:.0}): {what}", z0 + SLOT));
    }
    // (Where you start: out in the gap before the first slot, looking along
    // the near wall.)
    r.start = [CENTRE.x - 4.0, v + 3.0, first - 6.0, 160.0, -8.0];
    (near, far, r, legend)
}

/// A wall: its mass, less what is cut into it; and (but `bare`, as in the
/// junction catalogue, where only how the pieces meet is to be seen) what is
/// built on its faces, its columns, its cables.
fn wall(parts: &mut Parts, side: f32, seed: i32, massifs: &[Massif], solid: &Manifold, routing: &Routing, bare: bool) {
    parts.mark("wall");
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c1);
    let mut zones = routing.zones(side);
    zones.extend(routing.door_zones(side, massifs));

    // (Its mass is its solid, less what is cut into it: tunnels, halls.)
    let mut cuts: Vec<Manifold> = routing.tunnels.iter().filter(|t| t.side == side).map(|t| tunnel_cut(&massifs[t.m], t)).collect();
    cuts.extend(routing.ways.iter().filter(|w| w.side == side).filter_map(|w| way_cut(w, massifs)));
    cuts.extend(routing.flights.iter().filter(|f| f.side == side).filter_map(|f| flight_cut(&massifs[f.m], f)));
    cuts.extend(routing.places.iter().filter(|p| p.side == side).map(|p| place_cut(&massifs[p.m], p)));
    cuts.extend(routing.halls.iter().filter(|h| h.side == side).map(|h| hall_cut(&massifs[h.m], h)));
    parts.cuts = cuts
        .iter()
        .filter_map(|c| {
            let b = c.bounding_box()?;
            let (lo, hi) = (b.min(), b.max());
            Some((Vec3::new(lo[0] as f32, lo[1] as f32, lo[2] as f32), Vec3::new(hi[0] as f32, hi[1] as f32, hi[2] as f32), c.clone()))
        })
        .collect();
    for (m, ms) in massifs.iter().enumerate().filter(|_| !bare) {
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
        let (split, low, high) = (ms.split, ms.low, ms.high);
        for (part, (v0, v1, n)) in [(0.0, split, low), (split, HEIGHT, high)].into_iter().enumerate() {
            let w = nominal.moved(n);
            let k = seed + m * 31 + part as i32 * 7;
            let p = m * 2 + part as i32;
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
    if bare {
        parts.cuts.clear();
        return;
    }
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
        // (Half sunk, it reaches well into the rock: clear of what is cut
        // there.)
        let (x, y) = (b - a).normalize_or(Vec3::Y).any_orthonormal_pair();
        let ring: Vec<[f64; 3]> = (0..14)
            .flat_map(|i| {
                let t = i as f32 / 14.0 * std::f32::consts::TAU;
                let o = (x * t.cos() + y * t.sin()) * radius;
                [a + o, b + o]
            })
            .map(|p| [p.x as f64, p.y as f64, p.z as f64])
            .collect();
        let column = Manifold::hull_pts(&ring);
        if parts.cuts.iter().any(|(lo, hi, c)| (a.min(b) - radius).cmplt(*hi).all() && (a.max(b) + radius).cmpgt(*lo).all() && c.intersection(&column).volume() > 1e-3) {
            continue;
        }
        parts.stone.cylinder(a, b, radius, 14);
    }
    parts.cuts.clear();
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
    // (Flush where a route runs: no detail standing out there; tested as
    // far out as this piece stands, and what ends it: fins, frames, bays.)
    let blocked = parts.blocked_to((u0, u1), (v0, v1), n + 3.0);
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
        let want = (csize * STANDS[(h(7, k as i32) * 4.0) as usize % 4] * bold).min(6.0 * bold);
        let dn = if parts.blocked_to(cu, cv, n + want.max(3.0)) { 0.0 } else { want };
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
/// how wide, and how far it is set back into the rock (`recess`: 0 built
/// out from the face; carved into it, a gallery, all the way: its outer
/// edge flush with the face, under the rock's edge over it).
#[derive(Clone, Copy)]
struct Walkway {
    side: f32,
    z: (f32, f32),
    v0: f32,
    grade: f32,
    width: f32,
    recess: f32,
}

impl Walkway {
    fn v(&self, z: f32) -> f32 {
        self.v0 + self.grade * (z - self.z.0)
    }

    /// Whether it is a loggia: carved, wide enough for piers along its open
    /// front (they carry the rock over it).
    fn loggia(&self) -> bool {
        self.recess > 0.0 && self.width >= 3.0
    }
}

/// A loggia's piers: how far apart along the face (on the lattice, so the
/// piers of what meets line up), how wide, how deep in from its edge.
const PIERS: (f32, f32, f32) = (6.0, 1.2, 0.8);

/// How thick a walkway's shelf is at its outer edge.
const EDGE: f32 = 1.8;

/// Where a walkway's deck starts out from a massif's face at height `v`
/// (across a shaft, from the wall's line).
fn inner(m: &Massif, v: f32) -> f32 {
    if m.shaft { 0.0 } else { m.face(v) - 0.5 }
}

/// How high a carved way's opening is over its floor.
const GALLERY: f32 = 4.8;

/// Where a deck `width` wide lies across a massif's face at height `v`,
/// from its inner edge to its outer: out from the face; or `recess` further
/// back, carved into the rock.
fn deck_n(m: &Massif, v: f32, width: f32, recess: f32) -> (f32, f32) {
    let n0 = inner(m, v) - recess;
    (n0, n0 + width)
}

/// How high what a route takes along a wall reaches over its floor: its
/// headroom; or, carved, its opening and rock enough over it.
fn above(recess: f32) -> f32 {
    if recess > 0.0 { GALLERY + 1.5 } else { 3.5 }
}

/// How deep a place's terrace goes down beneath its floor (its slab and the
/// steps of its corbel back into the wall).
const CORBEL: f32 = 4.8;

/// A place on the way down, at the end of a walkway: a terrace on along the
/// face and out over the gap, its outer corners chamfered, on a corbel
/// stepping back into the wall; behind it a hall cut back into the rock,
/// wider than tall, opening onto the terrace through an arcade carved in
/// the rock of its front (an odd number of bays, the middle one before the
/// door); at the back of the hall a door, where the way goes on down inside
/// the rock (its tunnel). In its massif's frame: along the face from `u.0`
/// to `u.1` (`z`: where along the chasm; the walkway joining at `join`),
/// the floor's height, how far
/// out the terrace reaches (`out`, from the wall's line), the hall along
/// the face, its back and its front (behind the face by the arcade's
/// thickness), how high, how many bays.
#[derive(Clone, Copy)]
struct Place {
    side: f32,
    m: usize,
    u: (f32, f32),
    z: (f32, f32),
    join: f32,
    v: f32,
    out: f32,
    hall: (f32, f32),
    back: f32,
    front: f32,
    height: f32,
    bays: usize,
}

impl Place {
    /// The terrace's outline (along, out), convex: from the back of the hall
    /// out to its chamfered edge; half a metre on past the walkway's end
    /// (over it: they overlap, never just touch).
    fn outline(&self, m: &Massif, inset: f32, out: f32, back: f32) -> Vec<(f32, f32)> {
        let face = m.face(self.v);
        let c = (out - face - 4.0).clamp(0.0, 4.0);
        let over = |u: f32| if (u - self.join).abs() < 0.1 { 0.5 } else { -inset };
        let (a, b) = (self.u.0 - over(self.u.0), self.u.1 + over(self.u.1));
        vec![(a, back), (a, out - c), (a + c, out), (b - c, out), (b, out - c), (b, back)]
    }

    /// Where the door at the back of the hall is, along the face.
    fn door(&self) -> f32 {
        ((self.hall.0 + self.hall.1) * 0.5).round()
    }

    /// The bays of the arcade: each opening from where to where along the
    /// face, and the height its arch springs from.
    fn bays(&self) -> Vec<(f32, f32, f32)> {
        let c = self.door();
        let half = ((self.hall.1 - self.hall.0) * 0.5).min(c - self.hall.0).min(self.hall.1 - c);
        let bay = 2.0 * half / self.bays as f32;
        (0..self.bays)
            .map(|i| {
                let b0 = c - half + bay * i as f32;
                let open = bay - 1.8;
                (b0 + 0.9, b0 + bay - 0.9, self.v + self.height - open * 0.5 - 1.0)
            })
            .collect()
    }
}

/// A hall in the rock, a step of the way: broad and low, its ceiling on a
/// grid of square pillars left in the rock; entered by a door at the end of
/// a walkway, crossed along the aisle behind its front wall, left by a door
/// at its far end onto a walkway going on. In its massif's frame: from `u.0`
/// to `u.1` along the face, its floor's height, its front (behind the face by
/// the rock its doors pass through) and its back, how high; its doors along
/// the face (the way in, the way out); how wide they are.
#[derive(Clone, Copy)]
struct Hall {
    side: f32,
    m: usize,
    u: (f32, f32),
    v: f32,
    front: f32,
    back: f32,
    height: f32,
    doors: (f32, f32),
    width: f32,
}

/// The grid a hall's pillars stand on (m), and how thick they are.
const BAY: f32 = 6.0;
const PILLAR: f32 = 2.0;

impl Hall {
    /// Along the face, from its one end to the other.
    fn span(&self) -> (f32, f32) {
        (self.u.0.min(self.u.1), self.u.0.max(self.u.1))
    }

    /// Where its pillars stand (along, out): on its grid, inside it.
    fn pillars(&self) -> Vec<(f32, f32)> {
        let (a, b) = self.span();
        let along = ((b - a) / BAY).round() as i32;
        let deep = ((self.front - self.back) / BAY).round() as i32;
        (1..along).flat_map(|i| (1..deep).map(move |j| (a + BAY * i as f32, self.front - BAY * j as f32))).collect()
    }

    /// The middle of the aisle behind its front wall, before the first
    /// row of pillars.
    fn aisle(&self) -> f32 {
        self.front - (BAY - PILLAR * 0.5) * 0.5
    }
}

/// A randomized H-tree in a rectangle (`a` to `b`, in a wall's plane):
/// from a level trunk across its middle, each end branching square to it at
/// 1/√2 its length, and so on down (three levels), but some branches cut
/// off. Its segments, each with how deep in the tree it is. (`key` picks
/// which are cut; `half`, how wide a groove is each side of a segment, by
/// depth: a branch whose groove would come within a hand's breadth of any
/// but its parent's is cut off too, or a sliver of rock is left between.)
fn h_tree(a: Vec2, b: Vec2, key: i32, half: impl Fn(u32) -> f32) -> Vec<(Vec2, Vec2, u32)> {
    // (The whole tree, branches on both sides, spans 1.75 times its trunk
    // across and 1.24 times it up: its trunk as long as fits.)
    let size = b - a;
    let trunk = (size.x / 1.75).min(size.y / 1.24);
    let mut out: Vec<(Vec2, Vec2, u32)> = Vec::new();
    let groove = |p: Vec2, q: Vec2, depth: u32| (p.min(q) - half(depth), p.max(q) + half(depth));
    let mut grow = vec![((a + b) * 0.5, trunk, true, 0u32, 1i32, usize::MAX)];
    while let Some((c, len, level, depth, id, parent)) = grow.pop() {
        let along = if level { Vec2::new(len * 0.5, 0.0) } else { Vec2::new(0.0, len * 0.5) };
        let (p, q) = (c - along, c + along);
        let (lo, hi) = groove(p, q, depth);
        let crowded = out.iter().enumerate().filter(|(i, _)| *i != parent).any(|(_, &(p2, q2, d2))| {
            let (lo2, hi2) = groove(p2, q2, d2);
            (lo - 0.15).cmplt(hi2).all() && (hi + 0.15).cmpgt(lo2).all()
        });
        if crowded {
            continue;
        }
        let me = out.len();
        out.push((p, q, depth));
        if depth < 2 && len > 0.8 {
            for (j, end) in [p, q].into_iter().enumerate() {
                let branch = id * 2 + j as i32;
                if depth == 0 || hash01(key, branch, depth as i32, 0x4a7) < 0.78 {
                    grow.push((end, len / std::f32::consts::SQRT_2, !level, depth + 1, branch, me));
                }
            }
        }
    }
    out
}

/// How thick the plates on a hall's walls are: how deep their grooves.
const PLATE: f32 = 0.25;

/// The plates on a hall's back wall and its two end walls (where they face
/// the walker across it), in the slots cut for them: each a stone plate as
/// tall as the hall, a pattern of randomized H-trees cut through it (so its
/// grooves are as deep as it is thick), the trees as tall as the wall, side
/// by side. Drawn in the plate's plane and cut there, then given its depth:
/// a surface on the hall's walls, never cut into the rock, so it cannot
/// leave a sliver of it, however its pattern goes.
fn hall_plates(m: &Massif, h: &Hall) -> Vec<Manifold> {
    let (a, b) = h.span();
    let key = (a * 10.0) as i32 + (h.v * 10.0) as i32 * 7919 + h.side as i32 * 104729;
    let rect = |x0: f32, x1: f32, y0: f32, y1: f32| CrossSection::from_polygons_with_fill_rule(&[vec![[x0 as f64, y0 as f64], [x1 as f64, y0 as f64], [x1 as f64, y1 as f64], [x0 as f64, y1 as f64]]], FillRule::NonZero);
    // (Right-handed frames: as `stair` keeps its own.)
    let s = if m.wall.along.dot(Vec3::Y.cross(m.wall.out)) > 0.0 { 1.0 } else { -1.0 };
    let t = if m.wall.out.dot(Vec3::Y.cross(m.wall.along)) > 0.0 { 1.0 } else { -1.0 };
    let mut out = Vec::new();
    // Each wall: its width (along it), and where its plate stands, given
    // its pattern.
    let walls: [(f32, f32); 3] = [(a, b), (h.back - PLATE, h.front), (h.back - PLATE, h.front)];
    for (k, (x0, x1)) in walls.into_iter().enumerate() {
        let (y0, y1) = (h.v + 0.5, h.v + h.height - 0.5);
        let cells = (((x1 - x0) / ((y1 - y0) * std::f32::consts::SQRT_2)).round() as i32).max(1);
        let half = |depth: u32| 0.15 * 0.85f32.powi(depth as i32);
        let mut grooves = Vec::new();
        for c in 0..cells {
            let (c0, c1) = (x0 + (x1 - x0) * c as f32 / cells as f32, x0 + (x1 - x0) * (c + 1) as f32 / cells as f32);
            for (p, q, depth) in h_tree(Vec2::new(c0 + 0.4, y0), Vec2::new(c1 - 0.4, y1), key + k as i32 * 31 + c * 977, half) {
                let (lo, hi) = (p.min(q) - half(depth), p.max(q) + half(depth));
                grooves.push(rect(lo.x, hi.x, lo.y, hi.y));
            }
        }
        let plate = rect(x0, x1, h.v - 0.3, h.v + h.height).difference(&CrossSection::batch_union(&grooves));
        out.push(match k {
            0 => placed(&across(&plate, (h.back - PLATE) * s, h.back * s), m.wall.origin, m.wall.along, m.wall.out * s),
            1 => placed(&across(&plate, (a - PLATE) * t, a * t), m.wall.origin, m.wall.out, m.wall.along * t),
            _ => placed(&across(&plate, b * t, (b + PLATE) * t), m.wall.origin, m.wall.out, m.wall.along * t),
        });
    }
    out
}

/// What is carved out of the wall for a hall: the hall less its pillars
/// (left standing in the rock, carrying it), with the slots for the plates
/// on its back and end walls (see `hall_plates`), and the passages of its
/// doors (from just into the hall to out past the face).
fn hall_cut(m: &Massif, h: &Hall) -> Manifold {
    let (w, (a, b)) = (h.width * 0.5, h.span());
    let hall = wbox(&m.wall, (a - PLATE, b + PLATE), (h.v - 0.3, h.v + h.height), (h.back - PLATE, h.front));
    let pillars: Vec<Manifold> = h.pillars().iter().map(|&(u, n)| wbox(&m.wall, (u - PILLAR * 0.5, u + PILLAR * 0.5), (h.v - 1.0, h.v + h.height + 1.0), (n - PILLAR * 0.5, n + PILLAR * 0.5))).collect();
    let mut cut = vec![hall.difference(&Manifold::batch_union(&pillars))];
    for u in [h.doors.0, h.doors.1] {
        cut.push(wbox(&m.wall, (u - w, u + w), (h.v - 0.3, h.v + 3.0), (h.front - 0.5, m.face(h.v) + 2.0)));
    }
    Manifold::batch_union(&cut)
}

/// A hall's part of the routes: its floor (round its pillars) and its
/// doors' floors, as one solid; a light in every other bay of its aisle
/// and in each door; the way through it, and the space a walker needs.
fn hall(parts: &mut Parts, m: &Massif, h: &Hall) -> Manifold {
    let (w, (a, b)) = (h.width * 0.5, h.span());
    let pillars: Vec<Manifold> = h.pillars().iter().map(|&(u, n)| wbox(&m.wall, (u - PILLAR * 0.5, u + PILLAR * 0.5), (h.v - 1.0, h.v + 1.0), (n - PILLAR * 0.5, n + PILLAR * 0.5))).collect();
    let mut solid = vec![wbox(&m.wall, (a, b), (h.v - 0.3, h.v), (h.back, h.front)).difference(&Manifold::batch_union(&pillars))];
    let inner_v = inner(m, h.v);
    for u in [h.doors.0, h.doors.1] {
        solid.push(wbox(&m.wall, (u - w, u + w), (h.v - 0.3, h.v), (h.front - 0.5, inner_v + 0.1)));
        parts.lights.push((m.wall.at(u, h.v + 2.6, (h.front + inner_v) * 0.5), 10.0, 0.004));
    }
    // (Dim, dimmer than a tunnel: a lamp in every other bay.)
    let bays = ((b - a) / BAY).round() as i32;
    for i in (0..bays).step_by(2) {
        parts.lights.push((m.wall.at(a + BAY * (i as f32 + 0.5), h.v + h.height - 1.0, h.aisle()), 20.0, 0.004));
    }
    parts.mark("carving");
    for plate in hall_plates(m, h) {
        parts.stone.solid(&plate);
    }
    parts.mark("routes");
    for c in Routing::hall_claims(m, h).iter().filter(|c| c.kind == Use::Space) {
        let (lo, hi) = (c.lo, c.hi);
        let corners: Vec<[f64; 3]> = (0..8).map(|i| [0, 1, 2].map(|k| if i >> k & 1 == 1 { hi[k] as f64 } else { lo[k] as f64 })).collect();
        parts.clearance.push((format!("hall at {:.0} m", h.v), Manifold::hull_pts(&corners)));
    }
    let at = |u: f32, n: f32| m.wall.at(u, h.v + 1.2, n);
    parts.paths.push((format!("hall at {:.0} m", h.v), vec![at(h.doors.0, inner_v + 1.0), at(h.doors.0, h.aisle()), at(h.doors.1, h.aisle()), at(h.doors.1, inner_v + 1.0)]));
    info!("the chasm: a hall at {:?} ({:.0} by {:.0} m, {:.0} m high, {} pillars)", m.wall.at((a + b) * 0.5, h.v, h.front), b - a, h.front - h.back, h.height, h.pillars().len());
    Manifold::batch_union(&solid)
}

/// A prism in a face's frame: a convex outline (along, out) from `v.0` up to
/// `v.1`.
fn prism(w: &Wall, outline: &[(f32, f32)], v: (f32, f32)) -> Manifold {
    let pts: Vec<[f64; 3]> = outline
        .iter()
        .flat_map(|&(u, n)| [v.0, v.1].map(|y| {
            let p = w.at(u, y, n);
            [p.x as f64, p.y as f64, p.z as f64]
        }))
        .collect();
    Manifold::hull_pts(&pts)
}

/// What is carved out of the wall for a place: the hall, and the arcade's
/// openings through its front (each a round arch, out past the face).
fn place_cut(m: &Massif, p: &Place) -> Manifold {
    let face = m.face(p.v);
    let mut cut = vec![wbox(&m.wall, p.hall, (p.v - 0.3, p.v + p.height), (p.back, p.front))];
    for (a, b, spring) in p.bays() {
        let (c, r) = ((a + b) * 0.5, (b - a) * 0.5);
        let mut pts = Vec::new();
        for n in [p.front - 0.5, face + 1.0] {
            let mut at = |u: f32, v: f32| {
                let q = m.wall.at(u, v, n);
                pts.push([q.x as f64, q.y as f64, q.z as f64]);
            };
            at(a, p.v - 0.3);
            at(b, p.v - 0.3);
            for i in 0..=16 {
                let t = std::f32::consts::PI * i as f32 / 16.0;
                at(c + r * t.cos(), spring + r * t.sin());
            }
        }
        cut.push(Manifold::hull_pts(&pts));
    }
    Manifold::batch_union(&cut)
}

/// The space a walker needs on a place: over its terrace (in from its
/// edge), in its hall.
fn place_space(m: &Massif, p: &Place) -> Manifold {
    let face = m.face(p.v);
    Manifold::batch_union(&[
        prism(&m.wall, &p.outline(m, 0.3, p.out - 0.3, face + 0.2), (p.v + 0.1, p.v + 2.4)),
        wbox(&m.wall, (p.hall.0 + 0.3, p.hall.1 - 0.3), (p.v + 0.1, p.v + 2.4), (p.back + 0.3, p.front - 0.3)),
    ])
}

/// A place's part of the routes: the terrace's slab (under the hall too, its
/// floor) and the corbel stepping back beneath it, as one solid; a lit line
/// along the terrace's edge and along the top of the arcade inside, a light
/// in the hall; the way across it to the door.
fn place(parts: &mut Parts, m: &Massif, p: &Place) -> Manifold {
    let face = m.face(p.v);
    let reach = p.out - face;
    let edge = p.outline(m, 0.0, p.out + 0.05, 0.0);
    for e in edge[1..5].windows(2) {
        parts.glow.solid(&lit_line(&m.wall, (e[0].0, p.v - 0.9, e[0].1), (e[1].0, p.v - 0.9, e[1].1)));
    }
    parts.glow.solid(&lit_line(&m.wall, (p.hall.0 + 0.5, p.v + p.height, p.front - 0.3), (p.hall.1 - 0.5, p.v + p.height, p.front - 0.3)));
    let c = p.door();
    parts.lights.push((m.wall.at(c, p.v + p.height - 1.5, (p.back + p.front) * 0.5), 40.0, 0.35));
    parts.lights.push((m.wall.at(c, p.v + 5.0, (face + p.out) * 0.5), 35.0, 0.12));
    parts.clearance.push((format!("place at {:.0} m", p.v), place_space(m, p)));
    let (n0, n1) = deck_n(m, p.v, 2.0, 0.0);
    parts.paths.push((format!("place at {:.0} m", p.v), vec![m.wall.at(p.join, p.v + 1.2, (n0 + n1) * 0.5), m.wall.at(c, p.v + 1.2, face + 1.5)]));
    info!("the chasm: a place at {:?} (along {:?}, out {:?}, {:.0} m wide, hall {:.0} m deep, {:.0} m high, terrace {:.0} m out)", m.wall.at(c, p.v, face), m.wall.along, m.wall.out, p.u.1 - p.u.0, face - p.back, p.height, reach);
    place_solid(m, p)
}

/// A place's terrace as built: its slab (under the hall too, its floor) and
/// the corbel stepping back beneath it, as one solid.
fn place_solid(m: &Massif, p: &Place) -> Manifold {
    let face = m.face(p.v);
    let reach = p.out - face;
    let mut solid = vec![prism(&m.wall, &p.outline(m, 0.0, p.out, p.back - 0.5), (p.v - 1.2, p.v))];
    for k in 1..4 {
        let out = p.out - reach * k as f32 / 4.0;
        solid.push(prism(&m.wall, &p.outline(m, k as f32, out, face - 3.0), (p.v - 1.2 * (k + 1) as f32, p.v - 1.2 * k as f32 + 0.05)));
    }
    Manifold::batch_union(&solid)
}

/// Lamps along the line from `a` to `b` (each along, up, out in a wall's
/// frame): a short lit line, `long` long, at every whole `every` metres along
/// the face (so the lamps of what meets line up), hanging just under it.
/// (The way between places is lit now and then, not all along; places are
/// lit all along, so they read as places.)
fn lamps(w: &Wall, a: (f32, f32, f32), b: (f32, f32, f32), every: f32, phase: f32, long: f32) -> Vec<Manifold> {
    let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
    let at = |u: f32| {
        let f = ((u - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        (u, a.1 + (b.1 - a.1) * f, a.2 + (b.2 - a.2) * f)
    };
    let mut out = Vec::new();
    let mut u = ((lo - phase) / every).ceil() * every + phase;
    while u + long <= hi {
        out.push(lit_line(w, at(u), at(u + long)));
        u += every;
    }
    out
}

/// A thin lit line from `a` to `b` (each along, up, out in a wall's frame),
/// hanging just under that height.
fn lit_line(w: &Wall, a: (f32, f32, f32), b: (f32, f32, f32)) -> Manifold {
    let mut pts = Vec::new();
    for (u, v, n) in [a, b] {
        for (dv, dn) in [(0.0, -0.08), (0.0, 0.08), (-0.06, -0.08), (-0.06, 0.08)] {
            let p = w.at(u, v + dv, n + dn);
            pts.push([p.x as f64, p.y as f64, p.z as f64]);
        }
    }
    Manifold::hull_pts(&pts)
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
/// wide, set back into the rock as far as the walkways it joins (`recess`);
/// `z`: where it runs along the chasm.
#[derive(Clone, Copy)]
struct Flight {
    side: f32,
    m: usize,
    u: (f32, f32),
    v: (f32, f32),
    z: (f32, f32),
    width: f32,
    recess: f32,
}

/// A way through a wall (massif `m` on wall `side`), all on the lattice: a
/// door (`doors.0`: where along the face, how high) and a corridor straight
/// in; a stair down inside the rock in runs, parallel to the face, each
/// along a lane `n` from the wall's line (one run; or, back and forth,
/// alternate runs in two lanes, one deeper, with a landing across both at
/// every turn); a corridor back out to a door (`doors.1`), onto a walkway
/// going on from there. `width` wide throughout. (`entry`: where its first
/// corridor starts, if not at the face: the back of a place's hall.)
#[derive(Clone)]
struct Tunnel {
    side: f32,
    m: usize,
    doors: [(f32, f32); 2],
    runs: Vec<Run>,
    width: f32,
    entry: Option<f32>,
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
        // (From out in front of the door, where nothing may stand.)
        let mut out = vec![p(ua, va, inner(m, va) + 1.5), p(ua, va, na)];
        let landings = self.landings();
        for (k, r) in self.runs.iter().enumerate() {
            out.extend([p(r.u.0, r.v.0, r.n), p(r.u.1, r.v.1, r.n)]);
            if let (Some(&(lu, lv, _)), Some(next)) = (landings.get(k), self.runs.get(k + 1)) {
                out.extend([p(lu, lv, r.n), p(lu, lv, next.n)]);
            }
        }
        out.extend([p(ux, vx, nx), p(ux, vx, inner(m, vx) + 1.5)]);
        out
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
    let (below, above) = (0.3, h - 0.3);
    let mut cut = Vec::new();
    for (u, v, n) in t.corridors() {
        cut.push(wbox(&m.wall, (u - w, u + w), (v - below, v + above), (n - w, m.face(v) + 2.0)));
    }
    for r in &t.runs {
        let d = (r.u.1 - r.u.0).signum();
        let mut pts = Vec::new();
        for (u, v) in [(r.u.0 - d * w, r.v.0), (r.u.0, r.v.0), (r.u.1, r.v.1), (r.u.1 + d * w, r.v.1)] {
            for y in [v - below, v + above] {
                for n in [r.n - w, r.n + w] {
                    let p = m.wall.at(u, y, n);
                    pts.push([p.x as f64, p.y as f64, p.z as f64]);
                }
            }
        }
        // (The ceiling level a while on into the run before it slopes: a
        // walker stays at the floor's height until clear of it, reaching
        // well out over the stair.)
        for n in [r.n - w, r.n + w] {
            let p = m.wall.at(r.u.0 + d * HEAD, r.v.0 + above, n);
            pts.push([p.x as f64, p.y as f64, p.z as f64]);
        }
        cut.push(Manifold::hull_pts(&pts));
    }
    for (u, v, n) in t.landings() {
        cut.push(wbox(&m.wall, (u - w - 0.3, u + w + 0.3), (v - below, v + above), n));
    }
    Manifold::batch_union(&cut)
}

/// What a tunnel is built of: the floors of its corridors and landings, and
/// each run's steps and slope to walk on; and the space a walker needs over
/// each (named, for the checks).
struct TunnelParts {
    floors: Vec<Manifold>,
    runs: Vec<(Manifold, Manifold)>,
    spaces: Vec<(String, Manifold)>,
}

fn tunnel_parts(m: &Massif, t: &Tunnel) -> TunnelParts {
    let w = t.width * 0.5;
    let mut out = TunnelParts { floors: Vec::new(), runs: Vec::new(), spaces: Vec::new() };
    for (u, v, n) in t.corridors() {
        out.floors.push(wbox(&m.wall, (u - w, u + w), (v - 0.3, v), (n - w, inner(m, v) + 0.1)));
        out.spaces.push((format!("tunnel door at {v:.0} m"), wbox(&m.wall, (u - w + 0.2, u + w - 0.2), (v + 0.1, v + 2.4), (n, inner(m, v) + 0.5))));
    }
    for r in &t.runs {
        let (steps, slope, clear) = stair(m, r.u, r.v, (r.n - w + 0.2, r.n + w), None);
        out.runs.push((steps, slope));
        out.spaces.push((format!("tunnel stair at {:.0} m", r.v.0), clear));
        let d = (r.u.1 - r.u.0).signum();
        out.spaces.push((format!("the head of the tunnel stair at {:.0} m", r.v.0), wbox(&m.wall, (r.u.0.min(r.u.0 + d * (HEAD - 0.6)), r.u.0.max(r.u.0 + d * (HEAD - 0.6))), (r.v.0 + 0.1, r.v.0 + 2.0), (r.n - w + 0.3, r.n + w - 0.3))));
    }
    for (u, v, n) in t.landings() {
        out.floors.push(wbox(&m.wall, (u - w - 0.3, u + w + 0.3), (v - 0.3, v), n));
        out.spaces.push((format!("tunnel landing at {v:.0} m"), wbox(&m.wall, (u - w + 0.2, u + w - 0.2), (v + 0.1, v + 2.4), (n.0 + 0.2, n.1 - 0.2))));
    }
    out
}




/// How far on into a tunnel's run its ceiling stays level, and the space a
/// walker needs there at the floor's height before it drops onto the stair.
const HEAD: f32 = 2.5;

/// The routes, planned before anything is built (on the walls' shapes), on
/// the lattice: a way down from the rim where you start, as a chain of
/// human-scale steps. A short flight (12-24 steps) on down along the wall
/// from the end of the walkway it is on, onto a walkway going on from its
/// foot; now and then a door into the wall, a stair down inside it and a
/// door out lower down (the way past the walls' overhangs); now and then a
/// bridge across to the other wall, onto a walkway there; now and then a
/// place, its way on through a door at the back of its hall; and so on
/// down, each step clear of the walls (their overhangs, headroom) and of
/// every other; taking steps back when nothing fits. Stretches between
/// tunnels, bridges and places built out from the face, or carved into it.
/// And where you start.
struct Routing {
    ways: Vec<Walkway>,
    /// The steps of the way, in order, each with the walkway it comes out
    /// on.
    legs: Vec<Leg>,
    places: Vec<Place>,
    halls: Vec<Hall>,
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

/// A step of the way down, by index into what the routing holds, and the
/// walkway it comes out on: a flight; a tunnel; a place and its tunnel; a
/// bridge (its span); or (in the junction catalogue) a walkway where a way
/// starts afresh.
#[derive(Clone, Copy)]
enum Leg {
    Start(usize),
    Hall(usize, usize),
    Down(usize, usize),
    Through(usize, usize),
    Place(usize, usize, usize),
    Across(usize, usize),
}

/// The way down as a walker follows it, at foot height, each point with
/// what it is (`--opt walkbot` walks it).
#[derive(Resource)]
pub struct Way(pub Vec<(Vec3, String)>);

/// Places to stand and a way to walk, straight into a wall, for checking that
/// the body stands still against it (`--opt wallbot`): what each is, where
/// to stand (feet), which way to walk.
#[derive(Resource)]
pub struct WallTests(pub Vec<(String, Vec3, Vec3)>);

/// Walls to walk into, from the way down as planned: a walkway's face; a
/// tunnel corridor's side, the other side close; a corridor's end, with a
/// side right by it (a corner); a face at the bottom of the chasm.
fn wall_tests(routing: &Routing, near: &[Massif], far: &[Massif]) -> Vec<(String, Vec3, Vec3)> {
    let walls = |side: f32| if side < 0.0 { near } else { far };
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).normalize_or_zero();
    let mut out = Vec::new();
    if let Some((w, f)) = routing.ways.iter().filter(|w| w.recess == 0.0).find_map(|w| {
        let faces = walkway_faces(w, walls(w.side));
        (faces.len() == 1 && faces[0].2 - faces[0].1 > 6.0).then(|| (w, faces[0]))
    }) {
        let m = &walls(w.side)[f.0];
        out.push((format!("a walkway at {:.0} m, into its face", w.v0), m.wall.at((f.1 + f.2) * 0.5, w.v0, (f.4 + f.5) * 0.5), flat(-m.wall.out)));
    }
    // (A tunnel whose door opens off a walkway built out from the face: off
    // a gallery, a little way in from the door is still the gallery.)
    let tunnel = routing.legs.iter().find_map(|l| match *l {
        Leg::Through(t, k) if k > 0 && routing.ways[k - 1].recess == 0.0 && routing.tunnels[t].width >= 3.0 => Some(&routing.tunnels[t]),
        _ => None,
    });
    if let Some(t) = tunnel {
        let m = &walls(t.side)[t.m];
        let [(ua, va, na), _] = t.corridors();
        let (w, d) = (t.width * 0.5, (t.runs[0].u.1 - t.runs[0].u.0).signum());
        let along = flat(m.wall.along);
        out.push((format!("a tunnel corridor at {va:.0} m, into its side"), m.wall.at(ua, va, inner(m, va) - 1.5), along * -d));
        out.push((format!("a tunnel corridor at {va:.0} m, into its end by its side"), m.wall.at(ua - d * (w - 0.6), va, na + 0.5), flat(-m.wall.out)));
    }
    if let Some(m) = near.iter().filter(|m| !m.shaft).nth(1) {
        out.push(("a face at the bottom".into(), m.wall.at(m.len * 0.5, 3.0, m.face(3.0) + 2.5), flat(-m.wall.out)));
    }
    out
}


/// A flight as it is built: its steps (as seen), the slope walked on, the
/// space a walker needs over it.
fn flight_parts(m: &Massif, f: &Flight) -> (Manifold, Manifold, Manifold) {
    let (n0, n1) = deck_n(m, f.v.0, f.width, f.recess);
    stair(m, f.u, f.v, (n0, n1), Some((m.face(f.v.0) - 1.5).min(n0 - 1.0)))
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
/// (clear of the step at its split, with room), so what runs along it there
/// can keep to one line.
fn plain(m: &Massif, v: (f32, f32)) -> bool {
    m.shaft || v.1 < m.split - 1.0 || v.0 > m.split + 0.5
}

impl Routing {
    fn plan(seed: i32, near: &[Massif], far: &[Massif]) -> Routing {
        let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d3);
        let walls = |side: f32| if side < 0.0 { near } else { far };
        // Whether a piece's boxes fit the table (see `claims`): with the
        // rock and every piece already there, but where it joins another
        // (`joins`: each with its port) and, for what is carved, near those
        // a step further along (`before`).
        let rockf = |lo: Vec3, hi: Vec3| rock(near, far, lo, hi);
        let fits = |table: &Table, owner: usize, claims: &[Claim], joins: &[(usize, Claim)], before: &[usize]| table.misfit(owner, claims, joins, before, false, &rockf).is_none();
        let (lo, hi) = (CENTRE.z - LENGTH * 0.5 + 10.0, CENTRE.z + LENGTH * 0.5 - 10.0);
        let to_z = |m: &Massif, u: f32| m.z.0 + u / m.len * (m.z.1 - m.z.0);
        // A point along the chasm put on its face's lattice (a whole metre
        // along it).
        let snap = |side: f32, z: f32| locate(walls(side), z).map(|(m, u)| to_z(m, u.round()));
        // A walkway's outer edge, at deck height.
        let edge = |side: f32, z: f32, v: f32, width: f32, recess: f32| {
            let (m, u) = locate(walls(side), z)?;
            Some(m.wall.at(u, v, deck_n(m, v, width, recess).1 - 0.5))
        };
        // How far a stretch starting afresh (after a tunnel, a bridge) is
        // set back into the rock: mostly built out; now and then carved, all
        // the way back, flush with the face. (Part way, its floor ran on out
        // past the rock's edge over it, and a loggia's piers stood out in
        // front of it.)
        let pick = |k: i32, width: f32| if width >= 2.0 && r(k, 16) < 0.4 { width - 0.5 } else { 0.0 };
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
        // (within the chasm, on faces plain at its height, in the table as
        // piece `owner`, joining `joins` and a step on from `before`).
        let walkway_at = |table: &Table, owner: usize, joins: &[(usize, Claim)], before: &[usize], side: f32, z0: f32, v: f32, dir: f32, length: f32, width: f32, recess: f32| {
            let mut far = snap(side, (z0 + dir * length).clamp(lo, hi))?;
            // (Never ending by a joint between faces: on past it, far enough
            // that what goes on from its end starts on the same face.)
            if let Some(c) = walls(side).windows(2).map(|p| p[0].z.1).find(|c| (far - c).abs() < 1.5) {
                far = snap(side, (c + dir * 1.5).clamp(lo, hi))?;
            }
            // (On the way it goes, 4 m at least: held back at the chasm's
            // end, it would turn back under what came down to it.)
            if (far - z0) * dir < 4.0 {
                return None;
            }
            let (a, b) = (z0.min(far), z0.max(far));
            let w = Walkway { side, z: (a, b), v0: v, grade: 0.0, width, recess };
            // (The faces it passes, each built on: one it would pass for
            // less than half a metre along the chasm gets no deck, and
            // leaves a hole.)
            let passes: Vec<&Massif> = walls(side).iter().filter(|m| m.z.1 > a && m.z.0 < b).collect();
            if passes.iter().any(|m| m.z.1.min(b) - m.z.0.max(a) < 0.5) || (recess > 0.0 && passes.iter().any(|m| m.shaft)) {
                return None;
            }
            let depth = shelf_depth(width);
            let plain_all = passes.iter().all(|m| plain(m, (v - depth - 1.0, v + above(recess))));
            // (Never round a step: from one face to the next only where they
            // are one line, measured in the world, each face's depth being
            // from its own line. Round a step, the two decks only touched
            // where they met.)
            let passable = passes.windows(2).all(|p| (p[1].wall.at(0.0, v, inner(p[1], v)) - p[0].wall.at(p[0].len, v, inner(p[0], v))).dot(p[0].wall.out).abs() < 0.01);
            if !(plain_all && passable && v > 30.0) {
                return None;
            }
            fits(table, owner, &Self::walkway_claims(&w, walls(side)), joins, before).then_some(w)
        };
        // A stair back and forth inside the rock from a door at `ua` along
        // massif `mi`'s face, at height `v`: first along `d0` (or back, if
        // there is more room that way), its runs as long as fit along the
        // face, in two lanes (`face` and deeper, the one deeper than the
        // other), down as far as it takes to a level where a walkway outside
        // fits, `w` wide; in the table as piece `owner` (its door joining
        // `start`, a step on from `before`), the walkway out as `exit`
        // (a step on from `exit_before`). Where it does not fit, its lanes
        // further back.
        let descend = |table: &Table, owner: usize, start: (usize, Claim), before: &[usize], exit: usize, exit_before: &[usize], side: f32, mi: usize, ua: f32, v: f32, d0: f32, face: f32, w: f32, k: i32, length: f32| {
            let m = &walls(side)[mi];
            let (lo, hi) = (w + 2.0, m.len - w - 2.0);
            if ua < lo || ua > hi {
                return None;
            }
            let room = |d: f32| if d > 0.0 { hi - ua - w * 1.5 } else { ua - lo - w * 1.5 };
            let d0 = if room(d0) >= room(-d0) || room(d0) >= 12.0 { d0 } else { -d0 };
            let n = 4 * (12 + (r(k, 15) * 10.0) as i32).min((room(d0) / TREAD / 4.0) as i32);
            if n < 20 {
                return None;
            }
            let (run, drop) = (n as f32 * TREAD, n as f32 * RISER);
            let us = ua + d0 * w * 0.5;
            for back in [0.0, 8.0, 16.0, 24.0, 32.0] {
                let lanes = [face - back - w * 0.5, face - back - w * 1.5 - 1.5];
                if lanes[1] - w < -BACK + 6.0 {
                    return None;
                }
                let mut runs = Vec::new();
                let (mut u, mut vv, mut d) = (us, v, d0);
                for i in 0..40 {
                    let lane = lanes[i % 2];
                    runs.push(Run { u: (u, u + d * run), v: (vv, vv - drop), n: lane });
                    u += d * run;
                    vv -= drop;
                    if vv < 35.0 {
                        return None;
                    }
                    // Out here, if a walkway fits below the door.
                    let ux = u + d * w * 0.5;
                    let out = Self::port(m, ux, vv, w * 0.5 + 1.0);
                    if plain(m, (vv - 0.5, vv + 3.5))
                        && let Some(start_z) = snap(side, to_z(m, ux - d * (w * 0.5 + 0.5)))
                        && let Some(next) = walkway_at(table, exit, &[(owner, out)], exit_before, side, start_z, vv, d, length.max(8.0), w, pick(k, w)).or_else(|| walkway_at(table, exit, &[(owner, out)], exit_before, side, start_z, vv, d, length.max(8.0), w, 0.0))
                    {
                        let t = Tunnel { side, m: mi, doors: [(ua, v), (ux, vv)], runs, width: w, entry: None };
                        if fits(table, owner, &Self::tunnel_claims(m, &t), &[start], before) {
                            return Some((t, next));
                        }
                        break;
                    }
                    d = -d;
                }
            }
            None
        };
        let mut ways: Vec<Walkway> = Vec::new();
        let mut places: Vec<Place> = Vec::new();
        let mut halls: Vec<Hall> = Vec::new();
        let mut legs: Vec<Leg> = Vec::new();
        // (Each place's terrace edge, for what crosses the gap to keep clear
        // of.)
        let mut edges: Vec<(Vec3, Vec3, f32)> = Vec::new();
        let mut flights: Vec<Flight> = Vec::new();
        let mut tunnels: Vec<Tunnel> = Vec::new();
        let mut bridges: Vec<(f32, usize, usize)> = Vec::new();
        let mut spans: Vec<(Vec3, Vec3, f32)> = Vec::new();
        // What every piece takes (see `claims`). (Each in it as a number:
        // walkway `k` as 3k, the pieces of the leg down onto it as 3k + 1
        // and 3k + 2.)
        let mut table = Table::default();
        // Where the way down has got to: which wall, where along the chasm
        // (on the lattice), how high, which way it goes on, how wide its
        // walkways are and how far set back, the walkway it is on, and how
        // many steps since it last crossed.
        let first = snap(-1.0, CENTRE.z).unwrap_or(CENTRE.z);
        let (mut side, mut z, mut v) = (-1.0f32, first, level(HEIGHT));
        let mut dir = if r(0, 0) < 0.5 { 1.0f32 } else { -1.0 };
        let mut width = 3.0f32;
        let mut recess = 0.0f32;
        let mut on: Option<usize> = None;
        let mut since = 0;
        let mut history = Vec::new();
        let mut salt = 0;
        let mut stuck = 0;
        while v > 40.0 && flights.len() < 400 {
            let i = flights.len() as i32 + bridges.len() as i32 * 1000;
            enum Step {
                Hall(Hall, Walkway),
                Down(Flight, Walkway),
                Through(Tunnel, Walkway),
                Place(Place, Tunnel, Walkway),
                Across(f32, Walkway, (Vec3, Vec3, f32)),
            }
            // (The numbers of what comes next in the table: the walkway, the
            // leg's pieces; of the walkway it is on and the leg onto that.)
            let next = ways.len();
            let (wn, l1, l2) = (3 * next, 3 * next + 1, 3 * next + 2);
            let here_id = on.map(|o| 3 * o);
            let before: Vec<usize> = on.map(|o| vec![3 * o + 1, 3 * o + 2]).unwrap_or_default();
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
                if let Some(o) = on
                    && t >= 18
                {
                    let here = ways[o];
                    let w = here.width.min(3.0);
                    // (On the deck, a metre in from its ends at least.)
                    if here.z.1 - here.z.0 < w + 2.0 {
                        continue;
                    }
                    let zd = here.z.0 + w * 0.5 + 1.0 + (here.z.1 - here.z.0 - w - 2.0) * r(k, 14);
                    let Some((m, ud)) = locate(walls(side), zd) else { continue };
                    let mi = walls(side).iter().position(|x| std::ptr::eq(x, m)).unwrap();
                    if m.shaft || !plain(m, (v - 0.5, v + 3.5)) {
                        continue;
                    }
                    let d0 = if r(k, 13) < 0.5 { 1.0f32 } else { -1.0 };
                    // (The lanes deep enough behind the face anywhere down to
                    // the bottom.)
                    let face = (35.0..=v + 4.0).step_by_f32(2.0).map(|x| m.face(x)).fold(f32::MAX, f32::min);
                    let door = (here_id.unwrap(), Self::port(m, ud.round(), v, w * 0.5 + 1.0));
                    let Some((t, next)) = descend(&table, l1, door, &before, wn, &[here_id.unwrap()], side, mi, ud.round(), v, d0, face - 5.0, w, k, length) else { continue };
                    found = Some(Step::Through(t, next));
                    break;
                }
                // A place at the end of the walkway now and then (well below
                // the rim, and the last place): a terrace on along the face,
                // a hall behind it, and the way on down from a door at its
                // back, inside the rock.
                if let Some(o) = on
                    && t < 12
                    && r(k, 30) < 0.15
                    && v < HEIGHT - 40.0
                    && places.last().is_none_or(|p: &Place| p.v - v > 70.0)
                {
                    let here = ways[o];
                    let w = here.width.min(3.0);
                    let Some((m, uj)) = locate(walls(side), z - dir * 0.01) else { continue };
                    let mi = walls(side).iter().position(|x| std::ptr::eq(x, m)).unwrap();
                    let join = uj.round();
                    let long = (24.0 + 16.0 * r(k, 31)).round();
                    let (u0, u1) = if dir > 0.0 { (join, join + long) } else { (join - long, join) };
                    let height = (6.0 + 3.0 * r(k, 33)).round();
                    if m.shaft || u0 < 3.0 || u1 > m.len - 3.0 || !plain(m, (v - CORBEL - 1.0, v + height + 3.0)) {
                        continue;
                    }
                    let face = m.face(v);
                    let reach = (8.0 + 6.0 * r(k, 34)).min(m.room - 2.0).round();
                    if reach < 6.0 {
                        continue;
                    }
                    let hall = (u0 + 3.0, u1 - 3.0);
                    let bays = ((hall.1 - hall.0) / 7.0).ceil() as usize;
                    let p = Place {
                        side,
                        m: mi,
                        u: (u0, u1),
                        z: (to_z(m, u0) - 1.0, to_z(m, u1) + 1.0),
                        join,
                        v,
                        out: face + reach,
                        hall,
                        back: face - (12.0 + 6.0 * r(k, 32)).round(),
                        front: face - 1.8,
                        height,
                        bays: bays + 1 - bays % 2,
                    };
                    let pc = Self::place_claims(m, &p);
                    if !fits(&table, l1, &pc, &[(here_id.unwrap(), Self::port(m, p.join, p.v, 2.0))], &before) {
                        continue;
                    }
                    let (a, b) = (m.wall.at(u0, v, p.out), m.wall.at(u1, v, p.out));
                    if spans.iter().chain(&edges).any(|s| segment_gap(a, b, s.0, s.1) < 8.0 + CORBEL.max(s.2)) {
                        continue;
                    }
                    // (Its stair behind the hall, deeper than its back, and
                    // than the face anywhere down to the bottom; it and what
                    // comes out lower down clear of the place, tried with the
                    // place in the table.)
                    let deep = (35.0..=v + 4.0).step_by_f32(2.0).map(|x| m.face(x)).fold(f32::MAX, f32::min) - 5.0;
                    let d0 = if r(k, 13) < 0.5 { 1.0f32 } else { -1.0 };
                    let mark = table.len();
                    table.add(l1, &pc);
                    let found_here = descend(&table, l2, (l1, Self::hall(m, &p, w * 0.5 + 1.0)), &[here_id.unwrap()], wn, &[l1], side, mi, p.door(), v, d0, (p.back - 3.0).min(deep), w, k, length);
                    table.truncate(mark);
                    let Some((mut t, next)) = found_here else { continue };
                    t.entry = Some(p.back);
                    found = Some(Step::Place(p, t, next));
                    break;
                }
                // A hall in the rock now and then, entered at the end of the
                // walkway, the way on along the face only through it: on its
                // grid of bays, broad and low; behind the face by the rock
                // its doors pass through (and behind a gallery by more).
                if let Some(o) = on
                    && t < 12
                    && r(k, 40) < 0.12
                    && halls.last().is_none_or(|h: &Hall| h.v - v > 40.0)
                {
                    let here = ways[o];
                    let w = here.width.min(3.0);
                    let Some((m, uj)) = locate(walls(side), z - dir * 0.01) else { continue };
                    let mi = walls(side).iter().position(|x| std::ptr::eq(x, m)).unwrap();
                    let (bays, deep, height) = (4 + (r(k, 41) * 3.0) as i32, 2 + (r(k, 42) * 2.0) as i32, (5.0 + 3.0 * r(k, 43)).round());
                    let ua = uj.round() - dir * (w * 0.5 + 1.0);
                    let near = ua - dir * BAY * 0.5;
                    let far = near + dir * BAY * bays as f32;
                    let front = (m.face(v) - 3.0).min(deck_n(m, v, here.width, here.recess).0 - 2.0);
                    let h = Hall { side, m: mi, u: (near.min(far), near.max(far)), v, front, back: front - BAY * deep as f32, height, doors: (ua, far - dir * BAY * 0.5), width: w };
                    // (Rock over it: its ceiling at the rim left a skin there.)
                    if m.shaft || h.u.0 < 3.0 || h.u.1 > m.len - 3.0 || h.back < -BACK + 6.0 || v + height + 4.0 > HEIGHT || !plain(m, (v - 1.0, v + height + 2.0)) {
                        continue;
                    }
                    let hc = Self::hall_claims(m, &h);
                    let wd = w * 0.5 + 1.0;
                    // (A room in the rock: rock round it from all but the
                    // walkways at its doors.)
                    if table.misfit(l1, &hc, &[(here_id.unwrap(), Self::port(m, h.doors.0, v, wd))], &[], true, &rockf).is_some() {
                        continue;
                    }
                    let Some(start) = snap(side, to_z(m, h.doors.1 - dir * (w * 0.5 + 0.5))) else { continue };
                    let mark = table.len();
                    table.solo(l1);
                    table.add(l1, &hc);
                    let out = [(l1, Self::port(m, h.doors.1, v, wd))];
                    let here = [here_id.unwrap()];
                    let next = walkway_at(&table, wn, &out, &here, side, start, v, dir, length.max(8.0), w, pick(k, w)).or_else(|| walkway_at(&table, wn, &out, &here, side, start, v, dir, length.max(8.0), w, 0.0));
                    table.truncate(mark);
                    if let Some(next) = next {
                        found = Some(Step::Hall(h, next));
                        break;
                    }
                }
                // Through the wall now and then (more often where the way
                // down the face is blocked).
                let through = on.is_some() && !cross && r(k, 9) < if t >= 6 { 0.6 } else { 0.12 };
                if through {
                    let here = ways[on.unwrap()];
                    let w = here.width.min(3.0);
                    // The door anywhere along the walkway, on its deck a metre
                    // in from its ends at least (what lies beyond it along
                    // the walkway is left a dead end).
                    if here.z.1 - here.z.0 < w + 2.0 {
                        continue;
                    }
                    let zd = here.z.0 + w * 0.5 + 1.0 + (here.z.1 - here.z.0 - w - 2.0) * r(k, 14);
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
                    let t = Tunnel { side, m: mi, doors: [(ua, v), (ux, vb)], runs: vec![Run { u: (us, ue), v: (v, vb), n: lane }], width: w, entry: None };
                    let wd = w * 0.5 + 1.0;
                    if !fits(&table, l1, &Self::tunnel_claims(m, &t), &[(here_id.unwrap(), Self::port(m, ua, v, wd))], &before) {
                        continue;
                    }
                    let Some(start) = snap(side, to_z(m, ux - t_dir * (w * 0.5 + 0.5))) else { continue };
                    let out = [(l1, Self::port(m, ux, vb, wd))];
                    let here = [here_id.unwrap()];
                    if let Some(next) = walkway_at(&table, wn, &out, &here, side, start, vb, t_dir, length.max(8.0), w, pick(k, w)).or_else(|| walkway_at(&table, wn, &out, &here, side, start, vb, t_dir, length.max(8.0), w, 0.0)) {
                        found = Some(Step::Through(t, next));
                        break;
                    }
                    continue;
                }
                if !cross {
                    // Down: a short flight on along the wall, onto a walkway
                    // going on from its foot.
                    let try_dir = dir;
                    let Some((m, ua)) = locate(walls(side), z + try_dir * 0.01) else { continue };
                    let mi = walls(side).iter().position(|x| std::ptr::eq(x, m)).unwrap();
                    let ua = ua.round();
                    let n = 4 * (3 + (r(k, 5) * 4.0) as i32);
                    let ub = ua + try_dir * n as f32 * TREAD;
                    let vb = v - n as f32 * RISER;
                    if m.shaft || ub < 0.5 || ub > m.len - 0.5 || !plain(m, (vb - 3.5, v + above(recess))) {
                        continue;
                    }
                    let f = Flight { side, m: mi, u: (ua, ub), v: (v, vb), z: (to_z(m, ua), to_z(m, ub)), width, recess };
                    let fc = Self::flight_claims(m, &f);
                    let top: Vec<(usize, Claim)> = here_id.map(|h| (h, Self::port(m, ua, v, 2.0))).into_iter().collect();
                    if !fits(&table, l1, &fc, &top, &before) {
                        continue;
                    }
                    // (And the walkway at its foot, tried with the flight in
                    // the table.)
                    let mark = table.len();
                    table.add(l1, &fc);
                    let here: Vec<usize> = here_id.into_iter().collect();
                    let w = walkway_at(&table, wn, &[(l1, Self::port(m, ub, vb, 2.0))], &here, side, f.z.1, vb, try_dir, length, width, recess);
                    table.truncate(mark);
                    if let Some(w) = w {
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
                    let to_recess = pick(k, to_width);
                    let (Some(a), Some(b)) = (edge(side, zb, v, width, recess), edge(other, zb, v, to_width, to_recess)) else { continue };
                    let depth = (a.distance(b) / 18.0).clamp(2.5, 7.0);
                    if !clear(&[&spans[..], &edges[..]].concat(), a, b, depth) {
                        continue;
                    }
                    let reach = width.max(to_width);
                    let bc = Self::bridge_claims(a, b, (width, to_width));
                    if !fits(&table, l1, &bc, &[(here_id.unwrap(), Self::bridge_port(a, reach))], &before) {
                        continue;
                    }
                    let to_dir = if r(k, 8) < 0.5 { 1.0 } else { -1.0 };
                    let Some(start) = snap(other, zb - to_dir * 4.0) else { continue };
                    let mark = table.len();
                    table.add(l1, &bc);
                    let w = walkway_at(&table, wn, &[(l1, Self::bridge_port(b, reach))], &[here_id.unwrap()], other, start, v, to_dir, length.max(12.0), to_width, to_recess);
                    table.truncate(mark);
                    if let Some(w) = w {
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
                    let (nt, sp, nf, nb, (nn, np, nl, nh), state) = history.pop().unwrap();
                    halls.truncate(nh);
                    table.truncate(nt);
                    spans.truncate(sp);
                    flights.truncate(nf);
                    bridges.truncate(nb);
                    tunnels.truncate(nn);
                    places.truncate(np);
                    legs.truncate(nl);
                    edges.truncate(np);
                    ways.pop();
                    (side, z, v, dir, width, recess, on, since) = state;
                }
                salt += 1;
                // (Back at the rim: start from somewhere else along it.)
                if history.is_empty() {
                    z = snap(-1.0, CENTRE.z + (r(salt, 20) - 0.5) * 300.0).unwrap_or(CENTRE.z);
                    dir = if r(salt, 21) < 0.5 { 1.0 } else { -1.0 };
                }
                continue;
            };
            history.push((table.len(), spans.len(), flights.len(), bridges.len(), (tunnels.len(), places.len(), legs.len(), halls.len()), (side, z, v, dir, width, recess, on, since)));
            stuck = stuck.saturating_sub(1);
            let w = match step {
                Step::Hall(h, w) => {
                    legs.push(Leg::Hall(halls.len(), ways.len()));
                    table.solo(l1);
                    table.add(l1, &Self::hall_claims(&walls(h.side)[h.m], &h));
                    halls.push(h);
                    width = w.width;
                    since += 1;
                    w
                }
                Step::Down(f, w) => {
                    legs.push(Leg::Down(flights.len(), ways.len()));
                    table.add(l1, &Self::flight_claims(&walls(f.side)[f.m], &f));
                    dir = (f.z.1 - f.z.0).signum();
                    flights.push(f);
                    since += 1;
                    w
                }
                Step::Through(t, w) => {
                    legs.push(Leg::Through(tunnels.len(), ways.len()));
                    table.add(l1, &Self::tunnel_claims(&walls(t.side)[t.m], &t));
                    let last = t.runs[t.runs.len() - 1];
                    dir = (last.u.1 - last.u.0).signum();
                    tunnels.push(t);
                    width = w.width;
                    since += 1;
                    w
                }
                Step::Place(p, t, w) => {
                    legs.push(Leg::Place(places.len(), tunnels.len(), ways.len()));
                    let m = &walls(p.side)[p.m];
                    table.add(l1, &Self::place_claims(m, &p));
                    table.add(l2, &Self::tunnel_claims(m, &t));
                    edges.push((m.wall.at(p.u.0, p.v, p.out), m.wall.at(p.u.1, p.v, p.out), CORBEL));
                    places.push(p);
                    let last = t.runs[t.runs.len() - 1];
                    dir = (last.u.1 - last.u.0).signum();
                    tunnels.push(t);
                    width = w.width;
                    since += 1;
                    w
                }
                Step::Across(zb, w, span) => {
                    legs.push(Leg::Across(bridges.len(), ways.len()));
                    table.add(l1, &Self::bridge_claims(span.0, span.1, (width, w.width)));
                    let (a, b) = (on.unwrap(), ways.len());
                    bridges.push(if side < 0.0 { (zb, a, b) } else { (zb, b, a) });
                    spans.push(span);
                    dir = if (w.z.1 - zb).abs() > (w.z.0 - zb).abs() { 1.0 } else { -1.0 };
                    width = w.width;
                    since = 0;
                    w
                }
            };
            table.add(wn, &Self::walkway_claims(&w, walls(w.side)));
            recess = w.recess;
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
                // (On the same face, well in from its ends and its edge: near
                // a corner, a point behind the flight could be on the next
                // face's rim or off it.)
                let eye = m.wall.at((f.u.0 - d * 5.0).clamp(2.0, m.len - 2.0), HEIGHT + 1.7, top - 3.0);
                let look = m.wall.at(f.u.0 + d * 12.0, HEIGHT - 6.0, top + 2.0) - eye;
                [eye.x, HEIGHT + 1.7, eye.z, (-look.x).atan2(-look.z).to_degrees(), look.y.atan2(Vec2::new(look.x, look.z).length()).to_degrees()]
            })
            .unwrap_or([face_x(&plan(-1.0, seed), CENTRE.z) - 5.0, HEIGHT + 1.7, CENTRE.z, -130.0, -38.0]);
        Routing { ways, legs, places, halls, flights, tunnels, bridges, spans, start, length }
    }

    /// The way down as one line a walker follows, at foot height: from where
    /// you start along the rim to the first flight, and on, step by step:
    /// along each walkway (round its corners) to where the next step leaves
    /// it; down a flight; through a door, along a tunnel and out of its other
    /// door; across a place's terrace and through its hall; over a bridge;
    /// and along the last walkway to its end. Each point with what it is.
    fn route(&self, near: &[Massif], far: &[Massif]) -> Vec<(Vec3, String)> {
        let walls = |side: f32| if side < 0.0 { near } else { far };
        let mut out: Vec<(Vec3, String)> = Vec::new();
        // Along walkway `k` from where the walker is to `to`, by its corners.
        let along = |out: &mut Vec<(Vec3, String)>, k: usize, to: Vec3, what: String| {
            let w = &self.ways[k];
            let ms = walls(w.side);
            let from = out.last().map_or(to, |p| p.0);
            let faces = walkway_faces(w, ms);
            let mid = |f: &(usize, f32, f32, (f32, f32), f32, f32), u: f32| ms[f.0].wall.at(u, w.v0, (f.4 + f.5) * 0.5);
            // (Where a point is along the walkway: face by face, and along
            // each face; not by where it is along the chasm, which on angled
            // faces differs from its deck's middle to its edge.)
            let at = |p: Vec3| {
                faces
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let u = (p - ms[f.0].wall.origin).dot(ms[f.0].wall.along);
                        let off = (f.1 - u).max(u - f.2).max(0.0);
                        (off, i as f32 * 1e4 + u.clamp(f.1, f.2) - f.1)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map_or(0.0, |x| x.1)
            };
            let (a, b) = (at(from), at(to));
            let mut corners: Vec<(f32, Vec3)> = faces
                .windows(2)
                .enumerate()
                .flat_map(|(i, p)| [(i as f32 * 1e4 + p[0].2 - p[0].1, mid(&p[0], p[0].2)), ((i + 1) as f32 * 1e4, mid(&p[1], p[1].1))])
                .filter(|c| c.0 > a.min(b) && c.0 < a.max(b))
                .collect();
            if b < a {
                corners.reverse();
            }
            out.extend(corners.into_iter().map(|c| (c.1, format!("a corner of the walkway at {:.1} m", w.v0))));
            out.push((to, what));
        };
        // Through a tunnel from its corridor in (its first point, out before
        // the door, left out: the walker comes to the door along the middle
        // of the deck it is on), out onto the middle of walkway `k`'s deck.
        let tunnel = |out: &mut Vec<(Vec3, String)>, t: &Tunnel, k: usize| {
            let m = &walls(t.side)[t.m];
            let path = t.path(m);
            for (i, p) in path.iter().enumerate().take(path.len() - 1).skip(1) {
                out.push((*p - Vec3::Y * 1.2, format!("the tunnel from {:.0} m, point {i}", t.doors[0].1)));
            }
            let (ux, vx) = t.doors[1];
            let (n0, n1) = deck_n(m, vx, self.ways[k].width, self.ways[k].recess);
            out.push((m.wall.at(ux, vx, (n0 + n1) * 0.5), format!("out of the tunnel from {:.0} m", t.doors[0].1)));
        };
        let mut on: Option<usize> = None;
        for leg in &self.legs {
            match *leg {
                Leg::Start(k) => {
                    let w = &self.ways[k];
                    let (m, u) = locate(walls(w.side), w.z.0 + 0.5).unwrap();
                    let (n0, n1) = deck_n(m, w.v0, w.width, w.recess);
                    out.push((m.wall.at(u, w.v0, (n0 + n1) * 0.5), format!("jump to the walkway at {:.0} m", w.v0)));
                    on = Some(k);
                }
                Leg::Hall(i, k) => {
                    let h = &self.halls[i];
                    let m = &walls(h.side)[h.m];
                    let at = |u: f32, n: f32| m.wall.at(u, h.v, n);
                    if let Some(w) = on {
                        let (n0, n1) = deck_n(m, h.v, self.ways[w].width, self.ways[w].recess);
                        along(&mut out, w, at(h.doors.0, (n0 + n1) * 0.5), format!("before the door of the hall at {:.0} m", h.v));
                    }
                    out.push((at(h.doors.0, h.aisle()), format!("in the hall at {:.0} m", h.v)));
                    out.push((at(h.doors.1, h.aisle()), format!("across the hall at {:.0} m", h.v)));
                    let (n0, n1) = deck_n(m, h.v, self.ways[k].width, self.ways[k].recess);
                    out.push((at(h.doors.1, (n0 + n1) * 0.5), format!("out of the hall at {:.0} m", h.v)));
                    on = Some(k);
                }
                Leg::Down(i, k) => {
                    let f = &self.flights[i];
                    let m = &walls(f.side)[f.m];
                    let (n0, n1) = deck_n(m, f.v.0, f.width, f.recess);
                    let n = (n0 + n1) * 0.5;
                    let top = m.wall.at(f.u.0, f.v.0, n);
                    let what = format!("the top of the flight at {:.1} m", f.v.0);
                    match on {
                        Some(w) => along(&mut out, w, top, what),
                        None => out.push((top, what)),
                    }
                    out.push((m.wall.at(f.u.1, f.v.1, n), format!("the foot of the flight at {:.1} m", f.v.0)));
                    on = Some(k);
                }
                Leg::Through(i, k) => {
                    let t = &self.tunnels[i];
                    let m = &walls(t.side)[t.m];
                    if let Some(w) = on {
                        let (ua, va) = t.doors[0];
                        let (n0, n1) = deck_n(m, va, self.ways[w].width, self.ways[w].recess);
                        along(&mut out, w, m.wall.at(ua, va, (n0 + n1) * 0.5), format!("before the door at {:.0} m", va));
                    }
                    tunnel(&mut out, t, k);
                    on = Some(k);
                }
                Leg::Place(i, j, k) => {
                    let p = &self.places[i];
                    let m = &walls(p.side)[p.m];
                    if let Some(w) = on {
                        let (n0, n1) = deck_n(m, p.v, self.ways[w].width, self.ways[w].recess);
                        along(&mut out, w, m.wall.at(p.join, p.v, (n0 + n1) * 0.5), format!("the edge of the place at {:.0} m", p.v));
                    }
                    // (Out onto the terrace first, then along to the arcade:
                    // from a carved walkway's end, straight there clips the
                    // end of its slot.)
                    let face = m.face(p.v);
                    let into = (p.door() - p.join).signum();
                    out.push((m.wall.at(p.join + into * 1.5, p.v, (face + p.out) * 0.5), format!("onto the terrace of the place at {:.0} m", p.v)));
                    out.push((m.wall.at(p.door(), p.v, face + 1.5), format!("before the arcade of the place at {:.0} m", p.v)));
                    tunnel(&mut out, &self.tunnels[j], k);
                    on = Some(k);
                }
                Leg::Across(i, k) => {
                    let s = &self.spans[i];
                    // (Along the middle of the deck to the bridge, then out
                    // onto it, and in again at its far end: a loggia's piers
                    // stand along its edge.)
                    let zb = self.bridges[i].0;
                    let middle = |w: usize, p: Vec3| {
                        let w = &self.ways[w];
                        locate(walls(w.side), zb).map_or(p, |(m, u)| {
                            let (n0, n1) = deck_n(m, w.v0, w.width, w.recess);
                            m.wall.at(u, w.v0, (n0 + n1) * 0.5)
                        })
                    };
                    if let Some(w) = on {
                        along(&mut out, w, middle(w, s.0), format!("by the bridge at {:.0} m", s.0.y));
                    }
                    out.push((s.0, format!("the start of the bridge at {:.0} m", s.0.y)));
                    out.push((s.1, format!("the end of the bridge at {:.0} m", s.0.y)));
                    out.push((middle(k, s.1), format!("off the bridge at {:.0} m", s.0.y)));
                    on = Some(k);
                }
            }
        }
        // (On along the last walkway to its far end.)
        if let Some(k) = on {
            let w = &self.ways[k];
            let ms = walls(w.side);
            let faces = walkway_faces(w, ms);
            if let (Some(a), Some(b), Some(here)) = (faces.first(), faces.last(), out.last().map(|p| p.0)) {
                let ends = [ms[a.0].wall.at(a.1 + 0.5, w.v0, (a.4 + a.5) * 0.5), ms[b.0].wall.at(b.2 - 0.5, w.v0, (b.4 + b.5) * 0.5)];
                let end = if ends[0].distance(here) > ends[1].distance(here) { ends[0] } else { ends[1] };
                along(&mut out, k, end, "the bottom".into());
            }
        }
        out
    }

    /// Where the routes run along a wall (with headroom): its walkways, its
    /// flights.
    fn zones(&self, side: f32) -> Vec<Zone> {
        let mut out = Vec::new();
        for w in self.ways.iter().filter(|w| w.side == side) {
            out.push(Zone { z: w.z, v: (w.v0 - shelf_depth(w.width) - 1.0, w.v0 + above(w.recess) + 0.5) });
        }
        out.extend(self.flight_zones(side));
        for p in self.places.iter().filter(|p| p.side == side) {
            out.push(Zone { z: p.z, v: (p.v - CORBEL - 1.0, p.v + p.height + 2.0) });
        }
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
            .map(|f| Zone { z: (f.z.0.min(f.z.1) - 1.0, f.z.0.max(f.z.1) + 1.0), v: (f.v.1 - 4.0, f.v.0 + above(f.recess) + 0.5) })
            .collect()
    }
}

/// Routes: the walkways (open where a bridge joins), bridges across, united
/// in one solid; the flights, bare, their steps to be seen and a slope
/// beneath them to walk on.
/// A box in a face's frame, for a use (see `claims`): on the grid, square to
/// the world.
fn wclaim(w: &Wall, u: (f32, f32), v: (f32, f32), n: (f32, f32), kind: Use) -> Claim {
    Claim::new(w.at(u.0, v.0, n.0), w.at(u.1, v.1, n.1), kind)
}

/// How much of a box is rock: each massif's mass (its lower part and its
/// upper, a shaft's slot), either wall's.
fn rock(near: &[Massif], far: &[Massif], lo: Vec3, hi: Vec3) -> f32 {
    let mut total = 0.0;
    for m in near.iter().chain(far).filter(|m| m.z.0 < hi.z && m.z.1 > lo.z) {
        let bands: &[(f32, f32, f32)] = &if m.shaft { vec![(0.0, HEIGHT, SHAFT_DEPTH)] } else { vec![(0.0, m.split, m.low), (m.split, HEIGHT, m.high)] };
        for &(v0, v1, face) in bands {
            let (a, b) = (m.wall.at(0.0, v0, -BACK), m.wall.at(m.len, v1, face));
            let (ra, rb) = (a.min(b), a.max(b));
            let d = (hi.min(rb) - lo.max(ra)).max(Vec3::ZERO);
            total += d.x * d.y * d.z;
        }
    }
    total
}

/// A stair's boxes, step by step (see `stair`): its mass under the treads
/// (and back to `back`, if it reaches back into the wall), the space over
/// its slope, and (`cut`, if carved: how far out past the face) what is
/// carved for it, from under its steps to `open` over them.
fn stair_claims(m: &Massif, u: (f32, f32), v: (f32, f32), n: (f32, f32), back: Option<f32>, cut: Option<(f32, f32)>) -> Vec<Claim> {
    let ((ua, ub), (va, vb)) = (u, v);
    let count = ((va - vb) / RISER).round().max(1.0) as i32;
    let (riser, tread) = ((va - vb) / count as f32, (ub - ua) / count as f32);
    let mut out = Vec::new();
    for i in 0..count {
        let (u0, u1) = (ua + tread * i as f32, ua + tread * (i + 1) as f32);
        let (high, top) = (va - riser * i as f32, va - riser * (i + 1) as f32);
        out.push(wclaim(&m.wall, (u0, u1), (top - 1.2 - riser, top), (n.0, n.1), Use::Solid));
        if let Some(back) = back {
            out.push(wclaim(&m.wall, (u0, u1), (top - 4.0, top - 1.0), (back, n.0 + 1.0), Use::Solid));
        }
        out.push(wclaim(&m.wall, (u0, u1), (top + 0.1, high + 2.4), (n.0 + 0.7, n.1 - 0.1), Use::Space));
        if let Some((outer, open)) = cut {
            out.push(wclaim(&m.wall, (u0, u1), (top - 0.3, high + open), (n.0, outer), Use::Cut));
        }
    }
    out
}

/// What each piece of the way takes, as boxes (see `claims`).
impl Routing {
    fn walkway_claims(w: &Walkway, massifs: &[Massif]) -> Vec<Claim> {
        let (depth, reach) = (shelf_depth(w.width), deck_reach(w, massifs));
        let e = if w.loggia() { PIERS.2 + 0.1 } else { 0.1 };
        let faces = walkway_faces(w, massifs);
        let mut out = Vec::new();
        for &(k, us, ue, _, n0, n1) in &faces {
            let m = &massifs[k];
            // (The underside slopes from the edge's thickness back to the
            // shelf's depth: in four steps, each as deep as its back.)
            for j in 0..4 {
                let (a, b) = (n1 - reach * (j + 1) as f32 / 4.0, n1 - reach * j as f32 / 4.0);
                out.push(wclaim(&m.wall, (us, ue), (w.v0 - (EDGE + (depth - EDGE) * (j + 1) as f32 / 4.0), w.v0), (a, b), Use::Solid));
            }
            out.push(wclaim(&m.wall, (us, ue), (w.v0 + 0.1, w.v0 + 2.4), (n0 + 0.6, n1 - e), Use::Space));
            // (Headroom over it: no other route's stone low over it.)
            out.push(wclaim(&m.wall, (us, ue), (w.v0 + 2.4, w.v0 + above(w.recess)), (n0, n1), Use::Keep));
            if w.recess > 0.0 {
                out.push(wclaim(&m.wall, (us, ue), (w.v0 - 0.3, w.v0 + GALLERY), (n0, m.face(w.v0) + 1.0), Use::Cut));
            }
        }
        out
    }

    fn flight_claims(m: &Massif, f: &Flight) -> Vec<Claim> {
        let (n0, n1) = deck_n(m, f.v.0, f.width, f.recess);
        let back = (m.face(f.v.0) - 1.5).min(n0 - 1.0);
        let mut out = stair_claims(m, f.u, f.v, (n0, n1), Some(back), (f.recess > 0.0).then_some((m.face(f.v.0) + 1.0, GALLERY)));
        // (Headroom over it, as over a walkway.)
        let (lo, hi) = (f.u.0.min(f.u.1), f.u.0.max(f.u.1));
        out.push(wclaim(&m.wall, (lo, hi), (f.v.1 + 2.4, f.v.0 + above(f.recess)), (n0, n1), Use::Keep));
        // (Carved: a little on past its ends, into the walkways'.)
        if f.recess > 0.0 {
            let d = (f.u.1 - f.u.0).signum() * 0.3;
            for (u, v) in [(f.u.0, f.v.0), (f.u.1, f.v.1)] {
                let a = if u == f.u.0 { u - d } else { u + d };
                out.push(wclaim(&m.wall, (a.min(u), a.max(u)), (v - 0.3, v + GALLERY), (n0, m.face(f.v.0) + 1.0), Use::Cut));
            }
        }
        out
    }

    fn tunnel_claims(m: &Massif, t: &Tunnel) -> Vec<Claim> {
        let w = t.width * 0.5;
        let mut out = Vec::new();
        for (u, v, n) in t.corridors() {
            // (Before the door, on the walkway it opens onto: its width and
            // half a metre more each side, 3.5 m high, out across the deck.)
            out.push(wclaim(&m.wall, (u - w - 0.5, u + w + 0.5), (v + 0.05, v + 3.5), (inner(m, v) + 0.6, inner(m, v) + 3.0), Use::Keep));
            out.push(wclaim(&m.wall, (u - w, u + w), (v - 0.3, v), (n - w, inner(m, v) + 0.1), Use::Solid));
            out.push(wclaim(&m.wall, (u - w + 0.2, u + w - 0.2), (v + 0.1, v + 2.4), (n, inner(m, v) + 0.5), Use::Space));
            out.push(wclaim(&m.wall, (u - w, u + w), (v - 0.3, v + 3.0), (n - w, m.face(v) + 2.0), Use::Cut));
        }
        for r in &t.runs {
            out.extend(stair_claims(m, r.u, r.v, (r.n - w + 0.2, r.n + w), None, None));
            // (Its cut a little wider than its steps: from the lane's one side
            // to the other, from under its steps to 3 m over them; reaching
            // on past its ends, level at its head.)
            let count = ((r.v.0 - r.v.1) / RISER).round().max(1.0) as i32;
            let (riser, tread) = ((r.v.0 - r.v.1) / count as f32, (r.u.1 - r.u.0) / count as f32);
            for i in 0..count {
                let (u0, u1) = (r.u.0 + tread * i as f32, r.u.0 + tread * (i + 1) as f32);
                let (high, top) = (r.v.0 - riser * i as f32, r.v.0 - riser * (i + 1) as f32);
                out.push(wclaim(&m.wall, (u0.min(u1), u0.max(u1)), (top - 0.3, high + 3.0), (r.n - w, r.n + w), Use::Cut));
            }
            let d = (r.u.1 - r.u.0).signum();
            let head = (r.u.0 - d * w, r.u.0 + d * HEAD);
            out.push(wclaim(&m.wall, (head.0.min(head.1), head.0.max(head.1)), (r.v.0 - 0.3, r.v.0 + 3.0), (r.n - w, r.n + w), Use::Cut));
            let foot = (r.u.1 - d * 0.3, r.u.1 + d * w);
            out.push(wclaim(&m.wall, (foot.0.min(foot.1), foot.0.max(foot.1)), (r.v.1 - 0.3, r.v.1 + 3.0), (r.n - w, r.n + w), Use::Cut));
            let h = (r.u.0, r.u.0 + d * (HEAD - 0.6));
            out.push(wclaim(&m.wall, (h.0.min(h.1), h.0.max(h.1)), (r.v.0 + 0.1, r.v.0 + 2.0), (r.n - w + 0.3, r.n + w - 0.3), Use::Space));
        }
        for (u, v, n) in t.landings() {
            out.push(wclaim(&m.wall, (u - w - 0.3, u + w + 0.3), (v - 0.3, v), n, Use::Solid));
            out.push(wclaim(&m.wall, (u - w + 0.2, u + w - 0.2), (v + 0.1, v + 2.4), (n.0 + 0.2, n.1 - 0.2), Use::Space));
            out.push(wclaim(&m.wall, (u - w - 0.3, u + w + 0.3), (v - 0.3, v + 3.0), n, Use::Cut));
        }
        out
    }

    fn place_claims(m: &Massif, p: &Place) -> Vec<Claim> {
        let face = m.face(p.v);
        let reach = p.out - face;
        let span = |o: &[(f32, f32)]| o.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q.0), b.max(q.0)));
        let mut out = Vec::new();
        let slab = p.outline(m, 0.0, p.out, p.back - 0.5);
        out.push(wclaim(&m.wall, span(&slab), (p.v - 1.2, p.v), (p.back - 0.5, p.out), Use::Solid));
        for k in 1..4 {
            let o = p.out - reach * k as f32 / 4.0;
            out.push(wclaim(&m.wall, span(&p.outline(m, k as f32, o, face - 3.0)), (p.v - 1.2 * (k + 1) as f32, p.v - 1.2 * k as f32 + 0.05), (face - 3.0, o), Use::Solid));
        }
        out.push(wclaim(&m.wall, span(&p.outline(m, 0.3, p.out - 0.3, face + 0.2)), (p.v + 0.1, p.v + 2.4), (face + 0.2, p.out - 0.3), Use::Space));
        out.push(wclaim(&m.wall, (p.hall.0 + 0.3, p.hall.1 - 0.3), (p.v + 0.1, p.v + 2.4), (p.back + 0.3, p.front - 0.3), Use::Space));
        out.push(wclaim(&m.wall, p.hall, (p.v - 0.3, p.v + p.height), (p.back, p.front), Use::Cut));
        // (Room round it: under its corbel and over its arcade.)
        out.push(wclaim(&m.wall, p.u, (p.v - CORBEL - 2.0, p.v + p.height + 2.0), (p.back, p.out), Use::Keep));
        for (a, b, spring) in p.bays() {
            out.push(wclaim(&m.wall, (a, b), (p.v - 0.3, spring + (b - a) * 0.5), (p.front - 0.5, face + 1.0), Use::Cut));
        }
        out
    }

    fn hall_claims(m: &Massif, h: &Hall) -> Vec<Claim> {
        let (w, (a, b)) = (h.width * 0.5, h.span());
        let inner_v = inner(m, h.v);
        let mut out = vec![
            wclaim(&m.wall, (a, b), (h.v - 0.3, h.v), (h.back, h.front), Use::Solid),
            // (With the grooves cut into its walls.)
            wclaim(&m.wall, (a - 0.3, b + 0.3), (h.v - 0.3, h.v + h.height), (h.back - 0.3, h.front), Use::Cut),
        ];
        // (Along the aisle, from door to door.)
        let (lo, hi) = (h.doors.0.min(h.doors.1), h.doors.0.max(h.doors.1));
        out.push(wclaim(&m.wall, (lo - w + 0.2, hi + w - 0.2), (h.v + 0.1, h.v + 2.4), (h.front - BAY + PILLAR * 0.5 + 0.3, h.front - 0.3), Use::Space));
        for u in [h.doors.0, h.doors.1] {
            out.push(wclaim(&m.wall, (u - w - 0.5, u + w + 0.5), (h.v + 0.05, h.v + 3.5), (inner_v + 0.6, inner_v + 3.0), Use::Keep));
            out.push(wclaim(&m.wall, (u - w, u + w), (h.v - 0.3, h.v), (h.front - 0.5, inner_v + 0.1), Use::Solid));
            out.push(wclaim(&m.wall, (u - w + 0.2, u + w - 0.2), (h.v + 0.1, h.v + 2.4), (h.front - 0.3, inner_v + 0.5), Use::Space));
            out.push(wclaim(&m.wall, (u - w, u + w), (h.v - 0.3, h.v + 3.0), (h.front - 0.5, m.face(h.v) + 2.0), Use::Cut));
        }
        out
    }

    /// A bridge from `a` to `b` (on the walkways' edges, at deck height),
    /// reaching back under them by `reach` (see `span_solid`).
    fn bridge_claims(a: Vec3, b: Vec3, reach: (f32, f32)) -> Vec<Claim> {
        let along = (b - a).normalize_or(Vec3::X);
        let side = Vec3::new(-along.z, 0.0, along.x) * 1.5;
        let depth = (a.distance(b) / 18.0).clamp(3.0, 7.0);
        let low = (depth - EDGE) * 0.5 + EDGE;
        let mut out = vec![Claim::new(a - side - Vec3::Y * depth, b + side, Use::Solid)];
        for (end, dir, r) in [(a, -along, reach.0), (b, along, reach.1)] {
            let (c, h) = (end - Vec3::Y * low, (depth - EDGE) * 0.5);
            out.push(Claim::new(c - side - Vec3::Y * h, c + dir * (r + 1.0) + side + Vec3::Y * h, Use::Solid));
        }
        let wide = Vec3::new(-along.z, 0.0, along.x) * 1.3;
        out.push(Claim::new(a + along * 0.6 + Vec3::Y * 0.1 - wide, b - along * 0.6 + Vec3::Y * 2.4 + wide, Use::Space));
        // (Room round it, along the chasm either side and under it.)
        let room = Vec3::new(-along.z, 0.0, along.x) * 6.0;
        out.push(Claim::new(a - room - Vec3::Y * (depth + 2.0), b + room + Vec3::Y * 3.5, Use::Keep));
        out
    }

    /// The box round where a bridge joins a walkway at `p` (its end): back
    /// under the deck as far as it reaches, `r`, and along the chasm as far
    /// as the room kept round it.
    fn bridge_port(p: Vec3, r: f32) -> Claim {
        Claim::new(p - Vec3::new(r + 3.0, 12.0, 7.0), p + Vec3::new(r + 3.0, 5.0, 7.0), Use::Keep)
    }

    /// The box round where a place joins its tunnel: the hall, as far as
    /// the tunnel's corridor runs in from its back (`w` past it).
    fn hall(m: &Massif, p: &Place, w: f32) -> Claim {
        wclaim(&m.wall, (p.hall.0 - w, p.hall.1 + w), (p.v - 8.0, p.v + p.height + 1.0), (-BACK, p.out + 1.0), Use::Keep)
    }

    /// The box round where two pieces join on a face at `u` along it,
    /// height `v`: as far back as either reaches into the rock (a deck's
    /// underside, a stair's mass), out past a deck, up over a gallery's
    /// opening and the rock kept round it, a few metres along.
    fn port(m: &Massif, u: f32, v: f32, along: f32) -> Claim {
        wclaim(&m.wall, (u - along, u + along), (v - 9.0, v + GALLERY + ROCK + 1.0), (-BACK, m.face(v) + 10.0), Use::Keep)
    }

    /// The way down's boxes, piece by piece, in order, each tested against
    /// the rock and what came before it but where it joins another: where
    /// they do not fit.
    fn misfits(&self, near: &[Massif], far: &[Massif]) -> Vec<String> {
        let walls = |side: f32| if side < 0.0 { near } else { far };
        let rock = |lo: Vec3, hi: Vec3| rock(near, far, lo, hi);
        let ways = self.ways.len();
        let mut table = Table::default();
        let mut out = Vec::new();
        for (i, leg) in self.legs.iter().enumerate() {
            let (a, b) = (ways + 2 * i, ways + 2 * i + 1);
            // Each piece of the leg, and where it joins: the walkway before
            // (`i - 1`), the walkway after (`i`), the other piece of the leg.
            let mut pieces: Vec<(usize, &str, Vec<Claim>, Vec<(usize, Claim)>)> = Vec::new();
            match *leg {
                Leg::Start(_) => {}
                Leg::Hall(h, _) => {
                    let h = &self.halls[h];
                    let m = &walls(h.side)[h.m];
                    let w = h.width * 0.5 + 1.0;
                    pieces.push((a, "hall", Self::hall_claims(m, h), vec![(i.wrapping_sub(1), Self::port(m, h.doors.0, h.v, w)), (i, Self::port(m, h.doors.1, h.v, w))]));
                }
                Leg::Down(f, _) => {
                    let f = &self.flights[f];
                    let m = &walls(f.side)[f.m];
                    pieces.push((a, "flight", Self::flight_claims(m, f), vec![(i.wrapping_sub(1), Self::port(m, f.u.0, f.v.0, 2.0)), (i, Self::port(m, f.u.1, f.v.1, 2.0))]));
                }
                Leg::Through(t, _) => {
                    let t = &self.tunnels[t];
                    let m = &walls(t.side)[t.m];
                    let w = t.width * 0.5 + 1.0;
                    pieces.push((a, "tunnel", Self::tunnel_claims(m, t), vec![(i.wrapping_sub(1), Self::port(m, t.doors[0].0, t.doors[0].1, w)), (i, Self::port(m, t.doors[1].0, t.doors[1].1, w))]));
                }
                Leg::Place(p, t, _) => {
                    let (p, t) = (&self.places[p], &self.tunnels[t]);
                    let m = &walls(p.side)[p.m];
                    let w = t.width * 0.5 + 1.0;
                    // (The place and its tunnel join through the hall: the
                    // tunnel's corridor runs in from its back.)
                    let hall = Self::hall(m, p, w);
                    pieces.push((a, "place", Self::place_claims(m, p), vec![(i.wrapping_sub(1), Self::port(m, p.join, p.v, 2.0)), (b, hall)]));
                    pieces.push((b, "tunnel", Self::tunnel_claims(m, t), vec![(a, hall), (i, Self::port(m, t.doors[1].0, t.doors[1].1, w))]));
                }
                Leg::Across(k, _) => {
                    // (From the walkway before to the one after; its ends
                    // reach back under their decks as far as they are wide,
                    // as built.)
                    let s = self.spans[k];
                    let reach = (self.ways[i - 1].width, self.ways[i].width);
                    let r = reach.0.max(reach.1);
                    pieces.push((a, "bridge", Self::bridge_claims(s.0, s.1, reach), vec![(i.wrapping_sub(1), Self::bridge_port(s.0, r)), (i, Self::bridge_port(s.1, r))]));
                }
            }
            // (The walkway after joins this leg's pieces, at their ports.)
            let w = &self.ways[i];
            let joins: Vec<(usize, Claim)> = pieces.iter().flat_map(|(o, _, _, f)| f.iter().filter(|(x, _)| *x == i).map(move |(_, port)| (*o, *port))).collect();
            pieces.push((i, "walkway", Self::walkway_claims(w, walls(w.side)), joins));
            for (owner, what, claims, friends) in pieces {
                // (Two along the way, walkway and leg by turns, they are
                // neighbours: they may carve close.)
                let leg = |j: usize| [ways + 2 * j, ways + 2 * j + 1];
                let neighbours = if owner < ways {
                    let mut n = vec![i + 1];
                    if i > 0 {
                        n.push(i - 1);
                    }
                    n
                } else {
                    let mut n = leg(i + 1).to_vec();
                    if i > 0 {
                        n.extend(leg(i - 1));
                    }
                    n
                };
                let solo = what == "hall";
                if solo {
                    table.solo(owner);
                }
                if let Some(why) = table.misfit(owner, &claims, &friends, &neighbours, solo, &rock) {
                    out.push(format!("{what} {owner} (leg {i}): {why}"));
                }
                table.add(owner, &claims);
            }
        }
        out
    }
}

fn routes(parts: &mut Parts, routing: &Routing, near: &[Massif], far: &[Massif]) {
    let massifs = |side: f32| if side < 0.0 { near } else { far };
    let ways = &routing.ways;
    let mut solids = Vec::new();
    for (k, w) in ways.iter().enumerate() {
        // (Where nothing may stand at its edge: where bridges join, and
        // before the doors of tunnels into and out of it.)
        let mut openings: Vec<(f32, f32)> = routing.bridges.iter().filter(|b| b.1 == k || b.2 == k).map(|b| (b.0 - 3.0, b.0 + 3.0)).collect();
        for t in routing.tunnels.iter().filter(|t| t.side == w.side) {
            let m = &massifs(t.side)[t.m];
            for (u, v) in t.doors {
                let z = m.z.0 + u / m.len * (m.z.1 - m.z.0);
                if (v - w.v0).abs() < 0.05 && z > w.z.0 - 2.0 && z < w.z.1 + 2.0 {
                    let half = t.width * 0.5 * (m.z.1 - m.z.0).abs() / m.len + 0.5;
                    openings.push((z - half, z + half));
                }
            }
        }
        for h in routing.halls.iter().filter(|h| h.side == w.side && (h.v - w.v0).abs() < 0.05) {
            let m = &massifs(h.side)[h.m];
            for u in [h.doors.0, h.doors.1] {
                let z = m.z.0 + u;
                if z > w.z.0 - 2.0 && z < w.z.1 + 2.0 {
                    openings.push((z - h.width * 0.5 - 0.5, z + h.width * 0.5 + 0.5));
                }
            }
        }
        let built = walkway(parts, w, massifs(w.side), &openings);
        solids.push(built);
    }
    for p in &routing.places {
        parts.mark("place");
        solids.push(place(parts, &massifs(p.side)[p.m], p));
        parts.mark("routes");
    }
    for h in &routing.halls {
        solids.push(hall(parts, &massifs(h.side)[h.m], h));
    }
    for &(z, i, j) in &routing.bridges {
        let (a, b) = (&ways[i], &ways[j]);
        let (Some((ma, ua)), Some((mb, ub))) = (locate(near, z), locate(far, z)) else { continue };
        let pa = ma.wall.at(ua, a.v0, deck_n(ma, a.v0, a.width, a.recess).1 - 0.5);
        let pb = mb.wall.at(ub, b.v0, deck_n(mb, b.v0, b.width, b.recess).1 - 0.5);
        solids.push(span(parts, pa, pb, 3.0, (a.width, b.width)));
        // (Its way across, and the space a walker needs over it.)
        let along = (pb - pa).normalize_or(Vec3::X);
        parts.paths.push((format!("the bridge at {:.0} m", pa.y), vec![pa + Vec3::Y * 1.2 + along * 0.6, pb + Vec3::Y * 1.2 - along * 0.6]));
        parts.clearance.push((format!("the bridge at {:.0} m", pa.y), span_space(pa, pb)));
        info!("the chasm: a bridge from {:?} to {:?}", pa, pb);
    }
    for t in &routing.tunnels {
        let m = &massifs(t.side)[t.m];
        let line = |a: (f32, f32, f32), b: (f32, f32, f32)| lit_line(&m.wall, a, b);
        let top = 3.0 - 0.31;
        let mut lines = Vec::new();
        // The corridors: their floors, the space over them, a lamp in the
        // ceiling just inside the door.
        let pieces = tunnel_parts(m, t);
        solids.extend(pieces.floors);
        for (steps, slope) in &pieces.runs {
            parts.steps.solid(steps);
            parts.slope(slope);
        }
        parts.clearance.extend(pieces.spaces);
        for (i, (u, v, n)) in t.corridors().into_iter().enumerate() {
            let mouth = if i == 0 { t.entry } else { None }.unwrap_or(inner(m, v) + 0.3);
            let inward = (n - mouth).signum();
            lines.push(line((u, v + top, mouth + inward * 1.0), (u, v + top, mouth + inward * 2.0)));
        }
        // The runs: steps, the slope to walk on, the space over them; lit
        // every 12 m, a lamp in the ceiling with each light.
        for r in &t.runs {
            let count = ((r.u.1 - r.u.0).abs() / 12.0).ceil().max(1.0) as i32;
            let d = (r.u.1 - r.u.0).signum() * 0.5;
            for i in 0..=count {
                let f = i as f32 / count as f32;
                let (u, v) = (r.u.0 + (r.u.1 - r.u.0) * f, r.v.0 + (r.v.1 - r.v.0) * f);
                parts.lights.push((m.wall.at(u, v + 2.6, r.n), 10.0, 0.01));
                let slope = (r.v.1 - r.v.0) / (r.u.1 - r.u.0);
                lines.push(line((u - d, v + top - d * slope, r.n), (u + d, v + top + d * slope, r.n)));
            }
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
    let all = Manifold::batch_union(&solids);
    parts.stone.solid(&all);
    parts.routes = Some(all);
    for f in &routing.flights {
        let m = &massifs(f.side)[f.m];
        let (n0, _) = deck_n(m, f.v.0, f.width, f.recess);
        let (steps, slope, clear) = flight_parts(m, f);
        if f.recess > 0.0 {
            parts.dim.solid(&Manifold::batch_union(&lamps(&m.wall, (f.u.0, f.v.0 + GALLERY, n0 + 0.4), (f.u.1, f.v.1 + GALLERY, n0 + 0.4), 8.0, 0.0, 1.0)));
        }
        parts.clearance.push((format!("flight at {:.0} m", f.v.0), clear));
        let n = n0 + f.width * 0.5 + 0.25;
        let d = (f.u.1 - f.u.0).signum() * 0.3;
        parts.paths.push((format!("flight at {:.0} m", f.v.0), vec![m.wall.at(f.u.0 + d, f.v.0 + 1.2, n), m.wall.at(f.u.1 - d, f.v.1 + 1.2, n)]));
        info!("the chasm: a flight from {:?} to {:?} (out {:?})", m.wall.at(f.u.0, f.v.0, n0 + f.width), m.wall.at(f.u.1, f.v.1, n0 + f.width), m.wall.out);
        parts.steps.solid(&steps);
        parts.slope(&slope);
    }
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
        let (n0, n1) = deck_n(m, (vs + ve) * 0.5, w.width, w.recess);
        faces.push((k, to_u(zs), to_u(ze), (vs, ve), n0, n1));
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
    // (In from the edge; a loggia's, from behind its piers.)
    let e = if w.loggia() { PIERS.2 + 0.1 } else { 0.1 };
    let mut walk = Vec::new();
    for (i, &(k, us, ue, _, n0, n1)) in faces.iter().enumerate() {
        walk.push(region(&[plan(k, us, n1 - e), plan(k, ue, n1 - e), plan(k, ue, n0 + 0.6), plan(k, us, n0 + 0.6)]));
        if let Some(&(k2, us2, _, _, n02, n12)) = faces.get(i + 1) {
            walk.push(region(&[plan(k, ue, n1 - e), plan(k, ue, n0 + 0.6), plan(k2, us2, n12 - e), plan(k2, us2, n02 + 0.6)]));
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

/// What is carved out of the wall for a walkway set back into it (nothing
/// for one built out): along each face it passes, from the deck's inner
/// edge out past the face, from under its deck to the opening's top; round
/// a corner, the hull of the two ends; ending where its deck ends (carved
/// past it, with no floor over it, a trench lay against the slot's end
/// wall; what goes on from an end reaches into it, never just touches).
fn way_cut(w: &Walkway, massifs: &[Massif]) -> Option<Manifold> {
    if w.recess <= 0.0 {
        return None;
    }
    let faces = walkway_faces(w, massifs);
    let plan = |k: usize, u: f32, n: f32| {
        let p = massifs[k].wall.at(u, 0.0, n);
        Vec2::new(p.x, p.z)
    };
    let out = |k: usize| massifs[k].face(w.v0) + 1.0;
    let mut areas = Vec::new();
    for (i, &(k, us, ue, _, n0, _)) in faces.iter().enumerate() {
        areas.push(region(&[plan(k, us, n0), plan(k, ue, n0), plan(k, ue, out(k)), plan(k, us, out(k))]));
        if let Some(&(k2, us2, _, _, n02, _)) = faces.get(i + 1) {
            areas.push(region(&[plan(k, ue, n0), plan(k, ue, out(k)), plan(k2, us2, n02), plan(k2, us2, out(k2))]));
        }
    }
    Some(upright(&raised(&CrossSection::batch_union(&areas), -0.3, GALLERY), w))
}

/// What is carved out of the wall for a flight set back into it (nothing
/// for one built out): from its inner edge out past the face, from under
/// its steps to the opening's top, parallel to it; a little on past its
/// ends, into the walkways' (overlapping them).
fn flight_cut(m: &Massif, f: &Flight) -> Option<Manifold> {
    if f.recess <= 0.0 {
        return None;
    }
    let ((ua, ub), (va, vb)) = (f.u, f.v);
    let d = (ub - ua).signum() * 0.3;
    let s = if m.wall.along.dot(Vec3::Y.cross(m.wall.out)) > 0.0 { 1.0 } else { -1.0 };
    let side = CrossSection::from_polygons_with_fill_rule(
        &[vec![[(ua - d) as f64, (va - 0.3) as f64], [(ub + d) as f64, (vb - 0.3) as f64], [(ub + d) as f64, (vb + GALLERY) as f64], [(ua - d) as f64, (va + GALLERY) as f64]]],
        FillRule::NonZero,
    );
    let (n0, _) = deck_n(m, va, f.width, f.recess);
    Some(placed(&across(&side, n0 * s, (m.face(va) + 1.0) * s), m.wall.origin, m.wall.along, m.wall.out * s))
}

/// A walkway's deck as it is built (see `walkway`): its plan, how far it
/// reaches back from its edge (as far as the wall lies behind it, at most,
/// on every face it passes), and its solid set in place: bare, nothing at
/// its edge, its underside sloping back into the wall.
fn deck(w: &Walkway, massifs: &[Massif]) -> Option<(CrossSection, f32, Manifold)> {
    let depth = shelf_depth(w.width);
    let faces = walkway_faces(w, massifs);
    if faces.is_empty() {
        return None;
    }
    let reach = deck_reach(w, massifs);
    let plan = |k: usize, u: f32, n: f32| {
        let p = massifs[k].wall.at(u, 0.0, n);
        Vec2::new(p.x, p.z)
    };
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
    let plan = CrossSection::batch_union(&areas);
    let solid = upright(&Manifold::batch_union(&[raised(&plan, -EDGE, 0.0), Manifold::batch_union(&under).intersection(&raised(&plan, -60.0, 0.0))]), w);
    Some((plan, reach, solid))
}

/// How far a walkway's deck reaches back from its outer edge: as far as the
/// wall lies behind it, at most, on every face it passes.
fn deck_reach(w: &Walkway, massifs: &[Massif]) -> f32 {
    let depth = shelf_depth(w.width);
    let mut reach = w.width + 1.0;
    for &(k, _, _, (vs, ve), n0, n1) in &walkway_faces(w, massifs) {
        let m = &massifs[k];
        let back = (vs.min(ve) - depth..=vs.max(ve)).step_by_f32(2.0).map(|v| inner(m, v)).fold(n0, f32::min) - 1.0;
        reach = reach.max(n1 - back);
    }
    reach
}

/// A walkway, built as one solid from its plan. The plan: along each face
/// it passes, a strip from its outer edge back into the wall; where two
/// faces meet, the hull of their ends (a bevel at an outer corner; at an
/// inner one, inside the strips' overlap). Taken as one
/// region, so however short a face or sharp a turn, the outline is simply
/// that region's edge. On it: the deck,
/// its underside sloping back into the wall (clipped to the plan); bare,
/// nothing at its edge; a lit line along the edge (broken at its ends,
/// where bridges join); lights below. All of it in the deck's
/// frame, united, then set on the walkway's slope: returned, to be united
/// with the rest of the routes.
fn walkway(parts: &mut Parts, w: &Walkway, massifs: &[Massif], openings: &[(f32, f32)]) -> Manifold {
    let depth = shelf_depth(w.width);
    let flat = |p: Vec3| Vec2::new(p.x, p.z);
    let faces = walkway_faces(w, massifs);
    let Some((deck, reach, mut built)) = deck(w, massifs) else {
        return Manifold::empty();
    };
    // A loggia's piers along its open front, from its deck up into the rock
    // over it, on the lattice; not near its ends, nor where a bridge joins.
    if w.loggia() {
        let (every, wide, deep) = PIERS;
        // (Nor where they would stand in its own walking space: near a
        // corner, the next face's stretch reaches across this one's front.)
        let space = walk_space(w, massifs);
        let mut piers = vec![built];
        for &(k, us, ue, (vs, _), _, n1) in &faces {
            let m = &massifs[k];
            let to_z = |u: f32| m.z.0 + u / m.len * (m.z.1 - m.z.0);
            let mut u = ((us + wide + 0.6) / every).ceil() * every;
            while u <= ue - wide - 0.6 {
                let pier = wbox(&m.wall, (u - wide * 0.5, u + wide * 0.5), (vs - 0.05, vs + GALLERY + 0.3), (n1 - deep, n1));
                if !openings.iter().any(|&(z0, z1)| (z0 - 1.5..=z1 + 1.5).contains(&to_z(u))) && pier.intersection(&space).volume() < 1e-3 {
                    piers.push(pier);
                }
                u += every;
            }
        }
        built = Manifold::batch_union(&piers);
    }
    let plan = |k: usize, u: f32, n: f32| flat(massifs[k].wall.at(u, 0.0, n));
    // A band across a face's strip, from `a` to `b` along it.
    let across = |k: usize, a: f32, b: f32, n1: f32| region(&[plan(k, a, n1 + 2.0), plan(k, b, n1 + 2.0), plan(k, b, n1 - reach - 2.0), plan(k, a, n1 - reach - 2.0)]);
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
    // Where the lit edge breaks: where bridges join, and the walkway's two
    // ends.
    let mut gaps: Vec<(usize, f32, f32)> = Vec::new();
    let n1_of = |k: usize| faces.iter().find(|f| f.0 == k).map_or(0.0, |f| f.5);
    let cut = |list: &[(usize, f32, f32)]| CrossSection::batch_union(&list.iter().map(|&(k, a, b)| across(k, a, b, n1_of(k))).collect::<Vec<_>>());
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
    // A lit mark on the edge every 10 m along each face.
    let marks: Vec<(usize, f32, f32)> = faces
        .iter()
        .flat_map(|&(k, us, ue, ..)| {
            let first = (us / 10.0).ceil() as i32;
            let last = ((ue - 0.8) / 10.0).floor() as i32;
            (first..=last).map(move |i| (k, i as f32 * 10.0, i as f32 * 10.0 + 0.8))
        })
        .collect();
    let line = deck.offset(0.04, JoinType::Miter, 4.0, 0).difference(&deck).difference(&open).intersection(&cut(&marks));
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
    // Per face: lights below; carved, a line along the ceiling at the back.
    for &(k, us, ue, (vs, _), n0, n1) in &faces {
        let m = &massifs[k];
        if w.recess > 0.0 && ue - us > 1.0 {
            // (A loggia's between its piers.)
            let (every, phase) = if w.loggia() { (PIERS.0, PIERS.0 * 0.5 - 0.5) } else { (8.0, 0.0) };
            parts.dim.solid(&Manifold::batch_union(&lamps(&m.wall, (us + 0.4, vs + GALLERY, n0 + 0.4), (ue - 0.4, vs + GALLERY, n0 + 0.4), every, phase, 1.0)));
            info!("the chasm: a gallery from {:?} to {:?} (out {:?}, {} m back)", m.wall.at(us, vs, n1), m.wall.at(ue, vs, n1), m.wall.out, w.recess);
        }
        let lights = ((ue - us) / 70.0).round() as i32;
        for i in 0..lights {
            let u = us + (ue - us) * (i as f32 + 0.5) / lights as f32;
            parts.lights.push((m.wall.at(u, vs - depth - 4.0, (n0 + n1) * 0.5), 60.0, 0.35));
        }
    }
    built
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
    // (The slope a little under the floors at its ends: its end flush with
    // a floor, a walker sliding over that floor can catch on it, as on a
    // wall.)
    let (sa, sb) = (va - 0.03, vb - 0.03);
    let slope = CrossSection::from_polygons_with_fill_rule(&[vec![[ua as f64, sa as f64], [ub as f64, sb as f64], [ub as f64, (vb - 1.0) as f64], [ua as f64, (va - 1.0) as f64]]], FillRule::NonZero);
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
    for s in [-1.0, 1.0] {
        let off = across * s * (width * 0.5 - 0.2);
        parts.glow.beam(a + off - Vec3::Y * (depth + 0.05), b + off - Vec3::Y * (depth + 0.05), 0.15, 0.05, Vec3::Y);
    }
    span_solid(a, b, width, reach)
}

/// A bridge's girder as built (see `span`).
fn span_solid(a: Vec3, b: Vec3, width: f32, reach: (f32, f32)) -> Manifold {
    let along = (b - a).normalize_or(Vec3::X);
    let depth = (a.distance(b) / 18.0).clamp(3.0, 7.0);
    let mut solid = vec![bar(a - Vec3::Y * (depth * 0.5), b - Vec3::Y * (depth * 0.5), width, depth, Vec3::Y)];
    // On back under the walkways (below their decks).
    let low = (depth - EDGE) * 0.5 + EDGE;
    for (end, dir, r) in [(a, -along, reach.0), (b, along, reach.1)] {
        solid.push(bar(end - Vec3::Y * low, end + dir * (r + 1.0) - Vec3::Y * low, width, depth - EDGE, Vec3::Y));
    }
    Manifold::batch_union(&solid)
}

/// The space a walker needs over a bridge from `a` to `b` (on its deck),
/// short of its ends (where the walkways' spaces are).
fn span_space(a: Vec3, b: Vec3) -> Manifold {
    let along = (b - a).normalize_or(Vec3::X);
    bar(a + Vec3::Y * 1.25 + along * 0.6, b + Vec3::Y * 1.25 - along * 0.6, 2.6, 2.3, Vec3::Y)
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
