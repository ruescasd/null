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

use std::f32::consts::TAU;

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
    /// The chance a cell joins its neighbours in a longer one (up to three
    /// slots in a line).
    pub bars: f32,
    /// A city: each cell is a deck with buildings on it (see
    /// `cellunit.rs`), joined by walkways and stairs (0: plain cells).
    pub content: u32,
    /// Colossal wedges rising from the ground to decks near the edge.
    pub ramps: u32,
    pub ramp_grain: crate::mega::Grain,
    /// A void this many metres across the middle, nothing in it.
    pub void: f32,
    /// Round: the lattice bent into rings round a void (its radius in
    /// metres) and the number of rings; its rows run round the rings.
    pub radial: Option<(f32, u32)>,
    /// A reactor rising in the void, and this many cables hung from it to
    /// the cells round it.
    pub reactor: bool,
    pub cables: u32,
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
    /// The void in the middle, in slots across (none: 0).
    void: f32,
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
        x.abs() <= self.nx && z.abs() <= self.nz && (0..self.ny).contains(&y) && (x as f32).hypot(z as f32) >= self.void
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
    // Round: rings out from the void (x) by rows round them (z), as many
    // rows as make the inner ring's cells about square.
    let (nx, nz) = match s.radial {
        Some((inner, rings)) => (((rings.max(1) as i32) - 1) / 2, ((TAU * (inner + w * 0.5) / w).round() as i32 - 1) / 2),
        None => (((half.x / w - 0.5).floor() as i32).max(1), ((half.y / w - 0.5).floor() as i32).max(1)),
    };
    let mut l = Lattice {
        slots: HashMap::new(),
        nx,
        nz,
        ny: s.levels.max(2) as i32,
        cores: Vec::new(),
        thin: s.thin,
        void: if s.radial.is_some() { 0.0 } else { s.void / w * 0.5 },
    };
    let r = |a: i32, b: i32, c: i32| hash01(a * 7919 + c, b, 0x1a7, seed);
    // How central a column is: round, the inner rings (by the reactor);
    // else the middle.
    let radial = s.radial.is_some();
    let focus = |x: i32, z: i32| {
        if radial { 1.0 - (x + l.nx) as f32 / (2 * l.nx).max(1) as f32 } else { 1.0 - (x as f32 / l.nx as f32).abs().max((z as f32 / l.nz as f32).abs()) }
    };

    // Cores: on every third slot, some of them, taller in the middle.
    let (ox, oz) = ((r(1, 2, 3) * 3.0) as i32, (r(4, 5, 6) * 3.0) as i32);
    let mut cores = Vec::new();
    for x in (-l.nx..=l.nx).filter(|x| (x + ox).rem_euclid(3) == 0) {
        for z in (-l.nz..=l.nz).filter(|z| (z + oz).rem_euclid(3) == 0) {
            if r(x, z, 1) > s.cores || !l.inside((x, 0, z)) {
                continue;
            }
            let tall = 0.3 + 0.7 * (0.6 * focus(x, z) + 0.4 * r(x, z, 2));
            cores.push((x, z, ((l.ny as f32 * tall).round() as i32).clamp(2, l.ny)));
        }
    }
    if cores.is_empty() {
        cores.push((l.nx, 0, l.ny));
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

/// Cells joined into longer ones: runs of up to three slots along x or z
/// (with chance `share`), all standing or all hung so that their decks are
/// at one height. Each run and whether it lies along x.
fn bars(l: &Lattice, share: f32, radial: bool, seed: u32) -> Vec<(Vec<Key>, bool)> {
    let r = |a: i32, b: i32, c: i32| hash01(a * 7919 + c, b, 0xba5, seed);
    let mut keys: Vec<Key> = l.slots.iter().filter(|(_, v)| matches!(v, Slot::Cell(_))).map(|(k, _)| *k).collect();
    keys.sort();
    let hung = |k: &Key| matches!(l.slots.get(k), Some(Slot::Cell(Support::Hang(_))));
    let mut taken = std::collections::HashSet::new();
    let mut out = Vec::new();
    for k in keys {
        if !taken.insert(k) {
            continue;
        }
        let along_x = radial || r(k.0, k.2, k.1) < 0.5;
        let want = if r(k.0, k.2, k.1 + 50) < share { 2 + (r(k.0, k.2, k.1 + 90) * 2.0) as usize } else { 1 };
        let d = if along_x { (1, 0) } else { (0, 1) };
        let mut run = vec![k];
        while run.len() < want {
            let last = run[run.len() - 1];
            let next = (last.0 + d.0, last.1, last.2 + d.1);
            if !matches!(l.slots.get(&next), Some(Slot::Cell(_))) || hung(&next) != hung(&k) || taken.contains(&next) {
                break;
            }
            taken.insert(next);
            run.push(next);
        }
        out.push((run, along_x));
    }
    out
}

/// The lattice's solids, in a box `half` metres across (local x, z) from
/// `origin` (its base at the ground) turned to `dir`.
pub fn lattice(origin: Vec3, dir: Vec2, half: Vec2, s: Settings, tone: f32, seed: u32) -> Vec<Solid> {
    let (w, h) = (s.cell.0.max(4.0), s.cell.1.max(3.0));
    let l = grow(half, s, seed);
    let r = |a: i32, b: i32, c: i32| hash01(a * 7919 + c, b, 0x1a7, seed);
    let groups = bars(&l, s.bars, s.radial.is_some(), seed);
    let mut group_of: HashMap<Key, usize> = HashMap::new();
    for (i, (run, _)) in groups.iter().enumerate() {
        for &k in run {
            group_of.insert(k, i);
        }
    }

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
    let city = s.content > 0;
    // A deck's thickness, in a city.
    let deck = (h * 0.07).clamp(1.5, 4.0);
    let beam_depth = |load: f32| h * (0.14 + 0.04 * load.sqrt()).min(0.3);
    let core_half = |x: i32, z: i32| (w * 0.16 * (1.0 + 0.08 * load_of((x, 0, z)).sqrt())).min(w * 0.3);
    let structure = tone + 0.03;
    let hung = |k: Key| matches!(l.slots.get(&k), Some(Slot::Cell(Support::Hang(_))));
    // A cell's underside (its deck's, in a city): it floats on the gap
    // below it, or, hung, leaves most of the gap above it to its rods.
    let bottom = |k: Key| k.1 as f32 * h + if hung(k) { gap * 0.3 } else { gap };
    // How high a cell's room reaches.
    let ceiling = |k: Key| (k.1 + 1) as f32 * h - if hung(k) { gap * 0.7 } else { 0.0 };
    // A cell's top: what others land on (its deck, in a city).
    let surface = |k: Key| if city { bottom(k) + deck } else { ceiling(k) };
    // A cell's extent across (x, z) about its slot's centre; single cells
    // vary with `vary`.
    let extent = |k: Key| {
        let single = group_of.get(&k).is_none_or(|&g| groups[g].0.len() == 1);
        let vary = if single && !city { s.vary } else { 0.0 };
        let shrink = |a: i32| 1.0 - vary * 0.5 * r(k.0 * 5 + a, k.2 * 5 + k.1, 40);
        (w * 0.5 * shrink(0) - margin, w * 0.5 * shrink(1) - margin)
    };

    // The cores' shafts, whole.
    for &(x, z, top) in &l.cores {
        let c = at(x, z);
        let a = core_half(x, z);
        out.push(span(c + Vec3::new(-a, -FOUNDATION, -a), c + Vec3::new(a, top as f32 * h, a), structure));
    }
    // The cells: a box each in the lattice; a deck in a city, with a
    // complex composed on most (a few variants per length, reused).
    let mut composed: HashMap<(crate::cellunit::Kind, usize, i32, u32, i32), Vec<Solid>> = HashMap::new();
    // Round: the rings and rows, and where a lattice point lies on them.
    let rings = s.radial.map(|(inner, _)| (inner, TAU / (2 * l.nz + 1) as f32));
    let polar = |p: Vec3| -> (Vec3, f32, f32) {
        let (inner, step) = rings.unwrap_or((0.0, 0.0));
        let radius = inner + p.x + (l.nx as f32 + 0.5) * w;
        let angle = p.z / w * step;
        (Vec3::new(radius * angle.cos(), p.y, radius * angle.sin()), angle, radius)
    };
    // What stands on the decks, when round, placed on the rings directly.
    let mut placed: Vec<Solid> = Vec::new();
    for (i, (run, along_x)) in groups.iter().enumerate() {
        let (first, last) = (run[0], run[run.len() - 1]);
        let (ex, ez) = extent(first);
        let (a, b) = (at(first.0, first.2), at(last.0, last.2));
        let (lo, hi) = (a.min(b) - Vec3::new(ex, 0.0, ez), a.max(b) + Vec3::new(ex, 0.0, ez));
        let y0 = bottom(first);
        let y1 = if city { y0 + deck } else { ceiling(first) };
        out.push(span(Vec3::new(lo.x, y0, lo.z), Vec3::new(hi.x, y1, hi.z), tone));
        if !city {
            continue;
        }
        // What stands on it, composed with the cell as its frame: a few
        // variants per kind, length and height, reused.
        let len = run.len();
        let kind = crate::cellunit::pick(len, r(i as i32, 9, 63));
        let storeys = ((ceiling(first) - y1 - 1.5) / 4.5).floor() as i32;
        let variant = (r(i as i32, 5, 61) * 3.0) as u32;
        let place = Vec3::new((lo.x + hi.x) * 0.5, y1, (lo.z + hi.z) * 0.5);
        // Round, a deck is as wide across as its row is at its radius.
        let (on_ring, angle, radius) = polar(place);
        let across = match rings {
            Some((_, step)) => radius * step * 0.5 - margin,
            None => w * 0.5 - margin,
        };
        let half = (len as f32 * w * 0.5 - margin, across);
        let content = composed.entry((kind, len, storeys, variant, across.round() as i32)).or_insert_with(|| {
            crate::cellunit::unit(kind, half, storeys, tone, seed ^ (len as u32 * 7919 + variant * 104_729 + storeys as u32 * 31))
        });
        // Along the run, turned end for end at random.
        let flip = if r(i as i32, 7, 62) < 0.5 { std::f32::consts::PI } else { 0.0 };
        let turn = Quat::from_rotation_y(if *along_x { 0.0 } else { -std::f32::consts::FRAC_PI_2 } + flip);
        if rings.is_some() {
            let turn = Quat::from_rotation_y(-angle) * turn;
            placed.extend(content.iter().map(|s| Solid { center: on_ring + turn * s.center, rotation: turn * s.rotation, ..*s }));
        } else {
            out.extend(content.iter().map(|s| Solid { center: place + turn * s.center, rotation: turn * s.rotation, ..*s }));
        }
    }
    if city {
        let ex = w * 0.5 - margin;
        let fine = |lo: Vec3, hi: Vec3, albedo: f32| Solid { detail: true, ..span(lo, hi, albedo) };
        // Walkways across one or two empty slots to a deck at the same
        // height, and long stairs across one to a deck a level up: from
        // each slot's open sides, once per pair.
        let mut openings: std::collections::HashSet<(Key, (i32, i32))> = std::collections::HashSet::new();
        let mut cells: Vec<Key> = groups.iter().flat_map(|g| g.0.iter().copied()).collect();
        cells.sort();
        for &k in &cells {
            for d in SIDES {
                let step = |n: i32, dy: i32| (k.0 + d.0 * n, k.1 + dy, k.2 + d.1 * n);
                if group_of.get(&step(1, 0)) == group_of.get(&k) || !l.empty(step(1, 0)) {
                    continue;
                }
                let n_hat = Vec3::new(d.0 as f32, 0.0, d.1 as f32);
                let from = at(k.0, k.2) + n_hat * ex + Vec3::Y * surface(k);
                // A walkway.
                let mut linked = false;
                for gap in 1..=2 {
                    let t = step(gap + 1, 0);
                    if gap == 2 && !l.empty(step(2, 0)) {
                        break;
                    }
                    if group_of.contains_key(&t) && t > k && (surface(t) - surface(k)).abs() < 0.1 && r(k.0 * 3 + d.0, k.2 * 3 + d.1, 70 + k.1) < 0.75 {
                        let to = at(t.0, t.2) - n_hat * ex + Vec3::Y * surface(t);
                        let across = Vec3::new(-n_hat.z, 0.0, n_hat.x);
                        let (a, b) = (from - across * 3.0, to + across * 3.0);
                        let (lo, hi) = (a.min(b), a.max(b));
                        out.push(span(Vec3::new(lo.x, from.y - 1.0, lo.z), Vec3::new(hi.x, from.y, hi.z), tone + 0.04));
                        // A girder under it, deeper for a longer span.
                        let depth = ((to - from).length() / 22.0).max(1.5);
                        let (a, b) = (from - across * 1.0, to + across * 1.0);
                        out.push(span(Vec3::new(a.min(b).x, from.y - 1.0 - depth, a.min(b).z), Vec3::new(a.max(b).x, from.y - 1.0, a.max(b).z), structure));
                        for sgn in [-1.0, 1.0] {
                            let (a, b) = (from + across * (sgn * 2.9), to + across * (sgn * 2.9));
                            let pad = across.abs() * 0.075;
                            out.push(fine(a.min(b) - pad, a.max(b) + pad + Vec3::Y * 1.05, tone + 0.07));
                        }
                        openings.insert((k, d));
                        openings.insert((t, (-d.0, -d.1)));
                        linked = true;
                        break;
                    }
                    if !l.empty(t) {
                        break;
                    }
                }
                // A long stair to a deck a level up, across one slot.
                let t = step(2, 1);
                if !linked
                    && group_of.contains_key(&t)
                    && l.empty(step(1, 1))
                    && (surface(t) - surface(k) - h).abs() < 0.1
                    && r(k.0 * 5 + d.0, k.2 * 5 + d.1, 80 + k.1) < 0.6
                {
                    let to = at(t.0, t.2) - n_hat * ex + Vec3::Y * surface(t);
                    let (run, rise) = ((to - from).with_y(0.0).length(), to.y - from.y);
                    let steps = (rise / 0.45).ceil() as i32;
                    let (tread, riser) = (run / steps as f32, rise / steps as f32);
                    let across = Vec3::new(-n_hat.z, 0.0, n_hat.x);
                    for i in 0..steps {
                        let a = from + n_hat * (tread * i as f32) - across * 2.5;
                        let b = from + n_hat * (tread * (i + 1) as f32 + 0.02) + across * 2.5;
                        let y = from.y + riser * (i + 1) as f32;
                        out.push(fine(Vec3::new(a.min(b).x, y - 1.2, a.min(b).z), Vec3::new(a.max(b).x, y, a.max(b).z), tone + 0.05));
                    }
                    // A stringer under it, and railings, along the slope.
                    let length = (run * run + rise * rise).sqrt();
                    let pitch = rise.atan2(run);
                    let yaw = (-n_hat.z).atan2(n_hat.x);
                    let turn = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(pitch);
                    let mid = (from + to) * 0.5;
                    let down = turn * Vec3::NEG_Y;
                    out.push(Solid { rotation: turn, ..span(Vec3::ZERO, Vec3::ZERO, structure) });
                    let last = out.len() - 1;
                    out[last].center = mid + down * 2.6;
                    out[last].half = Vec3::new(length * 0.5, 1.2, 1.2);
                    for sgn in [-1.0, 1.0] {
                        out.push(Solid {
                            detail: true,
                            rotation: turn,
                            center: mid + across * (sgn * 2.45) + Vec3::Y * 0.55,
                            half: Vec3::new(length * 0.5, 0.5, 0.075),
                            ..span(Vec3::ZERO, Vec3::ONE, tone + 0.07)
                        });
                    }
                    openings.insert((k, d));
                    openings.insert((t, (-d.0, -d.1)));
                }
            }
        }
        // Rims: a railing along every open deck edge, open where a walkway
        // or stair lands; here and there a balcony out over the void.
        for &k in &cells {
            for d in SIDES {
                if group_of.get(&(k.0 + d.0, k.1, k.2 + d.1)) == group_of.get(&k) {
                    continue;
                }
                let n_hat = Vec3::new(d.0 as f32, 0.0, d.1 as f32);
                let along = Vec3::new(-n_hat.z, 0.0, n_hat.x);
                // The edge's ends: to the slot's end where the deck carries
                // on into the next slot of the cell, else to the deck's.
                let reach = |sgn: i32| {
                    let next = (k.0 + (along.x as i32) * sgn, k.1, k.2 + (along.z as i32) * sgn);
                    if group_of.get(&next) == group_of.get(&k) { w * 0.5 } else { ex }
                };
                let (u0, u1) = (-reach(-1), reach(1));
                let y = surface(k);
                let edge = at(k.0, k.2) + n_hat * (ex - 0.075) + Vec3::Y * y;
                let mut gaps: Vec<(f32, f32)> = Vec::new();
                if openings.contains(&(k, d)) {
                    gaps.push((-3.2, 3.2));
                } else if r(k.0 * 7 + d.0, k.2 * 7 + d.1, 90 + k.1) < 0.3 {
                    // A balcony: a slab out past the edge, railed round.
                    let half = 6.0 + r(k.0, k.2, 91 + k.1) * 8.0;
                    let off = (r(k.0, k.2, 92 + k.1) - 0.5) * (u1 - u0 - 2.0 * half - 4.0).max(0.0);
                    let deep = 5.0;
                    let (a, b) = (edge + along * (off - half), edge + along * (off + half) + n_hat * deep);
                    out.push(span(Vec3::new(a.min(b).x, y - 1.2, a.min(b).z), Vec3::new(a.max(b).x, y, a.max(b).z), tone + 0.04));
                    let rail = |p: Vec3, q: Vec3| {
                        let pad = if (q - p).x.abs() > (q - p).z.abs() { Vec3::new(0.0, 0.0, 0.075) } else { Vec3::new(0.075, 0.0, 0.0) };
                        fine(p.min(q) - pad, p.max(q) + pad + Vec3::Y * 1.05, tone + 0.07)
                    };
                    let (c0, c1) = (edge + along * (off - half), edge + along * (off + half));
                    out.push(rail(c0 + n_hat * deep, c1 + n_hat * deep));
                    out.push(rail(c0, c0 + n_hat * deep));
                    out.push(rail(c1, c1 + n_hat * deep));
                    gaps.push((off - half, off + half));
                }
                let mut u = u0;
                gaps.push((u1, u1));
                for (g0, g1) in gaps {
                    if g0 > u + 0.1 {
                        let (p, q) = (edge + along * u, edge + along * g0);
                        let pad = if along.x.abs() > 0.5 { Vec3::new(0.0, 0.0, 0.075) } else { Vec3::new(0.075, 0.0, 0.0) };
                        out.push(fine(p.min(q) - pad, p.max(q) + pad + Vec3::Y * 1.05, tone + 0.07));
                    }
                    u = u.max(g1);
                }
            }
        }
    }
    // What carries each slot of each cell, and the beams.
    for (&k, slot) in &l.slots {
        let c = at(k.0, k.2);
        let lk = load_of(k);
        match *slot {
            Slot::Core => {}
            Slot::Beam(along_x) => {
                let top = (k.1 + 1) as f32 * h;
                let depth = beam_depth(lk);
                let width = (w * 0.14 + 0.3 * lk.sqrt()).min(w * 0.35);
                let (ax, az) = if along_x { (w * 0.5 + 0.02, width * 0.5) } else { (width * 0.5, w * 0.5 + 0.02) };
                out.push(span(c + Vec3::new(-ax, top - depth, -az), c + Vec3::new(ax, top, az), tone + 0.05));
            }
            Slot::Cell(support) => {
                let (hx, hz) = extent(k);
                let base = k.1 as f32 * h;
                let y0 = bottom(k);
                let y1 = if city { y0 + deck } else { ceiling(k) };
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
                            // Posts at the corners, down to the cell below (or
                            // into the ground).
                            _ => {
                                let foot = if k.1 == 0 { -FOUNDATION } else { surface(below) };
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
                        let target = (k.0, k.1 - n, k.2);
                        let foot = if k.1 - n < 0 { -FOUNDATION } else { surface(target) };
                        let pier = (w * 0.06 * lk.sqrt()).clamp(1.0, w * 0.3);
                        out.push(span(c + Vec3::new(-pier, foot, -pier), c + Vec3::new(pier, y0, pier), structure));
                    }
                    Support::Cantilever(d) => {
                        // Two arms under the cell, from the parent's middle
                        // to the cell's far edge.
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
                        // Near the deck's edge in a city, clear of what is
                        // built on it.
                        let out_to = if city { 0.95 } else { 0.75 };
                        // From a beam: rods along its line, to its underside.
                        // From an outrigger cell: at the corners, to its
                        // underside.
                        let (holder, points) = match l.slots.get(&above) {
                            Some(Slot::Beam(along_x)) => {
                                let points = if *along_x {
                                    vec![Vec3::X * (hx * out_to), Vec3::X * (-hx * out_to)]
                                } else {
                                    vec![Vec3::Z * (hz * out_to), Vec3::Z * (-hz * out_to)]
                                };
                                ((k.1 + n + 1) as f32 * h - beam_depth(load_of(above)), points)
                            }
                            _ => {
                                let (ax, az) = extent(above);
                                let (px, pz) = (hx.min(ax) * out_to, hz.min(az) * out_to);
                                (bottom(above), vec![Vec3::new(px, 0.0, pz), Vec3::new(-px, 0.0, pz), Vec3::new(px, 0.0, -pz), Vec3::new(-px, 0.0, -pz)])
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
    // Ramps: colossal wedges from the ground up to decks near the edge, each
    // meeting a deck's outer side, turned off the grid.
    if s.ramps > 0 {
        let reach = |k: &Key| (k.0 as f32).hypot(k.2 as f32);
        let mut outer: Vec<Key> = groups.iter().flat_map(|g| g.0.iter().copied()).filter(|k| k.1 >= 2).collect();
        outer.sort_by(|a, b| reach(b).total_cmp(&reach(a)).then(a.cmp(b)));
        outer.truncate((outer.len() / 3).max(s.ramps as usize));
        let ex = w * 0.5 - margin;
        let mut used = std::collections::HashSet::new();
        for e in 0..s.ramps {
            if outer.is_empty() {
                break;
            }
            let pick = (r(e as i32, 11, 300) * outer.len() as f32) as usize % outer.len();
            let k = outer[pick];
            if !used.insert((k.0, k.2)) {
                continue;
            }
            // The deck's outer side, the way the cell lies from the middle.
            let away = Vec2::new(k.0 as f32, k.2 as f32).normalize_or(Vec2::X);
            let side = if away.x.abs() > away.y.abs() { Vec2::new(away.x.signum(), 0.0) } else { Vec2::new(0.0, away.y.signum()) };
            let edge = at(k.0, k.2) + Vec3::new(side.x, 0.0, side.y) * ex;
            let top = surface(k);
            // Off the grid: turned a little from straight out.
            let out_angle = side.y.atan2(side.x) + (r(e as i32, 12, 301) - 0.5) * 1.1;
            let slope = 0.45 + r(e as i32, 13, 302) * 0.25;
            let run = top / slope.tan();
            let width = 40.0 + r(e as i32, 14, 303) * 50.0;
            let mut local = Vec::new();
            crate::mega::wedge(&mut local, run, top, width, s.ramp_grain, tone + 0.02);
            // Its +x (rising) points back in towards the deck.
            let heading_in = out_angle + std::f32::consts::PI;
            let turn = Quat::from_rotation_y(-heading_in);
            let foot = edge - turn * Vec3::X * run;
            out.extend(local.into_iter().map(|q| Solid { center: foot + turn * q.center, rotation: turn * q.rotation, ..q }));
        }
    }
    // Round: bend the lattice into its rings, each piece placed and turned
    // at its own radius and stretched across to its row's width there.
    if let Some((_, step)) = rings {
        out = out
            .into_iter()
            .map(|q| {
                let (center, angle, radius) = polar(q.center);
                let mut half = q.half;
                if q.rotation == Quat::IDENTITY && !q.round {
                    half.z *= radius * step / w;
                }
                Solid { center, half, rotation: Quat::from_rotation_y(-angle) * q.rotation, ..q }
            })
            .collect();
        out.extend(placed);
    }
    // The reactor in the void, and cables from it to the cells nearest it.
    if s.reactor {
        let void = match s.radial {
            Some((inner, _)) => inner,
            None => s.void * 0.5,
        };
        let radius = void * 0.55;
        let height = l.ny as f32 * h * 1.25;
        out.extend(crate::reactor::reactor(Vec3::ZERO, radius, height, tone));
        // The cells nearest the void, and where each faces it.
        let mut near: Vec<(Vec3, Vec3)> = Vec::new();
        for g in &groups {
            for &k in &g.0 {
                let c = at(k.0, k.2);
                let (point, facing) = match rings {
                    Some(_) if k.0 == -l.nx => {
                        let (p, _, _) = polar(c - Vec3::X * (w * 0.5 - margin) + Vec3::Y * (bottom(k) - 1.0));
                        (p, -p.with_y(0.0).normalize_or_zero())
                    }
                    None if (k.0 as f32).hypot(k.2 as f32) < l.void + 1.6 => {
                        let to = (-c).with_y(0.0).normalize_or_zero();
                        let side = if to.x.abs() > to.z.abs() { Vec3::new(to.x.signum(), 0.0, 0.0) } else { Vec3::new(0.0, 0.0, to.z.signum()) };
                        (c + side * (w * 0.5 - margin) + Vec3::Y * (bottom(k) - 1.0), side)
                    }
                    _ => continue,
                };
                if k.1 >= 1 {
                    near.push((point, facing));
                }
            }
        }
        near.sort_by(|a, b| a.0.y.total_cmp(&b.0.y).then(a.0.x.total_cmp(&b.0.x)));
        for e in 0..s.cables.min(near.len() as u32) {
            let (point, facing) = near[(r(e as i32, 17, 400) * near.len() as f32) as usize % near.len()];
            // From the drum, a little above, round towards the cell.
            let toward = (point.with_y(0.0)).normalize_or_zero();
            let toward = if toward == Vec3::ZERO { -facing } else { toward };
            let from = toward * radius + Vec3::Y * (point.y + 30.0 + r(e as i32, 18, 401) * 120.0).min(height * 0.8);
            let span = (point - from).length();
            let thick = 2.5 + r(e as i32, 19, 402) * 3.5;
            out.extend(crate::reactor::cable(from, point, span * (0.06 + 0.06 * r(e as i32, 20, 403)), thick, tone - 0.03));
        }
    }
    let rot = Quat::from_rotation_y((-dir.y).atan2(dir.x));
    out.into_iter().map(|s| Solid { center: origin + rot * s.center, rotation: rot * s.rotation, ..s }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            cell: (24.0, 18.0),
            levels: 8,
            fill: 0.35,
            cluster: 0.5,
            thin: 0.3,
            hang: 0.4,
            vary: 0.2,
            cores: 0.5,
            bars: 0.5,
            content: 0,
            ramps: 0,
            ramp_grain: crate::mega::Grain::None,
            void: 0.0,
            radial: None,
            reactor: false,
            cables: 0,
        }
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
            // A city: decks with complexes on them, nothing below a deck.
            let city = Settings { cell: (96.0, 45.0), levels: 6, content: 6, ..settings() };
            let solids = lattice(Vec3::ZERO, Vec2::X, Vec2::splat(480.0), city, 0.15, seed);
            assert!(solids.len() > 5 * cells, "{}", solids.len());
        }
    }
}
