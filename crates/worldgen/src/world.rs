//! The world's density field: negative inside solid rock, positive in air.
//!
//! Everything is periodic in x and z with period `WorldConfig::size`, so the
//! world wraps seamlessly. The field is built from three layers:
//! - a 2D height field (warped ridges, flat basins, terraced mesas);
//! - 3D noise near the surface, which produces overhangs and eroded hoodoos;
//! - signed-distance primitives (monoliths, arches, spires) blended in with a
//!   smooth union so the structures read as grown out of the ground.

use glam::{Quat, Vec3};

use crate::noise::{Fbm, hash01};

#[derive(Clone, Copy, Debug)]
pub struct WorldConfig {
    /// Wrap period in metres. Must be a multiple of the column size and of
    /// every noise wavelength (powers of two are safest).
    pub size: f32,
    pub seed: u32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self { size: 16384.0, seed: 1 }
    }
}

/// Per-(x, z) values shared by every sample in a vertical line.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub height: f32,
    /// Amplitude of the 3D detail noise, in metres.
    pub detail_amp: f32,
    /// 0..1 strength of the fin (wall labyrinth) feature.
    pub fin: f32,
}

const FIN_HEIGHT: f32 = 26.0;

impl Column {
    /// Below this, the density is guaranteed to be solid terrain.
    pub fn min_y(&self) -> f32 {
        self.height - self.detail_amp * 1.1 - 2.0
    }

    /// Above this, terrain (excluding structures) is guaranteed to be air.
    pub fn max_y(&self) -> f32 {
        let fin_top = if self.fin > 0.0 { FIN_HEIGHT } else { 0.0 };
        self.height + (self.detail_amp * 1.1).max(fin_top) + 2.0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// Box with the given half extents.
    Slab { half: Vec3 },
    /// Ring standing in the local XY plane.
    Ring { major: f32, minor: f32 },
    /// Tapered pillar along local Y, centred on the origin.
    Spire { base: f32, top: f32, height: f32 },
}

/// A structure piece placed in the world.
#[derive(Clone, Copy, Debug)]
pub struct Primitive {
    pub center: Vec3,
    pub inv_rot: Quat,
    pub shape: Shape,
    /// Radius of a sphere around `center` that contains the whole shape.
    pub bound: f32,
}

impl Primitive {
    fn sdf_local(&self, p: Vec3) -> f32 {
        match self.shape {
            Shape::Slab { half } => {
                let q = p.abs() - half;
                q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
            }
            Shape::Ring { major, minor } => {
                let qx = (p.x * p.x + p.y * p.y).sqrt() - major;
                (qx * qx + p.z * p.z).sqrt() - minor
            }
            Shape::Spire { base, top, height } => {
                let p = Vec3::new(p.x, p.y + height * 0.5, p.z);
                let t = (p.y / height).clamp(0.0, 1.0);
                let r = base + (top - base) * t;
                let radial = (p.x * p.x + p.z * p.z).sqrt() - r;
                // Scale the radial term by the cone's slope so it stays close to
                // a true distance.
                let slope = (base - top) / height;
                let radial = radial / (1.0 + slope * slope).sqrt();
                let cap = (-p.y).max(p.y - height);
                radial.max(cap)
            }
        }
    }
}

pub struct World {
    pub config: WorldConfig,
    warp_x: Fbm,
    warp_z: Fbm,
    base: Fbm,
    ridges: Fbm,
    ridge_mask: Fbm,
    terrace_mask: Fbm,
    erosion_mask: Fbm,
    detail: Fbm,
    detail_warp: Fbm,
    fin_noise: Fbm,
    fin_mask: Fbm,
    strata: Fbm,
    primitives: Vec<Primitive>,
}

/// Polynomial smooth minimum. Returns the blended value and the weight of `b`.
#[inline]
fn smin(a: f32, b: f32, k: f32) -> (f32, f32) {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    (b + (a - b) * h - k * h * (1.0 - h), 1.0 - h)
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl World {
    pub fn new(config: WorldConfig) -> Self {
        let s = config.seed.wrapping_mul(0x9e37_79b9);
        let mut world = Self {
            config,
            warp_x: Fbm::new(512.0, 3, 0.5, s ^ 1),
            warp_z: Fbm::new(512.0, 3, 0.5, s ^ 2),
            base: Fbm::new(1024.0, 5, 0.5, s ^ 3),
            ridges: Fbm::new(256.0, 6, 0.5, s ^ 4),
            ridge_mask: Fbm::new(512.0, 2, 0.5, s ^ 5),
            terrace_mask: Fbm::new(512.0, 2, 0.5, s ^ 6),
            erosion_mask: Fbm::new(256.0, 2, 0.5, s ^ 7),
            detail: Fbm::new(64.0, 3, 0.45, s ^ 8),
            detail_warp: Fbm::new(128.0, 2, 0.5, s ^ 9),
            fin_noise: Fbm::new(64.0, 2, 0.3, s ^ 10),
            fin_mask: Fbm::new(256.0, 2, 0.5, s ^ 11),
            strata: Fbm::new(32.0, 2, 0.5, s ^ 12),
            primitives: Vec::new(),
        };
        world.primitives = world.place_structures();
        world
    }

    pub fn size(&self) -> f32 {
        self.config.size
    }

    pub fn primitives(&self) -> &[Primitive] {
        &self.primitives
    }

    /// Shortest signed offset from `b` to `a` along a wrapped axis.
    #[inline]
    pub fn wrap_delta(&self, a: f32, b: f32) -> f32 {
        let w = self.config.size;
        (a - b + w * 0.5).rem_euclid(w) - w * 0.5
    }

    pub fn column(&self, x: f32, z: f32) -> Column {
        let w = self.config.size;
        let qx = x + self.warp_x.sample2(x, z, w) * 140.0;
        let qz = z + self.warp_z.sample2(x, z, w) * 140.0;

        // Low areas are squashed into broad, flat basins.
        let base = self.base.sample2(qx, qz, w);
        let base = if base < 0.0 { base * 0.2 } else { base };

        let ridge_mask = smoothstep(-0.25, 0.45, self.ridge_mask.sample2(x, z, w));
        let r = self.ridges.ridged2(qx, qz, w);
        let mut height = base * 60.0 + r * r * 120.0 * ridge_mask + r * 10.0;

        // Stepped mesas: flat treads with abrupt risers.
        let terrace = smoothstep(0.05, 0.35, self.terrace_mask.sample2(x, z, w));
        if terrace > 0.0 {
            let step = 11.0;
            let k = height / step;
            let stepped = (k.floor() + smoothstep(0.5, 1.0, k - k.floor())) * step;
            height += (stepped - height) * terrace;
        }

        let erosion = smoothstep(0.0, 0.4, self.erosion_mask.sample2(x, z, w));
        let fin = smoothstep(0.2, 0.4, self.fin_mask.sample2(x, z, w));
        Column { height, detail_amp: 2.0 + 18.0 * erosion, fin }
    }

    /// Density of terrain only (no structures), using a precomputed column.
    pub fn terrain_density(&self, col: &Column, x: f32, y: f32, z: f32) -> f32 {
        let mut d = y - col.height;
        if y < col.min_y() || y > col.max_y() {
            return d;
        }
        let w = self.config.size;

        let s = self.detail_warp.sample3(x, y, z, w) * 10.0;
        d += self.detail.sample3(x + s, y * 1.3 + s, z - s, w) * col.detail_amp;

        if col.fin > 0.0 && y < col.height + FIN_HEIGHT {
            // Vertically stretched noise; its zero set forms tall curved sheets.
            let n = self.fin_noise.sample3(x, y * 0.3, z, w);
            let half_thickness = 0.045;
            // Gradient magnitude is roughly 2 per 64 m lattice cell.
            let sheet = (n.abs() - half_thickness) * 32.0;
            let top = y - col.height - FIN_HEIGHT * col.fin;
            d = smin(d, sheet.max(top), 1.5).0;
        }
        d
    }

    /// Primitives whose bounds overlap the axis-aligned box (x and z wrapped).
    pub fn primitives_in(&self, min: Vec3, max: Vec3) -> Vec<Primitive> {
        let c = (min + max) * 0.5;
        let half = (max - min) * 0.5;
        self.primitives
            .iter()
            .filter(|p| {
                let dx = self.wrap_delta(p.center.x, c.x).abs() - half.x;
                let dz = self.wrap_delta(p.center.z, c.z).abs() - half.z;
                let dy = (p.center.y - c.y).abs() - half.y;
                dx < p.bound && dy < p.bound && dz < p.bound
            })
            .copied()
            .collect()
    }

    /// Distance to the nearest structure, or `f32::MAX` if none are given.
    pub fn structure_distance(&self, prims: &[Primitive], p: Vec3) -> f32 {
        let mut d = f32::MAX;
        for prim in prims {
            let local = Vec3::new(
                self.wrap_delta(p.x, prim.center.x),
                p.y - prim.center.y,
                self.wrap_delta(p.z, prim.center.z),
            );
            if local.length_squared() > prim.bound * prim.bound {
                d = d.min(local.length() - prim.bound + 1.0);
                continue;
            }
            d = d.min(prim.sdf_local(prim.inv_rot * local));
        }
        d
    }

    /// Full density at a point, plus the structure material weight (0..1).
    pub fn sample(&self, col: &Column, prims: &[Primitive], p: Vec3) -> (f32, f32) {
        let terrain = self.terrain_density(col, p.x, p.y, p.z);
        if prims.is_empty() {
            return (terrain, 0.0);
        }
        let s = self.structure_distance(prims, p);
        if s == f32::MAX {
            return (terrain, 0.0);
        }
        smin(terrain, s, 2.5)
    }

    /// Greyscale albedo (linear) for a surface point.
    pub fn albedo(&self, p: Vec3, normal: Vec3, structure: f32) -> f32 {
        let w = self.config.size;
        let band = (p.y * 0.55 + self.strata.sample2(p.x, p.z, w) * 6.0).sin() * 0.5 + 0.5;
        let rock = 0.1 + 0.035 * band * band;
        let dust = 0.2 + 0.03 * self.strata.sample2(p.z, p.x, w);
        let flat = smoothstep(0.78, 0.94, normal.y);
        let ground = rock + (dust - rock) * flat;
        ground + (0.025 - ground) * structure.clamp(0.0, 1.0)
    }

    fn place_structures(&self) -> Vec<Primitive> {
        const CELL: f32 = 256.0;
        let w = self.config.size;
        let n = (w / CELL) as i32;
        let seed = self.config.seed ^ 0x5151_7a7a;
        let mut out = Vec::new();

        for cz in 0..n {
            for cx in 0..n {
                let r = |k: i32| hash01(cx, k, cz, seed);
                let x = (cx as f32 + 0.5 + (r(1) - 0.5) * 0.6) * CELL;
                let z = (cz as f32 + 0.5 + (r(2) - 0.5) * 0.6) * CELL;
                // Sit on the lowest nearby ground so pieces don't float.
                let ground = [(0.0, 0.0), (6.0, 0.0), (-6.0, 0.0), (0.0, 6.0), (0.0, -6.0)]
                    .iter()
                    .map(|(ox, oz)| self.column(x + ox, z + oz).height)
                    .fold(f32::MAX, f32::min);
                let yaw = r(3) * std::f32::consts::TAU;
                let kind = r(0);

                if kind < 0.3 {
                    continue;
                } else if kind < 0.6 {
                    // A group of tall, slightly leaning slabs.
                    let count = 1 + (r(4) * 3.0) as i32;
                    for i in 0..count {
                        let rr = |k: i32| hash01(cx, 100 + i * 10 + k, cz, seed);
                        let half = Vec3::new(2.5 + rr(0) * 5.0, 18.0 + rr(1) * 40.0, 0.8 + rr(2) * 1.6);
                        let offset = Quat::from_rotation_y(yaw)
                            * Vec3::new((i as f32 - (count - 1) as f32 * 0.5) * (half.x * 2.0 + 6.0), 0.0, 0.0);
                        let lean = Quat::from_rotation_z((rr(3) - 0.5) * 0.25)
                            * Quat::from_rotation_x((rr(4) - 0.5) * 0.15);
                        let rot = Quat::from_rotation_y(yaw + (rr(5) - 0.5) * 0.3) * lean;
                        out.push(Primitive {
                            center: Vec3::new(x + offset.x, ground + half.y - 8.0, z + offset.z),
                            inv_rot: rot.inverse(),
                            shape: Shape::Slab { half },
                            bound: half.length() + 1.0,
                        });
                    }
                } else if kind < 0.8 {
                    // A partly buried ring standing on edge.
                    let major = 22.0 + r(4) * 30.0;
                    let minor = 2.5 + r(5) * 4.0;
                    let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_z((r(6) - 0.5) * 0.4);
                    out.push(Primitive {
                        center: Vec3::new(x, ground + major * (0.35 + r(7) * 0.4), z),
                        inv_rot: rot.inverse(),
                        shape: Shape::Ring { major, minor },
                        bound: major + minor + 1.0,
                    });
                } else {
                    // A cluster of thin leaning spires.
                    let count = 3 + (r(4) * 5.0) as i32;
                    for i in 0..count {
                        let rr = |k: i32| hash01(cx, 200 + i * 10 + k, cz, seed);
                        let a = rr(0) * std::f32::consts::TAU;
                        let dist = rr(1) * 30.0;
                        let height = 30.0 + rr(2) * 70.0;
                        let base = 2.0 + rr(3) * 3.0;
                        let rot = Quat::from_rotation_y(a)
                            * Quat::from_rotation_x(0.05 + rr(4) * 0.2);
                        let (px, pz) = (x + a.cos() * dist, z + a.sin() * dist);
                        let bottom = Vec3::new(px, ground - 6.0, pz);
                        let mid = bottom + rot * Vec3::new(0.0, height * 0.5, 0.0);
                        out.push(Primitive {
                            center: mid,
                            inv_rot: rot.inverse(),
                            shape: Shape::Spire { base, top: 0.3, height },
                            bound: height * 0.5 + base + 1.0,
                        });
                    }
                }
            }
        }
        out
    }
}
