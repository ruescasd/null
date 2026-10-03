//! Megastructures: a handful of enormous geometric objects placed across the
//! world to give it scale, each built from boxes and tapered prisms.

use glam::{Quat, Vec3};

use crate::district::District;
use crate::mesh::ColumnMesh;
use crate::noise::hash01;

pub struct Landmark {
    /// Where the landmark's local origin sits in the world (on the ground).
    pub origin: Vec3,
    /// Geometry relative to `origin`.
    pub mesh: ColumnMesh,
    pub kind: &'static str,
}

/// Places landmarks on a jittered grid, some cells left empty. Each district
/// has its own kinds: beams over the floor, shards in the tiers, needles and
/// twisted towers in the stacks, bridges and hovering slabs in the broken
/// lands. `ground` gives the terrain height and `district` the district.
pub fn place(
    size: f32,
    seed: u32,
    ground: impl Fn(f32, f32) -> f32,
    district: impl Fn(f32, f32) -> District,
) -> Vec<Landmark> {
    // About one landmark every 2.7 km.
    let n = 6;
    let cell = size / n as f32;
    let seed = seed ^ 0x1a2d_3a4c;
    let mut out = Vec::new();
    for gz in 0..n {
        for gx in 0..n {
            let r = |k: i32| hash01(gx, k, gz, seed);
            if r(0) < 0.15 {
                continue;
            }
            let x = (gx as f32 + 0.5 + (r(1) - 0.5) * 0.5) * cell;
            let z = (gz as f32 + 0.5 + (r(2) - 0.5) * 0.5) * cell;
            let yaw = r(3) * std::f32::consts::TAU;
            let rr = |k: i32| r(100 + k);
            let mut b = Builder::default();
            let either = r(4) < 0.5;
            let kind = match district(x, z) {
                District::Floor => beams(&mut b, yaw, &rr),
                District::Tiers => shards(&mut b, yaw, &rr),
                District::Stacks if either => needle_field(&mut b, yaw, &rr),
                District::Stacks => twisted_tower(&mut b, yaw, &rr),
                District::Broken if either => {
                    bridge(&mut b, yaw, &rr, |dx, dz| ground(x + dx, z + dz) - ground(x, z))
                }
                District::Broken => hovering_slab(&mut b, yaw, &rr),
            };
            out.push(Landmark { origin: Vec3::new(x, ground(x, z), z), mesh: b.mesh, kind });
        }
    }
    out
}

/// Accumulates flat-shaded boxes and prisms.
#[derive(Default)]
struct Builder {
    mesh: ColumnMesh,
}

const ALBEDO: f32 = 0.16;

impl Builder {
    fn quad(&mut self, corners: [Vec3; 4], ao: [f32; 4]) {
        let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
        let base = self.mesh.positions.len() as u32;
        for (c, a) in corners.iter().zip(ao) {
            self.mesh.positions.push(c.to_array());
            self.mesh.normals.push(normal.to_array());
            self.mesh.albedo.push(ALBEDO);
            self.mesh.ao.push(a);
        }
        self.mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A prism with `sides` sides along local +Y from 0 to `height`, radius
    /// `r0` at the bottom and `r1` at the top, then rotated and moved.
    fn prism(&mut self, sides: u32, r0: f32, r1: f32, height: f32, rot: Quat, at: Vec3) {
        let ring = |r: f32, y: f32| -> Vec<Vec3> {
            (0..sides)
                .map(|i| {
                    let a = (i as f32 + 0.5) / sides as f32 * std::f32::consts::TAU;
                    at + rot * Vec3::new(a.cos() * r, y, a.sin() * r)
                })
                .collect()
        };
        let (bottom, top) = (ring(r0, 0.0), ring(r1, height));
        for i in 0..sides as usize {
            let j = (i + 1) % sides as usize;
            // Counter-clockwise seen from outside.
            self.quad([bottom[i], top[i], top[j], bottom[j]], [0.6, 1.0, 1.0, 0.6]);
        }
        let ct = at + rot * Vec3::new(0.0, height, 0.0);
        let cb = at;
        for i in 0..sides as usize {
            let j = (i + 1) % sides as usize;
            self.quad([ct, top[j], top[i], ct], [1.0; 4]);
            self.quad([cb, bottom[i], bottom[j], cb], [0.5; 4]);
        }
    }

    /// An axis-aligned (before rotation) box centred at `at`.
    fn cuboid(&mut self, half: Vec3, rot: Quat, at: Vec3) {
        // Corners in -1..1 box space; the lower ones get some occlusion.
        let faces: [[[f32; 3]; 4]; 6] = [
            [[1., -1., -1.], [1., 1., -1.], [1., 1., 1.], [1., -1., 1.]],
            [[-1., -1., 1.], [-1., 1., 1.], [-1., 1., -1.], [-1., -1., -1.]],
            [[-1., 1., -1.], [-1., 1., 1.], [1., 1., 1.], [1., 1., -1.]],
            [[-1., -1., 1.], [-1., -1., -1.], [1., -1., -1.], [1., -1., 1.]],
            [[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]],
            [[1., -1., -1.], [-1., -1., -1.], [-1., 1., -1.], [1., 1., -1.]],
        ];
        for face in faces {
            let corners = face.map(|c| at + rot * (half * Vec3::from(c)));
            let ao = face.map(|c| if c[1] < 0.0 { 0.7 } else { 1.0 });
            self.quad(corners, ao);
        }
    }
}

/// Two pylons joined by a thin deck high above the ground, with rods hanging
/// from it.
fn bridge(
    b: &mut Builder,
    yaw: f32,
    r: &impl Fn(i32) -> f32,
    ground: impl Fn(f32, f32) -> f32,
) -> &'static str {
    let span = 900.0 + r(0) * 700.0;
    let height = 240.0 + r(1) * 160.0;
    let rot = Quat::from_rotation_y(yaw);
    for end in [-0.5f32, 0.5] {
        let local = rot * Vec3::new(end * span, 0.0, 0.0);
        let g = ground(local.x, local.z);
        let at = Vec3::new(local.x, g - 20.0, local.z);
        b.prism(4, 55.0, 30.0, height - g + 50.0, rot, at);
    }
    let deck = Vec3::new(0.0, height + 30.0, 0.0);
    b.cuboid(Vec3::new(span * 0.5 + 40.0, 6.0, 14.0), rot, deck);
    b.cuboid(Vec3::new(span * 0.5 + 40.0, 2.0, 3.0), rot, deck + Vec3::Y * 26.0);
    let rods = (span / 45.0) as i32;
    for i in 1..rods {
        let t = i as f32 / rods as f32 - 0.5;
        let len = 30.0 + r(10 + i) * 120.0;
        let at = rot * Vec3::new(t * span, 0.0, 0.0) + Vec3::Y * (height + 24.0 - len);
        b.prism(4, 0.8, 0.8, len, rot, at);
    }
    "bridge"
}

/// A stack of boxes, each turned a little further, widening as it rises.
fn twisted_tower(b: &mut Builder, yaw: f32, r: &impl Fn(i32) -> f32) -> &'static str {
    let segments = 24 + (r(0) * 20.0) as i32;
    let seg_h = 22.0 + r(1) * 14.0;
    let twist = (0.04 + r(2) * 0.08) * if r(3) < 0.5 { -1.0 } else { 1.0 };
    let lean = Vec3::new(r(4) - 0.5, 0.0, r(5) - 0.5) * 2.0;
    for i in 0..segments {
        let t = i as f32 / segments as f32;
        let width = 28.0 + 60.0 * t * t;
        let rot = Quat::from_rotation_y(yaw + twist * i as f32);
        let at = Vec3::new(0.0, (i as f32 + 0.5) * seg_h - 10.0, 0.0) + lean * i as f32;
        let gap = if i % 5 == 4 { 0.35 } else { 0.48 };
        b.cuboid(Vec3::new(width, seg_h * gap, width * 0.45), rot, at);
    }
    "twisted tower"
}

/// A vast tilted slab hanging in the air, with needles pointing down at the
/// ground beneath it.
fn hovering_slab(b: &mut Builder, yaw: f32, r: &impl Fn(i32) -> f32) -> &'static str {
    let half = Vec3::new(220.0 + r(0) * 160.0, 9.0, 120.0 + r(1) * 80.0);
    let lift = 160.0 + r(2) * 140.0;
    let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_z((r(3) - 0.5) * 0.12);
    b.cuboid(half, rot, Vec3::Y * lift);
    for i in 0..14 {
        let p = Vec3::new((r(10 + i) - 0.5) * half.x * 1.6, 0.0, (r(30 + i) - 0.5) * half.z * 1.6);
        let len = 40.0 + r(50 + i) * (lift - 70.0);
        let at = rot * p + Vec3::Y * (lift - half.y);
        // Point down: rotate the prism upside down.
        b.prism(4, 2.5, 0.2, len, rot * Quat::from_rotation_x(std::f32::consts::PI), at);
    }
    "hovering slab"
}

/// A square grid of thin needles of varying height.
fn needle_field(b: &mut Builder, yaw: f32, r: &impl Fn(i32) -> f32) -> &'static str {
    let n = 7 + (r(0) * 6.0) as i32;
    let spacing = 30.0 + r(1) * 20.0;
    let rot = Quat::from_rotation_y(yaw);
    for i in 0..n {
        for j in 0..n {
            let k = i * n + j;
            let (u, v) = (i as f32 - (n - 1) as f32 * 0.5, j as f32 - (n - 1) as f32 * 0.5);
            let center = 1.0 - (u * u + v * v).sqrt() / (n as f32 * 0.7);
            let h = 60.0 + 380.0 * center.max(0.0) * (0.6 + 0.4 * r(10 + k));
            let at = rot * Vec3::new(u * spacing, -10.0, v * spacing);
            b.prism(4, 2.2, 1.2, h, rot, at);
        }
    }
    "needle field"
}

/// Colossal horizontal beams hanging high over the floor, unsupported: one
/// to three, each turned its own way, at different heights.
fn beams(b: &mut Builder, yaw: f32, r: &impl Fn(i32) -> f32) -> &'static str {
    let count = 1 + (r(0) * 3.0) as i32;
    for i in 0..count {
        let rr = |k: i32| r(10 + i * 10 + k);
        let half = Vec3::new(300.0 + rr(0) * 450.0, 7.0 + rr(1) * 6.0, 9.0 + rr(2) * 10.0);
        let height = 260.0 + rr(3) * 260.0 + i as f32 * 60.0;
        let turn = if i == 0 { 0.0 } else { (rr(4) - 0.5) * 1.6 };
        let rot = Quat::from_rotation_y(yaw + turn) * Quat::from_rotation_z((rr(5) - 0.5) * 0.06);
        let shift = Quat::from_rotation_y(yaw + 1.57) * Vec3::X * (rr(6) - 0.5) * 400.0;
        b.cuboid(half, rot, Vec3::Y * height + shift * (i as f32).min(1.0));
    }
    "beams"
}

/// A cluster of jagged triangular spikes stabbing hundreds of metres up.
fn shards(b: &mut Builder, yaw: f32, r: &impl Fn(i32) -> f32) -> &'static str {
    let count = 3 + (r(0) * 4.0) as i32;
    for i in 0..count {
        let rr = |k: i32| r(10 + i * 10 + k);
        let a = yaw + i as f32 / count as f32 * std::f32::consts::TAU + (rr(0) - 0.5);
        let dist = if i == 0 { 0.0 } else { 40.0 + rr(1) * 140.0 };
        let height = if i == 0 { 450.0 + rr(2) * 250.0 } else { 160.0 + rr(2) * 330.0 };
        let base = height * (0.12 + rr(3) * 0.08);
        let lean = Quat::from_rotation_y(a) * Quat::from_rotation_x(rr(4) * 0.35);
        let at = Vec3::new(a.cos() * dist, -25.0, a.sin() * dist);
        b.prism(3, base, 0.5, height, lean * Quat::from_rotation_y(rr(5) * 2.0), at);
    }
    "shards"
}
