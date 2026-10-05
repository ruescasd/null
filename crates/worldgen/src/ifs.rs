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

/// A leaf of the tree: a cell to fill, and where it sat.
#[derive(Clone, Copy, Debug)]
pub struct Leaf {
    pub block: Block,
    pub context: Context,
}

/// Runs the rule from `root` and returns the leaves, at most `max_leaves`.
pub fn generate(rule: &Rule, root: Block, seed: u32, max_leaves: usize) -> Vec<Leaf> {
    let mut leaves = Vec::new();
    split(rule, root, Context::default(), seed, 1, max_leaves, &mut leaves);
    leaves
}

fn split(
    rule: &Rule,
    block: Block,
    context: Context,
    seed: u32,
    path: u32,
    max_leaves: usize,
    out: &mut Vec<Leaf>,
) {
    let r = |k: i32| hash01(path as i32, k, block.level as i32, seed);
    let stops_early = block.level > 0 && r(0) < rule.stop_chance;
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
    let inner = |v: u32, n: u32| v > 0 && v + 1 < n;
    let keeps = |i: u32, j: u32, k: u32| {
        let interior = inner(i, nx) as u32 + inner(j, ny) as u32 + inner(k, nz) as u32;
        match rule.keep {
            Keep::All => true,
            Keep::Lattice => interior <= 1,
            Keep::ColumnsAndRoof => (!inner(i, nx) && !inner(k, nz)) || j + 1 == ny,
            Keep::Random(p) => random_keep(i, j, k, p),
            Keep::Skyline => j < column_height(i, k),
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
                let spans = j > 0 && !keeps(i, j - 1, k);
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
                split(rule, child, context, seed, child_path, max_leaves, out);
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
        let leaves = generate(&rule, root, 1, usize::MAX);
        assert_eq!(leaves.len(), 20 * 20 * 20);
        assert!(leaves.iter().all(|l| (l.block.half - Vec3::ONE).length() < 1e-4));
    }
}
