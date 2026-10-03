//! Structures from a library of modules and styles, read from a data file
//! (`data/structures.ron`) that is meant to be edited by hand.
//!
//! - A **module** fills a cell (an oriented box): a box, a slab, a column, a
//!   fin, a frame, a ramp, stairs, nothing, or a **group** of parts, each a
//!   sub-box of the cell filled by another module.
//! - A **style** is a fractal rule (see `ifs.rs`) plus **leaf rules** that
//!   pick a module for each leaf from where it sat in its parent's grid.
//! - Everything ends up as **solids**, boxes and wedges, which are meshed
//!   with hard edges and become box and wedge colliders.

use std::collections::BTreeMap;

use glam::{Quat, Vec3};
use serde::Deserialize;

use crate::ifs::{self, Block, Context, Keep, Leaf, Rule};
use crate::mesh::ColumnMesh;
use crate::noise::hash01;

/// The whole data file.
#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    #[serde(default)]
    pub modules: BTreeMap<String, Module>,
    pub styles: BTreeMap<String, Style>,
    #[serde(default)]
    pub structures: Vec<Placement>,
}

impl Library {
    pub fn parse(text: &str) -> Result<Self, String> {
        let library: Library = ron::from_str(text).map_err(|e| e.to_string())?;
        library.check()?;
        Ok(library)
    }

    /// Reports names that do not resolve, so mistakes show up when loading.
    fn check(&self) -> Result<(), String> {
        let known = |name: &str| BUILT_IN.contains(&name) || self.modules.contains_key(name);
        for (name, module) in &self.modules {
            if let Module::Group(parts) = module {
                for part in parts {
                    if !known(&part.module) {
                        return Err(format!("module '{name}': unknown module '{}'", part.module));
                    }
                }
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
    /// A wedge filling the cell, rising towards +x.
    Ramp,
    /// Steps rising towards +x.
    Stairs { steps: u32 },
    /// Parts placed inside the cell, each filled by another module.
    Group(Vec<Part>),
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

/// A piece of geometry: an oriented box, or a wedge (a box whose top slopes
/// from its -x bottom edge up to its +x top edge).
#[derive(Clone, Copy, Debug)]
pub struct Solid {
    pub wedge: bool,
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

/// Builds the solids of a structure whose base centre is at the origin.
pub fn build(library: &Library, placement: &Placement, max_leaves: usize) -> Vec<Solid> {
    let Some(style) = library.styles.get(&placement.style) else { return Vec::new() };
    let (sx, sy, sz) = placement.size;
    let root = Block {
        center: Vec3::Y * (sy * 0.5 - placement.sink),
        rotation: Quat::from_rotation_y(placement.yaw.to_radians()),
        half: Vec3::new(sx, sy, sz) * 0.5,
        level: 0,
    };
    let seed = placement.seed.wrapping_mul(0x9e37_79b9) ^ 0x5eed;
    let leaves = ifs::generate(&style.rule(), root, seed, max_leaves);
    let mut solids = Vec::new();
    for (n, leaf) in leaves.iter().enumerate() {
        let r = |k: i32| hash01(n as i32, k, leaf.block.level as i32, seed);
        let albedo = style.albedo + style.albedo_spread * (r(0) - 0.5) * 2.0;
        let Some((module, turn)) = choose(style, leaf, r(1), r(2)) else {
            solids.push(Solid { wedge: false, center: leaf.block.center, rotation: leaf.block.rotation, half: leaf.block.half, albedo });
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
        expand(library, &library.module(&module), cell, albedo, 0, &mut solids);
    }
    solids
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

fn expand(library: &Library, module: &Module, cell: Block, albedo: f32, depth: u32, out: &mut Vec<Solid>) {
    let solid = |b: Block, wedge: bool| Solid { wedge, center: b.center, rotation: b.rotation, half: b.half, albedo };
    let sub = |min: [f32; 3], max: [f32; 3]| sub_block(cell, Vec3::from(min), Vec3::from(max));
    match module {
        Module::Void => {}
        Module::Box => out.push(solid(cell, false)),
        Module::Ramp => out.push(solid(cell, true)),
        Module::Slab { thickness, at } => {
            let t = thickness.clamp(0.01, 1.0) * 2.0;
            let (lo, hi) = match at {
                Height::Bottom => (-1.0, -1.0 + t),
                Height::Middle => (-t * 0.5, t * 0.5),
                Height::Top => (1.0 - t, 1.0),
            };
            out.push(solid(sub([-1.0, lo, -1.0], [1.0, hi, 1.0]), false));
        }
        Module::Column { width } => {
            let w = width.clamp(0.01, 1.0);
            out.push(solid(sub([-w, -1.0, -w], [w, 1.0, w]), false));
        }
        Module::Fin { thickness, axis } => {
            let t = thickness.clamp(0.01, 1.0);
            let (min, max) = match axis {
                Axis::X => ([-t, -1.0, -1.0], [t, 1.0, 1.0]),
                Axis::Z => ([-1.0, -1.0, -t], [1.0, 1.0, t]),
            };
            out.push(solid(sub(min, max), false));
        }
        Module::Frame { bar } => {
            let b = bar.clamp(0.01, 0.5) * 2.0;
            for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                // Vertical posts at the four corners.
                let (x0, x1) = if sx < 0.0 { (-1.0, -1.0 + b) } else { (1.0 - b, 1.0) };
                let (z0, z1) = if sz < 0.0 { (-1.0, -1.0 + b) } else { (1.0 - b, 1.0) };
                out.push(solid(sub([x0, -1.0, z0], [x1, 1.0, z1]), false));
            }
            for y in [-1.0, 1.0] {
                let (y0, y1) = if y < 0.0 { (-1.0, -1.0 + b) } else { (1.0 - b, 1.0) };
                // Bars along x on the front and back, along z on the sides.
                out.push(solid(sub([-1.0, y0, -1.0], [1.0, y1, -1.0 + b]), false));
                out.push(solid(sub([-1.0, y0, 1.0 - b], [1.0, y1, 1.0]), false));
                out.push(solid(sub([-1.0, y0, -1.0], [-1.0 + b, y1, 1.0]), false));
                out.push(solid(sub([1.0 - b, y0, -1.0], [1.0, y1, 1.0]), false));
            }
        }
        Module::Stairs { steps } => {
            let n = (*steps).max(1) as f32;
            for s in 0..steps.max(&1).to_owned() {
                let x0 = -1.0 + 2.0 * s as f32 / n;
                let top = -1.0 + 2.0 * (s + 1) as f32 / n;
                out.push(solid(sub([x0, -1.0, -1.0], [x0 + 2.0 / n, top, 1.0]), false));
            }
        }
        Module::Group(parts) => {
            if depth > 8 {
                return;
            }
            for part in parts {
                let b = sub_block(cell, Vec3::from(part.min), Vec3::from(part.max));
                let b = turned(b, part.turn);
                expand(library, &library.module(&part.module), b, albedo, depth + 1, out);
            }
        }
    }
}

/// Flat-shaded mesh of the solids: hard edges everywhere. Undersides and the
/// lower parts of side faces are darker.
pub fn mesh(solids: &[Solid]) -> ColumnMesh {
    let mut mesh = ColumnMesh::default();
    for s in solids {
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
            for k in 1..corners.len() as u32 - 1 {
                mesh.indices.extend_from_slice(&[base, base + k, base + k + 1]);
            }
        };
        if s.wedge {
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
    mesh
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
                (min: (0, -0.6, -1), max: (1, 1, 1), module: "ramp", turn: 2),
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
    fn unknown_names_are_reported() {
        let bad = SAMPLE.replace("module: \"pier\"", "module: \"peir\"");
        assert!(Library::parse(&bad).unwrap_err().contains("peir"));
    }

    #[test]
    fn faces_point_outward() {
        for wedge in [false, true] {
            let s = Solid { wedge, center: Vec3::new(3.0, 1.0, -2.0), rotation: Quat::from_rotation_y(0.7), half: Vec3::new(2.0, 1.0, 3.0), albedo: 0.1 };
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
