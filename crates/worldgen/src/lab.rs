//! The structure lab: candidate patterns, judged on their own.
//!
//! The data file's `lab` list names candidates, each a form grown on a
//! plate-like footprint or a box style in a box. In the game, `--opt lab`
//! shows them in a row on flat, empty ground (see `PlateWorld::lab`), away
//! from districts and sites, so each pattern is judged for itself rather
//! than for whether it rescues a place. Those that work join the catalogue
//! the world draws on.

use glam::{Vec2, Vec3};
use serde::Deserialize;

use crate::forms::{Growth, Grower, Prism};
use crate::noise::hash01;
use crate::structure::{self, Library, Placement, Solid};

fn default_size() -> (f32, f32, f32) {
    (80.0, 120.0, 80.0)
}
fn default_footprint() -> f32 {
    60.0
}
fn default_variants() -> u32 {
    2
}

/// A candidate.
#[derive(Clone, Debug, Deserialize)]
pub struct LabEntry {
    pub name: String,
    /// What it is meant to try (shown on review pages).
    #[serde(default)]
    pub note: String,
    /// A box style, in a box of `size` (x, y, z) metres...
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default = "default_size")]
    pub size: (f32, f32, f32),
    /// ...or a form, grown on a plate-like footprint `footprint` metres
    /// from its centre to its corners.
    #[serde(default)]
    pub form: Option<String>,
    #[serde(default = "default_footprint")]
    pub footprint: f32,
    /// How many samples (different seeds) stand side by side: a pattern is
    /// judged on more than one roll of its dice.
    #[serde(default = "default_variants")]
    pub variants: u32,
    /// Pipework to dress it with (a name from `dressings`).
    #[serde(default)]
    pub dress: Option<String>,
    /// Which sample this is (set by `layout`).
    #[serde(skip)]
    pub variant: u32,
}

impl LabEntry {
    /// How far it reaches from its centre on the ground.
    pub fn radius(&self) -> f32 {
        match self.form {
            Some(_) => self.footprint,
            None => self.size.0.hypot(self.size.2) * 0.5,
        }
    }

    fn seed(&self) -> u32 {
        let name = self.name.bytes().fold(0x9e37_79b9u32, |h, b| h.rotate_left(5) ^ b as u32);
        (name ^ self.variant.wrapping_mul(0x27d4_eb2f)).wrapping_mul(0x85eb_ca6b)
    }

    /// Its name, with the sample's number after the first.
    pub fn label(&self) -> String {
        if self.variant == 0 { self.name.clone() } else { format!("{}_{}", self.name, self.variant + 1) }
    }
}

/// Where the lab starts, and the space between candidates (metres).
pub const LAB_START: (f32, f32) = (1400.0, 600.0);
const LAB_GAP: f32 = 140.0;

/// The candidates in a row along +x (their samples side by side), with the
/// centre of each.
pub fn layout(library: &Library) -> Vec<(LabEntry, (f32, f32))> {
    let mut x = LAB_START.0;
    let mut out = Vec::new();
    for entry in &library.lab {
        for variant in 0..entry.variants.max(1) {
            let sample = LabEntry { variant, ..entry.clone() };
            let r = sample.radius();
            x += r;
            out.push((sample, (x, LAB_START.1)));
            x += r + LAB_GAP;
        }
    }
    out
}

/// A plate-like footprint: an irregular convex heptagon `radius` from its
/// centre to its corners.
pub fn footprint(radius: f32, seed: u32) -> Vec<Vec2> {
    const N: i32 = 7;
    (0..N)
        .map(|i| {
            let r = |k: i32| hash01(i, k, seed as i32, 0x1ab);
            let a = (i as f32 + (r(0) - 0.5) * 0.5) / N as f32 * std::f32::consts::TAU;
            Vec2::new(a.cos(), a.sin()) * radius * (0.85 + 0.15 * r(1))
        })
        .collect()
}

/// Everything a candidate is made of, its base centre at the origin, dressed
/// with its pipework if it has any.
pub fn build(library: &Library, entry: &LabEntry, max_leaves: usize) -> (Vec<Solid>, Vec<Prism>) {
    let (mut solids, prisms) = build_bare(library, entry, max_leaves);
    if let Some(dressing) = entry.dress.as_ref().and_then(|d| library.dressings.get(d)) {
        let tone = 0.13;
        let pipes = crate::dressing::dress(dressing, &solids, &prisms, tone, entry.seed() ^ 0xd1e5);
        solids.extend(pipes);
    }
    (solids, prisms)
}

fn build_bare(library: &Library, entry: &LabEntry, max_leaves: usize) -> (Vec<Solid>, Vec<Prism>) {
    let seed = entry.seed();
    if let Some(style) = &entry.style {
        let placement = Placement { style: style.clone(), at: (0.0, 0.0), size: entry.size, yaw: 0.0, seed, sink: 0.0 };
        return (structure::build(library, &placement, max_leaves), Vec::new());
    }
    let Some(form) = &entry.form else { return (Vec::new(), Vec::new()) };
    let mut grower = Grower { library, budget: 60_000, leaves: max_leaves, out: Growth::default() };
    grower.grow(form, &footprint(entry.footprint, seed), 0.0, 0.13, seed, 0);
    let Growth { solids, mut prisms } = grower.out;
    // Into the ground, as on a site.
    for prism in &mut prisms {
        if prism.y0.abs() < 1e-3 {
            prism.y0 -= 4.0;
        }
    }
    (solids, prisms)
}

/// The height a candidate reaches.
pub fn height(solids: &[Solid], prisms: &[Prism]) -> f32 {
    let solid_top = solids
        .iter()
        .map(|s| {
            let mut top = f32::MIN;
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        top = top.max((s.center + s.rotation * (Vec3::new(x, y, z) * s.half)).y);
                    }
                }
            }
            top
        })
        .fold(0.0, f32::max);
    prisms.iter().map(|p| p.y1).fold(solid_top, f32::max)
}
