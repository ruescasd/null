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
    if (z - CENTRE.z).abs() >= LENGTH * 0.5 {
        return None;
    }
    let (near, far) = (face_x(&plan(-1.0, seed as i32), z), face_x(&plan(1.0, seed as i32 + 7919), z));
    ((x < near && x > near - BACK * 0.9) || (x > far && x < far + BACK * 0.9)).then_some(HEIGHT)
}

/// Where you start: on the rim of the near wall, looking over the edge
/// along it, down onto the stair to the first walkway.
pub fn start(seed: u32) -> [f32; 5] {
    let x = face_x(&plan(-1.0, seed as i32), CENTRE.z);
    [x - 5.0, HEIGHT + 1.7, CENTRE.z, -130.0, -38.0]
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

    /// A vertical prism over a triangle in plan (its points' heights
    /// ignored), from height 0 to `h`.
    fn prism(&mut self, tri: [Vec3; 3], h: f32) {
        let flat = |p: Vec3| Vec3::new(p.x, 0.0, p.z);
        let [mut a, b, mut c] = tri.map(flat);
        // (Counter-clockwise seen from above.)
        if (b - a).cross(c - a).y < 0.0 {
            std::mem::swap(&mut a, &mut c);
        }
        let up = Vec3::Y * h;
        let base = self.positions.len() as u32;
        for p in [a + up, b + up, c + up] {
            self.positions.push(p.to_array());
            self.normals.push([0.0, 1.0, 0.0]);
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
        for (p, q) in [(a, b), (b, c), (c, a)] {
            let n = (q - p).cross(Vec3::Y).normalize_or(Vec3::X);
            self.quad([q, q + up, p + up, p], n);
        }
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
        for (c, n) in faces {
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
    /// Where on the face being built routes run (walkways, stairs), as (u0,
    /// u1, v0, v1) in its frame: relief keeps flush there, so they have room.
    clear: Vec<(f32, f32, f32, f32)>,
    /// `--opt classical`: routes in an austere classical manner (arcades,
    /// solid parapets, arched bridges, gateways), to compare.
    classical: bool,
}

impl Parts {
    /// Whether a piece of the face being built overlaps where a route runs.
    fn blocked(&self, u: (f32, f32), v: (f32, f32)) -> bool {
        self.clear.iter().any(|&(a, b, c, d)| u.0 < b && u.1 > a && v.0 < d && v.1 > c)
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
    let mut parts = Parts { classical: args.opt("classical"), ..default() };
    // (`--opt chambers`: chambers cut into the walls, set aside for now.)
    let chambers = args.opt("chambers");
    // (The routes are planned first: the walls keep clear where they run.)
    let routing = Routing::plan(seed);
    let near = wall(&mut parts, -1.0, seed, &plan(1.0, seed + 7919), &routing, chambers);
    let far = wall(&mut parts, 1.0, seed + 7919, &plan(-1.0, seed), &routing, chambers);
    bridges(&mut parts, seed, &near, &far);
    crossings(&mut parts);
    routes(&mut parts, seed, &routing, &near, &far);
    web(&mut parts, seed, &near, &far);

    let stone = materials.add(StandardMaterial { base_color: Color::srgb(0.62, 0.62, 0.62), perceptual_roughness: 0.92, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.02, 0.02, 0.02), perceptual_roughness: 0.9, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(60.0), ..default() });
    let dim = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(8.0), ..default() });
    let cable = materials.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.05), perceptual_roughness: 0.6, ..default() });

    if let Some(collider) = parts.stone.collider() {
        commands.spawn((RigidBody::Static, collider, Transform::IDENTITY));
    }
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
/// heavy cables hanging down it. Returns its massifs (for what spans the
/// gap).
fn wall(parts: &mut Parts, side: f32, seed: i32, other: &[Stretch], routing: &Routing, chambers: bool) -> Vec<Massif> {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c1);
    let mut zones = routing.zones(side);
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
        let (u0, u1) = (0.0, len);
        // Where routes run along this face.
        let to_u = |z: f32| (z - stretch.z.0) / (stretch.z.1 - stretch.z.0).max(1e-3) * len;
        parts.clear = zones
            .iter()
            .filter(|z| z.z.1 > stretch.z.0 && z.z.0 < stretch.z.1)
            .map(|z| (to_u(z.z.0.max(stretch.z.0)), to_u(z.z.1.min(stretch.z.1)), z.v.0, z.v.1))
            .collect();
        if stretch.shaft {
            let w = nominal.moved(SHAFT_DEPTH);
            w.block(&mut parts.stone, (u0, u1), (0.0, HEIGHT), (-BACK, 0.0));
            // Black at the back, a strip of light running up it.
            w.block(&mut parts.dark, (u0, u1), (0.0, HEIGHT), (0.0, 0.05));
            let c = (u0 + u1) * 0.5;
            w.block(&mut parts.glow, (c - 0.15, c + 0.15), (0.0, HEIGHT), (0.05, 0.1));
            for k in 0..3 {
                parts.lights.push((w.at(c, HEIGHT * (0.2 + 0.3 * k as f32), 3.0), 100.0, 1.0));
            }
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
        let high = (low + delta).clamp(-4.0, low + room.max(2.0));
        let slope = ((high - low).abs() * (0.5 + 1.5 * r(m, 7))).min(split - 30.0).max(0.0);
        // The sloped face: an underside leaning out over the chasm, or a
        // battered stretch standing back; a rib now and then across it.
        if slope > 1.0 {
            let (a, b) = (split - slope, split);
            nominal.section(&mut parts.stone, (u0, u1), &[(a, -BACK), (a, low), (b, high), (b, -BACK)]);
            let ribs = (len / (6.0 + 10.0 * r(m, 8))).floor() as i32;
            for i in 1..ribs {
                let u = u0 + (u1 - u0) * i as f32 / ribs as f32;
                nominal.section(&mut parts.stone, (u - 0.6, u + 0.6), &[(a, low), (a, low + 1.2), (b, high + 1.2), (b, high)]);
            }
        }
        for (part, (v0, v1, n)) in [(0.0, split - slope, low), (split, HEIGHT, high)].into_iter().enumerate() {
            let w = nominal.moved(n);
            let k = seed + m * 31 + part as i32 * 7;
            let p = m * 2 + part as i32;
            // A place on a walkway passing it, if one fits.
            let reach = low + room - n;
            if let Some(t) = site(parts, routing, side, m as usize, &stretch, len, (v0, v1), n, reach, k) {
                hall(parts, &nominal, &t, (u0, u1), (v0, v1), k);
                zones.push(Zone { side, z: (t.z.0 - 10.0, t.z.1 + 10.0), v: (t.floor - 25.0, t.ceiling + 10.0) });
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
            w.block(&mut parts.stone, (u0, u1), (v0, v1), (-BACK - n, 0.0));
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
        massifs.push(Massif { wall: nominal, len, z: stretch.z, split, low, high, slope, room, shaft: false });
    }
    parts.clear.clear();
    // Where faces meet at an angle, their masses part behind the corner (or
    // overlap): the wedge between them filled (where they overlap, it lies
    // inside them).
    for pair in massifs.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let corner = b.wall.origin;
        parts.stone.prism([corner, corner - a.wall.out * BACK, corner - b.wall.out * BACK], HEIGHT);
    }
    let half = LENGTH * 0.5;
    // Giant half-sunk columns, spanning much of the height, standing on
    // whatever face is there.
    for k in 0..8 {
        let z = CENTRE.z - half + LENGTH * (k as f32 + 0.3 + 0.4 * r(k, 40)) / 8.0;
        let Some((m, u)) = locate(&massifs, z) else { continue };
        let radius = 3.0 + 4.0 * r(k, 41);
        let (v0, v1) = (HEIGHT * 0.2 * r(k, 42), HEIGHT * (0.5 + 0.5 * r(k, 43)));
        if crosses(&zones, (z - radius, z + radius), (v0, v1)) {
            continue;
        }
        let n = m.face(v0).max(m.face(v1)) + radius * 0.4;
        parts.stone.cylinder(m.wall.at(u, v0, n), m.wall.at(u, v1, n), radius, 14);
    }
    // Heavy cables hanging down the face in twisted pairs.
    for k in 0..24 {
        let z = CENTRE.z - half + LENGTH * r(k, 50);
        let Some((m, u)) = locate(&massifs, z) else { continue };
        let v0 = HEIGHT * (0.3 + 0.7 * r(k, 51));
        let drop = 40.0 + 200.0 * r(k, 52);
        let thick = 0.25 + 0.35 * r(k, 53);
        if crosses(&zones, (z - 2.0, z + 2.0), (v0 - drop, v0)) {
            continue;
        }
        let n = m.face(v0).max(m.face(v0 - drop)) + 1.5 + thick;
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
    massifs
}

/// A part of a massif in tall strata (40-140 m), a walkable ledge under
/// each (some carrying a line of light), each stratum's face in fractal
/// relief; zigzag stairs between the ledges.
fn strata(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c9);
    let mut top = v1;
    let mut stratum = 0;
    let mut ledges = Vec::new();
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
            ledges.push((bottom, bn, a, b));
        }
        // Each stratum its own boldness: bold steps, medium, or fine.
        let bold = [1.0, 0.5, 0.25][(r(stratum, 9) * 3.0) as usize % 3];
        relief(parts, w, (u0, u1), (bottom, top), 0.0, 0, bold, seed.wrapping_mul(97).wrapping_add(stratum));
        top = bottom - bh;
        stratum += 1;
    }
    // Stairs: zigzag flights between ledges.
    for (k, pair) in ledges.windows(2).enumerate() {
        let ((upper, un, a0, b0), (lower, ln, a1, b1)) = (pair[0], pair[1]);
        let (lo, hi) = (a0.max(a1) + 6.0, b0.min(b1) - 6.0);
        if hi - lo < 10.0 {
            continue;
        }
        let rise = upper - lower;
        let flights = (rise / 8.0).ceil().max(1.0) as usize;
        let step = rise / flights as f32;
        let run = (step * 1.3).min((hi - lo) * 0.45);
        let n = un.min(ln) * 0.5 + 0.3;
        let mut v = lower;
        let mut u = lo + (hi - lo - run) * r(k as i32, 30);
        for f in 0..flights {
            let dir = if f % 2 == 0 { 1.0 } else { -1.0 };
            let a = w.at(u, v, n);
            let b = w.at(u + dir * run, v + step, n);
            parts.stone.beam(a, b, 1.6, 0.4, Vec3::Y);
            u += dir * run;
            v += step;
            w.block(&mut parts.stone, (u - 1.2, u + 1.2), (v - 0.4, v), (n - 0.8, n + 0.8));
        }
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
/// and then; one a walkway), long enough there, clear of its stairs and
/// bridges, with the part's mass above and below it.
#[allow(clippy::too_many_arguments)]
fn site(parts: &Parts, routing: &Routing, side: f32, massif: usize, stretch: &Stretch, len: f32, (v0, v1): (f32, f32), face: f32, reach: f32, seed: i32) -> Option<Terrace> {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d5);
    let span = (stretch.z.1 - stretch.z.0).max(1e-3);
    let (to_u, to_z) = (|z: f32| (z - stretch.z.0) / span * len, |u: f32| stretch.z.0 + u / len * span);
    for (k, w) in routing.ways.iter().enumerate().filter(|(_, w)| w.side == side) {
        let i = k as i32;
        if r(i, 0) > 0.7 || parts.terraces.iter().any(|t| t.way == k) {
            continue;
        }
        let (us, ue) = (to_u(stretch.z.0.max(w.z.0)), to_u(stretch.z.1.min(w.z.1)));
        let height = 10.0 + 5.0 * r(i, 1);
        let width = (44.0 + 20.0 * r(i, 2)).max(height * 3.0);
        if ue - us < width + 30.0 {
            continue;
        }
        let a = us + 15.0 + (ue - us - 30.0 - width) * r(i, 3);
        let (z0, z1) = (to_z(a), to_z(a + width));
        let near = |z: f32| z > z0 - 20.0 && z < z1 + 20.0;
        if routing.stairs.iter().any(|s| s.2 == k && (near(s.0.0) || near(s.0.1))) || routing.bridges.iter().any(|b| (b.1 == k || b.2 == k) && near(b.0)) {
            continue;
        }
        // A step or few down into it from the walkway at either end.
        let (ga, gb) = (w.v(z0), w.v(z1));
        let floor = ga.min(gb) - 0.6;
        let ceiling = floor + height.max(w.width * 0.5 + 8.0 + (ga - gb).abs());
        if floor - 15.0 < v0 || ceiling + 15.0 > v1 {
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

/// A massif part with a hall cut into it (its place): the mass round it;
/// the face round it in strata; inside, piers (or classical, an arcade)
/// along its open front, the back wall in relief either side of a dark
/// doorway in a stepped frame, a lit line along the front of the ceiling,
/// a light within. (The walkway's part of it, gateways, steps and terrace,
/// comes with the walkway.)
fn hall(parts: &mut Parts, wall: &Wall, t: &Terrace, (u0, u1): (f32, f32), (v0, v1): (f32, f32), seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d6);
    let (a, b) = t.u;
    let (f, top) = (t.floor, t.ceiling);
    wall.block(&mut parts.stone, (u0, u1), (v0, f), (-BACK, t.face));
    wall.block(&mut parts.stone, (u0, u1), (top, v1), (-BACK, t.face));
    wall.block(&mut parts.stone, (u0, a), (f, top), (-BACK, t.face));
    wall.block(&mut parts.stone, (b, u1), (f, top), (-BACK, t.face));
    wall.block(&mut parts.stone, (a, b), (f, top), (-BACK, t.back));
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
        if parts.classical {
            let rad = (bay - 2.0) * 0.5;
            arch(&mut parts.stone, wall, (b0, b1), f, top - rad - 0.8, top, (t.face - 1.5, t.face), 1.0);
        } else if i > 0 {
            wall.block(&mut parts.stone, (b0 - 1.0, b0 + 1.0), (f, top), (t.face - 2.2, t.face - 0.2));
        }
    }
    wall.block(&mut parts.glow, (a, b), (top - 0.15, top), (t.face - 2.6, t.face - 2.4));
    // The back wall: a dark doorway in a stepped frame, relief either side.
    let inner = wall.moved(t.back);
    let c = (a + b) * 0.5;
    let (dw, dh) = (2.0 + r(0, 1), 6.0 + 3.0 * r(0, 2));
    inner.block(&mut parts.dark, (c - dw, c + dw), (f, f + dh), (0.0, 0.05));
    for s in 0..3 {
        let (o, d) = (0.7 * s as f32, 0.3 + 0.35 * (3 - s) as f32);
        inner.block(&mut parts.stone, (c - dw - o - 0.7, c - dw - o), (f, f + dh + o + 0.7), (0.0, d));
        inner.block(&mut parts.stone, (c + dw + o, c + dw + o + 0.7), (f, f + dh + o + 0.7), (0.0, d));
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

/// Bridges across the gap at a few heights: a deck on deep beams, braced
/// underneath.
fn bridges(parts: &mut Parts, seed: i32, near_m: &[Massif], far_m: &[Massif]) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c6);
    for k in 0..5 {
        let z = CENTRE.z - LENGTH * 0.4 + LENGTH * 0.8 * (k as f32 + r(k, 0) * 0.6) / 5.0;
        let v = HEIGHT * (0.2 + 0.75 * r(k, 1));
        let wide = 3.0 + 4.0 * r(k, 2);
        // Into each face, wherever it stands.
        let (Some((nm, nu)), Some((fm, fu))) = (locate(near_m, z), locate(far_m, z)) else { continue };
        let a = nm.wall.at(nu, v, nm.face(v) - 1.0);
        let b = fm.wall.at(fu, v, fm.face(v) - 1.0);
        let across = (b - a).cross(Vec3::Y).normalize_or(Vec3::Z);
        parts.stone.beam(a, b, wide, 0.8, Vec3::Y);
        // Deep beams under the deck's edges.
        for s in [-1.0, 1.0] {
            let off = across * s * (wide * 0.5 - 0.3);
            parts.stone.beam(a + off - Vec3::Y * 1.2, b + off - Vec3::Y * 1.2, 0.5, 1.6, Vec3::Y);
        }
        // Bracing: diagonals underneath, zigzag.
        let n = 8;
        for i in 0..n {
            let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            let (p, q) = (a.lerp(b, t0), a.lerp(b, t1));
            let low = Vec3::Y * -4.5;
            let (from, to) = if i % 2 == 0 { (p - Vec3::Y * 1.6, q + low) } else { (p + low, q - Vec3::Y * 1.6) };
            parts.stone.beam(from, to, 0.4, 0.4, across);
        }
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

/// How thick a walkway's deck is, how high its railing, and how long a
/// stair's flights run.
const DECK: f32 = 1.2;
const RAIL: f32 = 1.1;
const FLIGHT: f32 = 14.0;

/// Where a walkway's deck starts out from a massif's face at height `v`
/// (across a shaft, from the wall's line).
fn inner(m: &Massif, v: f32) -> f32 {
    if m.shaft { 0.0 } else { m.face(v) - 0.5 }
}

/// Where a route runs along a wall: along the chasm and in height.
struct Zone {
    side: f32,
    z: (f32, f32),
    v: (f32, f32),
}

/// Whether anything spanning `z` and `v` would cross a route.
fn crosses(zones: &[Zone], z: (f32, f32), v: (f32, f32)) -> bool {
    zones.iter().any(|q| z.0 < q.z.1 && z.1 > q.z.0 && v.0 < q.v.1 && v.1 > q.v.0)
}

/// The routes, planned before anything is built: the walkways; for each
/// wall the stairs that may join each walkway to the next one down (from
/// which end, the upper and lower walkway), and from the rim; the bridges
/// (where along the chasm, which walkways).
struct Routing {
    ways: Vec<Walkway>,
    stairs: Vec<((f32, f32), f32, usize, bool)>,
    bridges: Vec<(f32, usize, usize)>,
}

impl Routing {
    fn plan(seed: i32) -> Routing {
        let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d3);
        let half = LENGTH * 0.5;
        let mut ways: Vec<Walkway> = Vec::new();
        for (s, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            for i in 0..4 {
                let k = s as i32 * 10 + i;
                // The near wall's first passes where you start, just under
                // the rim; the others in bands down the wall.
                let (zc, length, vmid) = if side < 0.0 && i == 0 {
                    (CENTRE.z + (r(k, 0) - 0.5) * 100.0, 350.0 + 250.0 * r(k, 1), HEIGHT - 35.0)
                } else {
                    let band = (HEIGHT - 120.0) / 4.0;
                    let length = 250.0 + 450.0 * r(k, 1);
                    (CENTRE.z - half + length * 0.5 + (LENGTH - length) * r(k, 0), length, 40.0 + band * (3 - i) as f32 + band * r(k, 2))
                };
                let z0 = (zc - length * 0.5).max(CENTRE.z - half + 2.0);
                let z1 = (zc + length * 0.5).min(CENTRE.z + half - 2.0);
                let grade = (r(k, 3) - 0.5) * 0.08;
                ways.push(Walkway { side, z: (z0, z1), v0: vmid - grade * (z1 - z0) * 0.5, grade, width: 6.0 + 6.0 * r(k, 4) });
            }
        }
        // Stairs down, each walkway to the next one down on its wall, from
        // an end of the upper one out beyond it, over the lower (both ends
        // tried, in turn, when built).
        let mut stairs = Vec::new();
        for side in [-1.0f32, 1.0] {
            let mut on: Vec<usize> = (0..ways.len()).filter(|&i| ways[i].side == side).collect();
            let mid = |w: &Walkway| w.v((w.z.0 + w.z.1) * 0.5);
            on.sort_by(|&a, &b| mid(&ways[b]).total_cmp(&mid(&ways[a])));
            for pair in on.windows(2) {
                let (upper, lower) = (&ways[pair[0]], &ways[pair[1]]);
                for (end, dir) in [(upper.z.1, 1.0), (upper.z.0, -1.0)] {
                    let (a, b) = (end + dir * 1.5, end + dir * (1.5 + FLIGHT));
                    if a.min(b) > lower.z.0 + 4.0 && a.max(b) < lower.z.1 - 4.0 {
                        stairs.push(((a, b), upper.v(end), pair[1], false));
                    }
                }
            }
        }
        // From the rim where you start down to the highest walkway.
        let top = (0..ways.len()).find(|&i| ways[i].side < 0.0).unwrap_or(0);
        for k in 0..3 {
            let a = CENTRE.z + 4.0 + k as f32 * 18.0;
            stairs.push(((a, a + FLIGHT), HEIGHT, top, true));
        }
        // Bridges across, where walkways on the two walls pass at similar
        // heights: one for each such pair, where they are closest.
        let mut bridges = Vec::new();
        for (i, a) in ways.iter().enumerate().filter(|(_, w)| w.side < 0.0) {
            for (j, b) in ways.iter().enumerate().filter(|(_, w)| w.side > 0.0) {
                let (lo, hi) = (a.z.0.max(b.z.0) + 10.0, a.z.1.min(b.z.1) - 10.0);
                if hi <= lo {
                    continue;
                }
                let best = (0..=20).map(|k| lo + (hi - lo) * k as f32 / 20.0).min_by(|x, y| (a.v(*x) - b.v(*x)).abs().total_cmp(&(a.v(*y) - b.v(*y)).abs()));
                if let Some(z) = best
                    && (a.v(z) - b.v(z)).abs() <= 25.0
                {
                    bridges.push((z, i, j));
                }
            }
        }
        Routing { ways, stairs, bridges }
    }

    /// Where the routes run along a wall (with headroom).
    fn zones(&self, side: f32) -> Vec<Zone> {
        let mut out = Vec::new();
        for w in self.ways.iter().filter(|w| w.side == side) {
            let (a, b) = (w.v(w.z.0), w.v(w.z.1));
            out.push(Zone { side, z: w.z, v: (a.min(b) - DECK - 0.5, a.max(b) + 4.0) });
        }
        for &(z, top, lower, _) in &self.stairs {
            let w = &self.ways[lower];
            if w.side == side {
                out.push(Zone { side, z: (z.0.min(z.1) - 3.0, z.0.max(z.1) + 3.0), v: (w.v(z.0) - 1.0, top + 3.0) });
            }
        }
        out.retain(|z| z.side == side);
        out
    }
}

/// Routes: walkways along the walls, promenade-wide with railings (open
/// where a bridge or prow joins); broad zigzag stairs between each and the
/// next one down on the same wall, and from the rim where you start down to
/// the highest; bridges across where walkways on the two walls pass at
/// similar heights; prows jutting from them, places at the edge.
fn routes(parts: &mut Parts, seed: i32, routing: &Routing, near: &[Massif], far: &[Massif]) {
    let massifs = |side: f32| if side < 0.0 { near } else { far };
    let ways = &routing.ways;
    for (k, w) in ways.iter().enumerate() {
        // (The railing open where a bridge joins.)
        let openings: Vec<(f32, f32)> =
            routing.bridges.iter().filter(|b| b.1 == k || b.2 == k).map(|b| (b.0 - 3.0, b.0 + 3.0)).collect();
        let terraces: Vec<Terrace> = parts.terraces.iter().filter(|t| t.way == k).copied().collect();
        walkway(parts, w, massifs(w.side), seed.wrapping_add(k as i32 * 131), &openings, &terraces);
    }
    // Stairs: the first that fits for each walkway (and the rim).
    let mut done: Vec<(usize, bool)> = Vec::new();
    for &(z, top, lower, rim) in &routing.stairs {
        let key = (lower, rim);
        if done.contains(&key) {
            continue;
        }
        let w = &ways[lower];
        if stair(parts, massifs(w.side), z, top, w, rim) {
            done.push(key);
        }
    }
    for &(z, i, j) in &routing.bridges {
        let (a, b) = (&ways[i], &ways[j]);
        let (Some((ma, ua)), Some((mb, ub))) = (locate(near, z), locate(far, z)) else { continue };
        let pa = ma.wall.at(ua, a.v(z), inner(ma, a.v(z)) + a.width - 0.5);
        let pb = mb.wall.at(ub, b.v(z), inner(mb, b.v(z)) + b.width - 0.5);
        span(parts, pa, pb, 5.0);
    }
}

/// The stretches of `(a, b)` left once the `gaps` are taken out.
fn runs((a, b): (f32, f32), gaps: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = vec![(a, b)];
    for &(g0, g1) in gaps {
        out = out
            .into_iter()
            .flat_map(|(x, y)| {
                if g1 <= x || g0 >= y {
                    vec![(x, y)]
                } else {
                    [(x, g0), (g1, y)].into_iter().filter(|(p, q)| q - p > 0.5).collect()
                }
            })
            .collect();
    }
    out
}

/// A walkway: along each face it passes, a deck standing out from the face,
/// joined at the corners; a railing on posts along its outer edge, a line of
/// light under it, brackets beneath and lights below; now and then a prow;
/// through its places (see `place`).
fn walkway(parts: &mut Parts, w: &Walkway, massifs: &[Massif], seed: i32, openings: &[(f32, f32)], terraces: &[Terrace]) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d4);
    // The previous face's end: inner and outer edge (on top), railing top,
    // and whether the railing was open there.
    let mut last: Option<(Vec3, Vec3, Vec3, bool)> = None;
    for (k, m) in massifs.iter().enumerate() {
        let (zs, ze) = (m.z.0.max(w.z.0), m.z.1.min(w.z.1));
        if ze - zs < 0.5 {
            continue;
        }
        let to_u = |z: f32| (z - m.z.0) / (m.z.1 - m.z.0).max(1e-3) * m.len;
        let (us, ue) = (to_u(zs), to_u(ze));
        let (vs, ve) = (w.v(zs), w.v(ze));
        let v_at = |u: f32| vs + (ve - vs) * (u - us) / (ue - us).max(1e-3);
        let n0 = inner(m, (vs + ve) * 0.5);
        let n1 = n0 + w.width;
        let at = |u: f32, v: f32, n: f32| m.wall.at(u, v, n);
        // Where the railing is open: bridges, and a prow if there is one.
        let mut gaps: Vec<(f32, f32)> = openings.iter().map(|&(a, b)| (to_u(a), to_u(b))).collect();
        let k = k as i32;
        let terrace = terraces.iter().find(|t| t.massif == k as usize);
        // (The deck makes way for a place: its floor.)
        let through: Vec<(f32, f32)> = terrace.map(|t| t.u).into_iter().collect();
        gaps.extend(&through);
        let prow = (terrace.is_none() && r(k, 0) < 0.3 && m.room > w.width + 12.0 && ue - us > 40.0 && !m.shaft).then(|| {
            let reach = (10.0 + 15.0 * r(k, 1)).min(m.room - w.width);
            let width = (20.0 + 30.0 * r(k, 2)).min(ue - us - 10.0);
            let c = us + 5.0 + (ue - us - 10.0 - width) * r(k, 3);
            (c, c + width, reach)
        });
        if let Some((pu0, pu1, _)) = prow {
            gaps.push((pu0, pu1));
        }
        let open = |u: f32| gaps.iter().any(|&(a, b)| u >= a && u <= b);
        // The deck.
        for (a, b) in runs((us, ue), &through) {
            parts.stone.beam(at(a, v_at(a) - DECK * 0.5, (n0 + n1) * 0.5), at(b, v_at(b) - DECK * 0.5, (n0 + n1) * 0.5), w.width, DECK, Vec3::Y);
            // A line of light under the outer edge.
            parts.glow.beam(at(a, v_at(a) - DECK - 0.05, n1 - 0.3), at(b, v_at(b) - DECK - 0.05, n1 - 0.3), 0.15, 0.05, Vec3::Y);
        }
        if let Some(t) = terrace {
            place(parts, &m.wall, t, &v_at, (n0, n1));
        }
        // Joined to the last face's at the corner.
        if let Some((li, lo, lr, lopen)) = last {
            parts.stone.plate(&[li, lo, at(us, vs, n0), at(us, vs, n1)], vs - DECK, vs);
            if !lopen && !open(us) && !parts.classical {
                parts.stone.beam(lr, at(us, vs + RAIL, n1 - 0.15), 0.1, 0.1, Vec3::Y);
            }
        }
        // Classical: an arcade along the outer edge, a roof over the
        // walkway; otherwise a railing on posts. Open at the gaps.
        if parts.classical {
            loggia(parts, &m.wall, (us, ue), &v_at, (n0, n1), &gaps);
            if let Some((li, lo, _, _)) = last {
                let top = vs + ARCADE + 0.8;
                parts.stone.plate(&[li, lo, at(us, vs, n0), at(us, vs, n1)], top - 0.8, top);
            }
        }
        for (a, b) in runs((us, ue), &gaps).into_iter().filter(|_| !parts.classical) {
            parts.stone.beam(at(a, v_at(a) + RAIL, n1 - 0.15), at(b, v_at(b) + RAIL, n1 - 0.15), 0.1, 0.1, Vec3::Y);
            let posts = ((b - a) / 3.0).floor().max(1.0) as i32;
            for i in 0..=posts {
                let u = a + (b - a) * i as f32 / posts as f32;
                parts.stone.beam(at(u, v_at(u), n1 - 0.15), at(u, v_at(u) + RAIL, n1 - 0.15), 0.1, 0.1, m.wall.along);
            }
        }
        // Brackets beneath, back into the wall.
        let brackets = ((ue - us) / 18.0).floor() as i32;
        let depth = (w.width * 0.9).min(8.0);
        for i in 1..=brackets {
            let u = us + (ue - us) * i as f32 / (brackets + 1) as f32;
            if through.iter().any(|&(a, b)| u > a - 1.0 && u < b + 1.0) {
                continue;
            }
            let v = v_at(u) - DECK;
            m.wall.section(&mut parts.stone, (u - 0.4, u + 0.4), &[(v, n0), (v, n1 - 0.6), (v - depth, n0)]);
        }
        // Lights below, now and then.
        let lights = ((ue - us) / 70.0).round() as i32;
        for i in 0..lights {
            let u = us + (ue - us) * (i as f32 + 0.5) / lights as f32;
            parts.lights.push((at(u, vs - 7.0, (n0 + n1) * 0.5), 60.0, 0.35));
        }
        // The prow: a slab jutting from the walkway's edge over the void, a
        // flat top, a railing round it, its underside sloping back.
        if let Some((pu0, pu1, reach)) = prow {
            let v = v_at((pu0 + pu1) * 0.5);
            let thick = reach * (0.5 + 0.4 * r(k, 4));
            m.wall.section(&mut parts.stone, (pu0, pu1), &[(v - thick, n0), (v, n0), (v, n1 + reach), (v - 2.0, n1 + reach)]);
            let edge = n1 + reach - 0.15;
            if parts.classical {
                // Solid parapet walls round it.
                m.wall.block(&mut parts.stone, (pu0, pu1), (v, v + 1.0), (edge - 0.2, edge + 0.15));
                for u in [pu0, pu1] {
                    m.wall.block(&mut parts.stone, (u - 0.2, u + 0.2), (v, v + 1.0), (n1, edge));
                }
            } else {
                parts.stone.beam(at(pu0, v + RAIL, edge), at(pu1, v + RAIL, edge), 0.1, 0.1, Vec3::Y);
                for u in [pu0, pu1] {
                    parts.stone.beam(at(u, v + RAIL, n1), at(u, v + RAIL, edge), 0.1, 0.1, Vec3::Y);
                }
                let posts = ((pu1 - pu0) / 3.0).floor().max(1.0) as i32;
                for i in 0..=posts {
                    let u = pu0 + (pu1 - pu0) * i as f32 / posts as f32;
                    parts.stone.beam(at(u, v, edge), at(u, v + RAIL, edge), 0.1, 0.1, m.wall.along);
                }
            }
            parts.glow.beam(at(pu0, v - 2.1, edge), at(pu1, v - 2.1, edge), 0.15, 0.05, Vec3::Y);
            parts.lights.push((at((pu0 + pu1) * 0.5, v - 12.0, n1 + reach * 0.5), 80.0, 0.35));
        }
        last = Some((at(ue, ve, n0), at(ue, ve, n1), at(ue, ve + RAIL, n1 - 0.15), open(ue)));
    }
}

/// A walkway's part of a place: a gateway across it at each end, steps
/// down from it to the floor; the floor running on out past the face, its
/// corners chamfered, stepped beneath, a railing (classical, a parapet)
/// round its edge and a line of light under it, a light below.
fn place(parts: &mut Parts, wall: &Wall, t: &Terrace, v_at: &dyn Fn(f32) -> f32, (n0, n1): (f32, f32)) {
    let (a, b) = t.u;
    let f = t.floor;
    let at = |u: f32, v: f32, n: f32| wall.at(u, v, n);
    // The floor out past the face, stepped beneath.
    for s in 0..4 {
        let front = t.front - 3.0 * s as f32;
        if front < n1 - 0.5 {
            break;
        }
        let e = front - n1;
        let (y0, y1) = if s == 0 { (f - DECK, f) } else { (f - DECK - 1.6 * s as f32, f - DECK - 1.6 * (s - 1) as f32) };
        let pts = [at(a, 0.0, n0), at(b, 0.0, n0), at(b, 0.0, n1), at(b - e, 0.0, front), at(a + e, 0.0, front), at(a, 0.0, n1)];
        parts.stone.plate(&pts, y0, y1);
    }
    // Round its edge.
    let e = t.front - n1;
    let edge = [(a + 0.25, n1), (a + e + 0.1, t.front - 0.2), (b - e - 0.1, t.front - 0.2), (b - 0.25, n1)];
    for pair in edge.windows(2) {
        let ((ua, na), (ub, nb)) = (pair[0], pair[1]);
        let (p, q) = (at(ua, f, na), at(ub, f, nb));
        if parts.classical {
            parts.stone.beam(p + Vec3::Y * 0.5, q + Vec3::Y * 0.5, 0.35, 1.0, Vec3::Y);
        } else {
            parts.stone.beam(p + Vec3::Y * RAIL, q + Vec3::Y * RAIL, 0.1, 0.1, Vec3::Y);
            let posts = (p.distance(q) / 3.0).floor().max(1.0) as i32;
            for i in 0..=posts {
                let x = p.lerp(q, i as f32 / posts as f32);
                parts.stone.beam(x, x + Vec3::Y * RAIL, 0.1, 0.1, (q - p).normalize_or(Vec3::X));
            }
        }
        parts.glow.beam(p - Vec3::Y * (DECK + 0.05), q - Vec3::Y * (DECK + 0.05), 0.15, 0.05, Vec3::Y);
    }
    parts.lights.push((at((a + b) * 0.5, f - 12.0, (n1 + t.front) * 0.5), 80.0, 0.35));
    // At each end, a gateway across the walkway, and steps down inside.
    for (u, dir) in [(a, 1.0f32), (b, -1.0)] {
        let gv = v_at(u);
        let origin = wall.origin + wall.along * if dir > 0.0 { u - 1.2 } else { u };
        let gate = Wall { origin, along: wall.out, out: wall.along, offset: 0.0 };
        let (o0, o1) = (t.face - 1.0, n1 + 1.0);
        if parts.classical {
            let rad = (o1 - o0 - 2.0) * 0.5;
            arch(&mut parts.stone, &gate, (o0, o1), gv, gv + 4.0, gv + 4.0 + rad + 1.2, (0.0, 1.2), 1.0);
        } else {
            let h = gv + 5.0 + 0.25 * (n1 - n0);
            gate.block(&mut parts.stone, (o0, t.face + 0.5), (gv, h + 1.6), (0.0, 1.2));
            gate.block(&mut parts.stone, (n1 - 0.3, o1), (gv, h + 1.6), (0.0, 1.2));
            gate.block(&mut parts.stone, (o0, o1), (h, h + 1.6), (0.0, 1.2));
            gate.block(&mut parts.glow, (t.face + 0.5, n1 - 0.3), (h - 0.08, h), (0.5, 0.7));
        }
        let rise = gv - f;
        let steps = (rise / 0.3).ceil() as i32;
        for i in 0..steps - 1 {
            let s = u + dir * 0.45 * i as f32;
            let top = gv - rise * (i + 1) as f32 / steps as f32;
            wall.block(&mut parts.stone, (s.min(s + dir * 0.45), s.max(s + dir * 0.45)), (f - 0.1, top), (n0, n1));
        }
    }
}

/// A broad switchback stair against the wall: flights of steps running
/// back and forth between `z.0` and `z.1` in two lanes side by side (so each
/// flight has the one before it beside it, not under it), a landing across
/// both lanes at every turn; from the walkway `lower` (its deck must reach
/// under it) up to height `top`, the last flight ending at `z.0` (the end
/// nearest the walkway above). `rim`: the top landing reaches back onto the
/// rim. False if it does not fit there.
fn stair(parts: &mut Parts, massifs: &[Massif], z: (f32, f32), top: f32, lower: &Walkway, rim: bool) -> bool {
    let (Some((m, ua)), Some((mb, ub))) = (locate(massifs, z.0), locate(massifs, z.1)) else { return false };
    // (Within one face.)
    if !std::ptr::eq(m, mb) || m.shaft {
        return false;
    }
    let bottom = lower.v((z.0 + z.1) * 0.5);
    let rise = top - bottom;
    if !(6.0..=220.0).contains(&rise) {
        return false;
    }
    // The lanes, clear of the face all the way down, both on the lower deck.
    const LANE: f32 = 3.5;
    let n = (bottom..=top).step_by_f32(4.0).map(|v| m.face(v)).fold(m.face(top), f32::max) + 2.5;
    let lanes = [n, n + LANE + 0.2];
    if lanes[1] + LANE * 0.5 > inner(m, bottom) + lower.width {
        return false;
    }
    let flights = (rise / 6.0).ceil() as i32;
    let step = rise / flights as f32;
    let steps = (step / 0.3).ceil().max(1.0) as i32;
    let riser = step / steps as f32;
    // (Started so that the last flight ends at `ua`.)
    let start = if flights % 2 == 0 { ua } else { ub };
    let other = |u: f32| if u == ua { ub } else { ua };
    let (mut u, mut v) = (start, bottom);
    for f in 0..flights {
        let to = other(u);
        let lane = lanes[(f % 2) as usize];
        let (na, nb) = (lane - LANE * 0.5, lane + LANE * 0.5);
        // The slab beneath, and the steps on it.
        parts.stone.beam(m.wall.at(u, v - 0.7, lane), m.wall.at(to, v + step - 0.7, lane), LANE, 0.5, Vec3::Y);
        for i in 0..steps {
            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            let (s0, s1) = (u + (to - u) * t0, u + (to - u) * t1);
            let rise_to = v + riser * (i + 1) as f32;
            m.wall.block(&mut parts.stone, (s0.min(s1), s0.max(s1)), (rise_to - 0.6, rise_to), (na, nb));
        }
        // Classical: a solid parapet wall along the outer lane's flights.
        if parts.classical && f % 2 == 1 {
            let e = lanes[1] + LANE * 0.5 - 0.15;
            parts.stone.beam(m.wall.at(u, v + 0.5, e), m.wall.at(to, v + step + 0.5, e), 0.3, 1.0, Vec3::Y);
        }
        u = to;
        v += step;
        // A landing across both lanes at the turn, out beyond the flight's
        // end.
        let back = if rim && f == flights - 1 { m.face(top) - 1.5 } else { lanes[0] - LANE * 0.5 };
        let out = if to > other(to) { 3.0 } else { -3.0 };
        let (a, b) = (u.min(u + out), u.max(u + out));
        m.wall.block(&mut parts.stone, (a, b), (v - 0.5, v), (back, lanes[1] + LANE * 0.5));
    }
    // Classical: a solid wall between the two lanes, the flights' spine.
    if parts.classical {
        let c = (lanes[0] + lanes[1]) * 0.5;
        m.wall.block(&mut parts.stone, (ua.min(ub), ua.max(ub)), (bottom, top + 1.0), (c - 0.12, c + 0.12));
    }
    true
}

/// A bridge between two points on walkways: a deck with railings both sides
/// and lines of light under its edges.
fn span(parts: &mut Parts, a: Vec3, b: Vec3, width: f32) {
    let along = (b - a).normalize_or(Vec3::X);
    let across = along.cross(Vec3::Y).normalize_or(Vec3::Z);
    parts.stone.beam(a - Vec3::Y * DECK * 0.5, b - Vec3::Y * DECK * 0.5, width, DECK, Vec3::Y);
    if parts.classical {
        viaduct(parts, a, b, width);
        return;
    }
    for s in [-1.0, 1.0] {
        let off = across * s * (width * 0.5 - 0.15);
        parts.stone.beam(a + off + Vec3::Y * RAIL, b + off + Vec3::Y * RAIL, 0.1, 0.1, Vec3::Y);
        let posts = (a.distance(b) / 3.0).floor().max(1.0) as i32;
        for i in 0..=posts {
            let p = a.lerp(b, i as f32 / posts as f32) + off;
            parts.stone.beam(p, p + Vec3::Y * RAIL, 0.1, 0.1, along);
        }
        parts.glow.beam(a + off - Vec3::Y * (DECK + 0.05), b + off - Vec3::Y * (DECK + 0.05), 0.15, 0.05, Vec3::Y);
    }
}

/// How tall a classical walkway's arcade is, to the underside of its roof.
const ARCADE: f32 = 7.6;

/// An arched opening in a solid wall on a face: piers at both ends from `v0`
/// up to `vt`, the opening between them round-arched from the springing
/// `vs`, filled solid above the arch to `vt`; a ring standing a little proud
/// round the arch. `n`: the wall's depth range.
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
        // The ring, proud of the wall on its outer side.
        let k = 0.18 / r.max(0.5);
        let (px, py) = ((ux - c) * k, (vx - vs) * k);
        let (qx, qy) = ((uy - c) * k, (vy - vs) * k);
        g.beam(w.at(ux + px, vx + py, (n0 + n1) * 0.5), w.at(uy + qx, vy + qy, (n0 + n1) * 0.5), 0.35, n1 - n0 + 0.24, w.out);
    }
}

/// A walkway's arcade (classical): piers along its outer edge, round arches
/// between them, solid above up to a roof over the whole walkway; a low
/// parapet wall between the piers (open at the gaps); a light inside now
/// and then.
fn loggia(parts: &mut Parts, w: &Wall, (us, ue): (f32, f32), v_at: &dyn Fn(f32) -> f32, (n0, n1): (f32, f32), gaps: &[(f32, f32)]) {
    let bays = ((ue - us) / 5.6).round().max(1.0) as i32;
    let bay = (ue - us) / bays as f32;
    let (p0, p1) = (n1 - 0.9, n1);
    for i in 0..bays {
        let (b0, b1) = (us + bay * i as f32, us + bay * (i + 1) as f32);
        let v = v_at((b0 + b1) * 0.5);
        let pier = 0.9f32.min(bay * 0.25);
        let r = (bay - 2.0 * pier) * 0.5;
        arch(&mut parts.stone, w, (b0, b1), v, v + ARCADE - r - 0.6, v + ARCADE, (p0, p1), pier);
        if !gaps.iter().any(|&(a, b)| (b0 + b1) * 0.5 >= a && (b0 + b1) * 0.5 <= b) {
            w.block(&mut parts.stone, (b0 + pier, b1 - pier), (v, v + 1.0), (n1 - 0.6, n1 - 0.25));
        }
        if i % 5 == 2 {
            parts.lights.push((w.at((b0 + b1) * 0.5, v + ARCADE - 1.5, (n0 + n1) * 0.5), 25.0, 0.12));
        }
    }
    // The roof, over the whole walkway.
    let (va, vb) = (v_at(us), v_at(ue));
    parts.stone.beam(w.at(us, va + ARCADE + 0.4, (n0 + n1) * 0.5), w.at(ue, vb + ARCADE + 0.4, (n0 + n1) * 0.5), n1 - n0, 0.8, Vec3::Y);
}

/// A classical bridge: solid parapets, an arch beneath springing low at
/// both ends and rising to the deck, piers between arch and deck, and an
/// arched gateway at each end.
fn viaduct(parts: &mut Parts, a: Vec3, b: Vec3, width: f32) {
    let along = (b - a).normalize_or(Vec3::X);
    let across = along.cross(Vec3::Y).normalize_or(Vec3::Z);
    let span = a.distance(b);
    for s in [-1.0, 1.0] {
        let off = across * s * (width * 0.5 - 0.2);
        parts.stone.beam(a + off + Vec3::Y * 0.5, b + off + Vec3::Y * 0.5, 0.35, 1.0, Vec3::Y);
        parts.glow.beam(a + off - Vec3::Y * (DECK + 0.05), b + off - Vec3::Y * (DECK + 0.05), 0.15, 0.05, Vec3::Y);
    }
    // The arch below: low at the ends, just under the deck in the middle.
    let low = (span * 0.16).clamp(6.0, 30.0);
    let below = |t: f32| DECK + 0.6 + (low - DECK - 0.6) * (2.0 * t - 1.0).powi(2);
    let n = 20;
    for i in 0..n {
        let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let (p, q) = (a.lerp(b, t0) - Vec3::Y * below(t0), a.lerp(b, t1) - Vec3::Y * below(t1));
        parts.stone.beam(p, q, width * 0.8, 1.2, Vec3::Y);
    }
    // Piers from the arch up to the deck.
    let piers = (span / 5.0).floor() as i32;
    for i in 1..piers {
        let t = i as f32 / piers as f32;
        let p = a.lerp(b, t);
        let h = below(t) - DECK;
        if h > 1.2 {
            parts.stone.beam(p - Vec3::Y * below(t), p - Vec3::Y * DECK, width * 0.8, 0.6, along);
        }
    }
    // A gateway at each end: an arch across the deck.
    for (end, dir) in [(a, along), (b, -along)] {
        let pier = 0.9;
        let gate = Wall { origin: Vec3::new(0.0, 0.0, 0.0) + (end + dir * 1.0 - across * (width * 0.5 + pier)) * Vec3::new(1.0, 0.0, 1.0), along: across, out: dir, offset: 0.0 };
        let v = end.y;
        let r = width * 0.5;
        arch(&mut parts.stone, &gate, (0.0, width + 2.0 * pier), v, v + 3.6, v + 3.6 + r + 1.2, (-0.5, 0.5), pier);
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

/// A web of taut cables across the void at every angle, wall to wall.
fn web(parts: &mut Parts, seed: i32, near_m: &[Massif], far_m: &[Massif]) {
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
        parts.cables.push((rope_static(a, b, sag), thick));
    }
}
