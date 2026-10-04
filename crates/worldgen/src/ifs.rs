//! The fractal rule behind structures.
//!
//! A kaleidoscopic IFS is a recursive rule: split a block into a grid of
//! cells, keep some of them, maybe twist or lift each child, and repeat inside
//! every kept child. Rather than evaluating it as a distance field and meshing
//! the result (soft edges, millions of triangles, fidgety collision), the rule
//! is run as a tree, and every leaf is a cell that a module fills (see
//! `structure.rs`): hard edges by construction, and simple colliders.

use glam::{Quat, Vec3};
use serde::Deserialize;

use crate::noise::hash01;

/// An oriented box: a node of the tree.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub center: Vec3,
    pub rotation: Quat,
    /// Half extents along the block's own axes.
    pub half: Vec3,
    /// How many subdivisions deep this block is.
    pub level: u32,
}

/// Which cells of a block's grid survive into the next level.
#[derive(Clone, Copy, Debug, Deserialize)]
pub enum Keep {
    /// Every cell.
    All,
    /// Cells with at most one coordinate away from the grid's boundary: a
    /// Menger-like lattice of edges and corners.
    Lattice,
    /// The outer columns of cells (x and z on the boundary) plus the top
    /// layer: piers carrying a roof.
    ColumnsAndRoof,
    /// Each cell with this probability.
    Random(f32),
    /// A stepped skyline: each column of cells is kept up to a random height.
    Skyline,
    /// A massif: each column of cells is kept up to a height that falls from
    /// the centre to the edges by `slope` (a fraction of the block's height),
    /// broken up at random, and now and then an edge column is missing. The
    /// top two levels shape the mountain (a mountain of mountains); below
    /// them cells split as a skyline, so the surface breaks up into stepped
    /// columns standing on their floors rather than into rubble.
    Massif(f32),
}

/// The parameters of the rule.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub divisions: [u32; 3],
    pub keep: Keep,
    pub depth: u32,
    pub gap: f32,
    pub twist: Quat,
    pub stop_chance: f32,
    pub lift: f32,
    /// Blocks whose smallest side is below this (metres) are not split.
    pub min_size: f32,
}

/// Where a leaf sat in its parent's grid, for choosing what fills it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    pub index: [u32; 3],
    pub grid: [u32; 3],
    /// Stopped subdividing before reaching the full depth.
    pub early: bool,
    /// Nothing kept above it in its parent's grid.
    pub open_above: bool,
}

impl Context {
    fn on_boundary(&self, axis: usize) -> bool {
        self.grid[axis] > 0 && (self.index[axis] == 0 || self.index[axis] + 1 == self.grid[axis])
    }

    pub fn top(&self) -> bool {
        self.grid[1] > 0 && self.index[1] + 1 == self.grid[1]
    }

    pub fn bottom(&self) -> bool {
        self.grid[1] > 0 && self.index[1] == 0
    }

    /// On the boundary in both x and z: a vertical edge of the parent.
    pub fn corner(&self) -> bool {
        self.on_boundary(0) && self.on_boundary(2)
    }

    /// On the boundary in exactly one of x and z: a face of the parent.
    pub fn edge(&self) -> bool {
        self.on_boundary(0) != self.on_boundary(2)
    }

    /// On no boundary at all.
    pub fn interior(&self) -> bool {
        !(0..3).any(|a| self.on_boundary(a))
    }

    /// Direction from the parent's centre towards this cell, in the parent's
    /// x/z plane.
    pub fn outward(&self) -> (f32, f32) {
        let centered = |a: usize| self.index[a] as f32 - (self.grid[a] as f32 - 1.0) * 0.5;
        (centered(0), centered(2))
    }
}

/// A cut taken out of a structure's envelope, in the root block's own
/// coordinates (-1..1 on each axis, y up): what it contains is not built.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Cut {
    /// Everything inside this box.
    Box { min: (f32, f32, f32), max: (f32, f32, f32) },
    /// Everything beyond a plane: points p with p . normal > offset.
    Plane { normal: (f32, f32, f32), offset: f32 },
}

impl Cut {
    fn contains(&self, p: Vec3) -> bool {
        match *self {
            Cut::Box { min, max } => {
                let (lo, hi) = (Vec3::from(min).min(Vec3::from(max)), Vec3::from(min).max(Vec3::from(max)));
                p.cmpge(lo).all() && p.cmple(hi).all()
            }
            Cut::Plane { normal, offset } => p.dot(Vec3::from(normal)) > offset,
        }
    }
}

/// The root block and the cuts taken out of it.
struct Envelope<'a> {
    root: Block,
    cuts: &'a [Cut],
}

impl Envelope<'_> {
    fn inside(&self, world: Vec3) -> bool {
        let local = self.root.rotation.inverse() * (world - self.root.center) / self.root.half;
        !self.cuts.iter().any(|c| c.contains(local))
    }

    /// How much of a block lies inside: 0 none, 1 all, between partly
    /// (sampled on a 3 x 3 x 3 grid).
    fn share(&self, b: &Block) -> f32 {
        if self.cuts.is_empty() {
            return 1.0;
        }
        let mut n = 0;
        for x in [-1.0, 0.0, 1.0] {
            for y in [-1.0, 0.0, 1.0] {
                for z in [-1.0, 0.0, 1.0] {
                    let p = b.center + b.rotation * (Vec3::new(x, y, z) * b.half * 0.98);
                    n += self.inside(p) as u32;
                }
            }
        }
        n as f32 / 27.0
    }
}

/// A leaf of the tree: a cell to fill, and where it sat.
#[derive(Clone, Copy, Debug)]
pub struct Leaf {
    pub block: Block,
    pub context: Context,
}

/// Runs the rule from `root` and returns the leaves, at most `max_leaves`.
/// What `cuts` contains (see `Cut`) is carved out of the root first: cells
/// outside are dropped, cells across a cut's edge split on (even when they
/// would stop early) and at the last level are kept if their centre is in.
pub fn generate(rule: &Rule, root: Block, seed: u32, max_leaves: usize, cuts: &[Cut]) -> Vec<Leaf> {
    let mut leaves = Vec::new();
    let envelope = Envelope { root, cuts };
    split(rule, &envelope, root, Context::default(), seed, 1, max_leaves, &mut leaves);
    leaves
}

#[allow(clippy::too_many_arguments)]
fn split(
    rule: &Rule,
    envelope: &Envelope,
    block: Block,
    context: Context,
    seed: u32,
    path: u32,
    max_leaves: usize,
    out: &mut Vec<Leaf>,
) {
    let r = |k: i32| hash01(path as i32, k, block.level as i32, seed);
    let share = envelope.share(&block);
    if share <= 0.0 {
        return;
    }
    let partial = share < 1.0;
    let stops_early = block.level > 0 && r(0) < rule.stop_chance && !partial;
    if partial && block.level >= rule.depth && !envelope.inside(block.center) {
        return;
    }
    let leaf = block.level >= rule.depth
        || block.half.min_element() * 2.0 < rule.min_size
        || stops_early
        || out.len() >= max_leaves;
    if leaf {
        let early = block.level < rule.depth;
        out.push(Leaf { block, context: Context { early, ..context } });
        return;
    }
    let [nx, ny, nz] = rule.divisions;
    // For the skyline: a random kept height per column of cells.
    let column_height = |i: u32, k: u32| {
        let h = hash01(path as i32 * 31 + i as i32, k as i32, block.level as i32 + 7, seed);
        1 + (h * ny as f32) as u32
    };
    let cell = block.half * 2.0 / Vec3::new(nx as f32, ny as f32, nz as f32);
    let child_path = |i: u32, j: u32, k: u32| path.wrapping_mul(97).wrapping_add(1 + i + j * 7 + k * 49);
    // Kept at random, a cell also needs the cell below it: nothing hangs in
    // the air.
    let random_keep = |i: u32, j: u32, k: u32, p: f32| {
        (0..=j).all(|below| hash01(child_path(i, below, k) as i32, 1, block.level as i32 + 1, seed) < p)
    };
    // For the massif: how high each column of cells stands, falling with
    // the distance from the centre (0 in the middle, 1 at the middle of an
    // edge, more in the corners, which drop out: the footprint rounds off
    // at every level).
    let massif_height = |i: u32, k: u32, slope: f32| {
        let centered = |v: u32, n: u32| if n > 1 { ((v as f32 + 0.5) / n as f32 * 2.0 - 1.0).abs() } else { 0.0 };
        let (u, w) = (centered(i, nx), centered(k, nz));
        let edge = 1.0 - 1.0 / nx.max(nz) as f32;
        let r = (u * u + w * w).sqrt() / edge.max(1e-3);
        // The middle columns stand full height.
        let fall = ((r - 0.5) * 2.0).clamp(0.0, 1.0);
        let h = hash01(path as i32 * 31 + i as i32, k as i32, block.level as i32 + 11, seed);
        if r > 1.2 || (r > 0.95 && h < 0.2) {
            return 0;
        }
        let top = 1.0 - slope * fall.powf(0.8) + (h - 0.5) * 0.4;
        ((top * ny as f32).round() as u32).clamp(1, ny)
    };
    let inner = |v: u32, n: u32| v > 0 && v + 1 < n;
    let keeps = |i: u32, j: u32, k: u32| {
        let interior = inner(i, nx) as u32 + inner(j, ny) as u32 + inner(k, nz) as u32;
        match rule.keep {
            Keep::All => true,
            Keep::Lattice => interior <= 1,
            Keep::ColumnsAndRoof => (!inner(i, nx) && !inner(k, nz)) || j + 1 == ny,
            Keep::Random(p) => random_keep(i, j, k, p),
            Keep::Skyline => j < column_height(i, k),
            Keep::Massif(slope) if block.level < 2 => j < massif_height(i, k, slope),
            Keep::Massif(_) => j < column_height(i, k),
        }
    };
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let child_path = child_path(i, j, k);
                let rc = |key: i32| hash01(child_path as i32, key, block.level as i32 + 1, seed);
                if !keeps(i, j, k) {
                    continue;
                }
                // A cell with nothing under it spans a gap (a bridge, a
                // roof): it reaches across the grooves to its neighbours
                // instead of hanging between them.
                // The cell below, if the envelope carved it away, counts
                // as missing too (a lintel, an overhang).
                let carved_below = j > 0 && !envelope.cuts.is_empty() && {
                    let below = Block {
                        center: block.center
                            + block.rotation
                                * Vec3::new(
                                    (i as f32 + 0.5) * cell.x - block.half.x,
                                    (j as f32 - 0.5) * cell.y - block.half.y,
                                    (k as f32 + 0.5) * cell.z - block.half.z,
                                ),
                        rotation: block.rotation,
                        half: cell * 0.5,
                        level: block.level + 1,
                    };
                    envelope.share(&below) < 0.5
                };
                let spans = j > 0 && (!keeps(i, j - 1, k) || carved_below);
                let across = if spans { 2.0 - rule.gap } else { rule.gap };
                let local = Vec3::new(
                    (i as f32 + 0.5) * cell.x - block.half.x,
                    (j as f32 + 0.5) * cell.y - block.half.y + (rc(2) - 0.5) * rule.lift * cell.y,
                    (k as f32 + 0.5) * cell.z - block.half.z,
                );
                let child = Block {
                    center: block.center + block.rotation * local,
                    rotation: block.rotation * rule.twist,
                    // Grooves between neighbours, but floors stack flush.
                    half: cell * 0.5 * Vec3::new(across, 1.0, across),
                    level: block.level + 1,
                };
                let open_above = j + 1 == ny || !keeps(i, j + 1, k);
                let context = Context { index: [i, j, k], grid: [nx, ny, nz], early: false, open_above };
                split(rule, envelope, child, context, seed, child_path, max_leaves, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_is_a_menger_sponge() {
        let rule = Rule {
            divisions: [3, 3, 3],
            keep: Keep::Lattice,
            depth: 3,
            gap: 1.0,
            twist: Quat::IDENTITY,
            stop_chance: 0.0,
            lift: 0.0,
            min_size: 0.0,
        };
        let root = Block { center: Vec3::ZERO, rotation: Quat::IDENTITY, half: Vec3::splat(27.0), level: 0 };
        let leaves = generate(&rule, root, 1, usize::MAX, &[]);
        assert_eq!(leaves.len(), 20 * 20 * 20);
        assert!(leaves.iter().all(|l| (l.block.half - Vec3::ONE).length() < 1e-4));
    }
}
