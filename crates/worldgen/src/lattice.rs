//! A sparse lattice: big cells on a 3D grid, most of it left empty, every
//! filled cell carried by something you can read.
//!
//! Cores are slender shafts from the ground; beams span between cores that
//! line up. Cells gather round the cores, under a ceiling that falls away
//! from each core's top, so the whole reads as peaks with voids between
//! rather than a box. Each cell takes the upper part of its slot and floats
//! on a storey of air that shows what carries it: posts on the cell, beam or
//! core top below; a pier down over empty slots; outrigger arms out of a
//! core or a standing cell beside it; or rods from a beam or outrigger cell
//! above. Connectors are sized by the load they carry, so a pier under six
//! cells is massive and a rod holding one is slender. A cell that nothing
//! could carry is never placed.

use std::collections::HashMap;

use glam::{Quat, Vec2, Vec3};

use crate::noise::hash01;
use crate::structure::Solid;

/// How far columns and piers reach below the ground.
const FOUNDATION: f32 = 6.0;
/// The share of a slot's height under its cell, left to the structure
/// that carries it.
const GAP: f32 = 0.35;

/// The settings of a lattice (see [`crate::forms::Form::Lattice`]).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// A slot's width and height (metres).
    pub cell: (f32, f32),
    /// Layers of slots.
    pub levels: u32,
    /// The share of the slots within reach of the cores that are filled.
    pub fill: f32,
    /// How much cells gather next to filled neighbours (0: scattered).
    pub cluster: f32,
    /// How fast the ceiling falls away from the cores (0: flat).
    pub thin: f32,
    /// How much cells prefer hanging to standing (0..1).
    pub hang: f32,
    /// How much cells' sizes vary within their slots (0..1).
    pub vary: f32,
    /// The share of the cores' sub-grid points that are cores (0..1).
    pub cores: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Support {
    /// On posts on the ground, or on the cell, beam or core top below.
    Bearing,
    /// On a pier down to the cell this many slots below (or the ground, if
    /// it reaches below 0).
    Column(i32),
    /// On arms out of the core or standing cell in this direction.
    Cantilever((i32, i32)),
    /// On rods from the beam or outrigger cell this many slots above.
    Hang(i32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Slot {
    Cell(Support),
    /// A shaft, from the ground.
    Core,
    /// A beam along x (true) or z, spanning between two cores.
    Beam(bool),
}

type Key = (i32, i32, i32);

const SIDES: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

struct Lattice {
    slots: HashMap<Key, Slot>,
    nx: i32,
    nz: i32,
    ny: i32,
    /// Each core: its column and its height in slots.
    cores: Vec<(i32, i32, i32)>,
    thin: f32,
}

impl Lattice {
    /// How high cells may go over a column: from each core's top, down a
    /// slot (more with `thin`) per slot away; nothing more than two slots
    /// from a core.
    fn ceiling(&self, x: i32, z: i32) -> i32 {
        self.cores
            .iter()
            .filter_map(|&(cx, cz, top)| {
                let d = (x - cx).abs().max((z - cz).abs());
                (d <= 2).then(|| top - (d as f32 * (0.5 + 2.0 * self.thin)).round() as i32)
            })
            .max()
            .unwrap_or(-1)
    }

    fn inside(&self, (x, y, z): Key) -> bool {
        x.abs() <= self.nx && z.abs() <= self.nz && (0..self.ny).contains(&y)
    }

    fn empty(&self, k: Key) -> bool {
        self.inside(k) && !self.slots.contains_key(&k)
    }

    /// A core or a standing cell: others may reach out of it.
    fn stable(&self, k: Key) -> bool {
        matches!(self.slots.get(&k), Some(Slot::Core | Slot::Cell(Support::Bearing | Support::Column(_))))
    }

    /// What a pier may stand on: a standing cell (not a core: the pier
    /// would have to stand beside the shaft).
    fn footing(&self, k: Key) -> bool {
        matches!(self.slots.get(&k), Some(Slot::Cell(Support::Bearing | Support::Column(_))))
    }

    /// What a cell may bear on: any cell but a hanging one, a beam, or the
    /// top of a core.
    fn bears(&self, (x, y, z): Key) -> bool {
        match self.slots.get(&(x, y, z)) {
            Some(Slot::Cell(Support::Hang(_))) => false,
            Some(Slot::Core) => !matches!(self.slots.get(&(x, y + 1, z)), Some(Slot::Core)),
            Some(_) => true,
            None => false,
        }
    }

    /// What rods may hang from: a beam or an outrigger cell.
    fn holds(&self, k: Key) -> bool {
        matches!(self.slots.get(&k), Some(Slot::Beam(_) | Slot::Cell(Support::Cantilever(_))))
    }

    fn side_cells(&self, (x, y, z): Key) -> usize {
        SIDES.iter().filter(|(dx, dz)| matches!(self.slots.get(&(x + dx, y, z + dz)), Some(Slot::Cell(_)))).count()
    }

    /// The ways the empty slot `k` could be carried.
    fn supports(&self, (x, y, z): Key) -> Vec<Support> {
        let mut out = Vec::new();
        if y >= self.ceiling(x, z) {
            return out;
        }
        let below = (x, y - 1, z);
        if (y == 0 && self.side_cells((x, y, z)) > 0) || (y > 0 && self.bears(below)) {
            out.push(Support::Bearing);
        }
        if y > 0 && self.empty(below) {
            for k in 1..=4 {
                let target = (x, y - k, z);
                if y - k < 0 || self.footing(target) {
                    out.push(Support::Column(k));
                    break;
                }
                if !self.empty(target) {
                    break;
                }
            }
            for d in SIDES {
                if self.stable((x + d.0, y, z + d.1)) {
                    out.push(Support::Cantilever(d));
                }
            }
        }
        for k in 1..=2 {
            let target = (x, y + k, z);
            if self.holds(target) {
                out.push(Support::Hang(k));
                break;
            }
            if !self.empty(target) {
                break;
            }
        }
        out
    }

    /// Where a slot's load goes (none: the ground).
    fn parents(&self, (x, y, z): Key) -> Vec<Key> {
        match self.slots.get(&(x, y, z)) {
            Some(Slot::Cell(Support::Bearing)) if y > 0 => vec![(x, y - 1, z)],
            Some(Slot::Cell(Support::Column(k))) if y - k >= 0 => vec![(x, y - k, z)],
            Some(Slot::Cell(Support::Cantilever(d))) => vec![(x + d.0, y, z + d.1)],
            Some(Slot::Cell(Support::Hang(k))) => vec![(x, y + k, z)],
            Some(Slot::Core) if y > 0 => vec![(x, y - 1, z)],
            Some(Slot::Beam(along_x)) => {
                let d = if *along_x { (1, 0) } else { (0, 1) };
                let mut ends = Vec::new();
                for s in [1, -1] {
                    let mut p = (x, y, z);
                    loop {
                        p = (p.0 + d.0 * s, y, p.2 + d.1 * s);
                        match self.slots.get(&p) {
                            Some(Slot::Beam(_)) => continue,
                            Some(_) => ends.push(p),
                            None => {}
                        }
                        break;
                    }
                }
                ends
            }
            _ => vec![],
        }
    }
}

/// Grows the lattice's slots in a box `half` metres across.
fn grow(half: Vec2, s: Settings, seed: u32) -> Lattice {
    let w = s.cell.0.max(4.0);
    let mut l = Lattice {
        slots: HashMap::new(),
        nx: ((half.x / w - 0.5).floor() as i32).max(1),
        nz: ((half.y / w - 0.5).floor() as i32).max(1),
        ny: s.levels.max(2) as i32,
        cores: Vec::new(),
        thin: s.thin,
    };
    let r = |a: i32, b: i32, c: i32| hash01(a * 7919 + c, b, 0x1a7, seed);
    let focus = |x: i32, z: i32| 1.0 - (x as f32 / l.nx as f32).abs().max((z as f32 / l.nz as f32).abs());

    // Cores: on every third slot, some of them, taller in the middle.
    let (ox, oz) = ((r(1, 2, 3) * 3.0) as i32, (r(4, 5, 6) * 3.0) as i32);
    let mut cores = Vec::new();
    for x in (-l.nx..=l.nx).filter(|x| (x + ox).rem_euclid(3) == 0) {
        for z in (-l.nz..=l.nz).filter(|z| (z + oz).rem_euclid(3) == 0) {
            if r(x, z, 1) > s.cores {
                continue;
            }
            let tall = 0.3 + 0.7 * (0.6 * focus(x, z) + 0.4 * r(x, z, 2));
            cores.push((x, z, ((l.ny as f32 * tall).round() as i32).clamp(2, l.ny)));
        }
    }
    if cores.is_empty() {
        cores.push((0, 0, l.ny));
    }
    for &(x, z, top) in &cores {
        for y in 0..top {
            l.slots.insert((x, y, z), Slot::Core);
        }
    }
    l.cores = cores.clone();
    // Beams between cores three slots apart, at a level or two both reach.
    for &(x, z, top) in &cores {
        for (dx, dz, along_x) in [(3, 0, true), (0, 3, false)] {
            let Some(&(_, _, other)) = cores.iter().find(|c| c.0 == x + dx && c.1 == z + dz) else { continue };
            let mut y = top.min(other) - 1;
            let mut placed = 0;
            while y >= 2 && placed < 2 {
                if r(x * 3 + dx, z * 3 + dz, 10 + y) < 0.6 {
                    for t in 1..3 {
                        let p = (x + dx * t / 3, y, z + dz * t / 3);
                        if l.empty(p) {
                            l.slots.insert(p, Slot::Beam(along_x));
                        }
                    }
                    placed += 1;
                }
                y -= 2 + (r(x, z, 30 + y) * 3.0) as i32;
            }
        }
    }

    // Growth: cells added one at a time where something can carry them.
    let mut room = 0;
    for x in -l.nx..=l.nx {
        for z in -l.nz..=l.nz {
            room += l.ceiling(x, z).clamp(0, l.ny);
        }
    }
    let target = (room as f32 * s.fill.clamp(0.02, 0.9)) as usize;
    let mut placed = 0;
    let mut round = 0;
    while placed < target && round < 20_000 {
        round += 1;
        let mut candidates: Vec<(Key, Support, f32)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let filled: Vec<Key> = l.slots.keys().copied().collect();
        for (x, y, z) in filled {
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0), (0, -2, 0), (0, -3, 0), (0, -4, 0)] {
                let k = (x + dx, y + dy, z + dz);
                if !l.empty(k) || !seen.insert(k) {
                    continue;
                }
                // Neighbours draw cells in with `cluster`; more than two
                // make a wall, which is what this is meant to avoid.
                let near = l.side_cells(k) as f32;
                let crowd = if near > 2.0 { 0.15 } else { 1.0 + s.cluster * 2.0 * near };
                let spread = 1.0 - s.cluster * 0.6;
                // Up in the air more than on the ground.
                let up = if k.1 == 0 { 0.15 } else { 0.3 + k.1 as f32 / l.ny as f32 };
                let place = crowd * spread.max(0.2) * (0.5 + focus(k.0, k.2)) * up;
                for support in l.supports(k) {
                    let kind = match support {
                        Support::Bearing => 0.5,
                        Support::Column(_) => 0.7 * (1.0 - s.hang) + 0.1,
                        Support::Cantilever(_) => 1.0,
                        Support::Hang(_) => 0.2 + 3.0 * s.hang,
                    };
                    candidates.push((k, support, place * kind));
                }
            }
        }
        if candidates.is_empty() {
            break;
        }
        let total: f32 = candidates.iter().map(|c| c.2).sum();
        let mut pick = r(round, 77, 5) * total;
        let mut chosen = candidates[0];
        for c in &candidates {
            pick -= c.2;
            if pick <= 0.0 {
                chosen = *c;
                break;
            }
        }
        l.slots.insert(chosen.0, Slot::Cell(chosen.1));
        placed += 1;
    }
    l
}

/// The lattice's solids, in a box `half` metres across (local x, z) from
/// `origin` (its base at the ground) turned to `dir`.
pub fn lattice(origin: Vec3, dir: Vec2, half: Vec2, s: Settings, tone: f32, seed: u32) -> Vec<Solid> {
    let (w, h) = (s.cell.0.max(4.0), s.cell.1.max(3.0));
    let l = grow(half, s, seed);
    let r = |a: i32, b: i32, c: i32| hash01(a * 7919 + c, b, 0x1a7, seed);

    // Loads: each cell weighs one (a beam a third), passed down its supports.
    let mut load: HashMap<Key, f32> = HashMap::new();
    for (&k, slot) in &l.slots {
        let weight = match slot {
            Slot::Cell(_) => 1.0,
            Slot::Beam(_) => 0.3,
            Slot::Core => 0.0,
        };
        let mut front = vec![(k, weight)];
        let mut steps = 0;
        while let Some((p, wt)) = front.pop() {
            *load.entry(p).or_default() += wt;
            steps += 1;
            if steps > 400 {
                break;
            }
            let parents = l.parents(p);
            let share = wt / parents.len().max(1) as f32;
            front.extend(parents.into_iter().map(|q| (q, share)));
        }
    }
    let load_of = |k: Key| load.get(&k).copied().unwrap_or(0.0).max(1.0);

    let mut out = Vec::new();
    let boxed = |center: Vec3, half: Vec3, albedo: f32| Solid {
        detail: false,
        wedge: false,
        round: false,
        center,
        rotation: Quat::IDENTITY,
        half,
        albedo,
    };
    // A box from `lo` to `hi`.
    let span = |lo: Vec3, hi: Vec3, albedo: f32| boxed((lo + hi) * 0.5, (hi - lo).abs() * 0.5, albedo);
    let at = |x: i32, z: i32| Vec3::new(x as f32 * w, 0.0, z as f32 * w);
    let margin = w * 0.06;
    let gap = h * GAP;
    let beam_depth = |load: f32| h * (0.14 + 0.04 * load.sqrt()).min(0.3);
    let core_half = |x: i32, z: i32| (w * 0.16 * (1.0 + 0.08 * load_of((x, 0, z)).sqrt())).min(w * 0.3);
    let structure = tone + 0.03;

    // The cores' shafts, whole.
    for &(x, z, top) in &l.cores {
        let c = at(x, z);
        let a = core_half(x, z);
        out.push(span(c + Vec3::new(-a, -FOUNDATION, -a), c + Vec3::new(a, top as f32 * h, a), structure));
    }
    // A cell's extent across (x, z), with `vary`.
    let extent = |k: Key| {
        let shrink = |a: i32| 1.0 - s.vary * 0.5 * r(k.0 * 5 + a, k.2 * 5 + k.1, 40);
        (w * 0.5 * shrink(0) - margin, w * 0.5 * shrink(1) - margin)
    };
    for (&k, slot) in &l.slots {
        let c = at(k.0, k.2);
        let (base, top) = (k.1 as f32 * h, (k.1 + 1) as f32 * h);
        let lk = load_of(k);
        match *slot {
            Slot::Core => {}
            Slot::Beam(along_x) => {
                let depth = beam_depth(lk);
                let width = (w * 0.14 + 0.3 * lk.sqrt()).min(w * 0.35);
                let (ax, az) = if along_x { (w * 0.5 + 0.02, width * 0.5) } else { (width * 0.5, w * 0.5 + 0.02) };
                out.push(span(c + Vec3::new(-ax, top - depth, -az), c + Vec3::new(ax, top, az), tone + 0.05));
            }
            Slot::Cell(support) => {
                let (hx, hz) = extent(k);
                // The cell floats on the gap below it; a hung one leaves the
                // gap above it to its rods instead.
                let (y0, y1) = match support {
                    Support::Hang(_) => (base + gap * 0.3, top - gap * 0.7),
                    _ => (base + gap, top),
                };
                out.push(span(c + Vec3::new(-hx, y0, -hz), c + Vec3::new(hx, y1, hz), tone));
                match support {
                    Support::Bearing => {
                        let below = (k.0, k.1 - 1, k.2);
                        let post = (w * 0.03 * lk.sqrt()).clamp(0.5, w * 0.12);
                        match l.slots.get(&below) {
                            // A capital on the core's shaft.
                            Some(Slot::Core) => {
                                let a = core_half(k.0, k.2);
                                out.push(span(c + Vec3::new(-a, base, -a), c + Vec3::new(a, y0, a), structure));
                            }
                            // Two posts on the beam's line.
                            Some(Slot::Beam(along_x)) => {
                                for sgn in [-1.0, 1.0] {
                                    let p = if *along_x { c + Vec3::X * (sgn * hx * 0.7) } else { c + Vec3::Z * (sgn * hz * 0.7) };
                                    out.push(span(p + Vec3::new(-post, base, -post), p + Vec3::new(post, y0, post), structure));
                                }
                            }
                            // Posts at the corners, within the cell below (or
                            // into the ground).
                            _ => {
                                let foot = if k.1 == 0 { -FOUNDATION } else { base };
                                let (bx, bz) = if k.1 == 0 { (hx, hz) } else { extent(below) };
                                let (px, pz) = (hx.min(bx) * 0.7, hz.min(bz) * 0.7);
                                for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                                    let p = c + Vec3::new(sx * px, 0.0, sz * pz);
                                    out.push(span(p + Vec3::new(-post, foot, -post), p + Vec3::new(post, y0, post), structure));
                                }
                            }
                        }
                    }
                    Support::Column(n) => {
                        let foot = if k.1 - n < 0 { -FOUNDATION } else { (k.1 - n + 1) as f32 * h };
                        let pier = (w * 0.06 * lk.sqrt()).clamp(1.0, w * 0.3);
                        out.push(span(c + Vec3::new(-pier, foot, -pier), c + Vec3::new(pier, y0, pier), structure));
                    }
                    Support::Cantilever(d) => {
                        // Two arms in the gap, from the parent's middle to
                        // the cell's far edge.
                        let along = Vec3::new(d.0 as f32, 0.0, d.1 as f32);
                        let across = Vec3::new(-d.1 as f32, 0.0, d.0 as f32);
                        let reach = if d.0 != 0 { hx } else { hz };
                        let side = if d.0 != 0 { hz } else { hx };
                        let arm = (w * 0.04 * lk.sqrt()).clamp(0.5, w * 0.1);
                        let depth = (gap * (0.45 + 0.1 * lk.sqrt())).min(gap * 0.95);
                        let widen = across.abs() * arm;
                        for sgn in [-0.6, 0.6] {
                            let off = across * (sgn * side);
                            let (a, b) = (c + along * w + off, c - along * reach + off);
                            let (lo, hi) = (a.min(b) - widen, a.max(b) + widen);
                            out.push(span(Vec3::new(lo.x, y0 - depth, lo.z), Vec3::new(hi.x, y0, hi.z), structure));
                        }
                    }
                    Support::Hang(n) => {
                        let above = (k.0, k.1 + n, k.2);
                        let rod = (w * 0.008 * lk.sqrt()).clamp(0.2, w * 0.05);
                        // From a beam: rods along its line, to its underside.
                        // From an outrigger cell: at the corners, to the
                        // cell's underside.
                        let (holder, points) = match l.slots.get(&above) {
                            Some(Slot::Beam(along_x)) => {
                                let points = if *along_x {
                                    vec![Vec3::X * (hx * 0.75), Vec3::X * (-hx * 0.75)]
                                } else {
                                    vec![Vec3::Z * (hz * 0.75), Vec3::Z * (-hz * 0.75)]
                                };
                                ((k.1 + n + 1) as f32 * h - beam_depth(load_of(above)), points)
                            }
                            _ => {
                                let (ax, az) = extent(above);
                                let (px, pz) = (hx.min(ax) * 0.75, hz.min(az) * 0.75);
                                ((k.1 + n) as f32 * h + gap, vec![Vec3::new(px, 0.0, pz), Vec3::new(-px, 0.0, pz), Vec3::new(px, 0.0, -pz), Vec3::new(-px, 0.0, -pz)])
                            }
                        };
                        for q in points {
                            let p = c + q;
                            out.push(span(p + Vec3::new(-rod, y1, -rod), p + Vec3::new(rod, holder + 0.05, rod), structure + 0.02));
                        }
                    }
                }
            }
        }
    }
    let rot = Quat::from_rotation_y((-dir.y).atan2(dir.x));
    out.into_iter().map(|s| Solid { center: origin + rot * s.center, rotation: rot * s.rotation, ..s }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings { cell: (24.0, 18.0), levels: 8, fill: 0.35, cluster: 0.5, thin: 0.3, hang: 0.4, vary: 0.2, cores: 0.5 }
    }

    #[test]
    fn every_cell_reaches_the_ground() {
        for seed in 0..6 {
            let l = grow(Vec2::splat(160.0), settings(), seed);
            let cells = l.slots.values().filter(|v| matches!(v, Slot::Cell(_))).count();
            assert!(cells > 40, "{cells}");
            for &k in l.slots.keys() {
                // Follow the first parent until the ground.
                let mut p = k;
                for _ in 0..100 {
                    let parents = l.parents(p);
                    if parents.is_empty() {
                        break;
                    }
                    for q in &parents {
                        assert!(l.slots.contains_key(q), "{k:?}: {p:?} rests on empty {q:?}");
                    }
                    p = parents[0];
                }
                let grounded = match l.slots[&p] {
                    Slot::Cell(Support::Bearing) | Slot::Core => p.1 == 0,
                    Slot::Cell(Support::Column(n)) => p.1 - n < 0,
                    _ => false,
                };
                assert!(grounded, "{k:?} ends at {p:?} {:?}", l.slots[&p]);
            }
            let solids = lattice(Vec3::ZERO, Vec2::X, Vec2::splat(160.0), settings(), 0.15, seed);
            assert!(solids.len() >= cells);
        }
    }
}
