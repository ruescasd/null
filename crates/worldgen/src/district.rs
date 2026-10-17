//! Districts: the planet was once a single built surface, and each region
//! keeps the character of whatever it was for. A district is a set of rules
//! for the plate generator (how big plates are, which heights exist, pillars,
//! tone) and decides which megastructures appear there.
//!
//! The district map is a coarse jittered Voronoi diagram over the torus.
//! Discrete rules come from the nearest district; continuous ones (relief,
//! tone) are blended across borders so districts meet without accidental
//! cliffs.

use glam::DVec2;

use crate::noise::hash01;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Deserialize)]
pub enum District {
    /// Vast pale flat plates, like plazas: steps of a few centimetres.
    #[default]
    Floor,
    /// Terraced slopes of 2 m ledges: everything is a mantle away.
    Tiers,
    /// Dark, dense piers and pillars on small plates.
    Stacks,
    /// The original broken plate terrain, a bit of everything.
    Broken,
    /// An experiment (`--opt slopes`, everywhere): hills of a couple of
    /// hundred metres made of tilted plates, each to one of a few set
    /// gradients and eight headings, keeping their steps: landscape-scale
    /// earthworks, built, not natural (the user: verticality you can move
    /// through without big plate differences; the identity kept by the
    /// steps). Not on the district map.
    Slopes,
}

impl District {
    pub const ALL: [District; 5] = [District::Floor, District::Tiers, District::Stacks, District::Broken, District::Slopes];

    pub fn name(self) -> &'static str {
        match self {
            District::Floor => "floor",
            District::Tiers => "tiers",
            District::Stacks => "stacks",
            District::Broken => "broken",
            District::Slopes => "slopes",
        }
    }

    pub fn rules(self) -> Rules {
        match self {
            District::Floor => Rules {
                relief: 0.04,
                albedo: 0.24,
                albedo_spread: 0.03,
                split: [0.03, 0.02],
                split_mask_gain: [0.0, 0.0],
                split_rise: [1.0, 0.5],
                quantum0: 0.25,
                quantum: 0.25,
                nudge: 0.0,
                pillar_chance: 0.0,
                pillar_height: (0.0, 0.0),
                mesa_chance: 0.002,
                pit_chance: 0.003,
                marking_chance: 0.03,
                tilt: false,
                crest: false,
            },
            District::Tiers => Rules {
                relief: 1.3,
                albedo: 0.17,
                albedo_spread: 0.04,
                split: [0.5, 0.4],
                split_mask_gain: [0.0, 0.0],
                split_rise: [7.0, 4.0],
                quantum0: 2.0,
                quantum: 2.0,
                nudge: 0.0,
                pillar_chance: 0.0,
                pillar_height: (0.0, 0.0),
                mesa_chance: 0.004,
                pit_chance: 0.004,
                marking_chance: 0.01,
                tilt: false,
                crest: false,
            },
            District::Stacks => Rules {
                relief: 0.6,
                albedo: 0.09,
                albedo_spread: 0.03,
                split: [0.9, 0.6],
                split_mask_gain: [0.0, 0.0],
                split_rise: [4.0, 2.0],
                quantum0: 1.0,
                quantum: 0.5,
                nudge: 1.0,
                pillar_chance: 0.18,
                pillar_height: (5.0, 28.0),
                mesa_chance: 0.004,
                pit_chance: 0.01,
                marking_chance: 0.01,
                tilt: false,
                crest: false,
            },
            District::Broken => Rules {
                relief: 1.0,
                albedo: 0.155,
                albedo_spread: 0.05,
                split: [0.15, 0.1],
                split_mask_gain: [0.4, 0.5],
                split_rise: [7.0, 4.0],
                quantum0: 2.0,
                quantum: 0.75,
                nudge: 1.0,
                pillar_chance: 0.006,
                pillar_height: (3.0, 12.0),
                mesa_chance: 0.005,
                pit_chance: 0.007,
                marking_chance: 0.018,
                tilt: false,
                crest: false,
            },
            District::Slopes => *SLOPES.get_or_init(|| slopes(3)),
        }
    }
}

/// Generator rules for a district.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    /// Multiplies the landform's relief (blended across borders).
    pub relief: f64,
    /// Base plate albedo (blended) and its random spread.
    pub albedo: f64,
    pub albedo_spread: f64,
    /// Chance that a level-0 / level-1 plate splits regardless of slope,
    /// plus how much the regional split noise adds to it.
    pub split: [f64; 2],
    pub split_mask_gain: [f64; 2],
    /// Rise across a plate (metres) that forces a split, per level.
    pub split_rise: [f64; 2],
    /// Height step for the biggest plates, and for smaller plates following
    /// the landform. These set which ledge heights exist in the district.
    pub quantum0: f64,
    pub quantum: f64,
    /// Random nudge on smaller plates, in steps (0 = none).
    pub nudge: f64,
    /// Chance and height range of pillars among the finest plates.
    pub pillar_chance: f64,
    pub pillar_height: (f64, f64),
    /// Chance of raised mesas and sunken pits among the biggest plates.
    pub mesa_chance: f64,
    pub pit_chance: f64,
    /// Chance of a dark or light marker plate.
    pub marking_chance: f64,
    /// Plates tilted to follow the landform (to set gradients and
    /// headings), not flat.
    pub tilt: bool,
    /// Tilted plates level on crests: hills ending in flat tops.
    pub crest: bool,
}

/// The slopes experiment's rules, chosen once (`set_slopes`).
static SLOPES: std::sync::OnceLock<Rules> = std::sync::OnceLock::new();

/// Chooses the slopes experiment's version, and its relief if given: the
/// hills' total height, not their steps (`--set slopes_version`,
/// `slopes_relief`). Before any plate is made; later calls change nothing.
pub fn set_slopes(version: u32, relief: Option<f64>) {
    let mut rules = slopes(version);
    if let Some(r) = relief {
        rules.relief = r;
    }
    let _ = SLOPES.set(rules);
}

/// The slopes experiment's versions. 1: big plates, steps of 1 m: the
/// user's better first (neighbours near flush, read as tilted ground,
/// strata, a lunar hill). 3: big plates, big steps, level crests. (2, small
/// plates with big steps, read as the broken district, the distance as
/// blocks of houses: gone.)
fn slopes(version: u32) -> Rules {
    let (relief, split, mask, quantum, crest) = match version {
        1 => (2.0, [0.35, 0.15], [0.3, 0.3], 1.0, false),
        _ => (3.0, [0.1, 0.1], [0.0, 0.0], 2.5, true),
    };
    Rules {
        relief,
        albedo: 0.17,
        albedo_spread: 0.04,
        split,
        split_mask_gain: mask,
        // (Tilted, a plate follows the slope: it splits by chance only,
        // never because the land rises across it.)
        split_rise: [1000.0, 1000.0],
        quantum0: quantum,
        quantum,
        nudge: 0.0,
        pillar_chance: 0.0,
        pillar_height: (0.0, 0.0),
        mesa_chance: 0.0,
        pit_chance: 0.0,
        marking_chance: 0.01,
        tilt: true,
        crest,
    }
}

/// Size of the district map's cells, in metres (districts span ~1.5-3 km).
const CELL: f64 = 2048.0;
/// Width over which continuous rules blend across a border, in metres.
const BLEND: f64 = 250.0;

pub struct DistrictMap {
    size: f64,
    seed: u32,
    /// One district everywhere, if set (`--opt slopes`).
    pub force: Option<District>,
}

impl DistrictMap {
    pub fn new(size: f64, seed: u32) -> Self {
        Self { size, seed: seed ^ 0xd157_71c7, force: None }
    }

    fn cells(&self) -> i32 {
        (self.size / CELL).round().max(1.0) as i32
    }

    fn rand(&self, g: (i32, i32), k: i32) -> f64 {
        let n = self.cells();
        hash01(g.0.rem_euclid(n), k, g.1.rem_euclid(n), self.seed) as f64
    }

    fn site(&self, g: (i32, i32)) -> DVec2 {
        let j = |k| (self.rand(g, k) - 0.5) * 0.6;
        DVec2::new((g.0 as f64 + 0.5 + j(1)) * CELL, (g.1 as f64 + 0.5 + j(2)) * CELL)
    }

    fn kind(&self, g: (i32, i32)) -> District {
        // Weighted pick: floor 28%, tiers 24%, stacks 20%, broken 28%.
        let r = self.rand(g, 3);
        match r {
            r if r < 0.28 => District::Floor,
            r if r < 0.52 => District::Tiers,
            r if r < 0.72 => District::Stacks,
            _ => District::Broken,
        }
    }

    /// The 3x3 nearest cells with their distances.
    fn nearby(&self, p: DVec2) -> [((i32, i32), f64); 9] {
        let (cx, cz) = ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32);
        let mut out = [((0, 0), 0.0); 9];
        for (i, (dx, dz)) in (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz))).enumerate() {
            let g = (cx + dx, cz + dz);
            out[i] = (g, self.site(g).distance(p));
        }
        out
    }

    /// The district at a point.
    pub fn at(&self, x: f64, z: f64) -> District {
        if let Some(d) = self.force {
            return d;
        }
        let near = self.nearby(DVec2::new(x, z));
        let (g, _) = near.iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        self.kind(*g)
    }

    /// Relief and base albedo, blended smoothly across district borders.
    pub fn blended(&self, x: f64, z: f64) -> (f64, f64) {
        if let Some(d) = self.force {
            let r = d.rules();
            return (r.relief, r.albedo);
        }
        let near = self.nearby(DVec2::new(x, z));
        let closest = near.iter().map(|n| n.1).fold(f64::MAX, f64::min);
        let (mut relief, mut albedo, mut total) = (0.0, 0.0, 0.0);
        for (g, d) in near {
            let w = (-(d - closest) / BLEND * 3.0).exp();
            let rules = self.kind(g).rules();
            relief += rules.relief * w;
            albedo += rules.albedo * w;
            total += w;
        }
        (relief / total, albedo / total)
    }
}
