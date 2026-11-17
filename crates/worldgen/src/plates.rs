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

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use glam::{DVec2, Vec3};
use crate::planar::{self, Corner, Geometry, Key, Line};

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
    /// Slabs thrust up out of the ground, of which kind (see `slabs.rs`; 0
    /// none; the game's `--set slabs N`, to judge).
    pub slabs: u8,
    /// Kerbs along the flat plates' edges, near (see `bricks.rs`; the
    /// game's `--opt kerbs`).
    pub kerbs: bool,
    /// At each coarser level of detail, how far (m) a fitted plane may
    /// stand from the finest pieces' tops: each plate there drawn as planes
    /// fitted to them, one over it if near enough, else over its finer
    /// plates, down to the finest; pillars taller than it as plain prisms
    /// (see `visit_fitted`). 0: the level's plates at a fixed depth. (The
    /// game's from `--set far_px`: the far view, 2026-11-17.)
    pub far_tol: [f64; 3],
    /// How far below the world a void's walls reach (m, a height): the
    /// ground's height in a void. (How deep is open, the user, 2026-11-17.)
    pub void_depth: f64,
    /// The lowest a wall carries its pattern (m, out of the ground beyond
    /// it, at its tallest; the game's `--set wall_least`). (The user: on a
    /// short wall the panels do not fit, a few ticks with no panel's edge;
    /// and the cut is a measure of its own, a step one looks over plain, a
    /// wall that shuts the view panelled. 1.5: just under the eye, and
    /// always a course's line through it.)
    pub wall_least: f64,
    /// The largest height step any district's plates take (m; none by
    /// default): navigation's measure of the terrain lever, how much the
    /// ground joins up when its steps are made walkable (see
    /// docs/outdoor-navigation.md).
    pub step_cap: Option<f64>,
    /// Each pit's stair (see `stair`), worked out once and kept: a ground
    /// query in a pit reads it (creatures' feet ask many a frame). And each
    /// plate's wells (see `wells`), the same way.
    stairs: RwLock<HashMap<(i32, i32), Option<Arc<Stair>>>>,
    wells: RwLock<HashMap<Parent, Arc<HashMap<(i32, i32), f64>>>>,
}

/// A plate split into smaller ones: its level and its sites to it (the
/// rest none).
type Parent = (usize, [(i32, i32); 2]);

/// Wells (the user, 2026-11-15: no ground a player without the hook is
/// stuck in): a plate more than `WELL_REACH` below every plate round it
/// (a running jump and a mantle: the game's `player::hookless_reach`, under
/// its 2.45), each by its walkable top (a pillar's by its base, where you
/// walk round it), is raised to `WELL_STEP` below the lowest of them; not
/// by more than `WELL_MOST` (more, it is walled-in ground for a stair, and
/// a guard against a tower raised). (Stacks' rough relief left one in a
/// crack, 6.5 m below its sibling.)
const WELL_REACH: f64 = 2.45;
const WELL_STEP: f64 = 0.5;
const WELL_MOST: f64 = 8.0;
/// How long a border a body passes (m), and how often a well's edges are
/// sampled (m).
const WELL_NECK: f64 = 1.0;
const WELL_SAMPLE: f64 = 1.0;
/// Siblings within this many grid spacings are taken to be beside a plate.
const WELL_NEAR: f64 = 1.0;

mod bricks;
mod slabs;
pub use slabs::SlabInfo;

/// Flow inside a canal at a point.
#[derive(Clone, Copy, Debug)]
pub struct CanalFlow {
    /// Which canal (its index in the world's canals).
    pub canal: usize,
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
    /// Tilted (in a district whose plates tilt): its top rises by this
    /// much per metre from `center`, where it stands at `height`.
    pub tilt: DVec2,
    pub center: DVec2,
}

impl Plate {
    /// The height of its top at a point.
    fn top(&self, world: &PlateWorld, p: DVec2) -> f64 {
        if self.sloped { world.slope_at(p) } else { self.height + self.tilt.dot(p - self.center) }
    }
}

/// The gradients a tilted plate may take (rise per metre, about 0, 5, 9,
/// 14 and 19 degrees): set ones, as engineered ramps are, not the land's
/// own; and its heading, one of eight.
const TILTS: [f64; 5] = [0.0, 0.08, 0.16, 0.25, 0.35];
const HEADINGS: f64 = 8.0;
/// A tilted plate is level on a crest: where the land at its middle stands
/// `CREST_RISE` m above the land `CREST_REACH` m round it.
const CREST_REACH: f64 = 120.0;
const CREST_RISE: f64 = 4.0;

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
/// And whether a site splits (`splits`: its slope four landform samples):
/// asked again for every point a chain is found for (`chain`), the wells'
/// sampling round a plate 0.1 ms.
#[derive(Default)]
pub struct Cache {
    landform: HashMap<(usize, i32, i32), f64>,
    splits: HashMap<(usize, i32, i32), bool>,
    /// A fitted level (see `far_tol`): pillars as plain prisms, no kerbs,
    /// fitted within `tol`.
    plain: bool,
    tol: f64,
    /// The ground's heights on a grid round the column, for the sky's
    /// visibility at a coarser level (see `SkyGrid`).
    sky: Option<SkyGrid>,
    /// Whether each big plate is in a void (see `in_void`).
    voids: HashMap<(i32, i32), bool>,
}

/// How far below the world a void's walls reach by default (m, see
/// `PlateWorld::void_depth`): the shell the world is, stars below it (the
/// user, 2026-11-17), its walls black by then (see terrain.wgsl). (3 km,
/// its walls' tall triangles' depths fought between the depth prepass and
/// the main pass at a distance, whole walls blinking with the camera's
/// jitter, the user's "blink".)
pub const VOID_DEPTH: f64 = -600.0;

/// The ground's heights (`surface`) on a square grid, each found when first
/// asked for and kept: the sky's visibility at a coarser level of detail,
/// read between them, from 32 heights round each corner found anew 50 to
/// 75% of its column's build (the far levels drawn as fitted planes, the
/// user, 2026-11-17). Already of that level's coarser plates, not of the
/// fitted ones drawn, so stylised as before.
struct SkyGrid {
    origin: DVec2,
    step: f64,
    n: usize,
    heights: Vec<f64>,
}

impl SkyGrid {
    fn new(origin: DVec2, step: f64, n: usize) -> Self {
        Self { origin, step, n, heights: vec![f64::NAN; n * n] }
    }
}

/// How far a plate's cell reaches from its site, at most, in its grid's
/// squares: its sites are jittered by at most `JITTER` / 2 of a square each
/// way, so every point lies within 1.13 squares of the nearest, and a cell
/// clipped by its 24 neighbours within as much of its own; with a margin.
const CELL_REACH: f64 = 1.5;

/// Whether sites further than `CELL_REACH` are passed over (tests: off,
/// to show it changes nothing).
#[cfg(test)]
static SKIP_FAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// How far `p` is from convex `poly` (0 inside).
fn distance_to(p: DVec2, poly: &[DVec2]) -> f64 {
    let n = poly.len();
    let mut inside = n >= 3;
    let mut least = f64::INFINITY;
    for k in 0..n {
        let (a, b) = (poly[k], poly[(k + 1) % n]);
        let d = b - a;
        if d.perp_dot(p - a) < 0.0 {
            inside = false;
        }
        let t = if d.length_squared() > 0.0 { ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        least = least.min(p.distance(a + d * t));
    }
    if inside { 0.0 } else { least }
}

/// The plate `sites` to `level` (1 or 2) is split from.
fn parent(sites: &[(i32, i32); 3], level: usize) -> Parent {
    (level - 1, [sites[0], if level == 2 { sites[1] } else { (i32::MAX, i32::MAX) }])
}

/// A pit's way out (the user, 2026-11-14: no ground a player without the
/// hook can get stuck in; stairs added freely, a scale cue too): a flight
/// of steps cut along one edge of the pit, inside it, from its floor up to
/// the ground beyond. The strip within `STAIR_WIDTH` of that edge is cut
/// into steps square to it, each its own plate, so the mesher (walls and
/// all) and navigation have them as any plate. From `a` along `along` (a
/// unit; the edge's foot end), `inward` into the pit; the flight begins
/// `start` m along, each step `run` m; each step's top, the first the
/// landing at its foot (the edge's length before the flight too), the last
/// the ground beyond its top.
#[derive(Clone, Debug)]
pub struct Stair {
    a: DVec2,
    along: DVec2,
    inward: DVec2,
    length: f64,
    start: f64,
    run: f64,
    heights: Vec<f64>,
    albedo: f32,
}

/// A stair's width (m), the most a step rises (m: under what you and the
/// walkers step up, 0.58 and 0.6), how far a step runs (m; less, to the
/// least, where its edge is short).
const STAIR_WIDTH: f64 = 4.0;
const STAIR_RISE: f64 = 0.5;
const STAIR_RUN: f64 = 1.0;
const STAIR_RUN_LEAST: f64 = 0.35;
/// Its steps' plates are told by this in their site chain (`sites[2].0`).
const STAIR_TAG: i32 = i32::MIN;

impl Stair {
    /// Which step `p` is on, if it is in the stair's strip.
    fn step(&self, p: DVec2) -> Option<usize> {
        let d = (p - self.a).dot(self.inward);
        if !(0.0..=STAIR_WIDTH).contains(&d) {
            return None;
        }
        let n = self.heights.len();
        let t = ((p - self.a).dot(self.along) - self.start) / self.run;
        Some((t.floor().max(0.0) as usize).min(n - 1))
    }

    /// The hidden slope walked over its steps (the rooms' flights' way, see
    /// memory/stair-slopes; the user, 2026-11-15: you went up its steps in
    /// fits and starts, lifted a step at a time): a plane through every
    /// step's nose, from its landing to its top step, flush with both, the
    /// strip's width; walked on, never drawn (`ColumnMesh::walk`).
    fn emit_slope(&self, mesh: &mut crate::mesh::ColumnMesh, origin: (f64, f64)) {
        let n = self.heights.len();
        if n < 2 {
            return;
        }
        let (t0, t1) = (self.start, self.start + (n - 1) as f64 * self.run);
        let (h0, h1) = (self.heights[0], self.heights[n - 1]);
        let at = |t: f64, d: f64, h: f64| {
            let p = self.a + self.along * t + self.inward * d;
            [(p.x - origin.0) as f32, h as f32, (p.y - origin.1) as f32]
        };
        let first = mesh.walk.len() as u32;
        mesh.walk.extend([at(t0, 0.0, h0), at(t1, 0.0, h1), at(t1, STAIR_WIDTH, h1), at(t0, STAIR_WIDTH, h0)]);
        mesh.walk_indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }

    /// Its line `k` (`planar::Line::Stair`): a point on it and its normal,
    /// the strip's side below its inner side (0), a step's side below the
    /// edge to the next (`k` from 1).
    fn line(&self, k: u16) -> (DVec2, DVec2) {
        if k == 0 {
            (self.a + self.inward * STAIR_WIDTH, self.inward)
        } else {
            (self.a + self.along * (self.start + k as f64 * self.run), self.along)
        }
    }

    /// The strip and the rest of `poly` (convex, the pit's cell or a piece
    /// of it, big plate `site`'s): the steps' pieces, each with its step,
    /// and what is left.
    fn cut<G: Geometry + ?Sized>(&self, site: (i32, i32), poly: &[Corner], geometry: &G) -> (Vec<(Vec<Corner>, usize)>, Vec<Corner>) {
        let line = |k: usize| Line::Stair { site, k: k as u16 };
        let (mut strip, mut rest) = (Vec::new(), Vec::new());
        planar::clip(poly, line(0), true, geometry, &mut strip);
        planar::clip(poly, line(0), false, geometry, &mut rest);
        let n = self.heights.len();
        let mut steps = Vec::new();
        let mut spare = Vec::new();
        for k in 0..n {
            let mut piece = strip.clone();
            if k > 0 {
                planar::clip(&piece, line(k), false, geometry, &mut spare);
                std::mem::swap(&mut piece, &mut spare);
            }
            if k + 1 < n {
                planar::clip(&piece, line(k + 1), true, geometry, &mut spare);
                std::mem::swap(&mut piece, &mut spare);
            }
            if piece.len() >= 3 {
                steps.push((piece, k));
            }
        }
        (steps, rest)
    }
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
            slabs: 0,
            kerbs: false,
            far_tol: [0.0; 3],
            void_depth: VOID_DEPTH,
            wall_least: 1.5,
            step_cap: None,
            stairs: RwLock::default(),
            wells: RwLock::default(),
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
        self.sites == other.sites && forms(self) == forms(other) && self.slabs == other.slabs && self.kerbs == other.kerbs && self.wall_least == other.wall_least
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
        // (Tilted plates follow the landform's rolling part only: with its
        // ridges, far steeper than the steepest set gradient, they could
        // not, and stood in walls and jumbled blocks. The ridges fade in
        // across a border.)
        let b = self.districts.blended(x, z);
        let ridges = if b.ridges > 0.0 { self.ridges_at(x, z) * b.ridges } else { 0.0 };
        (self.rolling(x, z) + ridges) * b.relief
    }

    /// The landform without its ridges: its rolling, warped base.
    fn rolling(&self, x: f64, z: f64) -> f64 {
        let w = self.size as f32;
        let (xf, zf) = (x as f32, z as f32);
        let qx = xf + self.warp_x.sample2(xf, zf, w) * 300.0;
        let qz = zf + self.warp_z.sample2(xf, zf, w) * 300.0;
        let base = self.base.sample2(qx, qz, w) as f64;
        (if base < 0.0 { base * 0.25 } else { base }) * 90.0
    }

    /// The broad shape of the ground at a point: the landform without its
    /// ridges and warping, gently sloped everywhere.
    pub fn broad(&self, c: DVec2) -> f64 {
        let base = self.base.sample2(c.x as f32, c.y as f32, self.size as f32) as f64;
        let base = if base < 0.0 { base * 0.25 } else { base };
        base * 90.0 * self.districts.blended(c.x, c.y).relief
    }

    /// Height of a canal's floor at a point on its centreline (and of a
    /// station's court, at its centre): the broad shape of the ground (no
    /// ridges), so canals run in trenches through high ground and on
    /// embankments over low ground, with gentle grades.
    pub fn canal_floor(&self, c: DVec2) -> f64 {
        let base = self.base.sample2(c.x as f32, c.y as f32, self.size as f32) as f64;
        let base = if base < 0.0 { base * 0.25 } else { base };
        let relief = self.districts.blended(c.x, c.y).relief.min(self.canals.relief);
        // (Its rim 3.5 m under the ground's broad height: the ground beside
        // it above the rim on 99 edges in 100, so it lies in a cutting, the
        // plates' own walls down to it. A metre over that height, the ground
        // fell below its rim on a quarter of its edges, and its outer wall
        // stood above the ground there, a strip, the user.)
        base * 90.0 * relief - PIPE_RADIUS - 3.5
    }

    /// The canal flow at a point, if it is inside a canal's pipe.
    pub fn canal_at(&self, x: f32, z: f32) -> Option<CanalFlow> {
        if self.lab {
            return None;
        }
        let hit: CanalHit = self.canals.hit(DVec2::new(x as f64, z as f64), 0.0, &|c| self.canal_floor(c))?;
        (hit.offset.abs() < PIPE_RADIUS).then(|| CanalFlow {
            canal: hit.canal,
            dir: glam::Vec2::new(hit.flow_dir.x as f32, hit.flow_dir.y as f32),
            floor: hit.floor as f32,
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
        let hit = self.canals.nearest(DVec2::new(x as f64, z as f64), &|c| self.canal_floor(c))?;
        Some((
            glam::Vec2::new(hit.center.x as f32, hit.center.y as f32),
            glam::Vec2::new(hit.flow_dir.x as f32, hit.flow_dir.y as f32),
            hit.floor as f32,
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
        if self.in_void(cache, plate.sites[0]) {
            return self.void_depth;
        }
        if plate.sloped {
            return self.slope_at(p);
        }
        plate.top(self, p)
    }

    /// Whether a point is over a void (see `in_void`).
    pub fn is_void(&self, x: f32, z: f32) -> bool {
        let mut cache = Cache::default();
        let plate = self.plate_at(&mut cache, x as f64, z as f64, 0);
        self.in_void(&mut cache, plate.sites[0])
    }

    /// Whether the big plate `g` is in a void (by its site): none of it
    /// there, at every level of detail, so a void's coast is the big
    /// plates' edges (128 m Voronoi) along its warped outline.
    fn in_void(&self, cache: &mut Cache, g: (i32, i32)) -> bool {
        if self.districts.voids <= 0.0 {
            return false;
        }
        if let Some(&v) = cache.voids.get(&g) {
            return v;
        }
        let s = self.site(0, g);
        let v = self.districts.at(s.x, s.y) == District::Void;
        cache.voids.insert(g, v);
        v
    }

    /// The ground's solid top at a point, for the walls beside it: as
    /// `surface`, but where a pillar is grown (see `grows_pillars`), its
    /// base. (The pillar's form is open, stacked slabs: a wall stopped at
    /// the pillar's top was left out beside it, and the void under the world
    /// showed between its slabs, the user's hole.)
    fn solid(&self, cache: &mut Cache, x: f64, z: f64, max_level: usize) -> f64 {
        let p = DVec2::new(x, z);
        if !self.lab
            && let Some(h) = self.canals.surface(p, &|c| self.canal_floor(c))
        {
            return h;
        }
        let plate = self.plate_at(cache, x, z, max_level);
        if self.in_void(cache, plate.sites[0]) {
            return self.void_depth;
        }
        if plate.sloped {
            return self.slope_at(p);
        }
        let top = plate.top(self, p);
        let own = if plate.pillar > 0.0 && self.grows_pillars(max_level) && !cache.plain { top - plate.pillar } else { top };
        // (Fitted, what is drawn beside may be any depth.)
        if cache.tol > 0.0 && max_level < 2 { own.min(self.solid(cache, x, z, 2)) } else { own }
    }

    /// A fitted level's plate (see `far_tol`): the plate `sites` at `level` drawn as planes
    /// fitted to its pieces' tops (at full detail; a pillar's by its base):
    /// one over the whole plate if every piece's top is within `cache.tol`
    /// of it, else each of its next level's plates so, down to the finest;
    /// pillars taller than `cache.tol` on them as plain prisms. Its finest
    /// pieces found once, and shared down (found again for each plate a
    /// level down, half the far level's build).
    fn visit_fitted(&self, cache: &mut Cache, poly: &[Corner], sites: &[(i32, i32); 3], level: usize, f: &mut dyn FnMut(&mut Cache, &[Corner], &Plate, bool)) {
        let tol = cache.tol;
        cache.tol = 0.0;
        let mut leaves: Vec<(Vec<Corner>, Plate)> = Vec::new();
        let mut s = *sites;
        self.visit_plain(cache, poly, &mut s, level, 2, &mut |_, piece, plate, kept| {
            if kept {
                leaves.push((piece.to_vec(), *plate));
            }
        });
        cache.tol = tol;
        let all: Vec<&(Vec<Corner>, Plate)> = leaves.iter().collect();
        let mut s = *sites;
        self.fit_or_split(cache, poly, &mut s, level, &all, f);
    }

    /// See `visit_fitted`: the plate `sites` at `level`, its finest pieces
    /// `leaves`.
    fn fit_or_split(&self, cache: &mut Cache, poly: &[Corner], sites: &mut [(i32, i32); 3], level: usize, leaves: &[&(Vec<Corner>, Plate)], f: &mut dyn FnMut(&mut Cache, &[Corner], &Plate, bool)) {
        if level == 2 || !self.splits(level, sites[level]) {
            for (piece, plate) in leaves.iter().copied() {
                f(cache, piece, plate, true);
            }
            return;
        }
        if self.fit(cache, poly, sites, level, leaves, f) {
            return;
        }
        let next = level + 1;
        for (g, piece) in self.cells_within(next, poly) {
            // (Under `LEAST_PIECE`, nothing to mesh.)
            if planar::area(&piece) < LEAST_PIECE {
                continue;
            }
            sites[next] = g;
            let mine: Vec<&(Vec<Corner>, Plate)> = leaves.iter().copied().filter(|l| l.1.sites[next] == g).collect();
            self.fit_or_split(cache, &piece, sites, next, &mine, f);
        }
    }

    /// See `visit_fitted`: one plane over the plate `sites` at `level`, if
    /// every one of its `leaves`' corners is within `cache.tol` of it;
    /// whether it was.
    fn fit(&self, cache: &mut Cache, poly: &[Corner], sites: &[(i32, i32); 3], level: usize, leaves: &[&(Vec<Corner>, Plate)], f: &mut dyn FnMut(&mut Cache, &[Corner], &Plate, bool)) -> bool {
        let tol = cache.tol;
        // Corners and their tops, each weighed by its piece's share of area.
        let mut points: Vec<(DVec2, f64, f64)> = Vec::new();
        for (piece, plate) in leaves.iter().copied() {
            let corners = planar::points(piece);
            let w = planar::area(piece) / corners.len() as f64;
            for p in corners {
                points.push((p, plate.top(self, p) - plate.pillar, w));
            }
        }
        let total: f64 = points.iter().map(|q| q.2).sum();
        if total <= 0.0 {
            return false;
        }
        let c = points.iter().map(|q| q.0 * q.2).sum::<DVec2>() / total;
        let a = points.iter().map(|q| q.1 * q.2).sum::<f64>() / total;
        let (mut sxx, mut sxz, mut szz, mut sxh, mut szh) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for &(p, h, w) in &points {
            let d = p - c;
            sxx += w * d.x * d.x;
            sxz += w * d.x * d.y;
            szz += w * d.y * d.y;
            sxh += w * d.x * (h - a);
            szh += w * d.y * (h - a);
        }
        let det = sxx * szz - sxz * sxz;
        let tilt = if det.abs() > 1e-9 { DVec2::new((sxh * szz - szh * sxz) / det, (szh * sxx - sxh * sxz) / det) } else { DVec2::ZERO };
        let error = points.iter().map(|&(p, h, _)| (h - (a + tilt.dot(p - c))).abs()).fold(0.0, f64::max);
        if error > tol {
            return false;
        }
        let plane = Plate { height: a, tilt, center: c, sloped: false, pillar: 0.0, ..self.plate(cache, *sites, level) };
        let pieces = if self.lab { vec![poly.to_vec()] } else { self.canals.cut(poly.to_vec(), self) };
        for piece in pieces {
            f(cache, &piece, &plane, planar::area(&piece) >= LEAST_PIECE);
        }
        for (piece, plate) in leaves.iter().copied() {
            if plate.pillar > tol {
                f(cache, piece, plate, true);
            }
        }
        true
    }

    /// Whether pillars are grown from the library's form (at full detail),
    /// not drawn as plain prisms.
    fn grows_pillars(&self, max_level: usize) -> bool {
        max_level == 2 && self.library.as_ref().is_some_and(|l| l.forms.contains_key(PILLAR_FORM))
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
        self.rolling(x, z) + self.ridges_at(x, z)
    }

    /// The landform's ridges, on top of its rolling base.
    fn ridges_at(&self, x: f64, z: f64) -> f64 {
        let w = self.size as f32;
        let (xf, zf) = (x as f32, z as f32);
        let qx = xf + self.warp_x.sample2(xf, zf, w) * 300.0;
        let qz = zf + self.warp_z.sample2(xf, zf, w) * 300.0;
        let mask = smoothstep(-0.2, 0.5, self.ridge_mask.sample2(xf, zf, w) as f64);
        let r = self.ridges.ridged2(qx, qz, w) as f64;
        r * r * 160.0 * mask
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
    /// the rules of the district its biggest plate belongs to; a well
    /// raised (see `WELL_REACH`).
    fn plate(&self, cache: &mut Cache, sites: [(i32, i32); 3], level: usize) -> Plate {
        let mut plate = self.plate_raw(cache, sites, level);
        if level >= 1
            && !self.lab
            && let Some(up) = self.wells(parent(&sites, level)).get(&sites[level])
        {
            plate.height += up;
        }
        plate
    }

    /// As `plate`, no well raised.
    fn plate_raw(&self, cache: &mut Cache, sites: [(i32, i32); 3], level: usize) -> Plate {
        if self.lab {
            let albedo = 0.19 + 0.04 * (self.rand(level, sites[level], 6) as f32 - 0.5);
            return Plate { height: 0.0, albedo, level, sites, pillar: 0.0, sloped: false, tilt: DVec2::ZERO, center: DVec2::ZERO };
        }
        let s0 = self.site(0, sites[0]);
        let mut rules = self.districts.at(s0.x, s0.y).rules();
        if let Some(cap) = self.step_cap {
            rules.quantum0 = rules.quantum0.min(cap);
            rules.quantum = rules.quantum.min(cap);
        }
        let base_albedo = self.districts.blended(s0.x, s0.y).albedo;
        let step = |h: f64, q: f64| (h / q).round() * q;

        let l0 = self.landform_at_site(cache, 0, sites[0]);
        let mut height = step(l0, rules.quantum0);
        let mut pillar = 0.0;
        let mut sloped = false;
        let (mut tilt, mut center) = (DVec2::ZERO, DVec2::ZERO);
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
        if rules.terrace {
            height = step(self.landform_at_site(cache, level, sites[level]), rules.quantum);
        }
        if rules.tilt {
            // Its own height, in the district's steps, and the land's slope
            // at its middle, to the nearest set gradient and heading:
            // neighbours, each its own, meet in steps.
            let c = self.site(level, sites[level]);
            height = step(self.landform_at_site(cache, level, sites[level]), rules.quantum);
            let e = GRID[level] * 0.35;
            let g = DVec2::new(self.shaped(c.x + e, c.y) - self.shaped(c.x - e, c.y), self.shaped(c.x, c.y + e) - self.shaped(c.x, c.y - e)) / (2.0 * e);
            let rise = TILTS.iter().copied().min_by(|a, b| (a - g.length()).abs().total_cmp(&(b - g.length()).abs())).unwrap_or(0.0);
            // On a crest (the land higher here than round it), level: hills
            // end in flat tops and terraces, a built skyline, not a rounded
            // ridge.
            let r = CREST_REACH;
            let around = (self.shaped(c.x + r, c.y) + self.shaped(c.x - r, c.y) + self.shaped(c.x, c.y + r) + self.shaped(c.x, c.y - r)) * 0.25;
            let rise = if rules.crest && self.shaped(c.x, c.y) - around > CREST_RISE { 0.0 } else { rise };
            let sector = std::f64::consts::TAU / HEADINGS;
            let heading = (g.y.atan2(g.x) / sector).round() * sector;
            tilt = DVec2::new(heading.cos(), heading.sin()) * rise;
            center = c;
        }
        // Inside a site, its core and terraces.
        let p = self.site(level, sites[level]);
        if let Some((ground, distance)) = self.sites.at(p, 0.0) {
            let natural = self.landform_at_site(cache, level, sites[level]);
            if ground.earthwork.is_some() && distance > ground.core && distance < ground.radius {
                height = self.slope_at(p);
                pillar = 0.0;
                sloped = true;
                tilt = DVec2::ZERO;
            } else if let Some(h) = ground.height(distance, natural) {
                height = h;
                pillar = 0.0;
                tilt = DVec2::ZERO;
            }
        } else if let Some((ground, order)) = self.sites.order_ground(p) {
            // In an ordered site's field the land settles onto the site's
            // levels: near the site its heights follow the land's broad
            // shape in the site's steps (contour terraces), further out they
            // blend back into the plates' own heights; pillars give way.
            let s = ground.step.max(0.5);
            let terraced = ground.top + ((self.broad(p) - ground.top) / s).round() * s;
            let blended = height + (terraced - height) * order;
            height = if order > 0.3 { (blended / s).round() * s } else { blended };
            if order > 0.5 {
                pillar = 0.0;
            }
        }
        let marking = self.rand(level, sites[level], 7);
        if marking < rules.marking_chance * 0.66 {
            albedo = 0.07;
        } else if marking < rules.marking_chance {
            albedo = 0.26;
        }
        Plate { height, albedo: albedo as f32, level, sites, pillar, sloped, tilt, center }
    }

    /// The plate covering (x, z), using at most `max_level` levels of detail.
    pub fn plate_at(&self, cache: &mut Cache, x: f64, z: f64, max_level: usize) -> Plate {
        let g = self.nearest_site(0, DVec2::new(x, z));
        if let Some(stair) = self.stair(g)
            && let Some(k) = stair.step(DVec2::new(x, z))
        {
            return self.step_plate(g, &stair, k);
        }
        self.plate_plain(cache, x, z, max_level)
    }

    /// A pit's stair (see `Stair`), if it is a pit with one: from the pit's
    /// floor by its foot (its ground, under any pillar) up to the ground
    /// beyond its top, onto plain ground (not a pit, a mesa, a pillar, a
    /// canal or a site's ground), climbing half the pit's depth at least
    /// (a foot on high ground in a rough pit made a flight of no rise);
    /// along the edge whose flight fits at `STAIR_RUN` and comes out
    /// lowest, else the gentlest that fits at all.
    fn stair(&self, g: (i32, i32)) -> Option<Arc<Stair>> {
        if let Some(s) = self.stairs.read().unwrap().get(&g) {
            return s.clone();
        }
        let s = self.make_stair(&mut Cache::default(), g).map(Arc::new);
        self.stairs.write().unwrap().insert(g, s.clone());
        s
    }

    fn make_stair(&self, cache: &mut Cache, g: (i32, i32)) -> Option<Stair> {
        let s0 = self.site(0, g);
        let depth = self.pit_or_mesa(s0.x, s0.y).0?;
        if self.lab {
            return None;
        }
        let cell = self.big_cell(g);
        let n = cell.len();
        let centroid = cell.iter().copied().sum::<DVec2>() / n as f64;
        // (Fits at its run, how high it comes out, how gentle.)
        let mut best: Option<((bool, f64, f64), Stair)> = None;
        for i in 0..n {
            let (p, q) = (cell[i], cell[(i + 1) % n]);
            let length = p.distance(q);
            if length < 3.0 * STAIR_WIDTH {
                continue;
            }
            let mut inward = (q - p).perp() / length;
            if inward.dot(centroid - p) < 0.0 {
                inward = -inward;
            }
            for (a, along) in [(p, (q - p) / length), (q, (p - q) / length)] {
                let top = a + along * (length - 1.0) - inward * 0.5;
                let foot = a + along * 1.5 + inward * (STAIR_WIDTH + 1.5);
                // (Not where a site's ground sets the heights: by its foot,
                // its top, or beyond.)
                if self.pit_or_mesa(top.x, top.y) != (None, None) || (!self.lab && self.canals.surface(top, &|c| self.canal_floor(c)).is_some()) || [top, foot, a + inward, a + along * length + inward].iter().any(|q| self.sites.at(*q, 0.0).is_some()) {
                    continue;
                }
                let beyond = self.plate_plain(cache, top.x, top.y, 2);
                if beyond.pillar > 0.0 {
                    continue;
                }
                let floor = self.plate_plain(cache, foot.x, foot.y, 2);
                let (low, high) = (floor.top(self, foot) - floor.pillar, beyond.top(self, top));
                if high - low < depth * 0.5 {
                    continue;
                }
                let rises = ((high - low) / STAIR_RISE).ceil() as usize;
                let run = (length / (rises + 1) as f64).min(STAIR_RUN);
                let fits = run >= STAIR_RUN;
                if run < STAIR_RUN_LEAST {
                    continue;
                }
                // (Better: fitting; then coming out lower; then gentler.)
                let score = (fits, -high, run);
                if best.as_ref().is_some_and(|b| (b.0.0, b.0.1, b.0.2) >= score) {
                    continue;
                }
                let heights = (0..=rises).map(|k| low + (high - low) * k as f64 / rises as f64).collect();
                let start = length - (rises + 1) as f64 * run;
                best = Some((score, Stair { a, along, inward, length, start, run, heights, albedo: floor.albedo + 0.02 }));
            }
        }
        best.map(|b| b.1)
    }

    /// The stair out of the pit round a point, if it has one: its foot and
    /// top (along its outer edge), and their heights.
    pub fn pit_stair(&self, x: f64, z: f64) -> Option<(DVec2, f64, DVec2, f64)> {
        let g = self.nearest_site(0, DVec2::new(x, z));
        let st = self.stair(g)?;
        Some((st.a, st.heights[0], st.a + st.along * st.length, *st.heights.last()?))
    }

    /// A stair's step `k` as a plate: flat, told apart by its site chain.
    fn step_plate(&self, g: (i32, i32), stair: &Stair, k: usize) -> Plate {
        Plate { height: stair.heights[k], albedo: stair.albedo, level: 2, sites: [g, (STAIR_TAG, k as i32), (STAIR_TAG, k as i32)], pillar: 0.0, sloped: false, tilt: DVec2::ZERO, center: DVec2::ZERO }
    }

    /// As `plate_at`, but for any stair.
    fn plate_plain(&self, cache: &mut Cache, x: f64, z: f64, max_level: usize) -> Plate {
        let (sites, level) = self.chain(cache, x, z, max_level);
        self.plate(cache, sites, level)
    }

    /// The site chain of the plate at a point, and its level.
    fn chain(&self, cache: &mut Cache, x: f64, z: f64, max_level: usize) -> ([(i32, i32); 3], usize) {
        let p = DVec2::new(x, z);
        let mut sites = [(0, 0); 3];
        let mut level = 0;
        sites[0] = self.nearest_site(0, p);
        let mut splits = |level: usize, g: (i32, i32)| *cache.splits.entry((level, g.0, g.1)).or_insert_with(|| self.splits(level, g));
        while (level < max_level || self.site_detail(level, sites[level])) && splits(level, sites[level]) {
            level += 1;
            sites[level] = self.nearest_site(level, p);
        }
        (sites, level)
    }

    /// The wells among `parent`'s plates (see `WELL_REACH`): how much each
    /// (by its site) is raised; worked out once, from the plates as they
    /// are, and kept. Each of its plates that splits no further, flat:
    /// round its edges, sampled just across (three to an edge), the
    /// walkable tops beyond it, as they are; a canal there is a way out.
    fn wells(&self, parent: Parent) -> Arc<HashMap<(i32, i32), f64>> {
        if let Some(w) = self.wells.read().unwrap().get(&parent) {
            return w.clone();
        }
        let w = Arc::new(self.find_wells(parent));
        self.wells.write().unwrap().insert(parent, w.clone());
        w
    }

    fn find_wells(&self, (level, up): Parent) -> HashMap<(i32, i32), f64> {
        let mut cache = Cache::default();
        let mut out = HashMap::new();
        let mut corners = self.big_corners(up[0]);
        if level == 1 {
            corners = self.cell(1, up[1], &corners);
        }
        if corners.len() < 3 {
            return out;
        }
        let next = level + 1;
        let grid = GRID[next];
        let mut sites = [up[0], up[1], (0, 0)];
        // (Each plate's walkable top and site, first: a well is more than
        // `WELL_REACH` below those beside it, so a plate with a sibling
        // within a grid's spacing that is not, is none, and is not sampled
        // round, the costly part: 2 to 3 ms a parent, sampling them all.)
        // (Each kept, its piece and plate, for the second pass: made anew
        // there, a third of the wells' work.)
        let mut tops: Vec<(DVec2, f64)> = Vec::new();
        let mut kids: Vec<((i32, i32), Vec<DVec2>, Plate)> = Vec::new();
        for ((ix, iz), cell) in self.cells_within(next, &corners) {
            let piece = planar::points(&cell);
            if area(&piece) < LEAST_PIECE {
                continue;
            }
            sites[next] = (ix, iz);
            let plate = self.plate_raw(&mut cache, sites, next);
            tops.push((self.site(next, (ix, iz)), plate.height - plate.pillar));
            kids.push(((ix, iz), piece, plate));
        }
        {
            for ((ix, iz), piece, plate) in kids {
                sites[next] = (ix, iz);
                // (One that splits on is not a plate but its parts.)
                if next < 2 && self.splits(next, sites[next]) {
                    continue;
                }
                if plate.sloped || plate.tilt != DVec2::ZERO {
                    continue;
                }
                let top = plate.height - plate.pillar;
                let here = self.site(next, (ix, iz));
                if tops.iter().any(|&(at, t)| at != here && at.distance(here) <= grid * WELL_NEAR && t - top <= WELL_REACH) {
                    continue;
                }
                let centroid = piece.iter().copied().sum::<DVec2>() / piece.len() as f64;
                // (Each plate round it: how long a border, its lowest top.)
                let mut round: HashMap<[(i32, i32); 3], (f64, f64)> = HashMap::new();
                let mut canal = false;
                for e in 0..piece.len() {
                    let (a, b) = (piece[e], piece[(e + 1) % piece.len()]);
                    let length = a.distance(b);
                    if length < 1e-6 {
                        continue;
                    }
                    let mut outward = (b - a).perp() / length;
                    if outward.dot((a + b) * 0.5 - centroid) < 0.0 {
                        outward = -outward;
                    }
                    let n = (length / WELL_SAMPLE).ceil().max(1.0) as usize;
                    for k in 0..n {
                        let q = a + (b - a) * ((k as f64 + 0.5) / n as f64) + outward * 0.3;
                        if !self.lab && self.canals.surface(q, &|c| self.canal_floor(c)).is_some() {
                            canal = true;
                            continue;
                        }
                        let (mut chain, at) = self.chain(&mut cache, q.x, q.y, 2);
                        for c in chain.iter_mut().skip(at + 1) {
                            *c = (0, 0);
                        }
                        if at == next && chain[..=next] == sites[..=next] {
                            continue;
                        }
                        let beyond = self.plate_raw(&mut cache, chain, at);
                        let r = round.entry(chain).or_insert((0.0, f64::INFINITY));
                        r.0 += length / n as f64;
                        r.1 = r.1.min(beyond.top(self, q) - beyond.pillar);
                    }
                }
                // (Only a way a body passes: a sliver beside it, 0.2 m²,
                // taken for its lowest, left it a well still.)
                let lowest = round.values().filter(|r| r.0 >= WELL_NECK).map(|r| r.1).fold(f64::INFINITY, f64::min);
                if canal {
                    continue;
                }
                let by = lowest - WELL_STEP - top;
                if lowest.is_finite() && lowest - top > WELL_REACH && by <= WELL_MOST {
                    out.insert(sites[next], by);
                }
            }
        }
        out
    }

    /// The big plate round a point (its site's grid index).
    pub fn big_site(&self, x: f64, z: f64) -> (i32, i32) {
        self.nearest_site(0, DVec2::new(x, z))
    }

    /// How far the big plate round a point is sunk as a pit (m; none if it
    /// is not one), and raised as a mesa (m).
    pub fn pit_or_mesa(&self, x: f64, z: f64) -> (Option<f64>, Option<f64>) {
        let g = self.nearest_site(0, DVec2::new(x, z));
        let s0 = self.site(0, g);
        let rules = self.districts.at(s0.x, s0.y).rules();
        let r = self.rand(0, g, 4);
        if r < rules.mesa_chance {
            (None, Some(25.0 + self.rand(0, g, 5) * 60.0))
        } else if r < rules.mesa_chance + rules.pit_chance {
            (Some(6.0 + self.rand(0, g, 5) * 14.0), None)
        } else {
            (None, None)
        }
    }

    /// The outline of the big (128 m) plate around a point.
    pub fn big_plate(&self, x: f64, z: f64) -> Vec<DVec2> {
        self.big_cell(self.nearest_site(0, DVec2::new(x, z)))
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
                let corners = self.big_corners(g);
                if corners.len() < 3 {
                    continue;
                }
                let poly = planar::points(&corners);
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
                        let piece = planar::points(&self.cell(1, (ix, iz), &corners));
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

    /// The ground walked on at a point: as `height_at`, but on a pillar's
    /// plate, where its pillar is grown (narrower than its plate), its
    /// base, the ground you see there (`solid`). (By the pillar's top, a
    /// creature beside a pillar in the stacks stood in the air, the user,
    /// 2026-11-15.)
    pub fn walk_height_at(&self, x: f32, z: f32) -> f32 {
        self.solid(&mut Cache::default(), x as f64, z as f64, 2) as f32
    }

    /// The Voronoi cell of a site at `level`, intersected with `within`.
    fn cell(&self, level: usize, g: (i32, i32), within: &[Corner]) -> Vec<Corner> {
        Self::cell_by(level, g, within, self, &mut Vec::new())
    }

    /// As `cell`, its sites and lines by `geometry`, into a buffer kept from
    /// cell to cell (`spare`). (Each cell's 25 sites found anew, and a new
    /// outline made at each of its 24 clips, 1.5 µs a cell.)
    fn cell_by<G: Geometry + ?Sized>(level: usize, g: (i32, i32), within: &[Corner], geometry: &G, spare: &mut Vec<Corner>) -> Vec<Corner> {
        let mut poly = within.to_vec();
        for dz in -2..=2 {
            for dx in -2..=2 {
                if (dx, dz) == (0, 0) || poly.len() < 3 {
                    continue;
                }
                let h = (g.0 + dx, g.1 + dz);
                planar::clip(&poly, Line::bisector(level, g, h), g < h, geometry, spare);
                std::mem::swap(&mut poly, spare);
            }
        }
        poly
    }

    /// The square a cell of `level` round site `g` is cut from: three
    /// spacings each way (`Line::Bound`).
    fn bound(&self, level: usize, g: (i32, i32)) -> Vec<Corner> {
        let line = |k: usize| Line::Bound { level: level as u8, site: g, k: (k % 4) as u8 };
        (0..4)
            .map(|k| {
                let key = Key::of(line(k + 3), line(k));
                Corner::new(self.corner(key), key, line(k))
            })
            .collect()
    }

    /// Big plate `g`'s outline (its level-0 Voronoi cell), by its corners.
    fn big_corners(&self, g: (i32, i32)) -> Vec<Corner> {
        self.cell(0, g, &self.bound(0, g))
    }

    /// Big plate `g`'s outline.
    fn big_cell(&self, g: (i32, i32)) -> Vec<DVec2> {
        planar::points(&self.big_corners(g))
    }

    /// Meshes every plate whose level-0 site lies in the given column. At
    /// coarser LODs plates are not split as deeply.
    pub fn mesh_column(&self, lod: u32, cx: i32, cz: i32) -> ColumnMesh {
        let size = column_size(lod) as f64;
        let (x0, z0) = (cx as f64 * size, cz as f64 * size);
        let tol = self.far_tol[lod as usize];
        let plain = tol > 0.0;
        let max_level = 2usize.saturating_sub(lod as usize);
        let mut cache = Cache { plain, tol, ..Cache::default() };
        // (At the coarser levels, the sky's visibility from a grid of
        // heights round the column, 2 m apart at the middle level, 4 m at
        // the far; reaching as far as its plates do, a big square and a
        // half, and the farthest any is looked round.)
        if plain {
            let step = 2.0 * lod as f64;
            let reach = GRID[0] * CELL_REACH + 30.0;
            let n = ((size + 2.0 * reach) / step).ceil() as usize + 1;
            cache.sky = Some(SkyGrid::new(DVec2::new(x0 - reach, z0 - reach), step, n));
        }
        let mut mesh = ColumnMesh::default();

        let g0 = GRID[0];
        let (gx0, gz0) = ((x0 / g0).floor() as i32 - 1, (z0 / g0).floor() as i32 - 1);
        let (gx1, gz1) = (((x0 + size) / g0).ceil() as i32 + 1, ((z0 + size) / g0).ceil() as i32 + 1);
        for gz in gz0..=gz1 {
            for gx in gx0..=gx1 {
                let g = (gx, gz);
                let s = self.site(0, g);
                // (Nothing of a big plate in a void: no stair's slope nor
                // slab either, left floating, walked on in the air.)
                if s.x < x0 || s.x >= x0 + size || s.y < z0 || s.y >= z0 + size || self.in_void(&mut cache, g) {
                    continue;
                }
                let poly = self.big_corners(g);
                let mut sites = [g, (0, 0), (0, 0)];
                self.emit_split(&mut cache, &mut mesh, &poly, &mut sites, 0, max_level, (x0, z0));
                if let Some(stair) = self.stair(g) {
                    stair.emit_slope(&mut mesh, (x0, z0));
                }
                if let Some(slab) = self.slab(&mut cache, g, &planar::points(&poly)) {
                    self.emit_slab(&mut mesh, &slab, (x0, z0));
                }
            }
        }
        let scale = 4f64.powi(lod as i32);
        if !self.lab {
            self.canals.mesh_square(&mut mesh, (x0, z0), size, scale, &|c| self.canal_floor(c), &|p| self.is_void(p.x as f32, p.y as f32));
        }
        mesh
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_split(
        &self,
        cache: &mut Cache,
        mesh: &mut ColumnMesh,
        poly: &[Corner],
        sites: &mut [(i32, i32); 3],
        level: usize,
        max_level: usize,
        origin: (f64, f64),
    ) {
        self.visit_split(cache, poly, sites, level, max_level, &mut |cache, piece, plate, kept| {
            if kept {
                self.emit_prism(cache, mesh, &planar::points(piece), plate, max_level, origin);
            }
        });
    }

    /// Each plate's pieces within `poly` (its site chain from `level`
    /// down), cut by the canals, each with its plate and whether it is
    /// meshed (not under `LEAST_PIECE`): the mesher's and navigation's one
    /// source of the plates' outlines, a planar map (see `planar`).
    fn visit_split(&self, cache: &mut Cache, poly: &[Corner], sites: &mut [(i32, i32); 3], level: usize, max_level: usize, f: &mut dyn FnMut(&mut Cache, &[Corner], &Plate, bool)) {
        if poly.len() < 3 {
            return;
        }
        // (No ground in a void.)
        if level == 0 && self.in_void(cache, sites[0]) {
            return;
        }
        // A pit's stair: its steps cut out first, the rest as any plate.
        if level == 0
            && let Some(stair) = self.stair(sites[0])
        {
            let (steps, rest) = stair.cut(sites[0], poly, self);
            for (piece, k) in steps {
                let plate = self.step_plate(sites[0], &stair, k);
                let pieces = if self.lab { vec![piece] } else { self.canals.cut(piece, self) };
                for piece in pieces {
                    f(cache, &piece, &plate, planar::area(&piece) >= LEAST_PIECE);
                }
            }
            self.visit_plain(cache, &rest, sites, level, max_level, f);
            return;
        }
        self.visit_plain(cache, poly, sites, level, max_level, f);
    }

    /// As `visit_split`, but for any stair.
    fn visit_plain(&self, cache: &mut Cache, poly: &[Corner], sites: &mut [(i32, i32); 3], level: usize, max_level: usize, f: &mut dyn FnMut(&mut Cache, &[Corner], &Plate, bool)) {
        if poly.len() < 3 {
            return;
        }
        let coarse = level >= max_level && !self.site_detail(level, sites[level]);
        if coarse && cache.tol > 0.0 && self.splits(level, sites[level]) {
            // (Fitted, see `visit_fitted`.)
            self.visit_fitted(cache, poly, sites, level, f);
            return;
        } else if coarse || !self.splits(level, sites[level]) {
            let plate = self.plate(cache, *sites, level);
            let pieces = if self.lab { vec![poly.to_vec()] } else { self.canals.cut(poly.to_vec(), self) };
            for piece in pieces {
                f(cache, &piece, &plate, planar::area(&piece) >= LEAST_PIECE);
            }
            return;
        }
        // Pieces of the next level's Voronoi diagram inside this polygon.
        let next = level + 1;
        for (g, piece) in self.cells_within(next, poly) {
            sites[next] = g;
            // (Under `LEAST_PIECE`, nothing to mesh: told as a hole.)
            if planar::area(&piece) < LEAST_PIECE {
                let plate = self.plate(cache, *sites, next);
                f(cache, &piece, &plate, false);
                continue;
            }
            self.visit_plain(cache, &piece, sites, next, max_level, f);
        }
    }

    /// The cells of `level`'s sites within `poly` (by its corners, see
    /// `planar`), each with its site, in order: every site whose cell may
    /// meet it, clipped to it, those left empty passed over. The plates' and
    /// the wells' one way to them.
    fn cells_within(&self, level: usize, poly: &[Corner]) -> Vec<((i32, i32), Vec<Corner>)> {
        let grid = GRID[level];
        let at = planar::points(poly);
        let (mut min, mut max) = (at[0], at[0]);
        for p in &at {
            min = min.min(*p);
            max = max.max(*p);
        }
        let (ix0, iz0) = ((min.x / grid).floor() as i32 - 1, (min.y / grid).floor() as i32 - 1);
        let (ix1, iz1) = ((max.x / grid).ceil() as i32 + 1, (max.y / grid).ceil() as i32 + 1);
        // (A site further than `CELL_REACH` from `poly` has no part of it:
        // two in three of those round it, each clipped 24 times to nothing.)
        let reach = CELL_REACH * grid;
        // (The sites round them all, found once: found for each clip and
        // each corner made, a site's place, its jitter and the ordered
        // ground round it, the wells took three times as long.)
        let wide = (ix1 - ix0 + 5) as usize;
        let table: Vec<DVec2> = (iz0 - 2..=iz1 + 2).flat_map(|z| (ix0 - 2..=ix1 + 2).map(move |x| (x, z))).map(|h| self.site(level, h)).collect();
        let sites_here = Sites { world: self, level, x0: ix0 - 2, z0: iz0 - 2, wide, high: (iz1 - iz0 + 5) as usize, table: &table };
        let site = |h: (i32, i32)| table[(h.1 - iz0 + 2) as usize * wide + (h.0 - ix0 + 2) as usize];
        let mut spare = Vec::new();
        let mut out = Vec::new();
        for iz in iz0..=iz1 {
            for ix in ix0..=ix1 {
                #[cfg(test)]
                let skip = SKIP_FAR.load(std::sync::atomic::Ordering::Relaxed);
                #[cfg(not(test))]
                let skip = true;
                if skip && distance_to(site((ix, iz)), &at) > reach {
                    continue;
                }
                let piece = Self::cell_by(level, (ix, iz), poly, &sites_here, &mut spare);
                if piece.len() >= 3 {
                    out.push(((ix, iz), piece));
                }
            }
        }
        out
    }

    /// Every plate piece whose biggest plate's site lies within `lo`..`hi`
    /// (x, z), at full detail, by its corners (see `planar`), with its plate
    /// and whether `mesh_column` meshes it (pieces under `LEAST_PIECE` are
    /// left out: holes).
    pub fn plate_pieces(&self, lo: DVec2, hi: DVec2) -> Vec<(Vec<Corner>, Plate, bool)> {
        let mut cache = Cache::default();
        let mut out = Vec::new();
        let g0 = GRID[0];
        let (gx0, gz0) = ((lo.x / g0).floor() as i32 - 1, (lo.y / g0).floor() as i32 - 1);
        let (gx1, gz1) = ((hi.x / g0).ceil() as i32 + 1, (hi.y / g0).ceil() as i32 + 1);
        for gz in gz0..=gz1 {
            for gx in gx0..=gx1 {
                let g = (gx, gz);
                let s = self.site(0, g);
                if s.x < lo.x || s.x >= hi.x || s.y < lo.y || s.y >= hi.y {
                    continue;
                }
                let poly = self.big_corners(g);
                let mut sites = [g, (0, 0), (0, 0)];
                self.visit_split(&mut cache, &poly, &mut sites, 0, 2, &mut |_, piece, plate, kept| out.push((piece.to_vec(), *plate, kept)));
            }
        }
        out
    }


    /// A plate's top at `p` (x, z): flat, tilted, or on an earthwork.
    pub fn plate_top(&self, plate: &Plate, p: DVec2) -> f64 {
        plate.top(self, p)
    }

    /// Whether, at full detail, pillar plates grow the library's form (open
    /// stacked slabs narrower than their plate) rather than a solid prism.
    pub fn grows_pillars_at_full_detail(&self) -> bool {
        self.grows_pillars(2)
    }

    /// Fraction of open sky above a point on a plate, from the horizon angle
    /// in eight directions. Stylised rather than exact.
    fn sky_visibility(&self, cache: &mut Cache, p: DVec2, h: f64, max_level: usize) -> f32 {
        const DIST: [f64; 4] = [1.5, 4.0, 10.0, 25.0];
        let mut grid = cache.sky.take();
        let mut vis = 0.0;
        for i in 0..8 {
            let a = i as f64 / 8.0 * std::f64::consts::TAU;
            let dir = DVec2::new(a.cos(), a.sin());
            let mut max_sin: f64 = 0.0;
            for r in DIST {
                let q = p + dir * r;
                let dh = match &mut grid {
                    Some(grid) => self.grid_surface(cache, grid, q, max_level),
                    None => self.surface(cache, q.x, q.y, max_level),
                } - h;
                if dh > 0.0 {
                    max_sin = max_sin.max(dh / (dh * dh + r * r).sqrt());
                }
            }
            vis += 1.0 - max_sin;
        }
        cache.sky = grid;
        (vis / 8.0) as f32
    }

    /// The ground's height at `q` read between the grid's (`SkyGrid`), each
    /// found as first asked for; off the grid, found there.
    fn grid_surface(&self, cache: &mut Cache, grid: &mut SkyGrid, q: DVec2, max_level: usize) -> f64 {
        let t = (q - grid.origin) / grid.step;
        let (i, j) = (t.x.floor(), t.y.floor());
        if i < 0.0 || j < 0.0 || i as usize + 1 >= grid.n || j as usize + 1 >= grid.n {
            return self.surface(cache, q.x, q.y, max_level);
        }
        let (i, j) = (i as usize, j as usize);
        let mut at = |a: usize, b: usize| {
            let k = b * grid.n + a;
            if grid.heights[k].is_nan() {
                let p = grid.origin + DVec2::new(a as f64, b as f64) * grid.step;
                grid.heights[k] = self.surface(cache, p.x, p.y, max_level);
            }
            grid.heights[k]
        };
        let (fx, fz) = (t.x - i as f64, t.y - j as f64);
        let (h00, h10, h01, h11) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
        (h00 * (1.0 - fx) + h10 * fx) * (1.0 - fz) + (h01 * (1.0 - fx) + h11 * fx) * fz
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
        if plate.pillar > 0.0
            && self.grows_pillars(max_level)
            && !cache.plain
            && let Some(library) = &self.library
        {
            let base = Plate { height: plate.height - plate.pillar, pillar: 0.0, sloped: false, tilt: DVec2::ZERO, ..*plate };
            self.emit_prism(cache, mesh, poly, &base, max_level, origin);
            self.emit_pillar(library, mesh, poly, plate, origin);
            return;
        }
        let h = plate.height;
        let n = poly.len();
        let centroid = poly.iter().copied().sum::<DVec2>() / n as f64;
        // The top's height at each corner and at the centroid: flat, or on
        // an earthwork's slope.
        let top = |p: DVec2| plate.top(self, p);
        let tops: Vec<f64> = poly.iter().map(|&p| top(p)).collect();
        let center_top = top(centroid);

        // What lies just beyond each edge, for the walls: they reach just
        // below the lowest of it, and are left out where it stands as high
        // as the top (they would never be seen). Sampled just across the
        // edge, square to it, a little in from each end and every 8 m
        // between. (Three samples out from the centre, at the ends and the
        // middle, missed it: at the ends they fell on a third plate, so a
        // neighbour tilted away or a sunk station court was not seen, and
        // walls stopped short of it or were left out, holes onto the void
        // under the world, the user.)
        let mut lowest = tops.iter().copied().fold(h, f64::min);
        let mut beyond = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let edge = b - a;
            let length = edge.length();
            let mut edge_low = f64::MAX;
            if length > 1e-6 {
                let mut out = DVec2::new(edge.y, -edge.x) / length;
                if out.dot((a + b) * 0.5 - centroid) < 0.0 {
                    out = -out;
                }
                let inset = (0.3 / length).min(0.25);
                let between = (length / 8.0).ceil().clamp(1.0, 8.0) as usize;
                let ts = [inset, 1.0 - inset].into_iter().chain((0..between).map(|k| (k as f64 + 0.5) / between as f64));
                for t in ts {
                    let p = a + edge * t + out * 0.3;
                    edge_low = edge_low.min(self.solid(cache, p.x, p.y, max_level));
                }
                // (A station's court cuts the plates along its sides' lines
                // drawn on past it, so a wall facing it may be 50 m long and
                // border it for 8: samples could miss it, and the wall
                // stopped short of the court's floor, the user's hole.)
                if !self.lab
                    && let Some(f) = self.canals.court_floor_near(a, b, &|c| self.canal_floor(c))
                {
                    edge_low = edge_low.min(f);
                }
            }
            // (Fitted, what is drawn beside may be up to `tol` lower.)
            let edge_low = edge_low - cache.tol;
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
        let top_start = mesh.positions.len();
        if plate.sloped || plate.tilt != DVec2::ZERO {
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
        // The top tells the shader which plate it is, so the shader can go
        // on splitting it below what the geometry draws.
        let id = (self.rand(plate.level, plate.sites[plate.level], 11) * 1024.0).floor() as f32;
        let big = self.districts.at(centroid.x, centroid.y).rules().pieces >= 32.0;
        let code = crate::mesh::plate_code(plate.level, id, big);
        mesh.plate_of(mesh.positions.len() - top_start, code);
        // (Its kerbs, near: flat plates only.)
        if self.kerbs && max_level == 2 && !cache.plain && !plate.sloped && plate.tilt == DVec2::ZERO {
            self.emit_kerbs(mesh, poly, plate, h, origin);
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
            // (Its line: the way it faces and how far out, the same for the
            // pieces of a finer level along it.)
            let line = hash01((normal.y.atan2(normal.x) * 1e4).round() as i32, (normal.dot(a) * 100.0).round() as i32, 17, self.seed);
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
            // (Its plate and line, for its pattern, if it is tall enough:
            // see the walls in `terrain.wgsl`.)
            let tall = tops[i].max(tops[j]) - beyond[i] >= self.wall_least - 1e-3;
            mesh.plate_of(4, if tall { crate::mesh::wall_code(code, (line * 16.0) as u32) } else { 0.0 });
        }
    }
}

/// The least plate piece meshed (m²): only one of no area is left out.
/// (Every piece is meshed, however thin. At half a square metre, as it was
/// from the first prototype, the slivers left out were holes through the
/// ground into the void under the world, hundreds a patch; the user had seen
/// them. At a square centimetre a few still were. Found by the navigable
/// nodes' tests, 2026-11-03.)
const LEAST_PIECE: f64 = 1e-9;

/// The form terrain pillars grow, if the library has it.
const PILLAR_FORM: &str = "pillar";
/// Pieces per pillar at most.
const PILLAR_PIECES: usize = 120;

impl PlateWorld {
    /// Grows the pillar form on a pillar plate's outline, from its base up,
    /// and scales the result to the pillar's height exactly (so it matches
    /// coarser levels of detail, collision and `height_at`).
    fn emit_pillar(&self, library: &Library, mesh: &mut ColumnMesh, poly: &[DVec2], plate: &Plate, origin: (f64, f64)) {
        forms::mesh_into(mesh, &self.grown_pillar(library, poly, plate, origin));
    }

    /// A pillar plate's grown form: its prisms, `origin` taken off their
    /// points, scaled to the pillar's height exactly. The mesher's and
    /// navigation's one source of pillars.
    fn grown_pillar(&self, library: &Library, poly: &[DVec2], plate: &Plate, origin: (f64, f64)) -> Vec<forms::Prism> {
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
            return Vec::new();
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
        prisms
    }

    /// A pillar plate's grown form, as `mesh_column` meshes it at full
    /// detail: its prisms, their points less `origin` (none if pillars are
    /// not grown here).
    pub fn pillar_prisms(&self, poly: &[DVec2], plate: &Plate, origin: DVec2) -> Vec<forms::Prism> {
        match self.library.as_ref() {
            Some(library) if plate.pillar > 0.0 && self.grows_pillars(2) => self.grown_pillar(library, poly, plate, (origin.x, origin.y)),
            _ => Vec::new(),
        }
    }
}

/// Where the plates' lines and corners are (see `planar`).
impl Geometry for PlateWorld {
    fn site(&self, level: usize, g: (i32, i32)) -> DVec2 {
        PlateWorld::site(self, level, g)
    }

    fn line(&self, l: Line) -> (DVec2, DVec2) {
        match l {
            Line::Bisector { level, a, b } => self.bisector(level, a, b),
            Line::Canal { .. } | Line::Station { .. } => self.canals.line(l),
            Line::Stair { site, k } => self.stair(site).expect("a stair's line: its stair").line(k),
            Line::Bound { level, site, k } => {
                let s = PlateWorld::site(self, level as usize, site);
                let corner = |k: u8| {
                    let o = [(-3.0, -3.0), (3.0, -3.0), (3.0, 3.0), (-3.0, 3.0)][k as usize % 4];
                    s + DVec2::new(o.0, o.1) * GRID[level as usize]
                };
                let (p, q) = (corner(k), corner(k + 1));
                (p, (q - p).perp())
            }
            Line::Grid { a, b, c } => planar::grid_line(a, b, c),
        }
    }
}

/// The sites of one level round a polygon, found once (`table`, from `x0`,
/// `z0`, `wide` by `high`), for its cells' clipping; any other's, the
/// world's.
struct Sites<'a> {
    world: &'a PlateWorld,
    level: usize,
    x0: i32,
    z0: i32,
    wide: usize,
    high: usize,
    table: &'a [DVec2],
}

impl Geometry for Sites<'_> {
    fn site(&self, level: usize, g: (i32, i32)) -> DVec2 {
        let (x, z) = (g.0 - self.x0, g.1 - self.z0);
        if level == self.level && x >= 0 && z >= 0 && (x as usize) < self.wide && (z as usize) < self.high {
            self.table[z as usize * self.wide + x as usize]
        } else {
            PlateWorld::site(self.world, level, g)
        }
    }

    fn line(&self, l: Line) -> (DVec2, DVec2) {
        match l {
            Line::Bisector { level, a, b } => self.bisector(level, a, b),
            _ => self.world.line(l),
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

    /// The plates are one planar map (see `planar`): their pieces stitched
    /// and every corner rounded to a grid (1/1024 m), every edge inside the
    /// area has its twin, the other way, once, and itself once: no crack
    /// between pieces, none overlapping. All but edges along a canal's or a
    /// station's line, the corridor or court beyond them. Squares on two
    /// seeds, each district and the canals' and stairs' among them.
    #[test]
    fn plates_one_planar_map() {
        use std::collections::HashMap;
        let (mut edges_n, mut squares) = (0, 0);
        for seed in [1, 2] {
            let world = PlateWorld::new(crate::WORLD_SIZE, seed);
            let mut centres: Vec<DVec2> = (0..8).map(|k| DVec2::new((k * 1361 % 16) as f64 * 1024.0 + 300.0, (k * 7 % 16) as f64 * 1024.0 + 700.0)).collect();
            // (Over a canal and a station, for their cuts.)
            let c = &world.canals.lines[0];
            centres.push(c.start + c.dir * (c.length * 0.5));
            centres.push(world.canals.stations[0]);
            for c in centres {
                let (lo, hi) = (c - 300.0, c + 300.0);
                let mut pieces: Vec<Vec<Corner>> = world.plate_pieces(lo, hi).into_iter().map(|p| p.0).collect();
                planar::stitch(&mut pieces, &world);
                let grid = |p: DVec2| ((p.x * 1024.0).round() as i64, (p.y * 1024.0).round() as i64);
                let mut count: HashMap<((i64, i64), (i64, i64)), i32> = HashMap::new();
                let mut inside: Vec<((i64, i64), (i64, i64), Line)> = Vec::new();
                for piece in &pieces {
                    for k in 0..piece.len() {
                        let (a, b) = (piece[k], piece[(k + 1) % piece.len()]);
                        let (ga, gb) = (grid(a.at), grid(b.at));
                        if ga == gb {
                            continue;
                        }
                        *count.entry((ga, gb)).or_default() += 1;
                        let m = (a.at + b.at) * 0.5;
                        if m.x > lo.x + 200.0 && m.x < hi.x - 200.0 && m.y > lo.y + 200.0 && m.y < hi.y - 200.0 && !matches!(a.next, Line::Canal { .. } | Line::Station { .. }) {
                            inside.push((ga, gb, a.next));
                        }
                    }
                }
                let bad: Vec<&((i64, i64), (i64, i64), Line)> = inside.iter().filter(|(a, b, _)| count.get(&(*a, *b)) != Some(&1) || count.get(&(*b, *a)) != Some(&1)).collect();
                assert!(bad.is_empty(), "seed {seed}, round {c:?}: {} of {} edges inside without their twin, as {:?}", bad.len(), inside.len(), &bad[..bad.len().min(4)]);
                edges_n += inside.len();
                squares += 1;
            }
        }
        println!("{edges_n} edges inside {squares} squares, each with its twin");
    }

    /// Passing over sites further than `CELL_REACH` from a plate changes
    /// none of its pieces: the same pieces, in order, either way, over
    /// squares of every district on two seeds, sites' ordered ground among
    /// them. (Its own test: it alone sets `SKIP_FAR`.)
    #[test]
    fn far_cells_skipped_exactly() {
        for seed in [1, 2] {
            let world = PlateWorld::new(crate::WORLD_SIZE, seed);
            for k in 0..12 {
                let c = DVec2::new((k * 1361 % 16) as f64 * 1024.0 + 512.0, (k * 7 % 16) as f64 * 1024.0 + 512.0);
                let (lo, hi) = (c - 200.0, c + 200.0);
                SKIP_FAR.store(true, std::sync::atomic::Ordering::Relaxed);
                let skipped = world.plate_pieces(lo, hi);
                SKIP_FAR.store(false, std::sync::atomic::Ordering::Relaxed);
                let all = world.plate_pieces(lo, hi);
                SKIP_FAR.store(true, std::sync::atomic::Ordering::Relaxed);
                assert!(!all.is_empty());
                assert_eq!(skipped.len(), all.len(), "seed {seed}, square round {c:?}");
                for (a, b) in skipped.iter().zip(&all) {
                    assert_eq!((&a.0, a.1.sites, a.1.level, a.2), (&b.0, b.1.sites, b.1.level, b.2), "seed {seed}, square round {c:?}");
                }
            }
        }
    }

    /// What finding the wells costs (`WELL_REACH`), in four districts'
    /// squares: their pieces built twice on one world, the second time
    /// with every parent's wells found already. (Sampling every plate,
    /// 2 to 3 ms a parent; with the siblings' screen first, 0.4 to 0.5.)
    /// (`cargo test -p worldgen --profile fast wells_cost -- --ignored --nocapture`)
    #[test]
    #[ignore]
    fn wells_cost() {
        for (name, c) in [("tiers", DVec2::new(5248.0, 640.0)), ("stacks", DVec2::new(12928.0, 128.0)), ("broken", DVec2::new(7296.0, 640.0)), ("floor", DVec2::new(10880.0, 128.0))] {
            let world = PlateWorld::new(crate::WORLD_SIZE, 1);
            let (lo, hi) = (c - 320.0, c + 320.0);
            let t = std::time::Instant::now();
            let n = world.plate_pieces(lo, hi).len();
            let first = t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            world.plate_pieces(lo, hi);
            let again = t.elapsed().as_secs_f64();
            let parents = world.wells.read().unwrap().len();
            let raised = world.wells.read().unwrap().values().map(|w| w.len()).sum::<usize>();
            println!("{name}: {n} pieces; first {first:.2} s, again {again:.2} s: wells {:.2} s for {parents} parents ({:.1} ms each), {raised} raised", first - again, (first - again) * 1000.0 / parents as f64);
        }
    }

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

    /// A wall's number for its pattern comes from its line, so the finer
    /// level's pieces along a coarser plate's wall have the coarser wall's
    /// (its panels changed as it was approached, the user: it was its
    /// plate's).
    #[test]
    fn walls_keep_their_number_across_lods() {
        // (Each wall: its ends (m, world), the way it faces, its number.)
        fn walls(w: &PlateWorld, lod: u32, cx: i32, cz: i32) -> Vec<(DVec2, DVec2, DVec2, u32)> {
            let m = w.mesh_column(lod, cx, cz);
            let (x0, z0) = crate::mesh::column_origin(lod, cx, cz);
            let at = |i: usize| DVec2::new((m.positions[i][0] + x0) as f64, (m.positions[i][2] + z0) as f64);
            let wall = |i: usize| m.normals[i][1].abs() < 1e-3 && m.plate.get(i).is_some_and(|&c| c > 0.0);
            let mut out = Vec::new();
            let mut i = 0;
            while i < m.positions.len() {
                if !wall(i) {
                    i += 1;
                    continue;
                }
                let n = DVec2::new(m.normals[i][0] as f64, m.normals[i][2] as f64);
                out.push((at(i), at(i + 1), n, (m.plate[i] / 8192.0) as u32));
                i += 4;
            }
            out
        }
        let w = PlateWorld::new(4096.0, 3);
        let per = (column_size(1) / column_size(0)).round() as i32;
        let mut matched = 0;
        for c in 0..4 {
            let (cx, cz) = (c, c * 2 + 1);
            let coarse = walls(&w, 1, cx, cz);
            let mut fine = Vec::new();
            for dz in 0..per {
                for dx in 0..per {
                    fine.extend(walls(&w, 0, cx * per + dx, cz * per + dz));
                }
            }
            for &(a, b, n, number) in &coarse {
                let along = (b - a).normalize();
                for &(fa, fb, fn_, fine_number) in &fine {
                    let on = |p: DVec2| (p - a).dot(n).abs() < 0.01;
                    let (s0, s1) = ((fa - a).dot(along), (fb - a).dot(along));
                    let overlap = s0.max(s1).min((b - a).length()) - s0.min(s1).max(0.0);
                    if (fn_ - n).length() < 1e-3 && on(fa) && on(fb) && overlap > 0.1 {
                        assert_eq!(number, fine_number, "wall {a} {b} facing {n}");
                        matched += 1;
                    }
                }
            }
        }
        assert!(matched > 20, "only {matched} walls shared by both levels");
    }

    /// Only walls at least `wall_least` tall carry a pattern: a low step
    /// is left plain.
    #[test]
    fn low_walls_are_plain() {
        // (Wall vertices with a pattern, and without.)
        fn count(w: &PlateWorld) -> (usize, usize) {
            let (mut coded, mut plain) = (0, 0);
            for c in 0..6 {
                let m = w.mesh_column(0, c, c * 3);
                for (i, n) in m.normals.iter().enumerate() {
                    if n[1].abs() < 1e-3 {
                        if m.plate.get(i).is_some_and(|&p| p > 0.0) { coded += 1 } else { plain += 1 }
                    }
                }
            }
            (coded, plain)
        }
        let mut w = PlateWorld::new(4096.0, 3);
        w.wall_least = 0.0;
        let (all, none) = count(&w);
        assert!(all > 0 && none == 0, "{all} patterned, {none} plain with no least height");
        w.wall_least = 1.5;
        let (tall, low) = count(&w);
        assert!(tall > 0 && low > 0 && tall + low == all, "{tall} patterned, {low} plain of {all}");
    }
}
