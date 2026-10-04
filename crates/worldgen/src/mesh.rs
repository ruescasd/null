//! Surface nets meshing of one world column.
//!
//! The world is split horizontally into square columns of `COLUMN_CELLS` cells;
//! each column is meshed over the full vertical extent where the surface can
//! exist. Sampling uses one cell of padding on each side so vertices on the
//! border are identical in neighbouring columns, and each column only emits
//! quads for the sign-change edges it owns, so neighbours join without seams
//! or overlaps.

use std::collections::HashMap;

use glam::Vec3;

use crate::world::{Column, World};

/// Cells along each horizontal side of a column.
pub const COLUMN_CELLS: usize = 32;
/// Number of detail levels. Each level's cells are `LOD_FACTOR` times larger.
pub const LOD_LEVELS: u32 = 3;
pub const LOD_FACTOR: i32 = 4;

/// Edge length of one cell at a detail level, in metres.
pub fn voxel_size(lod: u32) -> f32 {
    (LOD_FACTOR as f32).powi(lod as i32)
}

/// Horizontal size of a column at a detail level, in metres.
pub fn column_size(lod: u32) -> f32 {
    COLUMN_CELLS as f32 * voxel_size(lod)
}

/// Samples per horizontal side: local cells -1..=COLUMN_CELLS.
const S: usize = COLUMN_CELLS + 3;

#[derive(Default, Debug)]
pub struct ColumnMesh {
    /// Positions relative to the column's (x, z) origin; y is absolute.
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Linear greyscale albedo per vertex.
    pub albedo: Vec<f32>,
    /// Sky visibility per vertex (0 = fully occluded, 1 = open sky).
    pub ao: Vec<f32>,
    /// The size of the face each vertex belongs to (its narrowest width,
    /// metres), so surface detail can scale with the geometry. Built faces
    /// fill it in; it may run short of `positions`, and missing entries are
    /// [`OPEN_GROUND`].
    pub face: Vec<f32>,
    pub indices: Vec<u32>,
}

/// The face size of everything that does not give one: open ground.
pub const OPEN_GROUND: f32 = 1000.0;

impl ColumnMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Records the size of the face whose `count` vertices were just added.
    pub fn face_size(&mut self, count: usize, size: f32) {
        let end = self.positions.len();
        self.face.resize(end - count, OPEN_GROUND);
        self.face.resize(end, size);
    }
}

/// The narrowest width of a flat convex polygon: over its edges, the
/// largest distance of any corner from that edge's line, at its smallest.
pub fn polygon_width(points: &[glam::Vec3]) -> f32 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let normal = (points[1] - points[0]).cross(points[2] - points[0]);
    let mut best = f32::INFINITY;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let Some(across) = normal.cross(b - a).try_normalize() else { continue };
        let far = points.iter().map(|p| (*p - a).dot(across).abs()).fold(0.0, f32::max);
        best = best.min(far);
    }
    if best.is_finite() { best } else { 0.0 }
}

/// World-space origin of a column (its minimum x and z corner).
pub fn column_origin(lod: u32, cx: i32, cz: i32) -> (f32, f32) {
    (cx as f32 * column_size(lod), cz as f32 * column_size(lod))
}

pub fn mesh_column(world: &World, lod: u32, cx: i32, cz: i32) -> ColumnMesh {
    let (x0, z0) = column_origin(lod, cx, cz);
    #[allow(non_snake_case)]
    let VOXEL = voxel_size(lod);
    let sample_x = |i: usize| x0 + (i as f32 - 1.0) * VOXEL;
    let sample_z = |k: usize| z0 + (k as f32 - 1.0) * VOXEL;

    let mut columns = Vec::with_capacity(S * S);
    for k in 0..S {
        for i in 0..S {
            columns.push(world.column(sample_x(i), sample_z(k)));
        }
    }

    // Vertical extent: where terrain or any overlapping structure can be.
    let mut y_min = columns.iter().map(Column::min_y).fold(f32::MAX, f32::min);
    let mut y_max = columns.iter().map(Column::max_y).fold(f32::MIN, f32::max);
    let probe = world.primitives_in(
        Vec3::new(sample_x(0), -1.0e4, sample_z(0)),
        Vec3::new(sample_x(S - 1), 1.0e4, sample_z(S - 1)),
    );
    for p in &probe {
        y_min = y_min.min(p.center.y - p.bound);
        y_max = y_max.max(p.center.y + p.bound);
    }
    let y_base = (y_min / VOXEL).floor() * VOXEL - 2.0 * VOXEL;
    let ny = (((y_max - y_base) / VOXEL).ceil() as usize) + 3;
    let sample_y = |j: usize| y_base + j as f32 * VOXEL;

    let prims = world.primitives_in(
        Vec3::new(sample_x(0), y_base, sample_z(0)),
        Vec3::new(sample_x(S - 1), sample_y(ny - 1), sample_z(S - 1)),
    );

    // Density grid, x fastest, then z, then y.
    let idx = |i: usize, j: usize, k: usize| (j * S + k) * S + i;
    let mut density = vec![0.0f32; S * S * ny];
    for k in 0..S {
        for i in 0..S {
            let col = &columns[k * S + i];
            let (x, z) = (sample_x(i), sample_z(k));
            for j in 0..ny {
                let p = Vec3::new(x, sample_y(j), z);
                density[idx(i, j, k)] = world.sample(col, &prims, p).0;
            }
        }
    }

    // One vertex per cell that straddles the surface.
    const NONE: u32 = u32::MAX;
    let cells = S - 1;
    let cidx = |i: usize, j: usize, k: usize| (j * cells + k) * cells + i;
    let mut cell_vertex = vec![NONE; cells * cells * (ny - 1)];
    let mut mesh = ColumnMesh::default();
    let mut world_pos = Vec::new();

    const CORNERS: [[usize; 3]; 8] = [
        [0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0],
        [0, 0, 1], [1, 0, 1], [0, 1, 1], [1, 1, 1],
    ];
    const EDGES: [[usize; 2]; 12] = [
        [0, 1], [2, 3], [4, 5], [6, 7],
        [0, 2], [1, 3], [4, 6], [5, 7],
        [0, 4], [1, 5], [2, 6], [3, 7],
    ];

    for j in 0..ny - 1 {
        for k in 0..cells {
            for i in 0..cells {
                let mut d = [0.0f32; 8];
                let mut inside = 0u8;
                for (c, o) in CORNERS.iter().enumerate() {
                    d[c] = density[idx(i + o[0], j + o[1], k + o[2])];
                    inside |= ((d[c] < 0.0) as u8) << c;
                }
                if inside == 0 || inside == 0xff {
                    continue;
                }
                let mut sum = Vec3::ZERO;
                let mut n = 0.0;
                for [a, b] in EDGES {
                    if (d[a] < 0.0) != (d[b] < 0.0) {
                        let t = d[a] / (d[a] - d[b]);
                        let pa = Vec3::new(CORNERS[a][0] as f32, CORNERS[a][1] as f32, CORNERS[a][2] as f32);
                        let pb = Vec3::new(CORNERS[b][0] as f32, CORNERS[b][1] as f32, CORNERS[b][2] as f32);
                        sum += pa + (pb - pa) * t;
                        n += 1.0;
                    }
                }
                let local = Vec3::new(i as f32, j as f32, k as f32) + sum / n;
                let p = Vec3::new(
                    x0 + (local.x - 1.0) * VOXEL,
                    y_base + local.y * VOXEL,
                    z0 + (local.z - 1.0) * VOXEL,
                );
                cell_vertex[cidx(i, j, k)] = world_pos.len() as u32;
                world_pos.push(p);
            }
        }
    }

    if world_pos.is_empty() {
        return mesh;
    }

    // Quads across every owned sign-change edge. An edge starting at sample p
    // along axis a is owned if p lies in local cells 0..COLUMN_CELLS (sample
    // index 1..=COLUMN_CELLS) horizontally. Axes (a, b, c) are cyclic, so
    // b x c = a and the quad below winds counter-clockwise seen from +a.
    let unit = |axis: usize| -> [usize; 3] {
        let mut u = [0; 3];
        u[axis] = 1;
        u
    };
    for j in 1..ny - 1 {
        for k in 1..=COLUMN_CELLS {
            for i in 1..=COLUMN_CELLS {
                let p = [i, j, k];
                let d0 = density[idx(i, j, k)];
                // Sample-space axes: 0 = x (i), 1 = y (j), 2 = z (k).
                for a in 0..3 {
                    let ea = unit(a);
                    let q = [p[0] + ea[0], p[1] + ea[1], p[2] + ea[2]];
                    if q[1] >= ny {
                        continue;
                    }
                    let d1 = density[idx(q[0], q[1], q[2])];
                    if (d0 < 0.0) == (d1 < 0.0) {
                        continue;
                    }
                    let (b, c) = ((a + 1) % 3, (a + 2) % 3);
                    let (eb, ec) = (unit(b), unit(c));
                    let cell = |sb: usize, sc: usize| {
                        let ci = p[0] - eb[0] * sb - ec[0] * sc;
                        let cj = p[1] - eb[1] * sb - ec[1] * sc;
                        let ck = p[2] - eb[2] * sb - ec[2] * sc;
                        cell_vertex[cidx(ci, cj, ck)]
                    };
                    let (v00, v10, v11, v01) = (cell(1, 1), cell(0, 1), cell(0, 0), cell(1, 0));
                    if [v00, v10, v11, v01].contains(&NONE) {
                        continue;
                    }
                    if d0 < 0.0 {
                        mesh.indices.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
                    } else {
                        mesh.indices.extend_from_slice(&[v00, v11, v10, v00, v01, v11]);
                    }
                }
            }
        }
    }

    // Normals from the analytic field's gradient, so they match across columns.
    let eps = 0.35 * VOXEL;
    let density_at = |p: Vec3| {
        let col = world.column(p.x, p.z);
        world.sample(&col, &prims, p)
    };
    mesh.positions.reserve(world_pos.len());
    mesh.normals.reserve(world_pos.len());
    mesh.albedo.reserve(world_pos.len());
    mesh.ao.reserve(world_pos.len());
    // AO rays only need approximate columns: quantise to the voxel grid and cache.
    let mut column_cache: HashMap<(i32, i32), Column> = HashMap::new();
    let mut solid = |p: Vec3| {
        let key = ((p.x / VOXEL).round() as i32, (p.z / VOXEL).round() as i32);
        let col = *column_cache
            .entry(key)
            .or_insert_with(|| world.column(key.0 as f32 * VOXEL, key.1 as f32 * VOXEL));
        world.sample(&col, &prims, p).0 < 0.0
    };
    for &p in &world_pos {
        let gx = density_at(p + Vec3::X * eps).0 - density_at(p - Vec3::X * eps).0;
        let gy = density_at(p + Vec3::Y * eps).0 - density_at(p - Vec3::Y * eps).0;
        let gz = density_at(p + Vec3::Z * eps).0 - density_at(p - Vec3::Z * eps).0;
        let normal = Vec3::new(gx, gy, gz).normalize_or(Vec3::Y);
        let structure = density_at(p).1;
        mesh.positions.push([p.x - x0, p.y, p.z - z0]);
        mesh.normals.push(normal.to_array());
        mesh.albedo.push(world.albedo(p, normal, structure));
        mesh.ao.push(sky_visibility(p, normal, VOXEL.sqrt(), &mut solid));
    }
    mesh
}

/// Cosine-weighted fraction of the hemisphere above `p` that is not blocked
/// by terrain within ~40 m, found by marching a fixed set of rays.
fn sky_visibility(p: Vec3, normal: Vec3, scale: f32, solid: &mut impl FnMut(Vec3) -> bool) -> f32 {
    const STEPS: [f32; 5] = [1.5, 4.0, 9.0, 18.0, 36.0];
    // (angle from the normal, number of rays around it)
    const RINGS: [(f32, usize); 3] = [(0.0, 1), (0.75, 6), (1.25, 6)];

    let (t, b) = normal.any_orthonormal_pair();
    let (mut seen, mut total) = (0.0, 0.0);
    for (ring, &(theta, count)) in RINGS.iter().enumerate() {
        let weight = theta.cos();
        for i in 0..count {
            let phi = (i as f32 + 0.5 * ring as f32) / count as f32 * std::f32::consts::TAU;
            let dir = normal * theta.cos() + (t * phi.cos() + b * phi.sin()) * theta.sin();
            total += weight;
            if !STEPS.iter().any(|&s| solid(p + (normal * 0.5 + dir * s) * scale)) {
                seen += weight;
            }
        }
    }
    seen / total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldConfig;

    #[test]
    fn column_borders_match_and_wrap() {
        let world = World::new(WorldConfig { size: 1024.0, seed: 7 });
        let n = (1024.0 / column_size(0)) as i32;
        // The column at the end of the world must equal the one before the start.
        let a = mesh_column(&world, 0, n - 1, 3);
        let b = mesh_column(&world, 0, -1, 3);
        assert!(!a.is_empty());
        assert_eq!(a.positions.len(), b.positions.len());
        for (pa, pb) in a.positions.iter().zip(&b.positions) {
            for c in 0..3 {
                assert!((pa[c] - pb[c]).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn indices_in_range() {
        let world = World::new(WorldConfig { size: 1024.0, seed: 3 });
        for cx in 0..4 {
            let m = mesh_column(&world, cx as u32 % LOD_LEVELS, cx, cx * 2);
            assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
            assert_eq!(m.indices.len() % 3, 0);
        }
    }
}
