//! The chasm (`--opt chasm`): a standalone place, apart from the torus world,
//! to explore the depths: two vast walls rising out of the haze with a gap
//! between them, crossed by bridges and a web of cables, the walls' detail
//! being their own geometry (bands, bays, frames within frames, rows of
//! openings, packed conduits, ledges, stairs), never dressing on a surface.
//! You start on the rim of one wall, looking across. The flat lab ground is
//! the floor, far below, lost in the haze.
//!
//! The walls run along z, centred on `CENTRE`; everything here is in metres.

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
/// The gap between the walls, their height, and the segment's length.
pub const WIDTH: f32 = 60.0;
pub const HEIGHT: f32 = 300.0;
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
pub fn rim(x: f32, z: f32) -> Option<f32> {
    let (dx, dz) = ((x - CENTRE.x).abs(), (z - CENTRE.z).abs());
    (dx > WIDTH * 0.5 && dx < WIDTH * 0.5 + BACK && dz < LENGTH * 0.5).then_some(HEIGHT)
}

/// Where you start: on the rim of the near wall, looking across.
pub fn start() -> [f32; 5] {
    [CENTRE.x - WIDTH * 0.5 - 6.0, HEIGHT + 1.7, CENTRE.z, -90.0, -14.0]
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

    /// An axis-aligned box from its centre and half-sizes.
    fn cuboid(&mut self, centre: Vec3, half: Vec3) {
        let c = centre;
        let (x, y, z) = (half.x, half.y, half.z);
        let p = |sx: f32, sy: f32, sz: f32| c + Vec3::new(sx * x, sy * y, sz * z);
        self.quad([p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.), p(1., -1., 1.)], Vec3::X);
        self.quad([p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.), p(-1., -1., -1.)], Vec3::NEG_X);
        self.quad([p(-1., 1., -1.), p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.)], Vec3::Y);
        self.quad([p(-1., -1., 1.), p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.)], Vec3::NEG_Y);
        self.quad([p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)], Vec3::Z);
        self.quad([p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)], Vec3::NEG_Z);
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

/// One wall's frame: `side` -1 for the near (low x) wall, +1 for the far;
/// `at(u, v, n)` is the point `u` along the chasm, `v` up and `n` out from
/// the face into the gap.
struct Wall {
    side: f32,
}

impl Wall {
    fn at(&self, u: f32, v: f32, n: f32) -> Vec3 {
        CENTRE + Vec3::new(self.side * (WIDTH * 0.5 - n), v, u)
    }

    /// A box on the wall from (u0, v0) to (u1, v1), standing out from `n0`
    /// to `n1`.
    fn block(&self, g: &mut Geometry, u: (f32, f32), v: (f32, f32), n: (f32, f32)) {
        let centre = self.at((u.0 + u.1) * 0.5, (v.0 + v.1) * 0.5, (n.0 + n.1) * 0.5);
        let half = Vec3::new((n.1 - n.0).abs() * 0.5, (v.1 - v.0).abs() * 0.5, (u.1 - u.0).abs() * 0.5);
        g.cuboid(centre, half);
    }
}

/// Everything the chasm is made of, by material.
#[derive(Default)]
struct Parts {
    stone: Geometry,
    dark: Geometry,
    glow: Geometry,
    /// Cables: points, thickness.
    cables: Vec<(Vec<Vec3>, f32)>,
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
    let seed = args.seed as i32;
    let mut parts = Parts::default();
    for side in [-1.0, 1.0] {
        wall(&mut parts, &Wall { side }, seed + if side < 0.0 { 0 } else { 7919 });
    }
    bridges(&mut parts, seed);
    web(&mut parts, seed);

    let stone = materials.add(StandardMaterial { base_color: Color::srgb(0.62, 0.62, 0.62), perceptual_roughness: 0.92, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.02, 0.02, 0.02), perceptual_roughness: 0.9, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(60.0), ..default() });
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
    let mut cable_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    tubes(&mut cable_mesh, parts.cables.iter().map(|(p, w)| (&p[..], (*w, *w), 0.05)));
    commands.spawn((Mesh3d(meshes.add(cable_mesh)), MeshMaterial3d(cable), Transform::IDENTITY));
    info!("the chasm: built");
}

/// A wall: its mass, then its face in strata (bands between), each stratum in
/// bays (pilasters between), each bay filled one of a few ways; ledges and
/// stairs; a few giant columns; heavy cables hanging down it.
fn wall(parts: &mut Parts, w: &Wall, seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c1);
    let half = LENGTH * 0.5;
    // The mass, its face set back a little (things stand out from it).
    w.block(&mut parts.stone, (-half, half), (0.0, HEIGHT), (-BACK, 0.0));
    // Strata, from the top down: heights 10-34 m.
    let mut top = HEIGHT;
    let mut stratum = 0;
    let mut ledges = Vec::new();
    while top > 0.0 {
        let h = (10.0 + 24.0 * r(stratum, 0)).min(top);
        let bottom = top - h;
        // The band under it: a rib; every third or so a ledge you can walk.
        let walkway = stratum % 3 == 1 || r(stratum, 1) < 0.2;
        let (bh, bn) = if walkway { (1.2, 4.0 + 2.0 * r(stratum, 2)) } else { (0.5 + 1.5 * r(stratum, 2), 0.8 + 1.8 * r(stratum, 3)) };
        if bottom > 0.0 {
            w.block(&mut parts.stone, (-half, half), (bottom - bh, bottom), (0.0, bn));
            // Some bands carry a thin line of light along their face.
            if r(stratum, 4) < 0.3 {
                w.block(&mut parts.glow, (-half, half), (bottom - bh * 0.6, bottom - bh * 0.4), (bn, bn + 0.05));
            }
            if walkway {
                ledges.push((bottom, bn));
            }
        }
        bays(parts, w, (bottom, top - if stratum == 0 { 0.0 } else { 0.0 }), seed, stratum);
        top = bottom - bh;
        stratum += 1;
    }
    // Stairs: zigzag flights between ledges, here and there along the wall.
    for (k, pair) in ledges.windows(2).enumerate() {
        let ((upper, un), (lower, ln)) = (pair[0], pair[1]);
        let u0 = -half + 40.0 + (LENGTH - 80.0) * r(k as i32, 30);
        let rise = upper - lower;
        let flights = (rise / 8.0).ceil().max(1.0) as usize;
        let n = un.min(ln) * 0.5 + 0.3;
        let mut v = lower;
        let mut u = u0;
        for f in 0..flights {
            let dir = if f % 2 == 0 { 1.0 } else { -1.0 };
            let step = rise / flights as f32;
            let run = step * 1.3;
            let a = w.at(u, v, n);
            let b = w.at(u + dir * run, v + step, n);
            parts.stone.beam(a, b, 1.6, 0.4, Vec3::Y);
            u += dir * run;
            v += step;
            // A landing.
            w.block(&mut parts.stone, (u - 1.2, u + 1.2), (v - 0.4, v), (n - 0.8, n + 0.8));
        }
    }
    // Giant half-sunk columns, spanning much of the height.
    for k in 0..4 {
        let u = -half + LENGTH * (k as f32 + 0.3 + 0.4 * r(k, 40)) / 4.0;
        let radius = 3.0 + 3.0 * r(k, 41);
        let (v0, v1) = (HEIGHT * 0.15 * r(k, 42), HEIGHT * (0.6 + 0.4 * r(k, 43)));
        parts.stone.cylinder(w.at(u, v0, radius * 0.4), w.at(u, v1, radius * 0.4), radius, 14);
    }
    // Heavy cables hanging down the face in twisted pairs.
    for k in 0..10 {
        let u = -half + LENGTH * r(k, 50);
        let v0 = HEIGHT * (0.4 + 0.6 * r(k, 51));
        let drop = 40.0 + 140.0 * r(k, 52);
        let thick = 0.25 + 0.35 * r(k, 53);
        for strand in 0..2 {
            let phase = strand as f32 * std::f32::consts::PI;
            let points: Vec<Vec3> = (0..24)
                .map(|i| {
                    let t = i as f32 / 23.0;
                    let a = phase + t * drop / 6.0;
                    w.at(u + a.cos() * thick * 1.1, v0 - drop * t, 1.5 + thick + a.sin() * thick * 1.1)
                })
                .collect();
            parts.cables.push((points, thick));
        }
    }
}

/// A stratum's bays and what fills them.
fn bays(parts: &mut Parts, w: &Wall, (v0, v1): (f32, f32), seed: i32, stratum: i32) {
    let r = |a: i32, b: i32| hash01(seed, stratum * 1000 + a, b, 0x7c2);
    let half = LENGTH * 0.5;
    let mut u = -half;
    let mut k = 0;
    while u < half {
        let width = (6.0 + 26.0 * r(k, 0)).min(half - u);
        // A pilaster at its start.
        let pw = 1.5 + 2.5 * r(k, 1);
        let pn = 1.5 + 1.5 * r(k, 2);
        w.block(&mut parts.stone, (u, u + pw), (v0, v1), (0.0, pn));
        let (b0, b1) = (u + pw, u + width);
        if b1 - b0 > 1.5 {
            match (r(k, 3) * 6.0) as u32 {
                0 => openings(parts, w, (b0, b1), (v0, v1), r(k, 4)),
                1 => frames(parts, w, (b0, b1), (v0, v1), 0.0, 4, r(k, 5)),
                2 => conduits(parts, w, (b0, b1), (v0, v1), seed, stratum * 100 + k),
                3 => louvres(parts, w, (b0, b1), (v0, v1), r(k, 6)),
                4 => {
                    frames(parts, w, (b0, b1), (v0, v1), 0.0, 2, r(k, 7));
                    let inset = 0.12 * (b1 - b0).min(v1 - v0);
                    openings(parts, w, (b0 + inset, b1 - inset), (v0 + inset, v1 - inset), r(k, 8));
                }
                // Bare: breathing room.
                _ => {}
            }
        }
        u += width;
        k += 1;
    }
}

/// Openings, tiny against the wall: either a fine speckle (rows of small
/// holes, irregular, broken by gaps, some rows denser), or long dark slits
/// running the bay. Never a grid of windows.
fn openings(parts: &mut Parts, w: &Wall, (u0, u1): (f32, f32), (v0, v1): (f32, f32), r: f32) {
    let seed = (r * 1.0e6) as i32;
    let h = |a: i32, b: i32| hash01(seed, a, b, 0x7c8);
    if r < 0.35 {
        // Slits: long horizontal slots, a few across the bay.
        let pitch = 1.5 + 3.0 * h(0, 0);
        let mut v = v0 + pitch * 0.5;
        let mut j = 0;
        while v < v1 - 0.3 {
            let (a, b) = (u0 + (u1 - u0) * 0.15 * h(j, 1), u1 - (u1 - u0) * 0.15 * h(j, 2));
            w.block(&mut parts.dark, (a, b), (v, v + 0.12 + 0.2 * h(j, 3)), (0.0, 0.06));
            v += pitch;
            j += 1;
        }
        return;
    }
    // Speckle: small holes (0.15-0.4 m) on an uneven pitch, rows broken.
    let size = 0.15 + 0.25 * h(0, 4);
    let mut v = v0 + size;
    let mut j = 0;
    while v < v1 - size {
        let pitch_u = size * (1.6 + 2.5 * h(j, 5));
        let keep = 0.35 + 0.6 * h(j, 6);
        let mut u = u0 + size;
        let mut i = 0;
        while u < u1 - size {
            if h(i * 7 + j, 7) < keep {
                w.block(&mut parts.dark, (u, u + size), (v, v + size * (1.0 + h(i, j + 9))), (0.0, 0.06));
            }
            u += pitch_u;
            i += 1;
        }
        v += size * (2.0 + 3.0 * h(j, 8));
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
fn bridges(parts: &mut Parts, seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c6);
    let near = Wall { side: -1.0 };
    let far = Wall { side: 1.0 };
    for k in 0..4 {
        let u = -LENGTH * 0.4 + LENGTH * 0.8 * (k as f32 + r(k, 0) * 0.6) / 4.0;
        let v = HEIGHT * (0.25 + 0.65 * r(k, 1));
        let wide = 3.0 + 4.0 * r(k, 2);
        let a = near.at(u, v, -1.0);
        let b = far.at(u, v, -1.0);
        parts.stone.beam(a, b, wide, 0.8, Vec3::Y);
        // Deep beams under the deck's edges.
        for s in [-1.0, 1.0] {
            let off = Vec3::Z * s * (wide * 0.5 - 0.3);
            parts.stone.beam(a + off - Vec3::Y * 1.2, b + off - Vec3::Y * 1.2, 0.5, 1.6, Vec3::Y);
        }
        // Bracing: diagonals underneath, zigzag.
        let n = 8;
        for i in 0..n {
            let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            let (p, q) = (a.lerp(b, t0), a.lerp(b, t1));
            let low = Vec3::Y * -4.5;
            let (from, to) = if i % 2 == 0 { (p - Vec3::Y * 1.6, q + low) } else { (p + low, q - Vec3::Y * 1.6) };
            parts.stone.beam(from, to, 0.4, 0.4, Vec3::Z);
        }
    }
}

/// A web of taut cables across the void at every angle, wall to wall.
fn web(parts: &mut Parts, seed: i32) {
    let r = |a: i32, b: i32| hash01(seed, a, b, 0x7c7);
    let near = Wall { side: -1.0 };
    let far = Wall { side: 1.0 };
    for k in 0..70 {
        let u0 = -LENGTH * 0.45 + LENGTH * 0.9 * r(k, 0);
        let u1 = u0 + (r(k, 1) - 0.5) * 120.0;
        let v0 = HEIGHT * (0.1 + 0.85 * r(k, 2));
        let v1 = (v0 + (r(k, 3) - 0.5) * 140.0).clamp(10.0, HEIGHT - 5.0);
        let a = near.at(u0, v0, 0.5);
        let b = far.at(u1.clamp(-LENGTH * 0.5, LENGTH * 0.5), v1, 0.5);
        let thick = if r(k, 4) < 0.2 { 0.25 + 0.3 * r(k, 5) } else { 0.05 + 0.1 * r(k, 5) };
        // Taut: a little sag, more for the long ones.
        let sag = a.distance(b) * (0.01 + 0.03 * r(k, 6));
        parts.cables.push((rope_static(a, b, sag), thick));
    }
}
