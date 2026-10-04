//! Sites: where structures grow out of the world.
//!
//! A grid of cells covers the torus; each cell may hold one site. Whether it
//! does, and what grows there, comes from the `sites` rules in the structure
//! library: each rule names a district, a style, a weight and a range of
//! sizes. Everything is decided by hashing the cell, so the world is the same
//! every time and wraps seamlessly. Sites keep clear of each other, of the
//! canals and of the hand-placed structures.
//!
//! A site reshapes the ground itself rather than standing on a slab: the
//! plates around it (see `plates.rs`) become a flat core at the site's own
//! height, where the structure stands, ringed by terraces that step from the
//! core to the surrounding ground. The core can rise above the ground (an
//! acropolis), sit level with it (a plaza) or sink into it (a court). Its
//! outline follows the plates, so it is ragged rather than drawn.

use glam::{DVec2, Quat, Vec2, Vec3};

use crate::forms::{self, Growth, Grower, Prism};
use serde::Deserialize;

use crate::canal::HALF_WIDTH;
use crate::district::District;
use crate::noise::hash01;
use crate::plates::PlateWorld;
use crate::structure::{self, Library, Placement, Solid};

/// The grid sites are placed on.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SiteGrid {
    /// Distance between cells, in metres (adjusted to fit the world exactly).
    #[serde(default = "default_spacing")]
    pub spacing: f32,
    /// Chance that a cell holds a site at all.
    #[serde(default = "default_chance")]
    pub chance: f32,
}

impl Default for SiteGrid {
    fn default() -> Self {
        Self { spacing: default_spacing(), chance: default_chance() }
    }
}

impl SiteGrid {
    /// The colossi's grid by default: about one per district.
    pub fn colossi() -> Self {
        Self { spacing: 2048.0, chance: 0.85 }
    }
}

/// Sites come in two layers on their own grids: ordinary sites, and colossi
/// (far bigger, far apart; ordinary sites keep clear of them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layer {
    Sites,
    Colossi,
}

impl Layer {
    fn grid(self, library: &Library) -> SiteGrid {
        match self {
            Layer::Sites => library.site_grid,
            Layer::Colossi => library.colossus_grid,
        }
    }

    fn rules(self, library: &Library) -> &[SiteRule] {
        match self {
            Layer::Sites => &library.sites,
            Layer::Colossi => &library.colossi,
        }
    }

    fn seed(self) -> u32 {
        match self {
            Layer::Sites => 0x517e_5eed,
            Layer::Colossi => 0xc0_1055,
        }
    }
}

fn default_spacing() -> f32 {
    768.0
}
fn default_chance() -> f32 {
    0.5
}
fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}
fn default_lift() -> (f32, f32) {
    (3.0, 9.0)
}
fn default_rings() -> u32 {
    3
}
fn default_ring_width() -> f32 {
    22.0
}
fn default_step() -> f32 {
    0.5
}
fn default_size() -> ((f32, f32, f32), (f32, f32, f32)) {
    ((60.0, 40.0, 60.0), (120.0, 80.0, 120.0))
}
fn default_core() -> (f32, f32) {
    (40.0, 90.0)
}
fn default_terrace_chance() -> f32 {
    0.35
}
fn default_stairs() -> f32 {
    0.5
}
fn default_footprint() -> (f32, f32) {
    (60.0, 120.0)
}

/// An earthwork instead of terraces: `shape` Round or Square, `ramps` how
/// much further the slope reaches along the four axes (0 none, 1 twice).
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct EarthworkRule {
    #[serde(default)]
    pub shape: EarthworkShape,
    #[serde(default)]
    pub ramps: f32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub enum EarthworkShape {
    Round,
    #[default]
    Square,
}

/// "In this district, structures of this style grow, this often, this big,
/// on this kind of ground."
#[derive(Clone, Debug, Deserialize)]
pub struct SiteRule {
    pub district: District,
    #[serde(default = "one")]
    pub weight: f32,
    /// A centrepiece of a box style...
    #[serde(default)]
    pub style: Option<String>,
    /// ...its smallest and largest size (x, y, z), metres.
    #[serde(default = "default_size")]
    pub size: ((f32, f32, f32), (f32, f32, f32)),
    /// The core's radius without a centrepiece, metres.
    #[serde(default = "default_core")]
    pub core: (f32, f32),
    /// The form grown on each plate of the core (not under the centrepiece).
    #[serde(default)]
    pub plates: Option<String>,
    /// The form grown on some of the terrace plates...
    #[serde(default)]
    pub terraces: Option<String>,
    /// ...this fraction of them.
    #[serde(default = "default_terrace_chance")]
    pub terrace_chance: f32,
    /// The fraction of plates with a higher neighbour that get stairs up
    /// to it (instead of a form).
    #[serde(default = "default_stairs")]
    pub stairs: f32,
    /// A form grown on the whole footprint: the big plate at the centre,
    /// scaled to `footprint` metres from the centre to its furthest corner.
    #[serde(default)]
    pub form: Option<String>,
    #[serde(default = "default_footprint")]
    pub footprint: (f32, f32),
    /// Slope the ground continuously instead of in terraces (over the same
    /// `rings` x `ring_width`).
    #[serde(default)]
    pub earthwork: Option<EarthworkRule>,
    /// Reshape the ground into a core and terraces (default), or stand
    /// straight on the ground as it is.
    #[serde(default = "yes")]
    pub plinth: bool,
    /// How far the core rises above the surrounding ground, smallest and
    /// largest, metres; negative sinks it into a court.
    #[serde(default = "default_lift")]
    pub lift: (f32, f32),
    /// Terraces between the core and the surrounding ground.
    #[serde(default = "default_rings")]
    pub rings: u32,
    /// Width of each terrace, metres.
    #[serde(default = "default_ring_width")]
    pub ring_width: f32,
    /// Terrace heights are multiples of this, metres.
    #[serde(default = "default_step")]
    pub step: f32,
    /// An ordered site: square terraces on a regular grid of plates, the
    /// centrepiece square to it, stairs up the four axes; and the plates
    /// around it regular too, their order fading over this many metres
    /// beyond the terraces (0: an ordinary site).
    #[serde(default)]
    pub order: f32,
}

/// The ground a site reshapes: circles around its centre, though the plates
/// that fall inside them make the actual outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SiteGround {
    /// World position (canonical, 0..size).
    pub center: DVec2,
    /// Height of the core, where the structure stands.
    pub top: f64,
    /// Plates whose site lies within `core` of the centre form the core...
    pub core: f64,
    /// ...those within `radius` the terraces.
    pub radius: f64,
    pub rings: u32,
    pub step: f64,
    /// A continuous earthwork instead of stepped terraces.
    pub earthwork: Option<Earthwork>,
    /// Ordered: square, on the world's grid, its order fading over this
    /// many metres beyond `radius` (0: an ordinary, round site).
    pub order: f64,
}

/// An ordered site's square reaches this fraction of its radius, so its
/// corners stay about as far out as a round site's edge.
const SQUARE: f64 = 0.85;

/// An earthwork: the ground sloping in crisp planes from the core's edge
/// to the natural ground, instead of stepping down in terraces. Raised
/// cores become mounds, sunk ones bowls; plates on the slope are tilted
/// facets that meet their neighbours seamlessly (see `plates.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Earthwork {
    pub square: bool,
    /// Orientation (radians) of the square and of the ramps.
    pub yaw: f64,
    /// How far the slope reaches beyond the core, metres...
    pub width: f64,
    /// ...and how much further along the four axes (0: none, 1: twice).
    pub ramps: f64,
}

impl SiteGround {
    /// How far an offset from the centre lies, as the site measures it:
    /// round, or (ordered) square on the world's axes.
    pub fn measure(&self, rel: DVec2) -> f64 {
        if self.order > 0.0 { rel.x.abs().max(rel.y.abs()) / SQUARE } else { rel.length() }
    }

    /// How ordered the plates are at an offset from the centre: 1 on the
    /// site, fading to 0 over `order` metres beyond it.
    pub fn orderliness(&self, rel: DVec2) -> f64 {
        if self.order <= 0.0 {
            return 0.0;
        }
        let d = self.measure(rel);
        let t = ((d - self.radius) / self.order).clamp(0.0, 1.0);
        1.0 - t * t * (3.0 - 2.0 * t)
    }

    /// The earthwork's surface at a point (any wrapped copy), where the
    /// ground's smooth landform is `natural`: the core's height on the
    /// core, the natural ground beyond the slope. None without an
    /// earthwork, or outside the site.
    pub fn surface(&self, p: DVec2, natural: f64, world_size: f64) -> Option<f64> {
        let e = self.earthwork?;
        let wrap = |d: f64| d - (d / world_size).round() * world_size;
        let rel = DVec2::new(wrap(p.x - self.center.x), wrap(p.y - self.center.y));
        if rel.length() >= self.radius {
            return None;
        }
        let (s, c) = e.yaw.sin_cos();
        let local = DVec2::new(rel.x * c + rel.y * s, -rel.x * s + rel.y * c);
        let d = if e.square { local.x.abs().max(local.y.abs()) } else { local.length() };
        if d <= self.core {
            return Some(self.top);
        }
        // On an axis 1, on a diagonal 0: ramps reach further along the axes.
        let axis = local.x.abs().max(local.y.abs()) / local.length().max(1e-6);
        let along = ((axis - std::f64::consts::FRAC_1_SQRT_2) / (1.0 - std::f64::consts::FRAC_1_SQRT_2)).clamp(0.0, 1.0);
        let width = e.width * (1.0 + e.ramps * along.powi(6));
        let t = ((d - self.core) / width).clamp(0.0, 1.0);
        Some(self.top + (natural - self.top) * t)
    }

    /// The height of a plate whose site is `distance` from the centre, where
    /// the ground's smooth landform is `natural`; None outside the site.
    pub fn height(&self, distance: f64, natural: f64) -> Option<f64> {
        if distance >= self.radius {
            return None;
        }
        if distance <= self.core {
            return Some(self.top);
        }
        // Ring 1 is the outermost terrace, ring `rings` the one by the core.
        let width = (self.radius - self.core) / self.rings as f64;
        let ring = ((self.radius - distance) / width).floor() + 1.0;
        let t = ring / (self.rings as f64 + 1.0);
        let h = natural + (self.top - natural) * t;
        Some((h / self.step).round() * self.step)
    }
}

/// A planned site: where it is (in the world's canonical 0..size range),
/// the ground it shapes and what grows there.
#[derive(Clone, Debug)]
pub struct Site {
    pub layer: Layer,
    /// The grid cell, which identifies the site in its layer.
    pub cell: (i32, i32),
    pub at: (f32, f32),
    pub seed: u32,
    /// A structure of a box style at the centre.
    pub centrepiece: Option<Placement>,
    /// None when the site stands on the ground as it is.
    pub ground: Option<SiteGround>,
    /// Forms grown on the core's plates and on some of the terraces'.
    pub plates: Option<String>,
    pub terraces: Option<String>,
    pub terrace_chance: f32,
    pub stairs: f32,
    /// A form grown on the whole footprint, and the footprint's radius.
    pub form: Option<(String, f32)>,
}

/// How far a site's centre may stray from its cell's centre, as a fraction
/// of the cell.
const JITTER: f32 = 0.15;
/// A site's outer radius at most, as a fraction of the cell: with the
/// jitter, neighbours can never touch.
const REACH: f32 = 0.5 - JITTER - 0.02;
/// Clearance between a site and a canal's lip, metres.
const CANAL_CLEARANCE: f32 = 12.0;
/// Footprints smaller than this (in either direction) are not worth placing.
const MIN_FOOTPRINT: f32 = 24.0;
/// The core reaches this far beyond the structure's corners: plates are
/// about 32 m across, so a lower terrace's plate can reach in this far.
const CORE_MARGIN: f32 = 24.0;

/// Cells per side and the cell size.
fn cells(world_size: f32, spacing: f32) -> (i32, f32) {
    let n = (world_size / spacing.max(16.0)).round().max(1.0) as i32;
    (n, world_size / n as f32)
}

/// The site in a grid cell, if any. Cheap: a few samples of the landform.
pub fn plan(library: &Library, world: &PlateWorld, cell: (i32, i32)) -> Option<Site> {
    plan_in(Layer::Sites, library, world, cell)
}

/// The site (or colossus) in a grid cell of a layer, if any.
pub fn plan_in(layer: Layer, library: &Library, world: &PlateWorld, cell: (i32, i32)) -> Option<Site> {
    let grid = layer.grid(library);
    let (n, size) = cells(world.size(), grid.spacing);
    let cell = (cell.0.rem_euclid(n), cell.1.rem_euclid(n));
    let seed = world.seed() ^ layer.seed();
    let r = |k: i32| hash01(cell.0, k, cell.1, seed);
    if r(0) >= grid.chance {
        return None;
    }
    let x = (cell.0 as f32 + 0.5 + (r(1) * 2.0 - 1.0) * JITTER) * size;
    let z = (cell.1 as f32 + 0.5 + (r(2) * 2.0 - 1.0) * JITTER) * size;
    let district = world.district(x as f64, z as f64);
    let rules: Vec<&SiteRule> = layer.rules(library).iter().filter(|s| s.district == district).collect();
    let total: f32 = rules.iter().map(|s| s.weight.max(0.0)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = r(3) * total;
    let rule = rules
        .iter()
        .copied()
        .find(|s| {
            pick -= s.weight.max(0.0);
            pick <= 0.0
        })
        .unwrap_or(rules[rules.len() - 1]);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;

    // The site's outer radius stays inside the cell, off the canals and off
    // the hand-placed structures.
    let mut reach = REACH * size;
    if let Some((center, _, _)) = world.nearest_canal(x, z) {
        let distance = (center - Vec2::new(x, z)).length();
        reach = reach.min(distance - HALF_WIDTH as f32 - CANAL_CLEARANCE);
    }
    for other in &library.structures {
        let wrap = |d: f32| d - (d / world.size()).round() * world.size();
        let distance = wrap(other.at.0 - x).hypot(wrap(other.at.1 - z));
        reach = reach.min(distance - placed_ground_radius(other) - CANAL_CLEARANCE);
    }
    // Ordinary sites keep clear of the colossi.
    if layer == Layer::Sites {
        for colossus in near_in(Layer::Colossi, library, world, x, z, 0.0) {
            let Some(ground) = colossus.ground else { continue };
            let wrap = |d: f32| d - (d / world.size()).round() * world.size();
            let distance = wrap(ground.center.x as f32 - x).hypot(wrap(ground.center.y as f32 - z));
            reach = reach.min(distance - ground.radius as f32 - CANAL_CLEARANCE);
        }
    }
    // The terraces' (or earthwork's) width, ramps included.
    let slope = if rule.plinth { rule.rings as f32 * rule.ring_width } else { 0.0 };
    let terraces = slope * (1.0 + rule.earthwork.map_or(0.0, |e| e.ramps.max(0.0)));
    let seed = (cell.0 as u32).wrapping_mul(73_856_093) ^ (cell.1 as u32).wrapping_mul(19_349_663) ^ seed;

    // A centrepiece shrinks (keeping its proportions) to fit what the
    // terraces leave; the core reaches a little beyond its corners.
    let mut form = None;
    let (centrepiece, core) = match &rule.style {
        Some(style) => {
            let ((x0, y0, z0), (x1, y1, z1)) = rule.size;
            let (mut sx, sy, mut sz) = (lerp(x0, x1, r(4)), lerp(y0, y1, r(5)), lerp(z0, z1, r(6)));
            let half_diagonal = sx.hypot(sz) * 0.5;
            let room = reach - terraces - CORE_MARGIN;
            if half_diagonal > room {
                let k = room.max(0.0) / half_diagonal;
                sx *= k;
                sz *= k;
            }
            if sx.min(sz) < MIN_FOOTPRINT {
                return None;
            }
            let placement = Placement {
                style: style.clone(),
                at: (x, z),
                size: (sx, sy, sz),
                // An ordered site's stands square to the grid.
                yaw: if rule.order > 0.0 { (r(7) * 4.0).floor() * 90.0 } else { r(7) * 360.0 },
                seed,
                sink: 0.0,
            };
            (Some(placement), sx.hypot(sz) * 0.5 + CORE_MARGIN)
        }
        None if rule.form.is_some() && rule.plinth => {
            // A form on the whole footprint, which shrinks to fit.
            let footprint = lerp(rule.footprint.0, rule.footprint.1, r(4)).min(reach - terraces - CORE_MARGIN);
            if footprint < MIN_FOOTPRINT {
                return None;
            }
            form = rule.form.clone().map(|f| (f, footprint));
            (None, footprint + CORE_MARGIN)
        }
        None => {
            if !rule.plinth || rule.plates.is_none() {
                return None;
            }
            let core = lerp(rule.core.0, rule.core.1, r(4)).min(reach - terraces);
            if core < MIN_FOOTPRINT {
                return None;
            }
            (None, core)
        }
    };

    let ground = rule.plinth.then(|| {
        let step = rule.step.max(0.05) as f64;
        let lift = lerp(rule.lift.0, rule.lift.1, r(8)) as f64;
        let natural = world.shaped(x as f64, z as f64);
        SiteGround {
            center: DVec2::new(x as f64, z as f64),
            top: ((natural + lift) / step).round() * step,
            core: core as f64,
            radius: (core + terraces) as f64,
            rings: rule.rings.max(1),
            step,
            earthwork: rule.earthwork.filter(|_| rule.order <= 0.0).map(|e| Earthwork {
                square: e.shape == EarthworkShape::Square,
                yaw: (r(7) * 360.0).to_radians() as f64,
                width: slope as f64,
                ramps: e.ramps.max(0.0) as f64,
            }),
            order: rule.order.max(0.0) as f64,
        }
    });
    Some(Site {
        layer,
        cell,
        at: (x, z),
        seed,
        centrepiece,
        ground,
        plates: rule.plates.clone().filter(|_| rule.plinth),
        terraces: rule.terraces.clone().filter(|_| rule.plinth),
        terrace_chance: rule.terrace_chance,
        stairs: rule.stairs,
        form,
    })
}

/// Every site whose cell lies within `radius` of (x, z), wrapping. Cells
/// repeat around the torus, so a small world can list a site twice.
pub fn near(library: &Library, world: &PlateWorld, x: f32, z: f32, radius: f32) -> Vec<Site> {
    near_in(Layer::Sites, library, world, x, z, radius)
}

/// The same for a layer.
pub fn near_in(layer: Layer, library: &Library, world: &PlateWorld, x: f32, z: f32, radius: f32) -> Vec<Site> {
    let (_, size) = cells(world.size(), layer.grid(library).spacing);
    let (cx, cz) = ((x / size).floor() as i32, (z / size).floor() as i32);
    let reach = (radius / size).ceil() as i32 + 1;
    let mut out = Vec::new();
    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let (gx, gz) = (cx + dx, cz + dz);
            let center = ((gx as f32 + 0.5) * size, (gz as f32 + 0.5) * size);
            if (center.0 - x).hypot(center.1 - z) > radius + size {
                continue;
            }
            if let Some(site) = plan_in(layer, library, world, (gx, gz)) {
                out.push(site);
            }
        }
    }
    out
}

/// Every site in the world.
pub fn all(library: &Library, world: &PlateWorld) -> Vec<Site> {
    all_in(Layer::Sites, library, world)
}

/// Every site of a layer.
pub fn all_in(layer: Layer, library: &Library, world: &PlateWorld) -> Vec<Site> {
    let (n, _) = cells(world.size(), layer.grid(library).spacing);
    (0..n * n).filter_map(|i| plan_in(layer, library, world, (i % n, i / n))).collect()
}

/// The ground of every site, by cell, for the plate generator to look up.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteTable {
    n: i32,
    cell: f64,
    size: f64,
    grounds: Vec<Option<SiteGround>>,
    /// The colossi's grid and grounds.
    big_n: i32,
    big_cell: f64,
    big: Vec<Option<SiteGround>>,
    /// The ground under the hand-placed structures.
    placed: Vec<SiteGround>,
    /// Whether any site is ordered.
    ordered: bool,
}

/// Terraces around a hand-placed structure's core.
const PLACED_RINGS: u32 = 2;
const PLACED_RING_WIDTH: f32 = 22.0;

/// How far a hand-placed structure's ground reaches from its centre (its
/// earthwork's ramps included).
fn placed_ground_radius(placement: &Placement) -> f32 {
    placement.size.0.hypot(placement.size.2) * 0.5
        + CORE_MARGIN
        + PLACED_RINGS as f32 * PLACED_RING_WIDTH * (1.0 + PLACED_RAMPS)
}

/// Hand-placed structures stand on a low square mound with short ramps.
const PLACED_LIFT: f64 = 5.0;
const PLACED_RAMPS: f32 = 0.5;

impl SiteTable {
    pub fn new(library: &Library, world: &PlateWorld) -> Self {
        let (n, cell) = cells(world.size(), library.site_grid.spacing);
        let grounds: Vec<Option<SiteGround>> = (0..n * n).map(|i| plan(library, world, (i % n, i / n)).and_then(|s| s.ground)).collect();
        let (big_n, big_cell) = cells(world.size(), library.colossus_grid.spacing);
        let big = (0..big_n * big_n)
            .map(|i| plan_in(Layer::Colossi, library, world, (i % big_n, i / big_n)).and_then(|s| s.ground))
            .collect();
        // Hand-placed structures stand on a flat core level with the ground
        // at their centre, like a site's.
        let placed = library
            .structures
            .iter()
            .map(|p| {
                let (x, z) = p.at;
                let step = default_step() as f64;
                let core = (p.size.0.hypot(p.size.2) * 0.5 + CORE_MARGIN) as f64;
                SiteGround {
                    center: DVec2::new(x as f64, z as f64),
                    top: ((world.shaped(x as f64, z as f64) + PLACED_LIFT) / step).round() * step,
                    core,
                    radius: placed_ground_radius(p) as f64,
                    rings: PLACED_RINGS,
                    step,
                    earthwork: Some(Earthwork {
                        square: true,
                        yaw: p.yaw.to_radians() as f64,
                        width: (PLACED_RINGS as f32 * PLACED_RING_WIDTH) as f64,
                        ramps: PLACED_RAMPS as f64,
                    }),
                    order: 0.0,
                }
            })
            .collect();
        let ordered = grounds.iter().flatten().any(|g: &SiteGround| g.order > 0.0);
        Self {
            ordered,
            n,
            cell: cell as f64,
            size: world.size() as f64,
            grounds,
            big_n,
            big_cell: big_cell as f64,
            big,
            placed,
        }
    }

    /// How ordered the plates are at `p`: the most any ordered site there
    /// makes them (see `SiteGround::orderliness`).
    pub fn order_at(&self, p: DVec2) -> f64 {
        self.order_ground(p).map_or(0.0, |(_, order)| order)
    }

    /// The ordered site whose field reaches `p` most strongly, and how
    /// strongly.
    pub fn order_ground(&self, p: DVec2) -> Option<(&SiteGround, f64)> {
        if !self.ordered {
            return None;
        }
        let (cx, cz) = ((p.x / self.cell).floor() as i32, (p.y / self.cell).floor() as i32);
        let wrap = |d: f64| d - (d / self.size).round() * self.size;
        let mut best: Option<(&SiteGround, f64)> = None;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (gx, gz) = ((cx + dx).rem_euclid(self.n), (cz + dz).rem_euclid(self.n));
                let Some(ground) = &self.grounds[(gz * self.n + gx) as usize] else { continue };
                let order = ground.orderliness(DVec2::new(wrap(p.x - ground.center.x), wrap(p.y - ground.center.y)));
                if order > 0.0 && best.is_none_or(|(_, o)| order > o) {
                    best = Some((ground, order));
                }
            }
        }
        best
    }

    /// The site whose ground (grown by `margin`) contains `p`, at any
    /// wrapped copy, and the distance from its centre.
    pub fn at(&self, p: DVec2, margin: f64) -> Option<(&SiteGround, f64)> {
        if self.n == 0 {
            return None;
        }
        let (cx, cz) = ((p.x / self.cell).floor() as i32, (p.y / self.cell).floor() as i32);
        let wrap = |d: f64| d - (d / self.size).round() * self.size;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (gx, gz) = ((cx + dx).rem_euclid(self.n), (cz + dz).rem_euclid(self.n));
                let Some(ground) = &self.grounds[(gz * self.n + gx) as usize] else { continue };
                let distance = ground.measure(DVec2::new(wrap(p.x - ground.center.x), wrap(p.y - ground.center.y)));
                if distance < ground.radius + margin {
                    return Some((ground, distance));
                }
            }
        }
        let (bx, bz) = ((p.x / self.big_cell).floor() as i32, (p.y / self.big_cell).floor() as i32);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (gx, gz) = ((bx + dx).rem_euclid(self.big_n), (bz + dz).rem_euclid(self.big_n));
                let Some(ground) = &self.big[(gz * self.big_n + gx) as usize] else { continue };
                let distance = wrap(ground.center.x - p.x).hypot(wrap(ground.center.y - p.y));
                if distance < ground.radius + margin {
                    return Some((ground, distance));
                }
            }
        }
        self.placed.iter().find_map(|ground| {
            let distance = wrap(ground.center.x - p.x).hypot(wrap(ground.center.y - p.y));
            (distance < ground.radius + margin).then_some((ground, distance))
        })
    }
}

/// A built site: the height its core stands at, and what grows there
/// relative to (x, base, z).
pub struct Built {
    pub base: f32,
    pub solids: Vec<Solid>,
    pub prisms: Vec<Prism>,
    pub flights: Vec<Flight>,
}

/// The pieces at least `min_width` across: what is worth drawing from afar
/// (steps and paving drop out; outlines stay).
pub fn wide_prisms(prisms: &[Prism], min_width: f32) -> Vec<Prism> {
    prisms
        .iter()
        .filter(|p| {
            let (lo, hi) = p.points.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
            (hi - lo).length() >= min_width
        })
        .cloned()
        .collect()
}

/// A flight of stairs (local coordinates, heights relative to the base).
#[derive(Clone, Copy, Debug)]
pub struct Flight {
    /// On the floor, a little before the lowest step.
    pub foot: Vec2,
    /// Horizontal unit direction up the flight.
    pub up: Vec2,
    pub floor: f32,
    pub rise: f32,
}

/// Pieces of buildings per site at most.
const MAX_PRISMS: usize = 60_000;
/// Everything standing on a plate reaches this far into it, so it never
/// floats where distant terrain is drawn a little low.
const FOUNDATION: f32 = 8.0;
/// Plates are inset this much before anything grows on them, so nothing
/// shares a wall with the plate (its foundation stays hidden inside).
const PLATE_MARGIN: f32 = 0.3;
/// Stairs: the highest step, the depth of each, the widest flight.
const STAIR_RISE: f32 = 0.45;
const STAIR_TREAD: f32 = 0.55;
const STAIR_WIDTH: f32 = 5.0;

/// Builds what grows on a site, which belongs in the background.
pub fn build(library: &Library, world: &PlateWorld, site: &Site, max_leaves: usize) -> Built {
    let (x, z) = site.at;
    let base = match &site.ground {
        Some(ground) => ground.top as f32,
        None => world.height_at(x, z),
    };
    let mut solids = match &site.centrepiece {
        Some(placement) => {
            let mut solids = structure::build(library, placement, max_leaves);
            // Its foundation, hidden in the core.
            let (sx, _, sz) = placement.size;
            solids.push(Solid {
                detail: false,
                wedge: false,
                round: false,
                center: Vec3::Y * -FOUNDATION * 0.5,
                rotation: Quat::from_rotation_y(placement.yaw.to_radians()),
                half: Vec3::new(sx * 0.49, FOUNDATION * 0.5, sz * 0.49),
                albedo: 0.11,
            });
            solids
        }
        None => Vec::new(),
    };
    let mut grower = Grower {
        library,
        budget: MAX_PRISMS,
        leaves: max_leaves.saturating_sub(solids.len()),
        out: Growth::default(),
    };
    let mut flights = Vec::new();
    if let Some(ground) = &site.ground {
        let r = |k: i32| hash01(site.cell.0, k, site.cell.1, site.seed ^ 0x9a7e);
        // One tone for the site, as if built of one material.
        let tone = 0.1 + 0.06 * r(0);
        let center = ground.center;
        let mut keep_clear = site.centrepiece.as_ref().map_or(0.0, |p| p.size.0.hypot(p.size.2) * 0.5 + 6.0);
        // A form on the whole footprint: the big plate here, scaled to fit.
        if let Some((form, radius)) = &site.form {
            let plate = world.big_plate(center.x, center.y);
            if plate.len() >= 3 {
                let local: Vec<Vec2> = plate.iter().map(|p| Vec2::new(p.x as f32, p.y as f32)).collect();
                let c = forms::centroid(&local);
                let reach = local.iter().map(|p| (*p - c).length()).fold(0.0, f32::max).max(1.0);
                let footprint: Vec<Vec2> = local.iter().map(|p| (*p - c) * (radius / reach)).collect();
                let start = grower.out.prisms.len();
                grower.grow(form, &footprint, 0.0, tone, site.seed ^ 0xf0f1, 0);
                for prism in &mut grower.out.prisms[start..] {
                    if prism.y0.abs() < 1e-3 {
                        prism.y0 -= FOUNDATION;
                    }
                }
                keep_clear = keep_clear.max(radius + 6.0);
            }
        }
        for plate in world.site_plates(ground) {
            let local: Vec<Vec2> =
                plate.points.iter().map(|p| Vec2::new((p.x - center.x) as f32, (p.y - center.y) as f32)).collect();
            let k = |i: i32| hash01(plate.key.0, i, plate.key.1, site.seed);
            let floor = (plate.height - base as f64) as f32;
            if plate.sloped {
                continue;
            }
            // On an ordered site, every plate on the four axes gets stairs.
            let c = forms::centroid(&local);
            let on_axis = ground.order > 0.0 && c.x.abs().min(c.y.abs()) < 12.0 && plate.distance > ground.core;
            if (on_axis || k(1) < site.stairs)
                && let Some((steps, flight)) = stairs(world, &local, center, plate.height as f32, floor, tone)
            {
                grower.out.prisms.extend(steps);
                flights.push(flight);
                continue;
            }
            let form = if plate.distance <= ground.core {
                if keep_clear > 0.0 && forms::centroid(&local).length() < keep_clear {
                    continue;
                }
                site.plates.as_ref()
            } else if k(0) < site.terrace_chance {
                site.terraces.as_ref()
            } else {
                None
            };
            if let Some(form) = form {
                let seed = (plate.key.0 as u32).wrapping_mul(0x2c1b_3c6d)
                    ^ (plate.key.1 as u32).wrapping_mul(0x297a_2d39)
                    ^ site.seed;
                let start = grower.out.prisms.len();
                grower.grow(form, &forms::inset(&local, PLATE_MARGIN), floor, tone, seed, 0);
                for prism in &mut grower.out.prisms[start..] {
                    if (prism.y0 - floor).abs() < 1e-3 {
                        prism.y0 -= FOUNDATION;
                    }
                }
            }
        }
    }
    solids.extend(grower.out.solids);
    Built { base, solids, prisms: grower.out.prisms, flights }
}

/// A flight of stairs on a plate (local coordinates, floor relative to the
/// site's base) up to its highest neighbour, if one is within reach.
fn stairs(
    world: &PlateWorld,
    plate: &[Vec2],
    center: DVec2,
    height: f32,
    floor: f32,
    tone: f32,
) -> Option<(Vec<Prism>, Flight)> {
    let inward = forms::inward_normals(plate);
    let n = plate.len();
    // The edge with the highest neighbour beyond it.
    let mut best: Option<(f32, usize)> = None;
    for i in 0..n {
        let (a, b) = (plate[i], plate[(i + 1) % n]);
        if (b - a).length() < 3.0 {
            continue;
        }
        let probe = (a + b) * 0.5 - inward[i] * 1.5;
        let beyond = world.height_at((center.x + probe.x as f64) as f32, (center.y + probe.y as f64) as f32);
        let rise = beyond - height;
        if (0.7..=8.0).contains(&rise) && best.is_none_or(|(r, _)| rise > r) {
            best = Some((rise, i));
        }
    }
    let (rise, i) = best?;
    let (a, b) = (plate[i], plate[(i + 1) % n]);
    let along = (b - a).normalize();
    let mid = (a + b) * 0.5;
    let width = ((b - a).length() * 0.7).min(STAIR_WIDTH) * 0.5;
    let steps = (rise / STAIR_RISE).ceil() as usize;
    let mut flight = Vec::new();
    for k in 1..=steps {
        // The lowest step reaches furthest into the plate; each rests on
        // the one below and on the plate.
        let depth = (steps - k + 1) as f32 * STAIR_TREAD;
        let rect = [
            mid - along * width,
            mid + along * width,
            mid + along * width + inward[i] * depth,
            mid - along * width + inward[i] * depth,
        ];
        let rect = forms::clip_to(&rect, plate);
        if rect.len() >= 3 {
            flight.push(Prism {
                points: rect,
                y0: floor - FOUNDATION,
                y1: floor + rise * k as f32 / steps as f32,
                top_scale: 1.0,
                lean: Vec2::ZERO,
                albedo: (tone - 0.015 * (k % 2) as f32).max(0.03),
            });
        }
    }
    let run = steps as f32 * STAIR_TREAD;
    let foot = Flight { foot: mid + inward[i] * (run + 1.5), up: -inward[i], floor, rise };
    Some((flight, foot))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::parse(
            r#"(
                styles: { "block": (divisions: (2, 2, 2), keep: All, depth: 1, leaves: [(on: Any, module: "box")]) },
                forms: { "tower": Inset(by: (1, 1), then: "block"), "block": Extrude(height: (5, 30)) },
                site_grid: (spacing: 768, chance: 0.6),
                sites: [
                    (district: Floor, style: "block", size: ((80, 20, 80), (200, 60, 200))),
                    (district: Tiers, style: "block", size: ((80, 20, 80), (200, 60, 200))),
                    (district: Stacks, style: "block", size: ((80, 20, 80), (200, 60, 200))),
                    (district: Broken, plates: "tower", terraces: "tower"),
                ],
            )"#,
        )
        .unwrap()
    }

    #[test]
    fn sites_are_spread_and_do_not_overlap() {
        let world = PlateWorld::new(16384.0, 7);
        let library = library();
        let sites = all(&library, &world);
        assert!(sites.len() > 100, "{} sites", sites.len());
        for (i, a) in sites.iter().enumerate() {
            for b in &sites[i + 1..] {
                let (ax, az) = a.at;
                let (bx, bz) = b.at;
                let wrap = |d: f32| d - (d / 16384.0).round() * 16384.0;
                let distance = wrap(ax - bx).hypot(wrap(az - bz));
                let reach = |s: &Site| s.ground.map_or(0.0, |g| g.radius as f32);
                assert!(distance > reach(a) + reach(b), "{:?} and {:?} overlap", a.cell, b.cell);
            }
        }
        // Wrapping: the cell past the edge is the first one.
        let first = plan(&library, &world, (0, 0)).map(|s| s.at);
        let n = (16384.0f32 / 768.0).round() as i32;
        assert_eq!(first, plan(&library, &world, (n, 0)).map(|s| s.at));
    }

    /// The highest upward face of a column's mesh above (x, z).
    fn mesh_height(mesh: &crate::ColumnMesh, origin: (f32, f32), x: f32, z: f32) -> Option<f32> {
        let (px, pz) = (x - origin.0, z - origin.1);
        let mut best: Option<f32> = None;
        for t in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
            if mesh.normals[t[0] as usize][1] < 0.3 {
                continue;
            }
            let d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
            if d.abs() < 1e-9 {
                continue;
            }
            let l0 = ((b[2] - c[2]) * (px - c[0]) + (c[0] - b[0]) * (pz - c[2])) / d;
            let l1 = ((c[2] - a[2]) * (px - c[0]) + (a[0] - c[0]) * (pz - c[2])) / d;
            let l2 = 1.0 - l0 - l1;
            if l0 >= -1e-4 && l1 >= -1e-4 && l2 >= -1e-4 {
                let y = l0 * a[1] + l1 * b[1] + l2 * c[1];
                best = Some(best.map_or(y, |h: f32| h.max(y)));
            }
        }
        best
    }

    #[test]
    fn mesh_matches_heights_around_sites() {
        let library = library();
        let world = PlateWorld::new(16384.0, 7).with_sites(&library);
        let size = crate::column_size(0);
        for site in all(&library, &world).iter().take(4) {
            let ground = site.ground.unwrap();
            let (x, z) = site.at;
            let mut columns = std::collections::HashMap::new();
            let mut bad = 0;
            for i in 0..400 {
                let a = i as f32 * 2.399;
                let r = ground.radius as f32 * (0.3 + 1.0 * (i as f32 / 400.0));
                let (px, pz) = (x + r * a.cos(), z + r * a.sin());
                let key = ((px / size).floor() as i32, (pz / size).floor() as i32);
                let mesh = columns.entry(key).or_insert_with(|| world.mesh_column(0, key.0, key.1));
                let origin = (key.0 as f32 * size, key.1 as f32 * size);
                let Some(m) = mesh_height(mesh, origin, px, pz) else { continue };
                let h = world.height_at(px, pz);
                if (m - h).abs() > 0.05 {
                    bad += 1;
                    eprintln!("{:?} at ({px:.1}, {pz:.1}), {:.0} m out: mesh {m:.2}, height_at {h:.2}", site.cell, r);
                }
            }
            assert_eq!(bad, 0);
        }
    }

    #[test]
    fn structures_stand_on_a_flat_core() {
        let library = library();
        let world = PlateWorld::new(16384.0, 7).with_sites(&library);
        let mut checked = 0;
        for site in all(&library, &world).iter().filter(|s| s.centrepiece.is_some()).take(20) {
            let ground = site.ground.unwrap();
            let placement = site.centrepiece.as_ref().unwrap();
            let (x, z) = site.at;
            let (sx, _, sz) = placement.size;
            // Under the footprint (corners included), the ground is the core.
            let yaw = placement.yaw.to_radians();
            for (u, v) in [(0.0, 0.0), (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0), (0.0, 1.0)] {
                let (lx, lz) = (u * sx * 0.5, v * sz * 0.5);
                let (wx, wz) = (x + lx * yaw.cos() + lz * yaw.sin(), z - lx * yaw.sin() + lz * yaw.cos());
                let h = world.height_at(wx, wz) as f64;
                assert!(
                    (h - ground.top).abs() < 1e-3,
                    "{:?}: {h} under the footprint, core at {}",
                    site.cell,
                    ground.top
                );
            }
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn earthworks_slope_without_holes() {
        let library = Library::parse(
            r#"(
                styles: {},
                forms: { "block": Extrude(height: (5, 10)) },
                site_grid: (spacing: 768, chance: 1),
                sites: [
                    (district: Floor, plates: "block", lift: (20, 20), rings: 3, ring_width: 25,
                        earthwork: (shape: Square, ramps: 1)),
                    (district: Tiers, plates: "block", lift: (20, 20), rings: 3, ring_width: 25,
                        earthwork: (shape: Round)),
                    (district: Stacks, plates: "block", lift: (-15, -15), rings: 3, ring_width: 25,
                        earthwork: (shape: Square)),
                    (district: Broken, plates: "block", lift: (20, 20), rings: 3, ring_width: 25,
                        earthwork: (shape: Round, ramps: 0.5)),
                ],
            )"#,
        )
        .unwrap();
        let world = PlateWorld::new(16384.0, 7).with_sites(&library);
        let size = crate::column_size(0);
        for site in all(&library, &world).iter().take(4) {
            let ground = site.ground.unwrap();
            let (x, z) = site.at;
            let mut columns = std::collections::HashMap::new();
            let (mut close, mut total) = (0, 0);
            for i in 0..600 {
                let a = i as f32 * 2.399;
                let r = ground.radius as f32 * (i as f32 / 600.0);
                let (px, pz) = (x + r * a.cos(), z + r * a.sin());
                // A column holds the plates whose big (128 m) site lies in it;
                // they reach several columns away, so look around.
                let (kx, kz) = ((px / size).floor() as i32, (pz / size).floor() as i32);
                let mut m: Option<f32> = None;
                for dz in -5..=5 {
                    for dx in -5..=5 {
                        let key = (kx + dx, kz + dz);
                        let mesh = columns.entry(key).or_insert_with(|| world.mesh_column(0, key.0, key.1));
                        let origin = (key.0 as f32 * size, key.1 as f32 * size);
                        if let Some(h) = mesh_height(mesh, origin, px, pz) {
                            m = Some(m.map_or(h, |o: f32| o.max(h)));
                        }
                    }
                }
                assert!(m.is_some(), "{:?}: no ground at ({px:.1}, {pz:.1})", site.cell);
                total += 1;
                // Facets interpolate the slope linearly between their
                // corners, where the slope bends (the core's edge, corners,
                // ramps): close, not exact.
                let error = (m.unwrap() - world.height_at(px, pz)).abs();
                // Towards the edge the slope blends into the natural
                // landform, which is not linear at all (ridges): looser there.
                if r < ground.radius as f32 * 0.8 {
                    assert!(error < 3.0, "{:?}: mesh {error} m off at ({px:.1}, {pz:.1})", site.cell);
                }
                if error < 1.2 {
                    close += 1;
                }
            }
            assert!(close as f32 > total as f32 * 0.95, "{:?}: only {close} of {total} close", site.cell);
        }
    }

    #[test]
    fn plates_grow_buildings_on_their_own_ground() {
        let library = library();
        let world = PlateWorld::new(16384.0, 7).with_sites(&library);
        let site = all(&library, &world).into_iter().find(|s| s.centrepiece.is_none()).unwrap();
        let built = build(&library, &world, &site, 1000);
        assert!(built.prisms.len() > 10, "{} prisms", built.prisms.len());
        // Each building stands on the plate under it.
        let (x, z) = site.at;
        for prism in &built.prisms {
            let c = forms::centroid(&prism.points);
            let ground = world.height_at(x + c.x, z + c.y);
            let y0 = prism.y0 + built.base;
            assert!(y0 <= ground + 1e-2, "prism at {c} floats: {y0} vs {ground}");
        }
    }
}
