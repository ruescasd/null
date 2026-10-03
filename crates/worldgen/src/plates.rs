//! A world of flat polygonal plates: a hierarchical Voronoi tessellation in
//! which every polygon is a horizontal top at its own height, extruded down
//! into a prism. The result is ledges and terraces with crisp edges.
//!
//! Three levels of cells (`GRID`): big plates cover gentle ground; on steep
//! ground (or at random) a plate splits into the pieces of the next level's
//! Voronoi diagram that fall inside it, and those can split again. Heights
//! follow a smooth landform, quantised, so neighbouring plates differ by small
//! steps rather than at random. Everything tiles with period `size`.
//!
//! Districts (see `district.rs`) set the rules: relief, which ledge heights
//! exist, pillars, tone. Canals (see `canal.rs`) cut their corridors through
//! the plates and bring their own geometry.

use std::collections::HashMap;

use glam::{DVec2, Vec3};

use crate::canal::{CanalHit, Canals, PIPE_RADIUS};
use crate::district::{District, DistrictMap};
use crate::mesh::{ColumnMesh, column_size};
use crate::noise::{Fbm, hash01};

/// Grid spacing of each level's Voronoi sites, in metres.
pub const GRID: [f64; 3] = [128.0, 32.0, 8.0];
/// Site offset from its grid cell centre, as a fraction of the grid (±half).
/// Kept at or below 0.6 so a 3x3 search always finds the nearest site.
const JITTER: f64 = 0.6;

pub struct PlateWorld {
    size: f64,
    seed: u32,
    warp_x: Fbm,
    warp_z: Fbm,
    base: Fbm,
    ridges: Fbm,
    ridge_mask: Fbm,
    split_mask: Fbm,
    pub districts: DistrictMap,
    pub canals: Canals,
}

/// Flow inside a canal at a point.
#[derive(Clone, Copy, Debug)]
pub struct CanalFlow {
    /// Horizontal unit direction the flow pushes along.
    pub dir: glam::Vec2,
    /// Height of the canal's floor (bottom of the pipe) here.
    pub floor: f32,
    /// Signed distance from the centreline.
    pub offset: f32,
    /// Unit direction in which `offset` grows.
    pub across: glam::Vec2,
}

/// The plate covering a point.
#[derive(Clone, Copy, Debug)]
pub struct Plate {
    pub height: f64,
    pub albedo: f32,
    /// Deepest level reached (0..=2).
    pub level: usize,
    /// Grid index of the site at each level down to `level`.
    pub sites: [(i32, i32); 3],
}

/// Per-call memo of landform heights at sites; they are the expensive part.
#[derive(Default)]
pub struct Cache {
    landform: HashMap<(usize, i32, i32), f64>,
}

#[inline]
fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl PlateWorld {
    pub fn new(size: f32, seed: u32) -> Self {
        let s = seed.wrapping_mul(0x9e37_79b9) ^ 0x7a7e;
        Self {
            size: size as f64,
            seed: s,
            warp_x: Fbm::new(1024.0, 3, 0.5, s ^ 1),
            warp_z: Fbm::new(1024.0, 3, 0.5, s ^ 2),
            base: Fbm::new(2048.0, 4, 0.5, s ^ 3),
            ridges: Fbm::new(512.0, 4, 0.45, s ^ 4),
            ridge_mask: Fbm::new(1024.0, 2, 0.5, s ^ 5),
            split_mask: Fbm::new(512.0, 2, 0.5, s ^ 6),
            districts: DistrictMap::new(size as f64, seed),
            canals: Canals::new(size as f64, seed),
        }
    }

    /// The district a point belongs to.
    pub fn district(&self, x: f64, z: f64) -> District {
        self.districts.at(x, z)
    }

    /// The landform scaled by the local district's relief: what plates fit.
    fn shaped(&self, x: f64, z: f64) -> f64 {
        self.landform(x, z) * self.districts.blended(x, z).0
    }

    /// Height of a canal's floor at a point on its centreline: the broad
    /// shape of the ground (no ridges), so canals run in trenches through
    /// high ground and on embankments over low ground, with gentle grades.
    pub fn canal_floor(&self, c: DVec2) -> f64 {
        let base = self.base.sample2(c.x as f32, c.y as f32, self.size as f32) as f64;
        let base = if base < 0.0 { base * 0.25 } else { base };
        let relief = self.districts.blended(c.x, c.y).0.min(0.8);
        base * 90.0 * relief - PIPE_RADIUS + 1.0
    }

    /// The canal flow at a point, if it is inside a canal's pipe.
    pub fn canal_at(&self, x: f32, z: f32) -> Option<CanalFlow> {
        let hit: CanalHit = self.canals.hit(DVec2::new(x as f64, z as f64), 0.0)?;
        (hit.offset.abs() < PIPE_RADIUS).then(|| CanalFlow {
            dir: glam::Vec2::new(hit.flow_dir.x as f32, hit.flow_dir.y as f32),
            floor: self.canal_floor(hit.center) as f32,
            offset: hit.offset as f32,
            across: glam::Vec2::new(hit.across.x as f32, hit.across.y as f32),
        })
    }

    /// The nearest point on any canal's centreline, its flow direction and
    /// floor height there.
    pub fn nearest_canal(&self, x: f32, z: f32) -> Option<(glam::Vec2, glam::Vec2, f32)> {
        let hit = self.canals.nearest(DVec2::new(x as f64, z as f64))?;
        Some((
            glam::Vec2::new(hit.center.x as f32, hit.center.y as f32),
            glam::Vec2::new(hit.flow_dir.x as f32, hit.flow_dir.y as f32),
            self.canal_floor(hit.center) as f32,
        ))
    }

    /// Height of whatever surface is at (x, z): a canal, or a plate.
    fn surface(&self, cache: &mut Cache, x: f64, z: f64, max_level: usize) -> f64 {
        let p = DVec2::new(x, z);
        if let Some(h) = self.canals.surface(p, &|c| self.canal_floor(c)) {
            return h;
        }
        self.plate_at(cache, x, z, max_level).height
    }

    pub fn size(&self) -> f32 {
        self.size as f32
    }

    fn rand(&self, level: usize, g: (i32, i32), k: i32) -> f64 {
        let n = (self.size / GRID[level]) as i32;
        hash01(g.0.rem_euclid(n), level as i32 * 1000 + k, g.1.rem_euclid(n), self.seed) as f64
    }

    /// Smooth large-scale terrain height that plates are fitted to.
    pub fn landform(&self, x: f64, z: f64) -> f64 {
        let w = self.size as f32;
        let (xf, zf) = (x as f32, z as f32);
        let qx = xf + self.warp_x.sample2(xf, zf, w) * 300.0;
        let qz = zf + self.warp_z.sample2(xf, zf, w) * 300.0;
        let base = self.base.sample2(qx, qz, w) as f64;
        let base = if base < 0.0 { base * 0.25 } else { base };
        let mask = smoothstep(-0.2, 0.5, self.ridge_mask.sample2(xf, zf, w) as f64);
        let r = self.ridges.ridged2(qx, qz, w) as f64;
        base * 90.0 + r * r * 160.0 * mask
    }

    fn site(&self, level: usize, g: (i32, i32)) -> DVec2 {
        let grid = GRID[level];
        let jx = (self.rand(level, g, 1) - 0.5) * JITTER;
        let jz = (self.rand(level, g, 2) - 0.5) * JITTER;
        DVec2::new((g.0 as f64 + 0.5 + jx) * grid, (g.1 as f64 + 0.5 + jz) * grid)
    }

    fn nearest_site(&self, level: usize, p: DVec2) -> (i32, i32) {
        let grid = GRID[level];
        let (cx, cz) = ((p.x / grid).floor() as i32, (p.y / grid).floor() as i32);
        let mut best = (cx, cz);
        let mut best_d = f64::MAX;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let g = (cx + dx, cz + dz);
                let d = self.site(level, g).distance_squared(p);
                if d < best_d {
                    best_d = d;
                    best = g;
                }
            }
        }
        best
    }

    fn landform_at_site(&self, cache: &mut Cache, level: usize, g: (i32, i32)) -> f64 {
        let n = (self.size / GRID[level]) as i32;
        let key = (level, g.0.rem_euclid(n), g.1.rem_euclid(n));
        if let Some(&h) = cache.landform.get(&key) {
            return h;
        }
        let p = self.site(level, g);
        let h = self.shaped(p.x, p.y);
        cache.landform.insert(key, h);
        h
    }

    /// Steepness of the landform around a site (metres of rise per metre).
    fn slope(&self, level: usize, g: (i32, i32)) -> f64 {
        let p = self.site(level, g);
        let e = GRID[level] * 0.35;
        let dx = self.shaped(p.x + e, p.y) - self.shaped(p.x - e, p.y);
        let dz = self.shaped(p.x, p.y + e) - self.shaped(p.x, p.y - e);
        (dx * dx + dz * dz).sqrt() / (2.0 * e)
    }

    fn splits(&self, level: usize, g: (i32, i32)) -> bool {
        let p = self.site(level, g);
        if level >= 2 {
            return false;
        }
        let rules = self.districts.at(p.x, p.y).rules();
        let mask = self.split_mask.sample2(p.x as f32, p.y as f32, self.size as f32) as f64;
        let chance = rules.split[level] + rules.split_mask_gain[level] * mask.max(0.0);
        self.slope(level, g) * GRID[level] > rules.split_rise[level] || self.rand(level, g, 3) < chance
    }

    /// Height and look of the plate with the given site chain, following
    /// the rules of the district its biggest plate belongs to.
    fn plate(&self, cache: &mut Cache, sites: [(i32, i32); 3], level: usize) -> Plate {
        let s0 = self.site(0, sites[0]);
        let rules = self.districts.at(s0.x, s0.y).rules();
        let base_albedo = self.districts.blended(s0.x, s0.y).1;
        let step = |h: f64, q: f64| (h / q).round() * q;

        let l0 = self.landform_at_site(cache, 0, sites[0]);
        let mut height = step(l0, rules.quantum0);
        let r = self.rand(0, sites[0], 4);
        if r < rules.mesa_chance {
            height += 25.0 + self.rand(0, sites[0], 5) * 60.0;
        } else if r < rules.mesa_chance + rules.pit_chance {
            height -= 6.0 + self.rand(0, sites[0], 5) * 14.0;
        }
        let mut albedo = base_albedo + rules.albedo_spread * (self.rand(0, sites[0], 6) - 0.5) * 2.0;
        if level >= 1 {
            // Follow the landform locally, in the district's steps, plus a nudge.
            let l1 = self.landform_at_site(cache, 1, sites[1]);
            let nudge = ((self.rand(1, sites[1], 4) - 0.5) * 2.0 * rules.nudge).round() * rules.quantum;
            height += step((l1 - l0) * 0.85, rules.quantum) + nudge;
            albedo += rules.albedo_spread * (self.rand(1, sites[1], 6) - 0.5);
        }
        if level >= 2 {
            let l1 = self.landform_at_site(cache, 1, sites[1]);
            let l2 = self.landform_at_site(cache, 2, sites[2]);
            height += step((l2 - l1) * 0.85, rules.quantum);
            if self.rand(2, sites[2], 4) < rules.pillar_chance {
                let (lo, hi) = rules.pillar_height;
                height += step(lo + self.rand(2, sites[2], 5) * (hi - lo), rules.quantum);
            }
            albedo += rules.albedo_spread * 0.6 * (self.rand(2, sites[2], 6) - 0.5);
        }
        let marking = self.rand(level, sites[level], 7);
        if marking < rules.marking_chance * 0.66 {
            albedo = 0.07;
        } else if marking < rules.marking_chance {
            albedo = 0.26;
        }
        Plate { height, albedo: albedo as f32, level, sites }
    }

    /// The plate covering (x, z), using at most `max_level` levels of detail.
    pub fn plate_at(&self, cache: &mut Cache, x: f64, z: f64, max_level: usize) -> Plate {
        let p = DVec2::new(x, z);
        let mut sites = [(0, 0); 3];
        let mut level = 0;
        sites[0] = self.nearest_site(0, p);
        while level < max_level && self.splits(level, sites[level]) {
            level += 1;
            sites[level] = self.nearest_site(level, p);
        }
        self.plate(cache, sites, level)
    }

    /// Height of the ground (plate or canal) at a point.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        self.surface(&mut Cache::default(), x as f64, z as f64, 2) as f32
    }

    /// Clips `poly` to the side of the bisector nearer `site` than `other`.
    fn clip(poly: &[DVec2], site: DVec2, other: DVec2) -> Vec<DVec2> {
        let mid = (site + other) * 0.5;
        let n = other - site;
        let side = |p: DVec2| (p - mid).dot(n);
        let mut out = Vec::with_capacity(poly.len() + 1);
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (sa, sb) = (side(a), side(b));
            if sa <= 0.0 {
                out.push(a);
            }
            if (sa <= 0.0) != (sb <= 0.0) {
                out.push(a + (b - a) * (sa / (sa - sb)));
            }
        }
        out
    }

    /// The Voronoi cell of a site at `level`, intersected with `within`.
    fn cell(&self, level: usize, g: (i32, i32), within: &[DVec2]) -> Vec<DVec2> {
        let s = self.site(level, g);
        let mut poly = within.to_vec();
        for dz in -2..=2 {
            for dx in -2..=2 {
                if (dx, dz) == (0, 0) || poly.len() < 3 {
                    continue;
                }
                poly = Self::clip(&poly, s, self.site(level, (g.0 + dx, g.1 + dz)));
            }
        }
        poly
    }

    /// Meshes every plate whose level-0 site lies in the given column. At
    /// coarser LODs plates are not split as deeply.
    pub fn mesh_column(&self, lod: u32, cx: i32, cz: i32) -> ColumnMesh {
        let size = column_size(lod) as f64;
        let (x0, z0) = (cx as f64 * size, cz as f64 * size);
        let max_level = 2usize.saturating_sub(lod as usize);
        let mut cache = Cache::default();
        let mut mesh = ColumnMesh::default();

        let g0 = GRID[0];
        let (gx0, gz0) = ((x0 / g0).floor() as i32 - 1, (z0 / g0).floor() as i32 - 1);
        let (gx1, gz1) = (((x0 + size) / g0).ceil() as i32 + 1, ((z0 + size) / g0).ceil() as i32 + 1);
        for gz in gz0..=gz1 {
            for gx in gx0..=gx1 {
                let g = (gx, gz);
                let s = self.site(0, g);
                if s.x < x0 || s.x >= x0 + size || s.y < z0 || s.y >= z0 + size {
                    continue;
                }
                let square = [
                    s + DVec2::new(-3.0, -3.0) * g0,
                    s + DVec2::new(3.0, -3.0) * g0,
                    s + DVec2::new(3.0, 3.0) * g0,
                    s + DVec2::new(-3.0, 3.0) * g0,
                ];
                let poly = self.cell(0, g, &square);
                let mut sites = [g, (0, 0), (0, 0)];
                self.emit_split(&mut cache, &mut mesh, &poly, &mut sites, 0, max_level, (x0, z0));
            }
        }
        let scale = 4f64.powi(lod as i32);
        self.canals.mesh_square(&mut mesh, (x0, z0), size, scale, &|c| self.canal_floor(c));
        mesh
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_split(
        &self,
        cache: &mut Cache,
        mesh: &mut ColumnMesh,
        poly: &[DVec2],
        sites: &mut [(i32, i32); 3],
        level: usize,
        max_level: usize,
        origin: (f64, f64),
    ) {
        if poly.len() < 3 {
            return;
        }
        if level >= max_level || !self.splits(level, sites[level]) {
            let plate = self.plate(cache, *sites, level);
            for piece in self.canals.cut(poly.to_vec()) {
                if area(&piece) > 0.5 {
                    self.emit_prism(cache, mesh, &piece, &plate, max_level, origin);
                }
            }
            return;
        }
        // Pieces of the next level's Voronoi diagram inside this polygon.
        let next = level + 1;
        let grid = GRID[next];
        let (mut min, mut max) = (poly[0], poly[0]);
        for p in poly {
            min = min.min(*p);
            max = max.max(*p);
        }
        let (ix0, iz0) = ((min.x / grid).floor() as i32 - 1, (min.y / grid).floor() as i32 - 1);
        let (ix1, iz1) = ((max.x / grid).ceil() as i32 + 1, (max.y / grid).ceil() as i32 + 1);
        for iz in iz0..=iz1 {
            for ix in ix0..=ix1 {
                let piece = self.cell(next, (ix, iz), poly);
                if piece.len() < 3 || area(&piece) < 0.5 {
                    continue;
                }
                sites[next] = (ix, iz);
                self.emit_split(cache, mesh, &piece, sites, next, max_level, origin);
            }
        }
    }

    /// Fraction of open sky above a point on a plate, from the horizon angle
    /// in eight directions. Stylised rather than exact.
    fn sky_visibility(&self, cache: &mut Cache, p: DVec2, h: f64, max_level: usize) -> f32 {
        const DIST: [f64; 4] = [1.5, 4.0, 10.0, 25.0];
        let mut vis = 0.0;
        for i in 0..8 {
            let a = i as f64 / 8.0 * std::f64::consts::TAU;
            let dir = DVec2::new(a.cos(), a.sin());
            let mut max_sin: f64 = 0.0;
            for r in DIST {
                let q = p + dir * r;
                let dh = self.surface(cache, q.x, q.y, max_level) - h;
                if dh > 0.0 {
                    max_sin = max_sin.max(dh / (dh * dh + r * r).sqrt());
                }
            }
            vis += 1.0 - max_sin;
        }
        (vis / 8.0) as f32
    }

    fn emit_prism(
        &self,
        cache: &mut Cache,
        mesh: &mut ColumnMesh,
        poly: &[DVec2],
        plate: &Plate,
        max_level: usize,
        origin: (f64, f64),
    ) {
        let h = plate.height;
        let n = poly.len();
        let centroid = poly.iter().copied().sum::<DVec2>() / n as f64;

        // Walls reach just below the lowest neighbouring plate.
        let mut lowest = h;
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            for p in [a, (a + b) * 0.5] {
                let out = p + (p - centroid).normalize_or_zero() * 0.75;
                lowest = lowest.min(self.surface(cache, out.x, out.y, max_level));
            }
        }
        let depth = (h - lowest + 1.0).max(1.0);

        let local = |p: DVec2, y: f64| [(p.x - origin.0) as f32, y as f32, (p.y - origin.1) as f32];
        let push = |mesh: &mut ColumnMesh, pos: [f32; 3], normal: Vec3, albedo: f32, ao: f32| {
            mesh.positions.push(pos);
            mesh.normals.push(normal.to_array());
            mesh.albedo.push(albedo);
            mesh.ao.push(ao);
            (mesh.positions.len() - 1) as u32
        };
        // Emits a triangle facing `normal`, whatever the input winding.
        let tri = |mesh: &mut ColumnMesh, i: [u32; 3], normal: Vec3| {
            let p = |k: u32| Vec3::from(mesh.positions[k as usize]);
            let face = (p(i[1]) - p(i[0])).cross(p(i[2]) - p(i[0]));
            if face.dot(normal) >= 0.0 {
                mesh.indices.extend_from_slice(&i);
            } else {
                mesh.indices.extend_from_slice(&[i[0], i[2], i[1]]);
            }
        };

        // Top: a fan around the centroid, so the interior gets its own AO.
        let corner_ao: Vec<f32> = poly
            .iter()
            .map(|&p| {
                let inset = p + (centroid - p).normalize_or_zero() * 0.4;
                self.sky_visibility(cache, inset, h, max_level)
            })
            .collect();
        let center_ao = self.sky_visibility(cache, centroid, h, max_level);
        let c = push(mesh, local(centroid, h), Vec3::Y, plate.albedo, center_ao);
        let first = mesh.positions.len() as u32;
        for (i, &p) in poly.iter().enumerate() {
            push(mesh, local(p, h), Vec3::Y, plate.albedo, corner_ao[i]);
        }
        for i in 0..n as u32 {
            tri(mesh, [c, first + i, first + (i + 1) % n as u32], Vec3::Y);
        }

        // Walls, darkening towards their foot.
        let wall_albedo = plate.albedo * 0.85;
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let edge = b - a;
            if edge.length_squared() < 1e-6 {
                continue;
            }
            let mut normal = DVec2::new(edge.y, -edge.x).normalize();
            if normal.dot((a + b) * 0.5 - centroid) < 0.0 {
                normal = -normal;
            }
            let normal = Vec3::new(normal.x as f32, 0.0, normal.y as f32);
            let top_a = push(mesh, local(a, h), normal, wall_albedo, corner_ao[i]);
            let top_b = push(mesh, local(b, h), normal, wall_albedo, corner_ao[(i + 1) % n]);
            let bot_a = push(mesh, local(a, h - depth), normal, wall_albedo, 0.0);
            let bot_b = push(mesh, local(b, h - depth), normal, wall_albedo, 0.0);
            tri(mesh, [top_a, bot_a, bot_b], normal);
            tri(mesh, [top_a, bot_b, top_b], normal);
        }
    }
}

fn area(poly: &[DVec2]) -> f64 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        a += p.x * q.y - q.x * p.y;
    }
    a.abs() * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plates_tile() {
        let w = PlateWorld::new(4096.0, 3);
        let mut cache = Cache::default();
        for i in 0..200 {
            let (x, z) = (i as f64 * 37.3, i as f64 * 91.7);
            let a = w.plate_at(&mut cache, x, z, 2).height;
            let b = w.plate_at(&mut cache, x + 4096.0, z - 4096.0, 2).height;
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn mesh_is_consistent() {
        let w = PlateWorld::new(4096.0, 3);
        let mut total = 0;
        for lod in 0..3 {
            for c in 0..6 {
                let m = w.mesh_column(lod, c, c * 3);
                assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
                assert_eq!(m.positions.len(), m.ao.len());
                total += m.indices.len();
            }
        }
        assert!(total > 0);
    }
}
