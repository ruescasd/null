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
//! the plates and bring their own geometry. Sites (see `sites.rs`) reshape
//! the plates around them into a flat core and terraces.

use std::{collections::HashMap, sync::Arc};

use glam::{DVec2, Vec3};

use crate::canal::{CanalHit, Canals, PIPE_RADIUS};
use crate::district::{District, DistrictMap};
use crate::mesh::{ColumnMesh, column_size};
use crate::noise::{Fbm, hash01};
use crate::forms::{self, Growth, Grower};
use crate::sites::{SiteGround, SiteTable};
use crate::structure::Library;

/// Grid spacing of each level's Voronoi sites, in metres.
pub const GRID: [f64; 3] = [128.0, 32.0, 8.0];
/// Site offset from its grid cell centre, as a fraction of the grid (±half).
/// Kept at or below 0.6 so a 3x3 search always finds the nearest site.
const JITTER: f64 = 0.6;
/// Big plates whose site is this close to a site's ground split, so the
/// site is made of 32 m plates (a big plate reaches about this far from its
/// own site).
const SITE_SPLIT_MARGIN: f64 = 160.0;

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
    sites: SiteTable,
    /// The structure library, for the form pillars grow (see `emit_pillar`).
    library: Option<Arc<Library>>,
    /// The lab's ground: flat plates, no canals (see `lab.rs`).
    lab: bool,
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
    /// How much of its height is a pillar raised above the ground's own
    /// level (0 for most plates).
    pub pillar: f64,
    /// On an earthwork's slope: its top follows the earthwork's surface
    /// (see `slope_at`) rather than lying flat at `height`.
    pub sloped: bool,
}

/// A plate belonging to a site: its outline (world coordinates near the
/// site's centre), height, and how far its own site lies from the centre.
#[derive(Clone, Debug)]
pub struct SitePlate {
    pub points: Vec<DVec2>,
    pub height: f64,
    pub distance: f64,
    /// Its level-1 grid index, which identifies it.
    pub key: (i32, i32),
    /// On an earthwork's slope (its top is not flat).
    pub sloped: bool,
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
            sites: SiteTable::default(),
            library: None,
            lab: false,
        }
    }

    /// The lab's ground: the same tessellation of plates, all flat at 0, no
    /// canals or sites; candidates are judged on it (see `lab.rs`).
    pub fn lab(size: f32, seed: u32) -> Self {
        Self { lab: true, ..Self::new(size, seed) }
    }

    /// The same world with the ground reshaped by the library's sites.
    pub fn with_sites(mut self, library: &Library) -> Self {
        self.sites = SiteTable::new(library, &self);
        self.library = Some(Arc::new(library.clone()));
        self
    }

    /// Whether two worlds' sites shape the ground alike (if not, terrain
    /// meshed for one is wrong for the other).
    pub fn same_ground(&self, other: &PlateWorld) -> bool {
        let forms = |w: &PlateWorld| w.library.as_ref().map(|l| format!("{:?}", l.forms));
        self.sites == other.sites && forms(self) == forms(other)
    }

    /// The world's seed (scrambled), for things derived from it.
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// The district a point belongs to.
    pub fn district(&self, x: f64, z: f64) -> District {
        self.districts.at(x, z)
    }

    /// The landform scaled by the local district's relief: what plates fit.
    pub fn shaped(&self, x: f64, z: f64) -> f64 {
        self.landform(x, z) * self.districts.blended(x, z).0
    }

    /// The broad shape of the ground at a point: the landform without its
    /// ridges and warping, gently sloped everywhere.
    pub fn broad(&self, c: DVec2) -> f64 {
        let base = self.base.sample2(c.x as f32, c.y as f32, self.size as f32) as f64;
        let base = if base < 0.0 { base * 0.25 } else { base };
        base * 90.0 * self.districts.blended(c.x, c.y).0
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
        if self.lab {
            return None;
        }
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
        if self.lab {
            return None;
        }
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
        if !self.lab
            && let Some(h) = self.canals.surface(p, &|c| self.canal_floor(c))
        {
            return h;
        }
        let plate = self.plate_at(cache, x, z, max_level);
        if plate.sloped {
            return self.slope_at(p);
        }
        plate.height
    }

    /// The height of an earthwork's slope at a point: its surface inside a
    /// site, the smooth landform beyond it. Continuous, so the tilted plates
    /// that sample it at their corners meet seamlessly.
    fn slope_at(&self, p: DVec2) -> f64 {
        // Towards the broad shape of the land, not its ridges: slopes stay
        // clean planes, and where an earthwork meets ridged ground its edge
        // is a crisp cut rather than a field of spikes.
        let natural = self.broad(p);
        self.sites.at(p, 0.0).and_then(|(ground, _)| ground.surface(p, natural, self.size)).unwrap_or(natural)
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
        // Near an ordered site the sites fall back onto the grid, and the
        // plates become regular squares.
        let cell = DVec2::new((g.0 as f64 + 0.5) * grid, (g.1 as f64 + 0.5) * grid);
        let jitter = JITTER * (1.0 - self.sites.order_at(cell));
        let jx = (self.rand(level, g, 1) - 0.5) * jitter;
        let jz = (self.rand(level, g, 2) - 0.5) * jitter;
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
        // Sites are made of 32 m plates, which do not split further.
        if let Some((ground, distance)) = self.sites.at(p, SITE_SPLIT_MARGIN) {
            if level == 0 {
                return true;
            }
            if distance < ground.radius {
                return false;
            }
        }
        let rules = self.districts.at(p.x, p.y).rules();
        let mask = self.split_mask.sample2(p.x as f32, p.y as f32, self.size as f32) as f64;
        let chance = rules.split[level] + rules.split_mask_gain[level] * mask.max(0.0);
        self.slope(level, g) * GRID[level] > rules.split_rise[level] || self.rand(level, g, 3) < chance
    }

    /// Height and look of the plate with the given site chain, following
    /// the rules of the district its biggest plate belongs to.
    fn plate(&self, cache: &mut Cache, sites: [(i32, i32); 3], level: usize) -> Plate {
        if self.lab {
            let albedo = 0.19 + 0.04 * (self.rand(level, sites[level], 6) as f32 - 0.5);
            return Plate { height: 0.0, albedo, level, sites, pillar: 0.0, sloped: false };
        }
        let s0 = self.site(0, sites[0]);
        let rules = self.districts.at(s0.x, s0.y).rules();
        let base_albedo = self.districts.blended(s0.x, s0.y).1;
        let step = |h: f64, q: f64| (h / q).round() * q;

        let l0 = self.landform_at_site(cache, 0, sites[0]);
        let mut height = step(l0, rules.quantum0);
        let mut pillar = 0.0;
        let mut sloped = false;
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
                pillar = step(lo + self.rand(2, sites[2], 5) * (hi - lo), rules.quantum);
                height += pillar;
            }
            albedo += rules.albedo_spread * 0.6 * (self.rand(2, sites[2], 6) - 0.5);
        }
        // Inside a site, its core and terraces.
        let p = self.site(level, sites[level]);
        if let Some((ground, distance)) = self.sites.at(p, 0.0) {
            let natural = self.landform_at_site(cache, level, sites[level]);
            if ground.earthwork.is_some() && distance > ground.core && distance < ground.radius {
                height = self.slope_at(p);
                pillar = 0.0;
                sloped = true;
            } else if let Some(h) = ground.height(distance, natural) {
                height = h;
                pillar = 0.0;
            }
        }
        let marking = self.rand(level, sites[level], 7);
        if marking < rules.marking_chance * 0.66 {
            albedo = 0.07;
        } else if marking < rules.marking_chance {
            albedo = 0.26;
        }
        Plate { height, albedo: albedo as f32, level, sites, pillar, sloped }
    }

    /// The plate covering (x, z), using at most `max_level` levels of detail.
    pub fn plate_at(&self, cache: &mut Cache, x: f64, z: f64, max_level: usize) -> Plate {
        let p = DVec2::new(x, z);
        let mut sites = [(0, 0); 3];
        let mut level = 0;
        sites[0] = self.nearest_site(0, p);
        while (level < max_level || self.site_detail(level, sites[level])) && self.splits(level, sites[level]) {
            level += 1;
            sites[level] = self.nearest_site(level, p);
        }
        self.plate(cache, sites, level)
    }

    /// The outline of the big (128 m) plate around a point.
    pub fn big_plate(&self, x: f64, z: f64) -> Vec<DVec2> {
        let g = self.nearest_site(0, DVec2::new(x, z));
        let s = self.site(0, g);
        let square = [
            s + DVec2::new(-3.0, -3.0) * GRID[0],
            s + DVec2::new(3.0, -3.0) * GRID[0],
            s + DVec2::new(3.0, 3.0) * GRID[0],
            s + DVec2::new(-3.0, 3.0) * GRID[0],
        ];
        self.cell(0, g, &square)
    }

    /// The plates a site's ground is made of, exactly as they are meshed.
    pub fn site_plates(&self, ground: &SiteGround) -> Vec<SitePlate> {
        let c = ground.center;
        let reach = ground.radius + SITE_SPLIT_MARGIN;
        let (g0, g1) = (GRID[0], GRID[1]);
        let mut cache = Cache::default();
        let mut out = Vec::new();
        let range = |v: f64| ((v - reach) / g0).floor() as i32 - 1..=((v + reach) / g0).ceil() as i32 + 1;
        for gz in range(c.y) {
            for gx in range(c.x) {
                let g = (gx, gz);
                let s = self.site(0, g);
                if s.distance(c) > reach || !self.splits(0, g) {
                    continue;
                }
                let square = [
                    s + DVec2::new(-3.0, -3.0) * g0,
                    s + DVec2::new(3.0, -3.0) * g0,
                    s + DVec2::new(3.0, 3.0) * g0,
                    s + DVec2::new(-3.0, 3.0) * g0,
                ];
                let poly = self.cell(0, g, &square);
                if poly.len() < 3 {
                    continue;
                }
                let (mut min, mut max) = (poly[0], poly[0]);
                for p in &poly {
                    min = min.min(*p);
                    max = max.max(*p);
                }
                for iz in (min.y / g1).floor() as i32 - 1..=(max.y / g1).ceil() as i32 + 1 {
                    for ix in (min.x / g1).floor() as i32 - 1..=(max.x / g1).ceil() as i32 + 1 {
                        let distance = ground.measure(self.site(1, (ix, iz)) - c);
                        if distance >= ground.radius {
                            continue;
                        }
                        let piece = self.cell(1, (ix, iz), &poly);
                        if piece.len() < 3 || area(&piece) < 0.5 {
                            continue;
                        }
                        let plate = self.plate(&mut cache, [g, (ix, iz), (0, 0)], 1);
                        out.push(SitePlate {
                            points: piece,
                            height: plate.height,
                            distance,
                            key: (ix, iz),
                            sloped: plate.sloped,
                        });
                    }
                }
            }
        }
        out
    }

    /// Whether a big plate is part of a site, which keeps its 32 m plates at
    /// every level of detail (so structures stand on them from afar too).
    fn site_detail(&self, level: usize, g: (i32, i32)) -> bool {
        level == 0 && self.sites.at(self.site(0, g), SITE_SPLIT_MARGIN).is_some()
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
        if !self.lab {
            self.canals.mesh_square(&mut mesh, (x0, z0), size, scale, &|c| self.canal_floor(c));
        }
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
        if (level >= max_level && !self.site_detail(level, sites[level])) || !self.splits(level, sites[level]) {
            let plate = self.plate(cache, *sites, level);
            let pieces = if self.lab { vec![poly.to_vec()] } else { self.canals.cut(poly.to_vec()) };
            for piece in pieces {
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
        // At full detail a pillar is grown from the `pillar` form on its
        // base, rather than drawn as one plain prism.
        if max_level == 2
            && plate.pillar > 0.0
            && let Some(library) = &self.library
            && library.forms.contains_key(PILLAR_FORM)
        {
            let base = Plate { height: plate.height - plate.pillar, pillar: 0.0, sloped: false, ..*plate };
            self.emit_prism(cache, mesh, poly, &base, max_level, origin);
            self.emit_pillar(library, mesh, poly, plate, origin);
            return;
        }
        let h = plate.height;
        let n = poly.len();
        let centroid = poly.iter().copied().sum::<DVec2>() / n as f64;
        // The top's height at each corner and at the centroid: flat, or on
        // an earthwork's slope.
        let top = |p: DVec2| if plate.sloped { self.slope_at(p) } else { h };
        let tops: Vec<f64> = poly.iter().map(|&p| top(p)).collect();
        let center_top = top(centroid);

        // What lies just beyond each edge (at its ends and middle), for the
        // walls: they reach just below the lowest of it, and are left out
        // where it stands as high as the top (they would never be seen).
        let mut lowest = tops.iter().copied().fold(h, f64::min);
        let mut beyond = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let mut edge_low = f64::MAX;
            for p in [a, (a + b) * 0.5, b] {
                let out = p + (p - centroid).normalize_or_zero() * 0.75;
                let s = self.surface(cache, out.x, out.y, max_level);
                edge_low = edge_low.min(s);
            }
            lowest = lowest.min(edge_low);
            beyond.push(edge_low);
        }
        let foot = lowest - 1.0;

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
            .enumerate()
            .map(|(i, &p)| {
                let inset = p + (centroid - p).normalize_or_zero() * 0.4;
                self.sky_visibility(cache, inset, tops[i], max_level)
            })
            .collect();
        let center_ao = self.sky_visibility(cache, centroid, center_top, max_level);
        if plate.sloped {
            // Each triangle its own flat facet: crisp, geometric slopes.
            for i in 0..n {
                let j = (i + 1) % n;
                let (pc, pa, pb) = (
                    Vec3::from(local(centroid, center_top)),
                    Vec3::from(local(poly[i], tops[i])),
                    Vec3::from(local(poly[j], tops[j])),
                );
                let mut normal = (pa - pc).cross(pb - pc).normalize_or(Vec3::Y);
                if normal.y < 0.0 {
                    normal = -normal;
                }
                let c = push(mesh, pc.to_array(), normal, plate.albedo, center_ao);
                let a = push(mesh, pa.to_array(), normal, plate.albedo, corner_ao[i]);
                let b = push(mesh, pb.to_array(), normal, plate.albedo, corner_ao[j]);
                tri(mesh, [c, a, b], normal);
            }
        } else {
            let c = push(mesh, local(centroid, h), Vec3::Y, plate.albedo, center_ao);
            let first = mesh.positions.len() as u32;
            for (i, &p) in poly.iter().enumerate() {
                push(mesh, local(p, h), Vec3::Y, plate.albedo, corner_ao[i]);
            }
            for i in 0..n as u32 {
                tri(mesh, [c, first + i, first + (i + 1) % n as u32], Vec3::Y);
            }
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
            let j = (i + 1) % n;
            if beyond[i] >= tops[i].max(tops[j]) - 1e-3 {
                continue;
            }
            let top_a = push(mesh, local(a, tops[i]), normal, wall_albedo, corner_ao[i]);
            let top_b = push(mesh, local(b, tops[j]), normal, wall_albedo, corner_ao[j]);
            let bot_a = push(mesh, local(a, foot), normal, wall_albedo, 0.0);
            let bot_b = push(mesh, local(b, foot), normal, wall_albedo, 0.0);
            tri(mesh, [top_a, bot_a, bot_b], normal);
            tri(mesh, [top_a, bot_b, top_b], normal);
        }
    }
}

/// The form terrain pillars grow, if the library has it.
const PILLAR_FORM: &str = "pillar";
/// Pieces per pillar at most.
const PILLAR_PIECES: usize = 120;

impl PlateWorld {
    /// Grows the pillar form on a pillar plate's outline, from its base up,
    /// and scales the result to the pillar's height exactly (so it matches
    /// coarser levels of detail, collision and `height_at`).
    fn emit_pillar(&self, library: &Library, mesh: &mut ColumnMesh, poly: &[DVec2], plate: &Plate, origin: (f64, f64)) {
        let local: Vec<glam::Vec2> =
            poly.iter().map(|p| glam::Vec2::new((p.x - origin.0) as f32, (p.y - origin.1) as f32)).collect();
        let floor = (plate.height - plate.pillar) as f32;
        let site = plate.sites[plate.level];
        let seed = (site.0 as u32).wrapping_mul(0x2c1b_3c6d) ^ (site.1 as u32).wrapping_mul(0x297a_2d39) ^ self.seed;
        let mut grower = Grower { library, budget: PILLAR_PIECES, leaves: 0, out: Growth::default() };
        grower.grow(PILLAR_FORM, &local, floor, plate.albedo, seed, 0);
        let mut prisms = grower.out.prisms;
        let top = prisms.iter().map(|p| p.y1).fold(floor, f32::max);
        if top <= floor + 0.01 {
            return;
        }
        let k = plate.pillar as f32 / (top - floor);
        for p in &mut prisms {
            p.y0 = floor + (p.y0 - floor) * k;
            p.y1 = floor + (p.y1 - floor) * k;
            // Into the base, so nothing shows a seam.
            if p.y0 <= floor + 1e-3 {
                p.y0 = floor - 0.5;
            }
        }
        forms::mesh_into(mesh, &prisms);
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
