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
//! and narrows (20-130 m) and the whole snakes a little. Everything here is
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
pub const LENGTH: f32 = 600.0;
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

/// Where you start: on the rim of the near wall, looking across.
pub fn start(seed: u32) -> [f32; 5] {
    let x = face_x(&plan(-1.0, seed as i32), CENTRE.z);
    [x - 6.0, HEIGHT + 1.7, CENTRE.z, -90.0, -14.0]
}

/// A stretch of a wall's plan: from `z.0` to `z.1` (absolute), its face
/// running from x `x.0` to `x.1`; a shaft (a deep narrow slot) or a massif.
#[derive(Clone, Copy)]
struct Stretch {
    z: (f32, f32),
    x: (f32, f32),
    shaft: bool,
}

/// A wall's plan, along the chasm: stretches 40-150 m long (shafts 4-10 m),
/// each wall standing 10-65 m from a centre line that wanders a little, so
/// the faces meet at angles, the gap widens and narrows and the walls are
/// rarely parallel. `side` -1 for the near wall (low x), +1 for the far.
fn plan(side: f32, seed: i32) -> Vec<Stretch> {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7d2);
    // The centre line wanders the same way for both walls.
    let drift = |z: f32| 20.0 * (z * 0.012 + 1.3).sin() + 9.0 * (z * 0.031 + 0.4).sin();
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
        CENTRE.x + drift(z) + side * (10.0 + 55.0 * t)
    };
    let mut out = Vec::new();
    let mut z = CENTRE.z - half;
    let mut k = 0;
    let mut x = at(z, 0);
    while z < CENTRE.z + half {
        // (No shaft where you start.)
        let shaft = r(k, 0) < 0.18 && !(z - 12.0..z + 12.0).contains(&CENTRE.z);
        let length = if shaft { 4.0 + 6.0 * r(k, 1) } else { 40.0 + 110.0 * r(k, 1) }.min(CENTRE.z + half - z);
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
    /// The chambers cut into the walls (for what joins them).
    chambers: Vec<Chamber>,
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
    let near = wall(&mut parts, -1.0, seed, chambers);
    let far = wall(&mut parts, 1.0, seed + 7919, chambers);
    bridges(&mut parts, seed, &near, &far);
    crossings(&mut parts);
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
    info!("the chasm: built ({} lights)", parts.lights.len());
}

/// A wall: massifs along it, each with its own face (set back, bulging, its
/// upper part often overhanging), some cut by deep shafts between them; each
/// part of a massif in strata of its own (so the bands do not line up across
/// the wall), or a colossal bare slab finely speckled; a few giant columns;
/// heavy cables hanging down it. Returns its massifs (for what spans the
/// gap).
fn wall(parts: &mut Parts, side: f32, seed: i32, chambers: bool) -> Vec<Massif> {
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
        let (u0, u1) = (0.0, len);
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
            massifs.push(Massif { wall: nominal, len, z: stretch.z, split: HEIGHT, low: SHAFT_DEPTH, high: SHAFT_DEPTH, shaft: true });
            continue;
        }
        let split = HEIGHT * (0.3 + 0.5 * r(m, 2));
        let low = -10.0 + 18.0 * r(m, 3);
        // (The top part never set back far: you start on the rim.)
        let high = (low + (r(m, 4) - 0.35) * 16.0).clamp(-4.0, 12.0);
        for (part, (v0, v1, n)) in [(0.0, split, low), (split, HEIGHT, high)].into_iter().enumerate() {
            let w = nominal.moved(n);
            let k = seed + m * 31 + part as i32 * 7;
            let p = m * 2 + part as i32;
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
        massifs.push(Massif { wall: nominal, len, z: stretch.z, split, low, high, shaft: false });
    }
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
    for k in 0..5 {
        let z = CENTRE.z - half + LENGTH * (k as f32 + 0.3 + 0.4 * r(k, 40)) / 5.0;
        let Some((m, u)) = locate(&massifs, z) else { continue };
        let radius = 3.0 + 4.0 * r(k, 41);
        let (v0, v1) = (HEIGHT * 0.2 * r(k, 42), HEIGHT * (0.5 + 0.5 * r(k, 43)));
        let n = m.face(v0).max(m.face(v1)) + radius * 0.4;
        parts.stone.cylinder(m.wall.at(u, v0, n), m.wall.at(u, v1, n), radius, 14);
    }
    // Heavy cables hanging down the face in twisted pairs.
    for k in 0..14 {
        let z = CENTRE.z - half + LENGTH * r(k, 50);
        let Some((m, u)) = locate(&massifs, z) else { continue };
        let v0 = HEIGHT * (0.3 + 0.7 * r(k, 51));
        let drop = 40.0 + 200.0 * r(k, 52);
        let thick = 0.25 + 0.35 * r(k, 53);
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
            // Ledges stop short of the massif's ends now and then.
            let (a, b) = if r(stratum, 5) < 0.3 { (u0 + (u1 - u0) * 0.3 * r(stratum, 6), u1 - (u1 - u0) * 0.3 * r(stratum, 7)) } else { (u0, u1) };
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
        if b - a > 4.0 && d - c > 4.0 {
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
    if size < 1.5 || depth >= 8 || (depth >= 3 && h(0, 0) < 0.1) {
        leaf(parts, w, (u0, u1), (v0, v1), n, seed);
        return;
    }
    // A run of bays between pilasters, now and then.
    if (1..=2).contains(&depth) && size > 10.0 && h(8, 0) < 0.15 {
        bays(parts, &w.moved(w.offset + n), (u0, u1), (v0, v1), seed);
        return;
    }
    // A run of fins along the longer side, alternate ones standing out.
    if depth >= 2 && size > 4.0 && h(1, 0) < 0.15 {
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
        let dn = (csize * STANDS[(h(7, k as i32) * 4.0) as usize % 4] * bold).min(6.0 * bold);
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
    for k in 0..6 {
        let z = CENTRE.z - LENGTH * 0.4 + LENGTH * 0.8 * (k as f32 + r(k, 0) * 0.6) / 6.0;
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

/// A web of taut cables across the void at every angle, wall to wall.
fn web(parts: &mut Parts, seed: i32, near_m: &[Massif], far_m: &[Massif]) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c7);
    for k in 0..90 {
        let z0 = CENTRE.z - LENGTH * 0.45 + LENGTH * 0.9 * r(k, 0);
        let z1 = (z0 + (r(k, 1) - 0.5) * 120.0).clamp(CENTRE.z - LENGTH * 0.5, CENTRE.z + LENGTH * 0.5);
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
