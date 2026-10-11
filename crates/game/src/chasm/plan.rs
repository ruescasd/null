//! A blockout of the chasm's plan (`--opt chasm --opt plan`, `--set plan=0`
//! an H-tree, `1` a looser tree; `--seed`): its large-scale shape as plain
//! blocks, to judge it before building on it. Rock, its top at the rim, and
//! the chasm carved through it on a coarse grid as a tree of branches, each
//! straight, each narrower than the one it springs from, side branches'
//! floors higher than the trunk's (the floors step down into the trunk).
//! The way down is to run mostly inside the rock, so what matters here is
//! the rock's shape between the branches: fins, blocks, corners, where the
//! inside meets the chasm.

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::{HEIGHT, Parts};

/// The grid's cell (metres), and how many cells across the plan is.
const CELL: f32 = 8.0;
const N: i32 = 256;

/// Where the plan's corner is (the world wraps round at 16 km: well inside).
const ORIGIN: Vec3 = Vec3::new(1000.0, 0.0, 1000.0);

/// A branch of the chasm: a straight run along x or z, from `a` to `b` (the
/// middle of its ends, in cells), `width` cells across, its floor `floor`
/// metres up.
#[derive(Clone, Copy)]
struct Branch {
    a: IVec2,
    b: IVec2,
    width: i32,
    floor: f32,
}

impl Branch {
    /// Its cells, as a rectangle (lo inclusive, hi exclusive).
    fn rect(&self, margin: i32) -> (IVec2, IVec2) {
        let h = self.width / 2;
        let lo = self.a.min(self.b) - IVec2::splat(h + margin);
        let hi = self.a.max(self.b) + IVec2::splat(self.width - h + margin);
        (lo, hi)
    }
}

/// The plan: per cell, rock (`None`) or the chasm and its floor's height.
struct Plan {
    cells: Vec<Option<f32>>,
    branches: Vec<Branch>,
}

impl Plan {
    fn new() -> Plan {
        Plan { cells: vec![None; (N * N) as usize], branches: Vec::new() }
    }

    fn get(&self, x: i32, z: i32) -> Option<f32> {
        if (0..N).contains(&x) && (0..N).contains(&z) { self.cells[(x + z * N) as usize] } else { Some(0.0) }
    }

    /// Carves a branch (where two meet, the lower floor).
    fn carve(&mut self, b: Branch) {
        let (lo, hi) = b.rect(0);
        for z in lo.y.max(0)..hi.y.min(N) {
            for x in lo.x.max(0)..hi.x.min(N) {
                let c = &mut self.cells[(x + z * N) as usize];
                *c = Some(c.map_or(b.floor, |f| f.min(b.floor)));
            }
        }
        self.branches.push(b);
    }

    /// Whether a branch (with `margin` cells of rock round it) would run
    /// into any carved before but `except` (the one it springs from).
    fn clashes(&self, b: &Branch, margin: i32, except: usize) -> bool {
        let (lo, hi) = b.rect(margin);
        if lo.x < 4 || lo.y < 4 || hi.x > N - 4 || hi.y > N - 4 {
            return true;
        }
        self.branches.iter().enumerate().any(|(i, o)| {
            let (a, c) = o.rect(0);
            i != except && lo.x < c.x && hi.x > a.x && lo.y < c.y && hi.y > a.y
        })
    }
}

/// How wide (cells) and how high the floor (m) of a branch so many levels
/// from the trunk.
fn level(k: usize) -> (i32, f32) {
    [(20, 0.0), (14, 110.0), (10, 210.0), (7, 290.0), (5, 350.0)][k.min(4)]
}

/// An H-tree: a trunk across the middle, at each end of each branch a
/// branch square to it, centred on it, about 0.7 times as long, narrower,
/// shallower; four levels, some branches cut off (by `seed`).
fn htree(seed: u32) -> Plan {
    let mut plan = Plan::new();
    let r = |a: i32, b: i32| hash01(seed as i32, a, b, 0x4e1);
    let mut stack = vec![(IVec2::new(N / 2, N / 2), IVec2::X, 120.0f32, 0usize, 0i32)];
    while let Some((mid, dir, len, k, key)) = stack.pop() {
        if k > 1 && r(key, 1) < 0.25 {
            continue;
        }
        let (width, floor) = level(k);
        let half = (len * 0.5).round() as i32;
        let b = Branch { a: mid - dir * half, b: mid + dir * half, width, floor };
        plan.carve(b);
        if k < 3 {
            let turn = IVec2::new(dir.y, dir.x);
            for (j, end) in [b.a, b.b].into_iter().enumerate() {
                stack.push((end, turn, len * 0.707, k + 1, key * 2 + j as i32 + 1));
            }
        }
    }
    plan
}

/// A looser tree: a trunk across the plan, branches springing square from
/// any branch at random points along it, either side, of random length,
/// narrower and shallower than it, keeping rock between them; three
/// levels.
fn tree(seed: u32) -> Plan {
    let mut plan = Plan::new();
    let r = |a: i32, b: i32| hash01(seed as i32, a, b, 0x4e2);
    let (width, floor) = level(0);
    plan.carve(Branch { a: IVec2::new(20, N / 2 + ((r(0, 0) - 0.5) * 40.0) as i32), b: IVec2::new(N - 20, N / 2 + ((r(0, 1) - 0.5) * 40.0) as i32), width, floor });
    let mut tries = 0;
    let mut i = 0;
    while i < plan.branches.len() && tries < 4000 {
        let parent = plan.branches[i];
        let k = (0..5).find(|&k| level(k).0 <= parent.width).unwrap_or(4);
        if k >= 3 {
            i += 1;
            continue;
        }
        let along = parent.b - parent.a;
        let len = along.abs().max_element();
        let dir = along / len.max(1);
        let turn = IVec2::new(dir.y, dir.x);
        let (cw, cf) = level(k + 1);
        // Children every so often along it.
        let mut t = 12 + (r(i as i32, 2) * 20.0) as i32;
        while t < len - 12 {
            tries += 1;
            let at = parent.a + dir * t;
            let side = if r(i as i32 * 31 + t, 3) < 0.5 { 1 } else { -1 };
            let l = (len as f32 * (0.3 + 0.5 * r(i as i32 * 31 + t, 4))).round() as i32;
            let start = at + turn * side * (parent.width / 2);
            let b = Branch { a: start, b: start + turn * side * l, width: cw, floor: cf };
            if !plan.clashes(&b, cw + 4, i) {
                plan.carve(b);
                t += cw * 3 + (r(i as i32 * 31 + t, 5) * 25.0) as i32;
            } else {
                t += 6;
            }
        }
        i += 1;
    }
    plan
}

/// The blockout: its rock as blocks (its top at the rim; its sides down to
/// the floors beside them), the floors that stand above the ground, into
/// `parts`; and where to start (on the rim at the trunk's middle, looking
/// along it).
pub(super) fn build(seed: u32, kind: u32) -> (Parts, [f32; 5]) {
    let plan = if kind == 0 { htree(seed) } else { tree(seed) };
    let mut parts = Parts::default();
    let g = &mut parts.stone;
    let at = |x: i32, z: i32, y: f32| ORIGIN + Vec3::new(x as f32 * CELL, y, z as f32 * CELL);
    // Tops: of the rock at the rim, of each floor above the ground; merged
    // along rows.
    for z in 0..N {
        let mut x = 0;
        while x < N {
            let here = plan.get(x, z);
            let mut e = x + 1;
            while e < N && plan.get(e, z) == here {
                e += 1;
            }
            let y = here.unwrap_or(HEIGHT);
            if y > 0.0 {
                g.quad([at(x, z, y), at(x, z + 1, y), at(e, z + 1, y), at(e, z, y)], Vec3::Y);
            }
            x = e;
        }
    }
    // Sides: wherever a cell stands higher than the one beside it, from
    // that one's height up to its own, facing it (merged along the edge).
    let height = |c: Option<f32>| c.unwrap_or(HEIGHT);
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let n = Vec3::new(dx as f32, 0.0, dz as f32);
        for line in 0..N {
            // (Along the edge: z for an edge across x, x for one across z.)
            let cell = |t: i32| if dx != 0 { (line, t) } else { (t, line) };
            let pair = |t: i32| {
                let (x, z) = cell(t);
                (height(plan.get(x, z)), height(plan.get(x + dx, z + dz)))
            };
            let mut t = 0;
            while t < N {
                let (h, o) = pair(t);
                if h <= o {
                    t += 1;
                    continue;
                }
                let mut e = t + 1;
                while e < N && pair(e) == (h, o) {
                    e += 1;
                }
                let edge = line + if dx + dz > 0 { 1 } else { 0 };
                let corners = if dx != 0 { [at(edge, t, o), at(edge, e, o), at(edge, e, h), at(edge, t, h)] } else { [at(t, edge, o), at(e, edge, o), at(e, edge, h), at(t, edge, h)] };
                let outward = (corners[1] - corners[0]).cross(corners[3] - corners[0]).dot(n) > 0.0;
                g.quad(if outward { corners } else { [corners[0], corners[3], corners[2], corners[1]] }, n);
                t = e;
            }
        }
    }
    let trunk = plan.branches[0];
    let (lo, hi) = trunk.rect(0);
    let eye = at(lo.x + 6, hi.y + 2, HEIGHT + 1.7);
    let look = at(hi.x, (lo.y + hi.y) / 2, HEIGHT * 0.4) - eye;
    let start = [eye.x, eye.y, eye.z, (-look.x).atan2(-look.z).to_degrees(), look.y.atan2(Vec2::new(look.x, look.z).length()).to_degrees()];
    info!("the plan: {} branches, {} cells of chasm in {} by {} km", plan.branches.len(), plan.cells.iter().filter(|c| c.is_some()).count(), N as f32 * CELL / 1000.0, N as f32 * CELL / 1000.0);
    (parts, start)
}

/// `--check plan`: each kind's plan for a few seeds as a map (a greyscale
/// image: rock white, the chasm darker the deeper its floor) in
/// `screenshots/plan/`.
pub fn maps() {
    let _ = std::fs::create_dir_all("screenshots/plan");
    for kind in 0..2 {
        for seed in 1..=4 {
            let plan = if kind == 0 { htree(seed) } else { tree(seed) };
            let mut bytes = format!("P5 {N} {N} 255
").into_bytes();
            for z in 0..N {
                for x in 0..N {
                    bytes.push(plan.get(x, z).map_or(235, |f| (40.0 + f / HEIGHT * 120.0) as u8));
                }
            }
            let path = format!("screenshots/plan/plan{kind}_{seed}.pgm");
            let _ = std::fs::write(&path, bytes);
            eprintln!("plan: {path}: {} branches", plan.branches.len());
        }
    }
}

/// Where you start in the blockout.
pub fn start(seed: u32, kind: u32) -> [f32; 5] {
    build(seed, kind).1
}
