//! Structures from a library of modules and styles, read from a data file
//! (`data/structures.ron`) that is meant to be edited by hand.
//!
//! - A **module** fills a cell (an oriented box): a box, a slab, a column, a
//!   fin, a frame, a ramp, stairs, nothing, or a **group** of parts, each a
//!   sub-box of the cell filled by another module.
//! - A **style** is a fractal rule (see `ifs.rs`) plus **leaf rules** that
//!   pick a module for each leaf from where it sat in its parent's grid.
//! - A module can also be a whole **structure**: a style run inside the cell.
//!   That nests scales: a region of plots, each plot a building, each
//!   building of modules.
//! - Everything ends up as **solids**, boxes and wedges, which are meshed
//!   with hard edges and become box and wedge colliders.

use std::collections::BTreeMap;

use glam::{Quat, Vec2, Vec3};
use serde::Deserialize;

use crate::ifs::{self, Block, Context, Keep, Leaf, Rule};
use crate::mesh::ColumnMesh;
use crate::noise::hash01;
use crate::forms::{self, Form};
use crate::lab::LabEntry;
use crate::sites::{SiteGrid, SiteRule};

/// The whole data file.
#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub modules: BTreeMap<String, Module>,
    pub styles: BTreeMap<String, Style>,
    #[serde(default)]
    pub structures: Vec<Placement>,
    /// Forms that grow buildings from plates (see `forms.rs`).
    #[serde(default)]
    pub forms: BTreeMap<String, Form>,
    /// The grid sites grow on, and what grows where (see `sites.rs`).
    #[serde(default)]
    pub site_grid: SiteGrid,
    #[serde(default)]
    pub sites: Vec<SiteRule>,
    /// The coarser grid of colossi, and what grows where (the same rules).
    #[serde(default = "SiteGrid::colossi")]
    pub colossus_grid: SiteGrid,
    #[serde(default)]
    pub colossi: Vec<SiteRule>,
    /// Candidate patterns for the lab (see `lab.rs`).
    #[serde(default)]
    pub lab: Vec<LabEntry>,
}

impl Library {
    pub fn parse(text: &str) -> Result<Self, String> {
        // Optional names are written plainly, without Some(...).
        let options = ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
        let library: Library = options.from_str(text).map_err(|e| e.to_string())?;
        library.check()?;
        Ok(library)
    }

    /// Reports names that do not resolve, so mistakes show up when loading.
    fn check(&self) -> Result<(), String> {
        let known = |name: &str| BUILT_IN.contains(&name) || self.modules.contains_key(name);
        for (name, module) in &self.modules {
            match module {
                Module::Group(parts) => {
                    for part in parts {
                        if !known(&part.module) {
                            return Err(format!("module '{name}': unknown module '{}'", part.module));
                        }
                    }
                }
                _ => {}
            }
        }
        for (name, style) in &self.styles {
            for leaf in &style.leaves {
                if !known(&leaf.module) {
                    return Err(format!("style '{name}': unknown module '{}'", leaf.module));
                }
            }
        }
        for placement in &self.structures {
            if !self.styles.contains_key(&placement.style) {
                return Err(format!("structure: unknown style '{}'", placement.style));
            }
        }
        forms::check(&self.forms, &self.styles)?;
        for entry in &self.lab {
            if let Some(style) = &entry.style
                && !self.styles.contains_key(style)
            {
                return Err(format!("lab '{}': unknown style '{style}'", entry.name));
            }
            if let Some(form) = &entry.form
                && !self.forms.contains_key(form)
            {
                return Err(format!("lab '{}': unknown form '{form}'", entry.name));
            }
        }
        for site in self.sites.iter().chain(&self.colossi) {
            if let Some(style) = &site.style
                && !self.styles.contains_key(style)
            {
                return Err(format!("site ({:?}): unknown style '{style}'", site.district));
            }
            for form in site.plates.iter().chain(&site.terraces).chain(&site.form) {
                if form != "nothing" && !self.forms.contains_key(form) {
                    return Err(format!("site ({:?}): unknown form '{form}'", site.district));
                }
            }
        }
        Ok(())
    }

    fn module(&self, name: &str) -> Module {
        if let Some(m) = self.modules.get(name) {
            return m.clone();
        }
        match name {
            "void" => Module::Void,
            "ramp" => Module::Ramp,
            _ => Module::Box,
        }
    }
}

/// Modules that exist without being declared.
const BUILT_IN: [&str; 3] = ["box", "void", "ramp"];

/// What fills a cell. Sizes are fractions of the cell (whose own extent is
/// -1..1 on each axis).
#[derive(Clone, Debug, Deserialize)]
pub enum Module {
    /// The whole cell.
    Box,
    /// Nothing.
    Void,
    /// A horizontal slab `thickness` (fraction of the cell height) thick.
    Slab {
        thickness: f32,
        #[serde(default)]
        at: Height,
    },
    /// A full-height square column `width` (fraction) wide.
    Column { width: f32 },
    /// A full-height plate across the cell, `thickness` (fraction) thick,
    /// facing along `axis`.
    Fin {
        thickness: f32,
        #[serde(default)]
        axis: Axis,
    },
    /// The cell's twelve edges as bars `bar` (fraction) thick: a hollow frame.
    Frame { bar: f32 },
    /// A wedge filling the cell, rising towards +x. Only where it is walkable
    /// (no steeper than 45 degrees); in a steeper cell it becomes stairs.
    Ramp,
    /// Steps rising towards +x, each at most `rise` metres high so they can
    /// be walked up; their number follows from the cell's height.
    Stairs {
        #[serde(default = "default_rise")]
        rise: f32,
    },
    /// Parts placed inside the cell, each filled by another module.
    Group(Vec<Part>),
}




fn default_rise() -> f32 {
    0.45
}


#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub enum Height {
    #[default]
    Bottom,
    Middle,
    Top,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub enum Axis {
    #[default]
    X,
    Z,
}

/// A part of a group: a sub-box of the cell, from `min` to `max` (each -1..1),
/// filled by `module`, turned by `turn` quarter turns about the vertical.
#[derive(Clone, Debug, Deserialize)]
pub struct Part {
    pub min: (f32, f32, f32),
    pub max: (f32, f32, f32),
    pub module: String,
    #[serde(default)]
    pub turn: u32,
}

/// A family of structures: the fractal rule, tone, and what fills the leaves.
#[derive(Clone, Debug, Deserialize)]
pub struct Style {
    pub divisions: (u32, u32, u32),
    pub keep: Keep,
    pub depth: u32,
    #[serde(default = "one")]
    pub gap: f32,
    /// Degrees each child is turned relative to its parent.
    #[serde(default)]
    pub twist: f32,
    #[serde(default)]
    pub stop_chance: f32,
    #[serde(default)]
    pub lift: f32,
    /// Blocks whose smallest side is below this (metres) are not split.
    #[serde(default = "default_min_size")]
    pub min_size: f32,
    #[serde(default = "default_albedo")]
    pub albedo: f32,
    #[serde(default = "default_spread")]
    pub albedo_spread: f32,
    /// How leaves are filled; with none, or none matching, a leaf is a box.
    #[serde(default)]
    pub leaves: Vec<LeafRule>,
}

fn one() -> f32 {
    1.0
}
fn default_min_size() -> f32 {
    1.6
}
fn default_albedo() -> f32 {
    0.13
}
fn default_spread() -> f32 {
    0.04
}

/// "Leaves at `on` may be filled with `module`, with this weight."
#[derive(Clone, Debug, Deserialize)]
pub struct LeafRule {
    pub on: Where,
    pub module: String,
    #[serde(default = "one")]
    pub weight: f32,
    #[serde(default)]
    pub turn: Turn,
}

/// Where a leaf sat in its parent's grid.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Where {
    Any,
    Top,
    /// Nothing kept above it in the parent's grid: the top of a column,
    /// wherever it stops (massifs, skylines).
    Summit,
    Bottom,
    /// On a vertical edge of the parent (boundary in both x and z).
    Corner,
    /// On a face of the parent (boundary in exactly one of x and z).
    Edge,
    /// Not on the parent's boundary at all.
    Interior,
    /// On any of the parent's boundaries.
    Outside,
    /// Stopped subdividing early (a bigger, plainer block).
    Early,
}

impl Where {
    fn matches(self, c: &Context) -> bool {
        match self {
            Where::Any => true,
            Where::Top => c.top(),
            Where::Summit => c.open_above,
            Where::Bottom => c.bottom(),
            Where::Corner => c.corner(),
            Where::Edge => c.edge(),
            Where::Interior => c.interior(),
            Where::Outside => !c.interior(),
            Where::Early => c.early,
        }
    }
}

/// How a chosen module is turned about the vertical.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub enum Turn {
    #[default]
    None,
    /// A random quarter turn.
    Random,
    /// The quarter turn that points the module's +x away from the parent's
    /// centre (ramps and stairs rising outward).
    Outward,
    /// The quarter turn that points +x towards the parent's centre.
    Inward,
}

/// Where to build a structure (for now, test placements in the data file).
#[derive(Clone, Debug, Deserialize)]
pub struct Placement {
    pub style: String,
    /// World x and z of the structure's centre.
    pub at: (f32, f32),
    /// Overall size in metres (x, y, z).
    pub size: (f32, f32, f32),
    /// Degrees about the vertical.
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub seed: u32,
    /// How far the base sits below the ground, in metres.
    #[serde(default)]
    pub sink: f32,
}

/// A piece of geometry: an oriented box, a wedge (a box whose top slopes
/// from its -x bottom edge up to its +x top edge), or a round tube (along
/// its x, `half.y` and `half.z` its radii).
#[derive(Clone, Copy, Debug)]
pub struct Solid {
    /// A fine piece (a step, a moulding, an arch's strip): it may be left
    /// out of the versions shown from afar, if it is small too.
    pub detail: bool,
    /// How brightly it glows (0: not at all; about 1: as bright as the
    /// light filaments).
    pub glow: f32,
    pub wedge: bool,
    pub round: bool,
    pub center: Vec3,
    pub rotation: Quat,
    pub half: Vec3,
    pub albedo: f32,
}

impl Style {
    pub fn rule(&self) -> Rule {
        Rule {
            divisions: [self.divisions.0.max(1), self.divisions.1.max(1), self.divisions.2.max(1)],
            keep: self.keep,
            depth: self.depth,
            gap: self.gap,
            twist: Quat::from_rotation_y(self.twist.to_radians()),
            stop_chance: self.stop_chance,
            lift: self.lift,
            min_size: self.min_size,
        }
    }
}

/// Builds the solids of a structure whose base centre is at the origin, with
/// at most `max_leaves` leaves in total (nested structures included).
pub fn build(library: &Library, placement: &Placement, max_leaves: usize) -> Vec<Solid> {
    let (sx, sy, sz) = placement.size;
    let root = Block {
        center: Vec3::Y * (sy * 0.5 - placement.sink),
        rotation: Quat::from_rotation_y(placement.yaw.to_radians()),
        half: Vec3::new(sx, sy, sz) * 0.5,
        level: 0,
    };
    let seed = placement.seed.wrapping_mul(0x9e37_79b9) ^ 0x5eed;
    let mut builder = Builder { library, budget: max_leaves, out: Vec::new() };
    builder.style(&placement.style, root, seed);
    let mut solids = builder.out;
    settle(&mut solids);
    // Seated on the ground: the lowest piece's bottom at the base (less the
    // sink), whatever the rules left out at the bottom.
    let lowest = solids.iter().map(Solid::bottom).fold(f32::MAX, f32::min);
    if lowest.is_finite() {
        let drop = lowest + placement.sink;
        for s in &mut solids {
            s.center.y -= drop;
            // What stands on the ground reaches into it, so it meets lower
            // plates under its footprint instead of hovering over them.
            if !s.wedge && !s.round && s.bottom() < -placement.sink + 0.01 {
                s.center.y -= FOUNDATION * 0.5;
                s.half.y += FOUNDATION * 0.5;
            }
        }
    }
    solids
}

/// How far pieces on the ground reach into it, metres.
const FOUNDATION: f32 = 8.0;

/// Nothing hangs in the air. Pieces connected to the base through others
/// (touching, sideways included: roofs, bridges) stay as they are; a piece
/// cut off from it reaches down to the highest thing below it, or to the
/// base. The rules cannot see what grew in the cell below, so this is
/// settled afterwards.
fn settle(solids: &mut [Solid]) {
    const CELL: f32 = 8.0;
    const TOUCH: f32 = 0.1;
    let base = solids.iter().map(Solid::bottom).fold(f32::MAX, f32::min);
    if !base.is_finite() {
        return;
    }
    let bounds = |s: &Solid| {
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let p = s.center + s.rotation * (Vec3::new(x, y, z) * s.half);
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
        (lo, hi)
    };
    let cells = |lo: Vec3, hi: Vec3| {
        let (x0, z0) = ((lo.x / CELL).floor() as i32, (lo.z / CELL).floor() as i32);
        let (x1, z1) = ((hi.x / CELL).floor() as i32, (hi.z / CELL).floor() as i32);
        (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| (x, z)))
    };
    let touching = |a: &(Vec3, Vec3), b: &(Vec3, Vec3)| {
        (0..3).all(|k| a.0[k] <= b.1[k] + TOUCH && b.0[k] <= a.1[k] + TOUCH)
    };
    let covers = |s: &Solid, p: Vec2| {
        let local = s.rotation.inverse() * (Vec3::new(p.x, s.center.y, p.y) - s.center);
        local.x.abs() <= s.half.x && local.z.abs() <= s.half.z
    };
    let mut boxes: Vec<(Vec3, Vec3)> = solids.iter().map(bounds).collect();
    let mut grid: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (i, &(lo, hi)) in boxes.iter().enumerate() {
        for key in cells(lo, hi) {
            grid.entry(key).or_default().push(i);
        }
    }
    let mut supported: Vec<bool> = boxes.iter().map(|b| b.0.y <= base + TOUCH).collect();
    let mut stack: Vec<usize> = (0..solids.len()).filter(|&i| supported[i]).collect();
    let mut loose: Vec<usize> = (0..solids.len()).filter(|&i| !supported[i]).collect();
    loose.sort_by(|&a, &b| boxes[a].0.y.total_cmp(&boxes[b].0.y));
    let mut next = 0;
    loop {
        // Everything connected to the supported pieces.
        while let Some(i) = stack.pop() {
            for key in cells(boxes[i].0, boxes[i].1) {
                for &j in grid.get(&key).into_iter().flatten() {
                    if !supported[j] && touching(&boxes[i], &boxes[j]) {
                        supported[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        // The lowest piece still loose reaches down to what is below, which
        // joins it (and whatever touches it) to the supported.
        while next < loose.len() && (supported[loose[next]] || solids[loose[next]].wedge) {
            next += 1;
        }
        let Some(&i) = loose.get(next) else { break };
        next += 1;
        let s = solids[i];
        let bottom = boxes[i].0.y;
        let mut highest = base;
        for (u, v) in [(0.0, 0.0), (0.8, 0.8), (-0.8, 0.8), (0.8, -0.8), (-0.8, -0.8)] {
            let p = s.center + s.rotation * Vec3::new(u * s.half.x, 0.0, v * s.half.z);
            let p = Vec2::new(p.x, p.z);
            let key = ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32);
            for &j in grid.get(&key).into_iter().flatten() {
                if j != i && supported[j] && covers(&solids[j], p) && boxes[j].1.y <= bottom + TOUCH {
                    highest = highest.max(boxes[j].1.y);
                }
            }
        }
        let drop = bottom - highest;
        let s = &mut solids[i];
        s.center.y -= drop * 0.5;
        s.half.y += drop * 0.5;
        boxes[i] = bounds(s);
        for key in cells(boxes[i].0, boxes[i].1) {
            grid.entry(key).or_default().push(i);
        }
        supported[i] = true;
        stack.push(i);
    }
}


struct Builder<'a> {
    library: &'a Library,
    /// Leaves still allowed, shared by nested structures.
    budget: usize,
    out: Vec<Solid>,
}

impl Builder<'_> {
    /// Runs a style's rule inside `root` and fills its leaves.
    fn style(&mut self, name: &str, root: Block, seed: u32) {
        let Some(style) = self.library.styles.get(name) else { return };
        let leaves = ifs::generate(&style.rule(), Block { level: 0, ..root }, seed, self.budget);
        self.budget = self.budget.saturating_sub(leaves.len());
        for (n, leaf) in leaves.iter().enumerate() {
            let r = |k: i32| hash01(n as i32, k, leaf.block.level as i32, seed);
            let albedo = style.albedo + style.albedo_spread * (r(0) - 0.5) * 2.0;
            let Some((module, turn)) = choose(style, leaf, r(1), r(2)) else {
                self.out.push(Solid { glow: 0.0,
                    detail: false,
                    wedge: false,
                    round: false,
                    center: leaf.block.center,
                    rotation: leaf.block.rotation,
                    half: leaf.block.half,
                    albedo,
                });
                continue;
            };
            let turns = match turn {
                Turn::None => 0,
                Turn::Random => (r(3) * 4.0) as u32 % 4,
                Turn::Outward | Turn::Inward => {
                    let (ox, oz) = leaf.context.outward();
                    let towards = if matches!(turn, Turn::Inward) { -1.0 } else { 1.0 };
                    facing_turns(ox * towards, oz * towards)
                }
            };
            let cell = turned(leaf.block, turns);
            let child_seed = seed.wrapping_mul(0x85eb_ca6b) ^ (n as u32).wrapping_mul(0xc2b2_ae35);
            let module = self.library.module(&module);
            self.expand(&module, cell, albedo, 0, child_seed);
        }
    }

    fn push(&mut self, b: Block, wedge: bool, albedo: f32) {
        self.out.push(Solid { glow: 0.0, detail: false, wedge, round: false, center: b.center, rotation: b.rotation, half: b.half, albedo });
    }


    fn expand(&mut self, module: &Module, cell: Block, albedo: f32, depth: u32, seed: u32) {
        let sub = |min: [f32; 3], max: [f32; 3]| sub_block(cell, Vec3::from(min), Vec3::from(max));
        match module {
            Module::Void => {}
            Module::Box => self.push(cell, false, albedo),
            Module::Ramp => {
                // Steeper than 45 degrees cannot be walked: use stairs instead.
                if cell.half.y > cell.half.x {
                    self.expand(&Module::Stairs { rise: default_rise() }, cell, albedo, depth, seed);
                } else {
                    self.push(cell, true, albedo);
                }
            }
            Module::Slab { thickness, at } => {
                let t = thickness.clamp(0.01, 1.0) * 2.0;
                let (lo, hi) = match at {
                    Height::Bottom => (-1.0, -1.0 + t),
                    Height::Middle => (-t * 0.5, t * 0.5),
                    Height::Top => (1.0 - t, 1.0),
                };
                self.push(sub([-1.0, lo, -1.0], [1.0, hi, 1.0]), false, albedo);
            }
            Module::Column { width } => {
                let w = width.clamp(0.01, 1.0);
                self.push(sub([-w, -1.0, -w], [w, 1.0, w]), false, albedo);
            }
            Module::Fin { thickness, axis } => {
                let t = thickness.clamp(0.01, 1.0);
                let (min, max) = match axis {
                    Axis::X => ([-t, -1.0, -1.0], [t, 1.0, 1.0]),
                    Axis::Z => ([-1.0, -1.0, -t], [1.0, 1.0, t]),
                };
                self.push(sub(min, max), false, albedo);
            }
            Module::Frame { bar } => {
                let b = bar.clamp(0.01, 0.5) * 2.0;
                for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    // Vertical posts at the four corners.
                    let (x0, x1) = if sx < 0.0 { (-1.0, -1.0 + b) } else { (1.0 - b, 1.0) };
                    let (z0, z1) = if sz < 0.0 { (-1.0, -1.0 + b) } else { (1.0 - b, 1.0) };
                    self.push(sub([x0, -1.0, z0], [x1, 1.0, z1]), false, albedo);
                }
                for y in [-1.0, 1.0] {
                    let (y0, y1) = if y < 0.0 { (-1.0, -1.0 + b) } else { (1.0 - b, 1.0) };
                    // Bars along x on the front and back, along z on the sides.
                    self.push(sub([-1.0, y0, -1.0], [1.0, y1, -1.0 + b]), false, albedo);
                    self.push(sub([-1.0, y0, 1.0 - b], [1.0, y1, 1.0]), false, albedo);
                    self.push(sub([-1.0, y0, -1.0], [-1.0 + b, y1, 1.0]), false, albedo);
                    self.push(sub([1.0 - b, y0, -1.0], [1.0, y1, 1.0]), false, albedo);
                }
            }
            Module::Stairs { rise } => {
                // Enough steps that none is higher than `rise`.
                let height = cell.half.y * 2.0;
                let n = (height / rise.max(0.05)).ceil().clamp(1.0, 256.0) as u32;
                let step = 2.0 / n as f32;
                for s in 0..n {
                    let x0 = -1.0 + step * s as f32;
                    let top = -1.0 + step * (s + 1) as f32;
                    self.push(sub([x0, -1.0, -1.0], [x0 + step, top, 1.0]), false, albedo);
                }
            }
            Module::Group(parts) => {
                if depth > 8 {
                    return;
                }
                for part in parts {
                    let b = sub_block(cell, Vec3::from(part.min), Vec3::from(part.max));
                    let b = turned(b, part.turn);
                    let module = self.library.module(&part.module);
                    self.expand(&module, b, albedo, depth + 1, seed.wrapping_add(depth + 1));
                }
            }
        }
    }
}

/// Picks a module for a leaf among the rules that match it, by weight.
fn choose(style: &Style, leaf: &Leaf, pick: f32, _spare: f32) -> Option<(String, Turn)> {
    let matching: Vec<&LeafRule> = style.leaves.iter().filter(|r| r.on.matches(&leaf.context)).collect();
    let total: f32 = matching.iter().map(|r| r.weight.max(0.0)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = pick * total;
    for rule in &matching {
        x -= rule.weight.max(0.0);
        if x <= 0.0 {
            return Some((rule.module.clone(), rule.turn));
        }
    }
    matching.last().map(|r| (r.module.clone(), r.turn))
}

/// The number of quarter turns about +y that best points +x along (dx, dz).
fn facing_turns(dx: f32, dz: f32) -> u32 {
    if dx.abs() < 1e-3 && dz.abs() < 1e-3 {
        return 0;
    }
    // Turning by +90 degrees about y maps +x to -z.
    let candidates = [(1.0, 0.0), (0.0, -1.0), (-1.0, 0.0), (0.0, 1.0)];
    (0..4)
        .max_by(|&a, &b| {
            let score = |t: usize| candidates[t].0 * dx + candidates[t].1 * dz;
            score(a).total_cmp(&score(b))
        })
        .unwrap() as u32
}

/// A cell turned by quarter turns: the same box, with its own frame rotated
/// (and so its x and z half extents swapped for odd turns).
fn turned(block: Block, turns: u32) -> Block {
    if turns % 4 == 0 {
        return block;
    }
    let half = if turns % 2 == 1 { Vec3::new(block.half.z, block.half.y, block.half.x) } else { block.half };
    Block {
        rotation: block.rotation * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 * turns as f32),
        half,
        ..block
    }
}

/// A sub-box of `cell`, given in the cell's own -1..1 coordinates.
fn sub_block(cell: Block, min: Vec3, max: Vec3) -> Block {
    let (lo, hi) = (min.min(max), min.max(max));
    let center = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    Block {
        center: cell.center + cell.rotation * (center * cell.half),
        rotation: cell.rotation,
        half: half * cell.half,
        level: cell.level,
    }
}

/// Flat-shaded mesh of the solids: hard edges everywhere. Undersides and the
/// lower parts of side faces are darker. Of boxes, only what no other box
/// covers is drawn.
pub fn mesh(solids: &[Solid]) -> ColumnMesh {
    mesh_tiles(solids, f32::INFINITY).pop().map(|(_, m)| m).unwrap_or_default()
}

/// The solids that still show from afar: all but the fine pieces whose
/// second largest extent is under `min` metres (arch strips, steps,
/// parapets, mouldings drop out; walls, decks, blocks and colossal piers
/// stay).
pub fn coarse(solids: &[Solid], min: f32) -> Vec<Solid> {
    solids
        .iter()
        .filter(|s| {
            if !s.detail {
                return true;
            }
            let mut e = [s.half.x, s.half.y, s.half.z];
            e.sort_by(f32::total_cmp);
            2.0 * e[1] >= min
        })
        .copied()
        .collect()
}

/// [`mesh`], in square tiles `tile` metres across by the solids' centres:
/// each tile's (x, z) index and mesh. Faces hidden by boxes in other tiles
/// are left out too.
pub fn mesh_tiles(solids: &[Solid], tile: f32) -> Vec<((i32, i32), ColumnMesh)> {
    let key = |s: &Solid| {
        if tile.is_finite() { ((s.center.x / tile).floor() as i32, (s.center.z / tile).floor() as i32) } else { (0, 0) }
    };
    let mut tiles: std::collections::HashMap<(i32, i32), ColumnMesh> = std::collections::HashMap::new();
    let boxes: Vec<_> = solids.iter().map(|s| (!s.wedge && !s.round).then_some((s.center, s.rotation, s.half))).collect();
    let groups = crate::cull::groups(&boxes);
    let mut culled = vec![false; solids.len()];
    for group in &groups {
        let extents: std::collections::HashMap<usize, (Vec3, Vec3)> = group.boxes.iter().map(|&(i, lo, hi)| (i, (lo, hi))).collect();
        for &(i, _, _) in &group.boxes {
            culled[i] = true;
        }
        for piece in crate::cull::visible(group) {
            let s = &solids[piece.solid];
            let mesh = tiles.entry(key(s)).or_default();
            let (lo, hi) = extents[&piece.solid];
            let (a, b) = crate::cull::others(piece.axis);
            let plane = if piece.positive { hi[piece.axis] } else { lo[piece.axis] };
            let r = piece.rect;
            let mut corners: Vec<Vec3> = [(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])]
                .iter()
                .map(|&(u, v)| {
                    let mut p = Vec3::ZERO;
                    p[piece.axis] = plane;
                    p[a] = u;
                    p[b] = v;
                    p
                })
                .collect();
            // The corners' order gives +x, -y, +z; turned for the other side.
            if piece.positive != (piece.axis != 1) {
                corners.reverse();
            }
            let mut normal = Vec3::ZERO;
            normal[piece.axis] = if piece.positive { 1.0 } else { -1.0 };
            let normal = group.frame * normal;
            let base = mesh.positions.len() as u32;
            for c in &corners {
                let p = group.frame * *c;
                mesh.positions.push(p.to_array());
                mesh.normals.push(normal.to_array());
                mesh.albedo.push(s.albedo);
                let height = ((p.y - s.center.y) / s.half.y.max(1e-6)).clamp(-1.0, 1.0);
                let ao = if normal.y < -0.5 { 0.45 } else { 0.75 + 0.25 * (height * 0.5 + 0.5) };
                mesh.ao.push(ao);
            }
            mesh.face_size(4, (hi[a] - lo[a]).min(hi[b] - lo[b]));
            if s.glow > 0.0 {
                mesh.glow_of(4, s.glow);
            }
            mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    for (s, _) in solids.iter().zip(&culled).filter(|(_, c)| !**c) {
        let mesh = tiles.entry(key(s)).or_default();
        let world = |c: [f32; 3]| s.center + s.rotation * (Vec3::from(c) * s.half);
        let ao_at = |c: [f32; 3], down: bool| if down { 0.45 } else { 0.75 + 0.25 * (c[1] * 0.5 + 0.5) };
        let mut face = |corners: &[[f32; 3]]| {
            let p: Vec<Vec3> = corners.iter().map(|&c| world(c)).collect();
            let normal = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
            let base = mesh.positions.len() as u32;
            for (i, c) in corners.iter().enumerate() {
                mesh.positions.push(p[i].to_array());
                mesh.normals.push(normal.to_array());
                mesh.albedo.push(s.albedo);
                mesh.ao.push(ao_at(*c, normal.y < -0.5));
            }
            mesh.face_size(corners.len(), crate::mesh::polygon_width(&p));
            if s.glow > 0.0 {
                mesh.glow_of(corners.len(), s.glow);
            }
            for k in 1..corners.len() as u32 - 1 {
                mesh.indices.extend_from_slice(&[base, base + k, base + k + 1]);
            }
        };
        if s.round {
            round(mesh, s);
        } else if s.wedge {
            // Bottom, back (+x), the slope, and the two triangular sides.
            face(&[[-1., -1., 1.], [-1., -1., -1.], [1., -1., -1.], [1., -1., 1.]]);
            face(&[[1., -1., -1.], [1., 1., -1.], [1., 1., 1.], [1., -1., 1.]]);
            face(&[[-1., -1., -1.], [-1., -1., 1.], [1., 1., 1.], [1., 1., -1.]]);
            face(&[[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.]]);
            face(&[[1., -1., -1.], [-1., -1., -1.], [1., 1., -1.]]);
        } else {
            face(&[[1., -1., -1.], [1., 1., -1.], [1., 1., 1.], [1., -1., 1.]]);
            face(&[[-1., -1., 1.], [-1., 1., 1.], [-1., 1., -1.], [-1., -1., -1.]]);
            face(&[[-1., 1., -1.], [-1., 1., 1.], [1., 1., 1.], [1., 1., -1.]]);
            face(&[[-1., -1., 1.], [-1., -1., -1.], [1., -1., -1.], [1., -1., 1.]]);
            face(&[[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]]);
            face(&[[1., -1., -1.], [-1., -1., -1.], [-1., 1., -1.], [1., 1., -1.]]);
        }
    }
    let mut tiles: Vec<_> = tiles.into_iter().collect();
    tiles.sort_by_key(|(k, _)| *k);
    tiles
}

/// Sides of a round solid.
const ROUND_SIDES: usize = 10;

/// A round solid: a smooth-shaded tube along its x, capped.
fn round(mesh: &mut ColumnMesh, s: &Solid) {
    let ring = |x: f32| -> Vec<(Vec3, Vec3)> {
        (0..ROUND_SIDES)
            .map(|k| {
                let a = k as f32 / ROUND_SIDES as f32 * std::f32::consts::TAU;
                let local = Vec3::new(x * s.half.x, a.cos() * s.half.y, a.sin() * s.half.z);
                let normal = s.rotation * Vec3::new(0.0, a.cos(), a.sin());
                (s.center + s.rotation * local, normal)
            })
            .collect()
    };
    let (a, b) = (ring(-1.0), ring(1.0));
    // Narrow, as far as the panelling goes: round surfaces carry no bands.
    let width = (2.0 * s.half.y.min(s.half.z)).min(1.0);
    let base = mesh.positions.len() as u32;
    for (p, n) in a.iter().chain(&b) {
        mesh.positions.push(p.to_array());
        mesh.normals.push(n.to_array());
        mesh.albedo.push(s.albedo);
        mesh.ao.push(0.75 + 0.25 * n.y.max(0.0));
    }
    mesh.face_size(2 * ROUND_SIDES, width);
    if s.glow > 0.0 {
        mesh.glow_of(2 * ROUND_SIDES, s.glow);
    }
    let n = ROUND_SIDES as u32;
    for k in 0..n {
        let j = (k + 1) % n;
        mesh.indices.extend_from_slice(&[base + k, base + n + j, base + n + k, base + k, base + j, base + n + j]);
    }
    // Caps.
    for (ring, sign) in [(&a, -1.0f32), (&b, 1.0)] {
        let normal = s.rotation * Vec3::X * sign;
        let start = mesh.positions.len() as u32;
        for (p, _) in ring.iter() {
            mesh.positions.push(p.to_array());
            mesh.normals.push(normal.to_array());
            mesh.albedo.push(s.albedo);
            mesh.ao.push(0.7);
        }
        mesh.face_size(ROUND_SIDES, width);
        for k in 1..n - 1 {
            if sign > 0.0 {
                mesh.indices.extend_from_slice(&[start, start + k, start + k + 1]);
            } else {
                mesh.indices.extend_from_slice(&[start, start + k + 1, start + k]);
            }
        }
    }
}

/// The points of a round solid's outline in its own frame (for a convex
/// collider).
pub fn round_points(half: Vec3) -> Vec<Vec3> {
    (0..8)
        .flat_map(|k| {
            let a = k as f32 / 8.0 * std::f32::consts::TAU;
            [-1.0, 1.0].map(|x| Vec3::new(x * half.x, a.cos() * half.y, a.sin() * half.z))
        })
        .collect()
}

impl Solid {
    /// The height of its lowest corner.
    pub fn bottom(&self) -> f32 {
        let mut low = f32::MAX;
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    low = low.min((self.center + self.rotation * (Vec3::new(x, y, z) * self.half)).y);
                }
            }
        }
        low
    }
}

/// The corners of a wedge in its own frame (for a convex collider).
pub fn wedge_points(half: Vec3) -> Vec<Vec3> {
    [[-1., -1., -1.], [-1., -1., 1.], [1., -1., -1.], [1., -1., 1.], [1., 1., -1.], [1., 1., 1.]]
        .iter()
        .map(|&c| Vec3::from(c) * half)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"(
        modules: {
            "plinth": Slab(thickness: 0.2),
            "pier": Column(width: 0.3),
            "porch": Group([
                (min: (-1, -1, -1), max: (1, -0.6, 1), module: "plinth"),
                (min: (-1, -0.6, -1), max: (1, -0.4, 1), module: "ramp", turn: 2),
            ]),
        },
        styles: {
            "test": (
                divisions: (3, 2, 3),
                keep: ColumnsAndRoof,
                depth: 2,
                gap: 0.9,
                leaves: [
                    (on: Top, module: "plinth"),
                    (on: Corner, module: "pier", weight: 2),
                    (on: Edge, module: "porch", turn: Outward),
                    (on: Any, module: "box", weight: 0.2),
                ],
            ),
        },
        structures: [ (style: "test", at: (0, 0), size: (60, 30, 60)) ],
    )"#;

    #[test]
    fn sample_library_builds() {
        let lib = Library::parse(SAMPLE).expect("parses");
        let solids = build(&lib, &lib.structures[0], 10_000);
        assert!(!solids.is_empty());
        assert!(solids.iter().any(|s| s.wedge));
        let m = mesh(&solids);
        // Faces point away from their solid's centre.
        for (tri, s) in m.indices.chunks(3).zip(std::iter::repeat(())) {
            let _ = s;
            let p = |i: u32| Vec3::from(m.positions[i as usize]);
            let face = (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0]));
            assert!(face.dot(Vec3::from(m.normals[tri[0] as usize])) > 0.0);
        }
    }

    #[test]
    fn steep_ramps_become_walkable_stairs() {
        let lib = Library::parse(r#"(
            styles: { "one": (divisions: (1, 1, 1), keep: All, depth: 0, leaves: [(on: Any, module: "ramp")]) },
            structures: [ (style: "one", at: (0, 0), size: (4, 10, 4)) ],
        )"#)
        .unwrap();
        let solids = build(&lib, &lib.structures[0], 100);
        assert!(solids.iter().all(|s| !s.wedge), "too steep for a ramp");
        // 10 m in steps of at most 0.45 m.
        assert!(solids.len() >= 23);
        let mut tops: Vec<f32> = solids.iter().map(|s| s.center.y + s.half.y).collect();
        tops.sort_by(f32::total_cmp);
        for pair in tops.windows(2) {
            assert!(pair[1] - pair[0] <= 0.45 + 1e-3);
        }
    }


    #[test]
    fn unknown_names_are_reported() {
        let bad = SAMPLE.replace("module: \"pier\"", "module: \"peir\"");
        assert!(Library::parse(&bad).unwrap_err().contains("peir"));
    }

    #[test]
    fn faces_point_outward() {
        for wedge in [false, true] {
            let s = Solid { glow: 0.0, detail: false, wedge, round: false, center: Vec3::new(3.0, 1.0, -2.0), rotation: Quat::from_rotation_y(0.7), half: Vec3::new(2.0, 1.0, 3.0), albedo: 0.1 };
            let m = mesh(&[s]);
            for tri in m.indices.chunks(3) {
                let p = |i: u32| Vec3::from(m.positions[i as usize]);
                let centroid = (p(tri[0]) + p(tri[1]) + p(tri[2])) / 3.0;
                let n = Vec3::from(m.normals[tri[0] as usize]);
                // The slope passes through the box's centre, so measure from
                // the solid's own centroid.
                let inside = if wedge { Vec3::new(1.0 / 3.0, -1.0 / 3.0, 0.0) } else { Vec3::ZERO };
                let reference = s.center + s.rotation * (inside * s.half);
                assert!(n.dot(centroid - reference) > 0.0, "wedge={wedge}");
            }
        }
    }
}
