//! Fractal structures: distance-estimated fractals turned into meshes with
//! surface nets, so they get collision, shadows and the world's lighting.
//!
//! Kept to a few iterations so they read as architecture rather than noise.
//! Fold-only fractals (KIFS) keep flat faces and straight edges and read as
//! built; the Mandelbox's sphere inversion reads as coral or pumice, so it is
//! kept only for comparison.
//!
//! Shapes are given as functions returning (distance in metres, trap), where
//! the trap is an orbit-trap value in 0..1 used for tone. Negative distance
//! is inside.

use std::thread;

use glam::{Mat3, Vec3};

use crate::mesh::ColumnMesh;

/// A shape: distance (metres, negative inside) and a 0..1 tone value.
pub trait Shape: Sync {
    fn sample(&self, p: Vec3) -> (f32, f32);
    /// Axis-aligned bounds of the solid, in metres.
    fn bounds(&self) -> (Vec3, Vec3);
}

/// A Mandelbox of scale 2 filling a cube of `size` metres, its lowest fifth
/// cut away so it can sit sunk into the ground.
pub struct MandelboxBlock {
    pub size: f32,
    pub iterations: u32,
    /// Escape radius: points whose orbit stays within it after
    /// `iterations` count as solid. Few iterations give chunky nested boxes.
    pub bailout: f32,
}

impl Shape for MandelboxBlock {
    fn sample(&self, p: Vec3) -> (f32, f32) {
        // A scale-2 Mandelbox spans -6..6.
        let k = 12.0 / self.size;
        let q = p * k;
        let (mut z, mut dr, mut trap) = (q, 1.0f32, f32::MAX);
        for _ in 0..self.iterations {
            z = z.clamp(Vec3::splat(-1.0), Vec3::splat(1.0)) * 2.0 - z;
            let r2 = z.length_squared();
            if r2 < 0.25 {
                z *= 4.0;
                dr *= 4.0;
            } else if r2 < 1.0 {
                z /= r2;
                dr /= r2;
            }
            z = z * 2.0 + q;
            dr = dr * 2.0 + 1.0;
            trap = trap.min(r2);
        }
        // Solid where the orbit stays bounded; the distance estimate gives
        // the magnitude on both sides.
        let r = z.length();
        let de = (r / dr.abs()) / k;
        let d = if r < self.bailout { -de } else { de };
        // Cut off below the bottom fifth.
        let d = d.max(-(p.y + self.size * 0.3));
        (d, (trap * 1.5).clamp(0.0, 1.0))
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let h = self.size * 0.5;
        (Vec3::new(-h, -self.size * 0.3, -h), Vec3::splat(h))
    }
}

/// A kaleidoscopic IFS (folds, sorts, rotations and scaling only, no sphere
/// inversion, so faces stay flat and edges straight: architecture rather than
/// coral), mapped onto a box of `size` metres standing on y = 0 and narrowing
/// by `taper` towards the top. With scale 3 and offset (1, 1, 1) it is a
/// twisted Menger sponge; other offsets give less familiar masses.
pub struct Kifs {
    pub size: Vec3,
    pub taper: f32,
    pub iterations: u32,
    pub scale: f32,
    pub offset: Vec3,
    /// Rotation applied at each fold; small angles break the symmetry.
    pub twist: Mat3,
}

impl Shape for Kifs {
    fn sample(&self, p: Vec3) -> (f32, f32) {
        // Map onto the fractal's -1..1 cube, narrowing with height.
        let t = (p.y / self.size.y).clamp(0.0, 1.0);
        let narrow = 1.0 - self.taper * t;
        let half = self.size * 0.5 * Vec3::new(narrow, 1.0, narrow);
        let q = Vec3::new(p.x, p.y - self.size.y * 0.5, p.z) / half;
        let (scale, offset) = (self.scale, self.offset);
        let (mut z, mut trap) = (q, f32::MAX);
        for _ in 0..self.iterations {
            z = self.twist * z;
            z = z.abs();
            if z.x < z.y {
                z = Vec3::new(z.y, z.x, z.z);
            }
            if z.x < z.z {
                z = Vec3::new(z.z, z.y, z.x);
            }
            if z.y < z.z {
                z = Vec3::new(z.x, z.z, z.y);
            }
            z = z * scale - offset * (scale - 1.0);
            if z.z < -0.5 * offset.z * (scale - 1.0) {
                z.z += offset.z * (scale - 1.0);
            }
            trap = trap.min((z - Vec3::new(1.0, 0.0, 0.0)).length_squared());
        }
        let cube = (z.abs() - Vec3::ONE).max_element();
        // Back to metres; the narrowest scale keeps the estimate conservative.
        let d = cube * scale.powi(-(self.iterations as i32)) * half.min_element();
        (d, (trap * 0.15).clamp(0.0, 1.0))
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let h = self.size * 0.5;
        (Vec3::new(-h.x, 0.0, -h.z), Vec3::new(h.x, self.size.y, h.z))
    }
}

/// Meshes a shape with surface nets at `voxel` metres, with smooth normals,
/// distance-field ambient occlusion and tone from the orbit trap.
pub fn build(shape: &dyn Shape, voxel: f32) -> ColumnMesh {
    let (lo, hi) = shape.bounds();
    let min = lo - Vec3::splat(voxel * 2.0);
    let dims = ((hi + Vec3::splat(voxel * 2.0) - min) / voxel).ceil().as_uvec3() + 1;
    let (nx, ny, nz) = (dims.x as usize, dims.y as usize, dims.z as usize);

    // Sample the grid, a slab of z per thread.
    let mut values = vec![0.0f32; nx * ny * nz];
    let threads = thread::available_parallelism().map_or(8, |n| n.get());
    let per = nz.div_ceil(threads);
    thread::scope(|scope| {
        for (t, slab) in values.chunks_mut(per * nx * ny).enumerate() {
            scope.spawn(move || {
                for (i, v) in slab.iter_mut().enumerate() {
                    let z = t * per + i / (nx * ny);
                    let y = (i / nx) % ny;
                    let x = i % nx;
                    let p = min + Vec3::new(x as f32, y as f32, z as f32) * voxel;
                    *v = shape.sample(p).0;
                }
            });
        }
    });

    let (positions, indices) = surface_nets(&values, [nx, ny, nz]);
    let world: Vec<Vec3> = positions.iter().map(|g| min + *g * voxel).collect();

    // Normals, occlusion and tone per vertex, also in parallel.
    let mut normals = vec![[0.0f32; 3]; world.len()];
    let mut ao = vec![0.0f32; world.len()];
    let mut albedo = vec![0.0f32; world.len()];
    let per = world.len().div_ceil(threads).max(1);
    thread::scope(|scope| {
        for (((points, n), a), c) in world
            .chunks(per)
            .zip(normals.chunks_mut(per))
            .zip(ao.chunks_mut(per))
            .zip(albedo.chunks_mut(per))
        {
            scope.spawn(move || {
                let d = |p: Vec3| shape.sample(p).0;
                let e = voxel * 0.5;
                for (i, &p) in points.iter().enumerate() {
                    let g = Vec3::new(
                        d(p + Vec3::X * e) - d(p - Vec3::X * e),
                        d(p + Vec3::Y * e) - d(p - Vec3::Y * e),
                        d(p + Vec3::Z * e) - d(p - Vec3::Z * e),
                    );
                    let normal = g.normalize_or(Vec3::Y);
                    n[i] = normal.to_array();
                    // Inigo Quilez's distance-field AO: how much closer the
                    // nearest surface is than the step along the normal.
                    let mut occlusion = 0.0;
                    let mut weight = 1.0;
                    for k in 0..5 {
                        let h = voxel * 1.5 * 2f32.powi(k);
                        occlusion += (h - d(p + normal * h)).max(0.0) / h * weight;
                        weight *= 0.6;
                    }
                    a[i] = (1.0 - occlusion * 0.45).clamp(0.15, 1.0);
                    let trap = shape.sample(p).1;
                    c[i] = 0.06 + 0.16 * trap;
                }
            });
        }
    });

    ColumnMesh {
        positions: world.iter().map(|p| p.to_array()).collect(),
        normals,
        albedo,
        ao,
        face: Vec::new(),
        glow: Vec::new(),
        indices,
    }
}

/// Surface nets over a full grid of samples (x fastest, then y, then z):
/// one vertex per cell the surface crosses, one quad per crossing edge.
/// Returns vertex positions in grid units and triangle indices, facing out
/// of the negative (solid) region.
pub fn surface_nets(values: &[f32], [nx, ny, nz]: [usize; 3]) -> (Vec<Vec3>, Vec<u32>) {
    const NONE: u32 = u32::MAX;
    let idx = |x: usize, y: usize, z: usize| (z * ny + y) * nx + x;
    let (cx, cy, cz) = (nx - 1, ny - 1, nz - 1);
    let cidx = |x: usize, y: usize, z: usize| (z * cy + y) * cx + x;
    let mut cell_vertex = vec![NONE; cx * cy * cz];
    let mut positions = Vec::new();

    const CORNERS: [[usize; 3]; 8] =
        [[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0], [0, 0, 1], [1, 0, 1], [0, 1, 1], [1, 1, 1]];
    const EDGES: [[usize; 2]; 12] = [
        [0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7],
    ];
    for z in 0..cz {
        for y in 0..cy {
            for x in 0..cx {
                let mut d = [0.0f32; 8];
                let mut inside = 0u8;
                for (c, o) in CORNERS.iter().enumerate() {
                    d[c] = values[idx(x + o[0], y + o[1], z + o[2])];
                    inside |= ((d[c] < 0.0) as u8) << c;
                }
                if inside == 0 || inside == 0xff {
                    continue;
                }
                let (mut sum, mut count) = (Vec3::ZERO, 0.0);
                for [a, b] in EDGES {
                    if (d[a] < 0.0) != (d[b] < 0.0) {
                        let t = d[a] / (d[a] - d[b]);
                        let pa = Vec3::new(CORNERS[a][0] as f32, CORNERS[a][1] as f32, CORNERS[a][2] as f32);
                        let pb = Vec3::new(CORNERS[b][0] as f32, CORNERS[b][1] as f32, CORNERS[b][2] as f32);
                        sum += pa + (pb - pa) * t;
                        count += 1.0;
                    }
                }
                cell_vertex[cidx(x, y, z)] = positions.len() as u32;
                positions.push(Vec3::new(x as f32, y as f32, z as f32) + sum / count);
            }
        }
    }

    // A crossing edge from sample p along axis a is shared by the four cells
    // p, p - b, p - c and p - b - c; with (a, b, c) cyclic the quad below
    // winds counter-clockwise seen from +a.
    let mut indices = Vec::new();
    let dims = [nx, ny, nz];
    for z in 1..nz - 1 {
        for y in 1..ny - 1 {
            for x in 1..nx - 1 {
                let p = [x, y, z];
                let d0 = values[idx(x, y, z)];
                for a in 0..3 {
                    let mut q = p;
                    q[a] += 1;
                    if q[a] >= dims[a] {
                        continue;
                    }
                    let d1 = values[idx(q[0], q[1], q[2])];
                    if (d0 < 0.0) == (d1 < 0.0) {
                        continue;
                    }
                    let (b, c) = ((a + 1) % 3, (a + 2) % 3);
                    let cell = |sb: usize, sc: usize| {
                        let mut k = p;
                        k[b] -= sb;
                        k[c] -= sc;
                        if k[0] >= cx || k[1] >= cy || k[2] >= cz {
                            return NONE;
                        }
                        cell_vertex[cidx(k[0], k[1], k[2])]
                    };
                    let (v00, v10, v11, v01) = (cell(1, 1), cell(0, 1), cell(0, 0), cell(1, 0));
                    if [v00, v10, v11, v01].contains(&NONE) {
                        continue;
                    }
                    if d0 < 0.0 {
                        indices.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
                    } else {
                        indices.extend_from_slice(&[v00, v11, v10, v00, v01, v11]);
                    }
                }
            }
        }
    }
    (positions, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ball;
    impl Shape for Ball {
        fn sample(&self, p: Vec3) -> (f32, f32) {
            (p.length() - 10.0, 0.5)
        }
        fn bounds(&self) -> (Vec3, Vec3) {
            (Vec3::splat(-10.0), Vec3::splat(10.0))
        }
    }

    #[test]
    fn ball_mesh_is_closed_and_round() {
        let mesh = build(&Ball, 1.0);
        assert!(!mesh.indices.is_empty());
        for p in &mesh.positions {
            assert!((Vec3::from(*p).length() - 10.0).abs() < 0.6);
        }
        // Normals point outwards.
        for (p, n) in mesh.positions.iter().zip(&mesh.normals) {
            assert!(Vec3::from(*p).dot(Vec3::from(*n)) > 0.0);
        }
    }

    #[test]
    fn fractals_produce_geometry() {
        let block = build(&MandelboxBlock { size: 60.0, iterations: 4, bailout: 6.0 }, 1.5);
        let spire = build(
            &Kifs {
                size: Vec3::new(20.0, 80.0, 20.0),
                taper: 0.55,
                iterations: 3,
                scale: 3.0,
                offset: Vec3::ONE,
                twist: Mat3::from_rotation_y(0.2),
            },
            1.0,
        );
        assert!(block.indices.len() > 1000);
        assert!(spire.indices.len() > 1000);
    }
}
