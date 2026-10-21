//! Tilted-native ground: an experiment (the user's idea, 2026-10-21). A land
//! where jagged tilted formations are the norm and flat ground the
//! exception, as Valheim's Mistlands; not tilted things added on flat
//! plates, which read as objects, or, as landscape, as a clumped mass.
//!
//! Its grain first, what makes tilted parts read as one landscape rather
//! than a jumble: fault blocks (big Voronoi cells), each with its strike,
//! the way its ridges run; in each, cells long along the strike (Voronoi in a
//! frame stretched along it), each row a ridge, each cell a sheet rising
//! across the strike from its low edge to its escarpment, every sheet in a
//! block dipping the same way. Across the grain, a sawtooth: slow dip
//! slopes, each ending in a cliff. Where blocks meet, faults: the grain
//! turns at once. Built as solids (`patch`) to walk in the game (`--set
//! tilted on`); the `tilted` example draws it from above.
//!
//! Irregular (the user: "room for more irregularity"): ridges of uneven
//! width (each row shifted across its own way), bending gently along their
//! length (each block's frame bent, a sine along it), an uneven sawtooth
//! (dip and height varying sheet to sheet), and now and then a sheet slumped,
//! dropped and tipped the other way, breaking its ridge.
//!
//! Pockets of order (the user's, after the spire site): its own rules
//! tightening, not a pattern laid on it; towards a pocket's core, rows even
//! out, ends line up, the tilt levels off, the sheets are cut into square
//! pieces, and they settle onto terraces stepping up every way from a sunken
//! floor: a stepped bowl in the jagged land, merging back into it at its
//! edge as its steps shrink with its order.
//!
//! Its escarpments overhang: each leans out over the drop, a rock face from
//! the ground beyond up to its top's edge reaching out.

use glam::{DVec2, DVec3};

use crate::noise::{Fbm, hash01};

/// Fault blocks across the world, per side (each some 1.6 km).
pub const BLOCKS: i32 = 10;
/// Cells: how far apart along the strike and across it (m); how far a cell's
/// middle strays from its place along, and across (of the spacing); how far
/// each row is shifted across, its own way (of the spacing: uneven widths).
const CELL: (f64, f64) = (700.0, 70.0);
const JITTER: (f64, f64) = (0.9, 0.12);
const ROW_SHIFT: f64 = 0.4;
/// Ridges bending: in so many of the blocks (bent in all, in step, they read
/// as a swirl, the user), by up to so many rows either way, once in so many
/// cells along.
const BEND: (f64, f64, f64) = (0.5, 1.0, 3.0);
/// Sheets: how steep their dip slope (degrees, at least and at most); how
/// far they plunge along the strike (degrees, either way, at most); raised
/// or sunk, each its own (m, either way); the land's broad relief under
/// them (m, either way).
const DIP: (f64, f64) = (8.0, 45.0);
const PLUNGE: f64 = 6.0;
const OFFSET: f64 = 14.0;
const RELIEF: f64 = 40.0;
/// Slumped sheets: the chance of one, how far it is dropped (m, at least and
/// at most), and its dip turned the other way, so much of it.
const SLUMP: (f64, (f64, f64), f64) = (0.1, (15.0, 40.0), 0.6);
/// How far apart (m, along the strike) the points of a sheet's outline, so
/// its top follows its ridge's bend.
const STEP: f64 = 30.0;
/// Overhangs: an escarpment (an edge where the ground beyond drops this far,
/// m, at least) leans out, its top reaching out so much of the drop, up to
/// so far (m), its face rising from its foot this far (m) under the ground
/// beyond. (A lip on a sheer cliff, the same band along every one, read as a
/// strip added to it, the user.)
const OVERHANG: (f64, f64, f64) = (8.0, 0.4, 12.0);
const FOOT: f64 = 0.5;
/// How far (m) a leaning face stands clear of the ground under it, at least.
const CLEAR: f64 = 1.0;
/// Pockets of order (the user's, after the spire site's transition from
/// wild plates to a grid): the chance a block has one, how far from its
/// middle it may be (m, either way), its core (m, its radius: wholly
/// ordered) and how far its order reaches (m, fading); its terraces: so many
/// rows (and pieces along them) level round its middle, each out from them
/// a step of so much (m) higher, its floor sunk so far (m) under the land
/// round it; and its sheets cut along into pieces so long (m), as its rows
/// are wide: its order a grid, a stepped bowl (rows alone, it was a stretch
/// of ridges flattened, open along them).
const POCKET: (f64, f64, f64, f64) = (0.35, 300.0, 150.0, 450.0);
const TERRACES: (f64, f64, f64, f64) = (1.0, 6.0, 30.0, 70.0);

/// The ground's grain.
pub struct Tilted {
    size: f64,
    seed: u32,
    relief: Fbm,
}

/// A pocket of order: its middle (in the world, and in its block's cells'
/// frame), and the height of its floor.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pocket {
    middle: DVec2,
    cells: DVec2,
    floor: f64,
}

/// A fault block's frame: its middle, its strike (a unit) and across it,
/// which way its sheets rise across it (+1 or -1), its bend (rows, radians a
/// cell along, radians), and its pocket of order, if it has one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    origin: DVec2,
    strike: DVec2,
    across: DVec2,
    side: f64,
    bend: (f64, f64, f64),
    pocket: Option<Pocket>,
}

impl Frame {
    /// How ordered the ground is at a point (0 wild, 1 in its pocket's
    /// core), and its pocket.
    fn order(&self, p: DVec2) -> (f64, Option<&Pocket>) {
        let Some(pocket) = &self.pocket else { return (0.0, None) };
        let t = ((POCKET.3 - p.distance(pocket.middle)) / (POCKET.3 - POCKET.2)).clamp(0.0, 1.0);
        (t * t * (3.0 - 2.0 * t), Some(pocket))
    }

    fn bend(&self, x: f64) -> f64 {
        self.bend.0 * (x * self.bend.1 + self.bend.2).sin()
    }

    /// A point in its cells' frame (stretched, unbent: cells round, rows
    /// straight).
    fn to_cells(&self, p: DVec2) -> DVec2 {
        let d = p - self.origin;
        let x = d.dot(self.strike) / CELL.0;
        DVec2::new(x, d.dot(self.across) / CELL.1 - self.bend(x))
    }

    /// And back, into the world.
    fn to_world(&self, q: DVec2) -> DVec2 {
        self.origin + self.strike * (q.x * CELL.0) + self.across * ((q.y + self.bend(q.x)) * CELL.1)
    }
}

/// A sheet: its fault block's frame, its own place in the block's cells, its
/// middle (in the world, and in its cells' frame), how steep (rise per
/// metre) across and along, and its height at its middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    frame: Frame,
    pub block: (i32, i32),
    pub id: (i32, i32),
    pub middle: DVec2,
    site: DVec2,
    pub dip: f64,
    pub plunge: f64,
    pub base: f64,
    /// Which piece of it along (in a pocket of order: see `TERRACES`), if
    /// cut; else, wherever a point falls.
    piece: Option<i32>,
}

impl Tilted {
    pub fn new(size: f32, seed: u32) -> Self {
        Self { size: size as f64, seed: seed.wrapping_mul(0x2545_f491) ^ 0x7117, relief: Fbm::new(1024.0, 2, 0.5, seed ^ 0x7e17) }
    }

    fn r(&self, a: i32, b: i32, k: i32) -> f64 {
        hash01(a, b, k, self.seed) as f64
    }

    /// A fault block's middle (its Voronoi site).
    fn block_site(&self, b: (i32, i32)) -> DVec2 {
        let s = self.size / BLOCKS as f64;
        let w = (b.0.rem_euclid(BLOCKS), b.1.rem_euclid(BLOCKS));
        DVec2::new((b.0 as f64 + 0.5 + (self.r(w.0, w.1, 1) - 0.5) * 0.7) * s, (b.1 as f64 + 0.5 + (self.r(w.0, w.1, 2) - 0.5) * 0.7) * s)
    }

    /// The fault block a point is in.
    pub fn block_at(&self, p: DVec2) -> (i32, i32) {
        let s = self.size / BLOCKS as f64;
        let (cx, cz) = ((p.x / s).floor() as i32, (p.y / s).floor() as i32);
        let mut best = ((cx, cz), f64::MAX);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let b = (cx + dx, cz + dz);
                let d = self.block_site(b).distance_squared(p);
                if d < best.1 {
                    best = (b, d);
                }
            }
        }
        best.0
    }

    /// A block's frame.
    fn frame(&self, b: (i32, i32)) -> Frame {
        let w = (b.0.rem_euclid(BLOCKS), b.1.rem_euclid(BLOCKS));
        let a = self.r(w.0, w.1, 3) * std::f64::consts::PI;
        let strike = DVec2::new(a.cos(), a.sin());
        let side = if self.r(w.0, w.1, 4) < 0.5 { -1.0 } else { 1.0 };
        let bent = if self.r(w.0, w.1, 15) < BEND.0 { BEND.1 * (0.4 + 0.6 * self.r(w.0, w.1, 9)) } else { 0.0 };
        let bend = (bent, std::f64::consts::TAU / BEND.2, self.r(w.0, w.1, 10) * std::f64::consts::TAU);
        let mut frame = Frame { origin: self.block_site(b), strike, across: DVec2::new(-strike.y, strike.x), side, bend, pocket: None };
        if self.r(w.0, w.1, 16) < POCKET.0 {
            let middle = frame.origin + DVec2::new(self.r(w.0, w.1, 17) - 0.5, self.r(w.0, w.1, 18) - 0.5) * 2.0 * POCKET.1;
            let floor = self.relief.sample2(middle.x as f32, middle.y as f32, self.size as f32) as f64 * RELIEF - TERRACES.2;
            frame.pocket = Some(Pocket { middle, cells: frame.to_cells(middle), floor });
        }
        frame
    }

    /// Sheet `id`'s middle in its block's cells' frame: its row shifted
    /// across its own way, itself strayed a little; in a pocket of order,
    /// less so, none in its core (its rows even, its ends in line).
    fn site(&self, block: (i32, i32), id: (i32, i32)) -> DVec2 {
        let (i, j) = id;
        let k = (block.0 * 977 + i, block.1 * 977 + j);
        let frame = self.frame(block);
        let wild = 1.0 - frame.order(frame.to_world(DVec2::new(i as f64 + 0.5, j as f64 + 0.5))).0;
        let row = (self.r(block.0 * 977, block.1 * 977 + j, 11) - 0.5) * 2.0 * ROW_SHIFT;
        DVec2::new(i as f64 + 0.5 + (self.r(k.0, k.1, 5) - 0.5) * JITTER.0 * wild, j as f64 + 0.5 + (row + (self.r(k.0, k.1, 6) - 0.5) * JITTER.1) * wild)
    }

    /// The pockets of order within `r` m of a point: their middles (for
    /// finding them).
    pub fn pockets_near(&self, x: f64, z: f64, r: f64) -> Vec<(f64, f64)> {
        let s = self.size / BLOCKS as f64;
        let mut found = Vec::new();
        for bz in ((z - r) / s).floor() as i32 - 1..=((z + r) / s).floor() as i32 + 1 {
            for bx in ((x - r) / s).floor() as i32 - 1..=((x + r) / s).floor() as i32 + 1 {
                if let Some(p) = self.frame((bx, bz)).pocket
                    && p.middle.distance(DVec2::new(x, z)) <= r
                    && self.block_at(p.middle) == (bx, bz)
                {
                    found.push((p.middle.x, p.middle.y));
                }
            }
        }
        found
    }

    /// Sheet `id` of `block`.
    fn sheet(&self, block: (i32, i32), id: (i32, i32)) -> Sheet {
        let frame = self.frame(block);
        let site = self.site(block, id);
        let middle = frame.to_world(site);
        let k = (block.0 * 977 + id.0, block.1 * 977 + id.1);
        let mut dip = (DIP.0 + (DIP.1 - DIP.0) * self.r(k.0, k.1, 7)).to_radians().tan();
        let plunge = ((self.r(k.0, k.1, 8) - 0.5) * 2.0 * PLUNGE).to_radians().tan();
        let mut base = self.relief.sample2(middle.x as f32, middle.y as f32, self.size as f32) as f64 * RELIEF + (self.r(k.0, k.1, 12) - 0.5) * 2.0 * OFFSET;
        // (Now and then slumped: dropped, tipped the other way.)
        if self.r(k.0, k.1, 13) < SLUMP.0 {
            base -= SLUMP.1.0 + (SLUMP.1.1 - SLUMP.1.0) * self.r(k.0, k.1, 14);
            dip = -dip * SLUMP.2;
        }
        Sheet { frame, block, id, middle, site, dip, plunge, base, piece: None }
    }

    /// The sheet a point is on.
    pub fn sheet_at(&self, p: DVec2) -> Sheet {
        let block = self.block_at(p);
        let q = self.frame(block).to_cells(p);
        let (i0, j0) = (q.x.floor() as i32, q.y.floor() as i32);
        let mut best = ((0, 0), f64::MAX);
        for j in j0 - 2..=j0 + 2 {
            for i in i0 - 2..=i0 + 1 {
                let dist = self.site(block, (i, j)).distance_squared(q);
                if dist < best.1 {
                    best = ((i, j), dist);
                }
            }
        }
        self.sheet(block, best.0)
    }

    /// The ground's height at a point: its sheet's dip slope there.
    pub fn height(&self, p: DVec2) -> f64 {
        self.sheet_at(p).top(p)
    }

    /// The same, at (`x`, `z`).
    pub fn height_xz(&self, x: f64, z: f64) -> f64 {
        self.height(DVec2::new(x, z))
    }

    /// A sheet's outline (counter-clockwise, in the world, a point every
    /// `STEP` m or so): its cell in its block's cells' frame, bent into the
    /// world, cut at the faults (its block's own Voronoi cell).
    fn outline(&self, block: (i32, i32), id: (i32, i32)) -> Vec<DVec2> {
        let frame = self.frame(block);
        let s = self.site(block, id);
        let mut q = vec![s + DVec2::new(-4.0, -3.0), s + DVec2::new(4.0, -3.0), s + DVec2::new(4.0, 3.0), s + DVec2::new(-4.0, 3.0)];
        for dj in -3..=3 {
            for di in -4..=4 {
                if (di, dj) == (0, 0) {
                    continue;
                }
                let o = self.site(block, (id.0 + di, id.1 + dj));
                q = clip(&q, (s + o) * 0.5, o - s);
            }
        }
        // (A point wherever an edge crosses a line of a grid `STEP` m apart
        // along the strike, so it bends with its ridge; on one grid, two
        // sheets' shared edge is cut at the same points from either side,
        // and their tops meet without a crack.)
        let grid = STEP / CELL.0;
        let mut fine = Vec::new();
        for k in 0..q.len() {
            let (a, b) = (q[k], q[(k + 1) % q.len()]);
            fine.push(a);
            let (lo, hi) = (a.x.min(b.x), a.x.max(b.x));
            let mut cuts: Vec<f64> = ((lo / grid).floor() as i64 + 1..=(hi / grid).ceil() as i64 - 1).map(|g| g as f64 * grid).filter(|&x| x > lo && x < hi).collect();
            if b.x < a.x {
                cuts.reverse();
            }
            fine.extend(cuts.iter().map(|&x| a.lerp(b, (x - a.x) / (b.x - a.x))));
        }
        let mut w: Vec<DVec2> = fine.iter().map(|&p| frame.to_world(p)).collect();
        let b = frame.origin;
        for dz in -2..=2 {
            for dx in -2..=2 {
                if (dx, dz) == (0, 0) || w.len() < 3 {
                    continue;
                }
                let o = self.block_site((block.0 + dx, block.1 + dz));
                w = clip(&w, (b + o) * 0.5, o - b);
            }
        }
        if area(&w) < 0.0 {
            w.reverse();
        }
        w
    }

    /// The sheets of a square patch (`centre`, `span` m across), raised by
    /// `lift` (m), each a solid down to `floor`: its top, following its
    /// ridge's bend, its sheer sides (the sawtooth's escarpments), cut at the
    /// patch's edge.
    pub fn patch(&self, centre: (f64, f64), span: f64, lift: f64, floor: f64) -> Vec<SheetMesh> {
        let (centre, half) = (DVec2::new(centre.0, centre.1), span * 0.5);
        let (lo, hi) = (centre - DVec2::splat(half), centre + DVec2::splat(half));
        let s = self.size / BLOCKS as f64;
        let mut out = Vec::new();
        for bz in ((lo.y / s).floor() as i32 - 1)..=((hi.y / s).floor() as i32 + 1) {
            for bx in ((lo.x / s).floor() as i32 - 1)..=((hi.x / s).floor() as i32 + 1) {
                let block = (bx, bz);
                let frame = self.frame(block);
                // (Its cells in reach of the patch: the patch's corners in
                // its cells' frame, and a margin, its bend too.)
                let corners = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)].map(|c| frame.to_cells(c));
                let (qmin, qmax) = corners.iter().fold((DVec2::MAX, DVec2::MIN), |(a, b), &c| (a.min(c), b.max(c)));
                let margin = BEND.1.ceil() as i32 + 2;
                for j in (qmin.y.floor() as i32 - margin)..=(qmax.y.ceil() as i32 + margin) {
                    for i in (qmin.x.floor() as i32 - 2)..=(qmax.x.ceil() as i32 + 2) {
                        let mut o = self.outline(block, (i, j));
                        for (at, normal) in [(lo, DVec2::NEG_X), (lo, DVec2::NEG_Y), (hi, DVec2::X), (hi, DVec2::Y)] {
                            if o.len() >= 3 {
                                o = clip(&o, at, normal);
                            }
                        }
                        if o.len() < 3 || area(&o) < 1.0 {
                            continue;
                        }
                        let sheet = self.sheet(block, (i, j));
                        for (piece, part) in sheet.pieces(&o) {
                            out.push(Sheet { piece, ..sheet }.solid(&part, lift, floor, self));
                        }
                    }
                }
            }
        }
        out
    }
}

impl Sheet {
    /// Its dip slope's height at a point (across and along its ridge, as it
    /// bends); in a pocket of order, levelling to its row's terrace (its
    /// floor's height round its middle row, a step higher each row out).
    pub fn top(&self, p: DVec2) -> f64 {
        let cells = self.frame.to_cells(p);
        let q = cells - self.site;
        let wild = self.base + self.dip * self.frame.side * q.y * CELL.1 + self.plunge * q.x * CELL.0;
        let (order, pocket) = self.frame.order(p);
        let Some(pocket) = pocket.filter(|_| order > 0.0) else { return wild };
        // (Its terrace: steps out from the pocket's middle, by rows across
        // and by pieces along, whichever is further: a square bowl.)
        let piece = TERRACES.3 / CELL.0;
        let k = self.piece.unwrap_or((cells.x / piece).floor() as i32);
        let across = (self.id.1 as f64 + 0.5 - pocket.cells.y).abs().round();
        let along = (((k as f64 + 0.5) * piece - pocket.cells.x).abs() / piece).round();
        let terrace = pocket.floor + TERRACES.1 * (across.max(along) - TERRACES.0).max(0.0);
        wild + (terrace - wild) * order
    }

    /// Its outline cut along into pieces (`TERRACES.3` long), where a pocket
    /// of order reaches it; else whole.
    fn pieces(&self, outline: &[DVec2]) -> Vec<(Option<i32>, Vec<DVec2>)> {
        let near = self.frame.pocket.is_some_and(|pk| outline.iter().any(|p| p.distance(pk.middle) < POCKET.3 + TERRACES.3));
        if !near {
            return vec![(None, outline.to_vec())];
        }
        let piece = TERRACES.3 / CELL.0;
        let xs: Vec<f64> = outline.iter().map(|&p| self.frame.to_cells(p).x).collect();
        let (lo, hi) = xs.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        let mut out = Vec::new();
        for k in (lo / piece).floor() as i32..=(hi / piece).floor() as i32 {
            let from = self.frame.origin + self.frame.strike * (k as f64 * piece * CELL.0);
            let to = self.frame.origin + self.frame.strike * ((k + 1) as f64 * piece * CELL.0);
            let mut part = clip(outline, from, -self.frame.strike);
            if part.len() >= 3 {
                part = clip(&part, to, self.frame.strike);
            }
            if part.len() >= 3 && area(&part).abs() > 1e-3 {
                out.push((Some(k), part));
            }
        }
        out
    }

    /// Its solid over `outline`: its top (raised by `lift`), in facets
    /// (ear-clipped: a fan from its middle folded over itself where it
    /// bends); its sides down to `floor`; and each escarpment (an edge where
    /// `world`'s ground beyond drops away) leaning out over the drop.
    fn solid(&self, outline: &[DVec2], lift: f64, floor: f64, world: &Tilted) -> SheetMesh {
        let mut m = SheetMesh { middle: self.middle, positions: Vec::new(), normals: Vec::new(), indices: Vec::new(), tones: Vec::new() };
        let top = |p: DVec2| DVec3::new(p.x, self.top(p) + lift, p.y);
        let at = |p: DVec2, y: f64| DVec3::new(p.x, y + lift, p.y);
        let n = outline.len();
        // (Counter-clockwise seen from above is clockwise in x, z: wound
        // so, each faces up.)
        // (Shaded smooth where it curves: its tops, its leaning faces;
        // flat-shaded, a curved face broke into saw-teeth of light and
        // dark, the user. Its sheer cliffs are flat, crisp.)
        let slope = |p: DVec2| {
            let (dx, dz) = (DVec2::X * 0.5, DVec2::Y * 0.5);
            DVec3::new(-(self.top(p + dx) - self.top(p - dx)), 1.0, -(self.top(p + dz) - self.top(p - dz))).normalize()
        };
        for t in triangulate(outline) {
            let c = [outline[t[0]], outline[t[1]], outline[t[2]]];
            smooth(&mut m, c.map(top), c.map(slope), DVec3::Y);
        }
        // Its edges: which way out of it, and how far the ground beyond drops
        // below its top (an escarpment, if far).
        let out: Vec<DVec2> = (0..n).map(|i| (outline[(i + 1) % n] - outline[i]).perp().normalize_or_zero() * -1.0).collect();
        let drop: Vec<f64> = (0..n)
            .map(|i| {
                let mid = (outline[i] + outline[(i + 1) % n]) * 0.5;
                self.top(mid) - world.height(mid + out[i] * 0.5)
            })
            .collect();
        let high: Vec<bool> = drop.iter().map(|&d| d >= OVERHANG.0).collect();
        // Its points along an escarpment: which way out, how far its top
        // reaches out over the drop (none where a run of escarpment ends:
        // there it stands sheer), and where its face's foot is, just under
        // the ground beyond.
        let mut reach = vec![0.0; n];
        let mut way = vec![DVec2::ZERO; n];
        let mut foot = vec![0.0; n];
        for i in 0..n {
            let prev = (i + n - 1) % n;
            let d = drop[prev].min(drop[i]).max(0.0);
            let h = hash01((outline[i].x * 0.5).floor() as i32, (outline[i].y * 0.5).floor() as i32, 2, world.seed) as f64;
            way[i] = (out[prev] + out[i]).normalize_or(out[i]);
            foot[i] = (world.height(outline[i] + way[i] * 0.3) - FOOT).min(self.top(outline[i]));
            if high[prev] && high[i] {
                reach[i] = (d * OVERHANG.1).min(OVERHANG.2) * (0.4 + 0.6 * h);
            }
        }
        // (Each face clear of the ground under it, at its top edge and
        // halfway up, at its points and between them, by `CLEAR`; where not,
        // reaching half as far, again, then not at all. Its escarpment judged
        // at its middle, the ground beyond could rise along it, or a third
        // sheet under its reach.)
        let under = |i: usize, r: f64| {
            let p = outline[i];
            let edge = p + way[i] * r;
            (edge, self.top(edge), p + way[i] * r * 0.5, (self.top(edge) + foot[i]) * 0.5)
        };
        let clear = |q: DVec2, y: f64| y >= world.height(q) + CLEAR;
        for _ in 0..5 {
            let mut changed = false;
            for i in 0..n {
                let j = (i + 1) % n;
                let (ea, ya, ma, ha) = under(i, reach[i]);
                let (eb, yb, mb, hb) = under(j, reach[j]);
                let fits = |r: f64, e: DVec2, y: f64, m: DVec2, h: f64| r == 0.0 || (clear(e, y) && clear(m, h));
                if !fits(reach[i], ea, ya, ma, ha) {
                    reach[i] = if reach[i] < 1.0 { 0.0 } else { reach[i] * 0.5 };
                    changed = true;
                } else if (reach[i] > 0.0 || reach[j] > 0.0) && !(clear((ea + eb) * 0.5, (ya + yb) * 0.5) && clear((ma + mb) * 0.5, (ha + hb) * 0.5)) {
                    for k in [i, j] {
                        reach[k] = if reach[k] < 1.0 { 0.0 } else { reach[k] * 0.5 };
                    }
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for i in 0..n {
            let j = (i + 1) % n;
            let (a, b) = (outline[i], outline[j]);
            if a.distance_squared(b) < 1e-8 {
                continue;
            }
            let (fa, fb) = (at(a, floor - lift), at(b, floor - lift));
            let outward = DVec3::new(out[i].x, 0.0, out[i].y);
            if !high[i] {
                face(&mut m, [top(a), fb, top(b)], outward);
                face(&mut m, [top(a), fa, fb], outward);
                continue;
            }
            // Its escarpment leaning out: its top going on out (as its dip
            // slope), its face from there down to its foot in the ground
            // beyond (shaded smooth: square to the edge and to the lean), and
            // on down under it, sheer.
            let (a2, b2) = (a + way[i] * reach[i], b + way[j] * reach[j]);
            let (ta2, tb2) = (top(a2), top(b2));
            let (ga, gb) = (at(a, foot[i]), at(b, foot[j]));
            smooth(&mut m, [top(a), top(b), tb2], [slope(a), slope(b), slope(b2)], DVec3::Y);
            smooth(&mut m, [top(a), tb2, ta2], [slope(a), slope(b2), slope(a2)], DVec3::Y);
            let along = DVec3::new(b.x - a.x, 0.0, b.y - a.y);
            let lean = |up: DVec3, low: DVec3| {
                let n = (up - low).cross(along).normalize_or(outward);
                if n.dot(outward) < 0.0 { -n } else { n }
            };
            let (na, nb) = (lean(ta2, ga), lean(tb2, gb));
            smooth(&mut m, [ta2, tb2, gb], [na, nb, nb], outward);
            smooth(&mut m, [ta2, gb, ga], [na, nb, na], outward);
            face(&mut m, [ga, gb, fb], outward);
            face(&mut m, [ga, fb, fa], outward);
        }
        // (Each sheet its own tone, as the plates have.)
        let tone = 0.14 + 0.1 * hash01(self.block.0 * 977 + self.id.0, self.block.1 * 977 + self.id.1, 7, world.seed);
        m.tones.resize(m.positions.len(), tone);
        m
    }

    /// Its overhangs (for testing): each point of a leaning face out over
    /// the drop (beyond its outline, not on it), and its height.
    #[cfg(test)]
    fn lips(&self, outline: &[DVec2], world: &Tilted) -> Vec<(DVec2, f64)> {
        let mesh = self.solid(outline, 0.0, -100.0, world);
        let n = outline.len();
        let beyond = |p: DVec2| {
            let near = (0..n).map(|i| {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                let t = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-12)).clamp(0.0, 1.0);
                p.distance(a + (b - a) * t)
            });
            let inside = (0..n).filter(|&i| {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x)
            }).count() % 2 == 1;
            !inside && near.fold(f64::MAX, f64::min) > 0.2
        };
        mesh.indices
            .chunks(3)
            .filter(|t| mesh.normals[t[0] as usize][1] < -0.02)
            .flat_map(|t| t.iter().map(|&k| mesh.positions[k as usize]).collect::<Vec<_>>())
            .map(|p| (DVec2::new(p[0] as f64, p[2] as f64), p[1] as f64))
            .filter(|&(p, _)| beyond(p))
            .collect()
    }
}

/// A triangle facing (more or less) `hint`, shaded smooth: its corners'
/// own normals (`normals`, each turned to face as it does); none if it has
/// no area.
pub(crate) fn smooth(m: &mut SheetMesh, corners: [DVec3; 3], normals: [DVec3; 3], hint: DVec3) {
    let geometric = (corners[2] - corners[0]).cross(corners[1] - corners[0]);
    if geometric.length_squared() < 1e-12 {
        return;
    }
    let (order, facing) = if geometric.dot(hint) < 0.0 { ([0u32, 1, 2], -geometric) } else { ([0, 2, 1], geometric) };
    let k = m.positions.len() as u32;
    for (c, n) in corners.iter().zip(normals) {
        m.positions.push(c.as_vec3().to_array());
        let n = if n.dot(facing) < 0.0 { -n } else { n };
        m.normals.push(n.as_vec3().to_array());
    }
    m.indices.extend(order.map(|o| k + o));
}

/// A triangle facing (more or less) `hint`, its normal its own; none if it
/// has no area.
pub(crate) fn face(m: &mut SheetMesh, corners: [DVec3; 3], hint: DVec3) {
    let mut normal = (corners[2] - corners[0]).cross(corners[1] - corners[0]);
    if normal.length_squared() < 1e-12 {
        return;
    }
    let mut order = [0u32, 2, 1];
    if normal.dot(hint) < 0.0 {
        normal = -normal;
        order = [0, 1, 2];
    }
    let normal = normal.normalize().as_vec3().to_array();
    let k = m.positions.len() as u32;
    for c in corners {
        m.positions.push(c.as_vec3().to_array());
        m.normals.push(normal);
    }
    m.indices.extend(order.map(|o| k + o));
}

/// A sheet built: its middle, and its mesh in the world (triangles wound
/// counter-clockwise seen from outside), with each point's tone (its
/// albedo, grey).
pub struct SheetMesh {
    pub middle: DVec2,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub tones: Vec<f32>,
}

/// A polygon cut by a line (through `at`, square to `normal`): what lies on
/// its near side.
pub(crate) fn clip(p: &[DVec2], at: DVec2, normal: DVec2) -> Vec<DVec2> {
    let side = |q: DVec2| (q - at).dot(normal);
    let mut out = Vec::new();
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa <= 0.0 {
            out.push(a);
        }
        if (sa <= 0.0) != (sb <= 0.0) {
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    out
}

/// A simple polygon's triangles (ear clipping), counter-clockwise; if it
/// is not simple, the rest as a fan.
pub(crate) fn triangulate(p: &[DVec2]) -> Vec<[usize; 3]> {
    let mut idx: Vec<usize> = (0..p.len()).collect();
    if area(p) < 0.0 {
        idx.reverse();
    }
    let inside = |q: DVec2, a: DVec2, b: DVec2, c: DVec2| (b - a).perp_dot(q - a) > 0.0 && (c - b).perp_dot(q - b) > 0.0 && (a - c).perp_dot(q - c) > 0.0;
    let mut tris = Vec::new();
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&i| {
            let (a, b, c) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            (p[b] - p[a]).perp_dot(p[c] - p[b]) > 1e-9 && !idx.iter().any(|&j| j != a && j != b && j != c && inside(p[j], p[a], p[b], p[c]))
        });
        let Some(i) = ear else {
            for k in 1..m - 1 {
                tris.push([idx[0], idx[k], idx[k + 1]]);
            }
            return tris;
        };
        tris.push([idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]]);
        idx.remove(i);
    }
    tris.push([idx[0], idx[1], idx[2]]);
    tris
}

/// A polygon's signed area (counter-clockwise, positive).
pub(crate) fn area(p: &[DVec2]) -> f64 {
    (0..p.len()).map(|i| p[i].perp_dot(p[(i + 1) % p.len()])).sum::<f64>() * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheets_tile() {
        // Round the world's middle, and round a pocket of order.
        tile(DVec2::new(8192.0, 8192.0));
        tile(DVec2::new(9089.0, 7136.0));
    }

    /// A patch's sheets tile it: their outlines' areas (and their tops'
    /// triangles') add up to the patch's, exactly (measured in the outlines'
    /// own f64: the mesh's f32, some 8 km out, is off by a millimetre a
    /// point); and each top is the sheet the height lookup finds there.
    fn tile(centre: DVec2) {
        let t = Tilted::new(crate::WORLD_SIZE, 1);
        let span = 1200.0;
        let (lo, hi) = (centre - DVec2::splat(span * 0.5), centre + DVec2::splat(span * 0.5));
        let s = t.size / BLOCKS as f64;
        let (mut outlines, mut triangles, mut astray) = (0.0, 0.0, 0);
        for bz in ((lo.y / s).floor() as i32 - 1)..=((hi.y / s).floor() as i32 + 1) {
            for bx in ((lo.x / s).floor() as i32 - 1)..=((hi.x / s).floor() as i32 + 1) {
                for j in -40..40 {
                    for i in -6..6 {
                        let mut o = t.outline((bx, bz), (i, j));
                        for (at, normal) in [(lo, DVec2::NEG_X), (lo, DVec2::NEG_Y), (hi, DVec2::X), (hi, DVec2::Y)] {
                            if o.len() >= 3 {
                                o = clip(&o, at, normal);
                            }
                        }
                        if o.len() >= 3 {
                            let tris = triangulate(&o);
                            outlines += area(&o);
                            triangles += tris.iter().map(|t| area(&[o[t[0]], o[t[1]], o[t[2]]])).sum::<f64>();
                            // (Its biggest triangle's middle: there, the
                            // lookup finds it.)
                            if let Some(big) = tris.iter().max_by(|a, b| area(&[o[a[0]], o[a[1]], o[a[2]]]).abs().total_cmp(&area(&[o[b[0]], o[b[1]], o[b[2]]]).abs()))
                                && area(&o) > 1.0
                                && t.sheet_at((o[big[0]] + o[big[1]] + o[big[2]]) / 3.0).middle != t.sheet((bx, bz), (i, j)).middle
                            {
                                astray += 1;
                            }
                        }
                    }
                }
            }
        }
        let sheets = t.patch((centre.x, centre.y), span, 0.0, -100.0);
        println!("{} sheets; outlines {outlines:.1} m2, their triangles {triangles:.1}, of {:.1}; {astray} not where their lookup is", sheets.len(), span * span);
        assert!((outlines - span * span).abs() < 1e-3 && (triangles - span * span).abs() < 1e-3, "outlines {outlines} m2, triangles {triangles}, of {}", span * span);
        assert_eq!(astray, 0, "{astray} sheets' tops are not where their lookup finds them");
    }

    #[test]
    fn sheets_face_out() {
        // Every triangle of a patch's sheets wound to face the way its
        // normal says (wound in, the sheets' sides were culled from outside:
        // holes onto the ground under them, the user).
        let t = Tilted::new(crate::WORLD_SIZE, 1);
        let mut bad = 0;
        let mut all = 0;
        for s in t.patch((8192.0, 8192.0), 1200.0, 0.0, -100.0) {
            for tri in s.indices.chunks(3) {
                let p = |k: u32| glam::Vec3::from(s.positions[k as usize]);
                let face = (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0]));
                let normal = glam::Vec3::from(s.normals[tri[0] as usize]);
                all += 1;
                if face.length() > 1e-4 && face.dot(normal) <= 0.0 {
                    bad += 1;
                }
            }
        }
        assert_eq!(bad, 0, "{bad} of {all} triangles face in");
    }

    #[test]
    fn lips_clear() {
        // Overhangs there are, and every point of every leaning face out
        // over the drop stands clear of the ground beneath it.
        let t = Tilted::new(crate::WORLD_SIZE, 1);
        let (lo, hi) = (DVec2::splat(8192.0 - 600.0), DVec2::splat(8192.0 + 600.0));
        let (mut points, mut low) = (0, f64::MAX);
        for gz in -1..=1 {
            for gx in -1..=1 {
                let block = t.block_at(DVec2::new(8192.0 + gx as f64 * 600.0, 8192.0 + gz as f64 * 600.0));
                for j in -20..20 {
                    for i in -4..4 {
                        let mut o = t.outline(block, (i, j));
                        for (at, normal) in [(lo, DVec2::NEG_X), (lo, DVec2::NEG_Y), (hi, DVec2::X), (hi, DVec2::Y)] {
                            if o.len() >= 3 {
                                o = clip(&o, at, normal);
                            }
                        }
                        if o.len() < 3 {
                            continue;
                        }
                        for (p, y) in t.sheet(block, (i, j)).lips(&o, &t) {
                            // (Just out from the cliff, the ground beyond.)
                            let clear = y - t.height(p);
                            if clear < 50.0 {
                                points += 1;
                                low = low.min(clear);
                            }
                        }
                    }
                }
            }
        }
        println!("{points} points of leaning faces out over the drop, the lowest {low:.2} m over the ground");
        assert!(points > 100, "few overhangs: {points}");
        assert!(low > 0.5, "a lip {low} m over the ground beneath it");
    }

    #[test]
    fn pockets_order() {
        // In a pocket's core: level ground (no tilt across a sheet), each
        // sheet on a terrace a whole number of steps over its floor; out of
        // its reach, wild.
        let t = Tilted::new(crate::WORLD_SIZE, 1);
        let pockets = t.pockets_near(8192.0, 8192.0, 4000.0);
        assert!(!pockets.is_empty(), "no pockets");
        let mut checked = 0;
        for (x, z) in pockets.iter().take(4) {
            let middle = DVec2::new(*x, *z);
            let sheet = t.sheet_at(middle);
            let pocket = sheet.frame.pocket.unwrap();
            for k in 0..40 {
                let a = k as f64 * 0.157;
                let p = middle + DVec2::new(a.cos(), a.sin()) * (k as f64 * 3.0).min(POCKET.2 * 0.9);
                let s = t.sheet_at(p);
                if s.block != sheet.block {
                    continue;
                }
                // (Level: the same a step to either side on its sheet, in
                // its piece.)
                let piece = |q: DVec2| (s.frame.to_cells(q).x / (TERRACES.3 / CELL.0)).floor();
                for d in [DVec2::X, DVec2::Y] {
                    let q = p + d;
                    if t.sheet_at(q).middle == s.middle && piece(q) == piece(p) {
                        assert!((s.top(q) - s.top(p)).abs() < 1e-6, "a pocket's core not level at {p}");
                    }
                }
                let steps = (s.top(p) - pocket.floor) / TERRACES.1;
                assert!((steps - steps.round()).abs() < 1e-6 && steps > -1e-6, "a terrace {steps} steps over its floor");
                checked += 1;
            }
        }
        println!("{} pockets near; {checked} points in their cores level, on terraces", pockets.len());
        assert!(checked > 20);
    }

    #[test]
    fn ridges_vary() {
        // Over a patch: ridges of uneven width (sheets measured across), and
        // some sheets slumped (dipping the other way).
        let t = Tilted::new(crate::WORLD_SIZE, 1);
        let sheets = t.patch((8192.0, 8192.0), 2000.0, 0.0, -100.0);
        let widths: Vec<f64> = sheets
            .iter()
            .filter_map(|s| {
                let sheet = t.sheet_at(s.middle);
                let across = sheet.frame.across;
                let mut w = 0;
                for k in -150..=150 {
                    if t.sheet_at(s.middle + across * k as f64).middle == sheet.middle {
                        w += 1;
                    }
                }
                (w > 0).then_some(w as f64)
            })
            .collect();
        let (lo, hi) = widths.iter().fold((f64::MAX, f64::MIN), |(a, b), &w| (a.min(w), b.max(w)));
        let slumped = sheets.iter().filter(|s| t.sheet_at(s.middle).dip < 0.0).count();
        println!("{} sheets, {lo:.0}-{hi:.0} m wide, {slumped} slumped", sheets.len());
        assert!(hi > lo * 2.5, "widths {lo}-{hi} m: too even");
        assert!(slumped > 0, "none slumped");
    }
}
