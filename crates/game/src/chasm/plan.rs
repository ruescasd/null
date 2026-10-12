//! A blockout of the chasm's plan (`--opt chasm --opt plan`, `--seed`): its
//! large-scale shape as plain blocks, to judge it before building on it.
//! Rock, its top at the rim, and the chasm carved through it on a coarse
//! grid as a tree of straight branches: a trunk as wide as the chasm (its
//! faces stepping in and out, 30-230 m apart), branches springing square
//! from it and from each other, each narrower than the one it springs from.
//! The chasm has no floor: every branch falls as far as the rock goes, the
//! bottom lost in the haze. The way down is to run mostly inside the rock,
//! so what matters here is the rock's shape between the branches: fins,
//! blocks, corners, where the inside meets the chasm.

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::Parts;

pub mod network;

/// The grid's cell (metres), and how many cells across the plan is.
const CELL: f32 = 8.0;
const N: i32 = 256;

/// Where the plan's corner is (the world wraps round at 16 km: well inside).
const ORIGIN: Vec3 = Vec3::new(1000.0, 0.0, 1000.0);

/// The rim's height: high enough that the ground below is lost in the haze.
const RIM: f32 = 1500.0;

/// A branch of the chasm: a straight run along x or z, its centre line from
/// `a` to `b` (cells), each side's face stepping in and out in stretches, at
/// most `reach` cells from it; `level` branchings from the trunk.
#[derive(Clone, Copy)]
struct Branch {
    a: IVec2,
    b: IVec2,
    reach: i32,
    level: u8,
}

impl Branch {
    /// The rectangle it may take (lo inclusive, hi exclusive), with
    /// `margin` cells round it.
    fn rect(&self, margin: i32) -> (IVec2, IVec2) {
        let lo = self.a.min(self.b) - IVec2::splat(self.reach + margin);
        let hi = self.a.max(self.b) + IVec2::splat(self.reach + 1 + margin);
        (lo, hi)
    }
}

/// The plan: per cell, rock (`None`) or the chasm (the level of the
/// branch there nearest the trunk).
struct Plan {
    cells: Vec<Option<u8>>,
    branches: Vec<Branch>,
}

impl Plan {
    fn new() -> Plan {
        Plan { cells: vec![None; (N * N) as usize], branches: Vec::new() }
    }

    fn get(&self, x: i32, z: i32) -> Option<u8> {
        if (0..N).contains(&x) && (0..N).contains(&z) { self.cells[(x + z * N) as usize] } else { Some(0) }
    }

    /// Carves a branch: along it in stretches (longer the wider the
    /// branch), each side's face a random distance out (the trunk's from a
    /// sixth of its reach to all of it, as the chasm's; a branch's from
    /// half, so it doesn't pinch), never the same as the stretch before's,
    /// so each joint is a step.
    fn carve(&mut self, b: Branch, r: &dyn Fn(i32, i32) -> f32) {
        let along = b.b - b.a;
        let len = along.abs().max_element().max(1);
        let dir = along / len;
        let side = IVec2::new(dir.y.abs(), dir.x.abs());
        let key = self.branches.len() as i32 * 97;
        let least = if b.level == 0 { 0.17 } else { 0.55 };
        let out = |k: i32, s: i32| ((b.reach as f32) * (least + (1.0 - least) * r(key + k, s))).round().max(2.0) as i32;
        let (mut t, mut k) = (0, 0);
        let (mut lo, mut hi) = (out(0, 10), out(0, 11));
        while t < len {
            let run = (b.reach as f32 * (0.8 + 2.5 * r(key + k, 12))).round().max(3.0) as i32;
            for u in t..(t + run).min(len) {
                let c = b.a + dir * u;
                for w in -lo..hi {
                    let p = c + side * w;
                    if (0..N).contains(&p.x) && (0..N).contains(&p.y) {
                        let cell = &mut self.cells[(p.x + p.y * N) as usize];
                        *cell = Some(cell.map_or(b.level, |l| l.min(b.level)));
                    }
                }
            }
            t += run;
            k += 1;
            let (l, h) = (out(k, 10), out(k, 11));
            lo = if l == lo { if l < b.reach { l + 1 } else { l - 1 } } else { l };
            hi = if h == hi { if h < b.reach { h + 1 } else { h - 1 } } else { h };
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

/// How far each face may stand from a branch's centre line (cells), so many
/// levels from the trunk: the trunk's as the chasm's (15-115 m), each level
/// narrower.
fn reach(level: u8) -> i32 {
    [14, 9, 6, 4][level.min(3) as usize]
}

/// The tree: a trunk across the plan, branches springing square from any
/// branch at random points along it, either side, of random length,
/// narrower than it, keeping rock between them; four levels.
fn tree(seed: u32) -> Plan {
    let mut plan = Plan::new();
    let r = |a: i32, b: i32| hash01(seed as i32, a, b, 0x4e2);
    let z = N / 2 + ((r(0, 0) - 0.5) * 60.0) as i32;
    plan.carve(Branch { a: IVec2::new(0, z), b: IVec2::new(N, z), reach: reach(0), level: 0 }, &r);
    let mut i = 0;
    while i < plan.branches.len() {
        let parent = plan.branches[i];
        if parent.level >= 3 {
            i += 1;
            continue;
        }
        let along = parent.b - parent.a;
        let len = along.abs().max_element();
        let dir = along / len.max(1);
        let turn = IVec2::new(dir.y, dir.x);
        let level = parent.level + 1;
        let cr = reach(level);
        // (The trunk's branches far apart, so theirs have room to grow.)
        let spread = if level == 1 { 45.0 } else { 14.0 };
        // Children every so often along it, each from its centre line out,
        // keeping rock between it and any other but its parent (its own
        // reach and a few cells); if one clashes, a shorter one.
        let mut t = cr + 2 + (r(i as i32, 2) * 12.0) as i32;
        while t < len - cr - 2 {
            let at = parent.a + dir * t;
            let key = i as i32 * 31 + t;
            // (Sides mostly in turn, so both sides of a branch get some.)
            let side = if (r(key, 3) < 0.25) == (plan.branches.len() % 2 == 0) { 1 } else { -1 };
            let l = (len as f32 * (0.35 + 0.55 * r(key, 4))).round().max(cr as f32 * 4.0) as i32;
            let placed = [l, l / 2].into_iter().filter(|&l| l > parent.reach + cr * 3).find_map(|l| {
                let b = Branch { a: at, b: at + turn * side * l, reach: cr, level };
                (!plan.clashes(&b, cr + 3, i)).then_some(b)
            });
            if let Some(b) = placed {
                plan.carve(b, &r);
            }
            t += if placed.is_some() { cr * 2 + 6 + (r(key, 5) * spread) as i32 } else { 3 };
        }
        i += 1;
    }
    plan
}

/// Where the blockout leaves its surfaces open, for what is built into the
/// rock (see `network::region`): cells of the top at the rim; bands (from,
/// to, in metres up) of a face, by its cell and which way it faces (as
/// `network::DIRS`).
#[derive(Default)]
struct Holes {
    tops: std::collections::HashSet<IVec2>,
    faces: std::collections::HashMap<(IVec2, usize), Vec<(f32, f32)>>,
}

/// The blockout: its rock as blocks, its top at the rim, its faces down to
/// the ground (lost in the haze), into `parts`; and where to start (on the
/// rim at the trunk's near end, looking along it).
pub(super) fn build(seed: u32) -> (Parts, [f32; 5]) {
    let plan = tree(seed);
    let mut parts = Parts::default();
    blockout(&plan, &Holes::default(), &mut parts.stone);
    let at = |x: i32, z: i32, y: f32| ORIGIN + Vec3::new(x as f32 * CELL, y, z as f32 * CELL);
    let trunk = plan.branches[0];
    let (lo, hi) = trunk.rect(0);
    let eye = at(lo.x + 12, hi.y + 1, RIM + 1.7);
    let look = at(lo.x + 120, (lo.y + hi.y) / 2, RIM * 0.85) - eye;
    let start = [eye.x, eye.y, eye.z, (-look.x).atan2(-look.z).to_degrees(), look.y.atan2(Vec2::new(look.x, look.z).length()).to_degrees()];
    info!("the plan: {} branches, {} cells of chasm in {} by {} km", plan.branches.len(), plan.cells.iter().filter(|c| c.is_some()).count(), N as f32 * CELL / 1000.0, N as f32 * CELL / 1000.0);
    (parts, start)
}

/// The rock's surfaces: its top at the rim (merged along rows), its faces
/// from the ground to the rim facing the chasm (merged along each edge);
/// but where `holes` opens them, a cell's top left out, a face's cell
/// standing alone, its bands left out.
fn blockout(plan: &Plan, holes: &Holes, g: &mut super::Geometry) {
    let at = |x: i32, z: i32, y: f32| ORIGIN + Vec3::new(x as f32 * CELL, y, z as f32 * CELL);
    let top = |x: i32, z: i32| plan.get(x, z).is_none() && !holes.tops.contains(&IVec2::new(x, z));
    for z in 0..N {
        let mut x = 0;
        while x < N {
            let here = top(x, z);
            let mut e = x + 1;
            while e < N && top(e, z) == here {
                e += 1;
            }
            if here {
                g.quad([at(x, z, RIM), at(x, z + 1, RIM), at(e, z + 1, RIM), at(e, z, RIM)], Vec3::Y);
            }
            x = e;
        }
    }
    let rock = |x: i32, z: i32| plan.get(x, z).is_none();
    for (k, (dx, dz)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
        let n = Vec3::new(dx as f32, 0.0, dz as f32);
        // A face from `t` to `e` along its edge, from `y0` up to `y1`.
        let mut quad = |line: i32, t: i32, e: i32, y0: f32, y1: f32| {
            let edge = line + if dx + dz > 0 { 1 } else { 0 };
            let corners = if dx != 0 { [at(edge, t, y0), at(edge, e, y0), at(edge, e, y1), at(edge, t, y1)] } else { [at(t, edge, y0), at(e, edge, y0), at(e, edge, y1), at(t, edge, y1)] };
            let outward = (corners[1] - corners[0]).cross(corners[3] - corners[0]).dot(n) > 0.0;
            g.quad(if outward { corners } else { [corners[0], corners[3], corners[2], corners[1]] }, n);
        };
        for line in 0..N {
            // (Along the edge: z for an edge across x, x for one across z.)
            let cell = |t: i32| if dx != 0 { IVec2::new(line, t) } else { IVec2::new(t, line) };
            let face = |t: i32| {
                let c = cell(t);
                rock(c.x, c.y) && !rock(c.x + dx, c.y + dz)
            };
            let open = |t: i32| holes.faces.get(&(cell(t), k));
            let mut t = 0;
            while t < N {
                if !face(t) {
                    t += 1;
                    continue;
                }
                if let Some(bands) = open(t) {
                    let mut bands = bands.clone();
                    bands.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let mut y = 0.0;
                    for (a, b) in bands {
                        if a > y {
                            quad(line, t, t + 1, y, a);
                        }
                        y = y.max(b);
                    }
                    if y < RIM {
                        quad(line, t, t + 1, y, RIM);
                    }
                    t += 1;
                    continue;
                }
                let mut e = t + 1;
                while e < N && face(e) && open(e).is_none() {
                    e += 1;
                }
                quad(line, t, e, 0.0, RIM);
                t = e;
            }
        }
    }
}

/// `--check plan`: the plan for a few seeds as a map (a greyscale image:
/// rock white, the chasm darker the nearer the trunk) in
/// `screenshots/plan/`.
pub fn maps() {
    let _ = std::fs::create_dir_all("screenshots/plan");
    for seed in 1..=8 {
        let plan = tree(seed);
        let mut bytes = format!("P5 {N} {N} 255\n").into_bytes();
        for z in 0..N {
            for x in 0..N {
                bytes.push(plan.get(x, z).map_or(235, |l| 40 + 35 * l));
            }
        }
        let path = format!("screenshots/plan/plan_{seed}.pgm");
        let _ = std::fs::write(&path, bytes);
        eprintln!("plan: {path}: {} branches", plan.branches.len());
    }
}

/// Where you start in the blockout.
pub fn start(seed: u32) -> [f32; 5] {
    build(seed).1
}
