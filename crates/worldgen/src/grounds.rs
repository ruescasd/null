//! The ground lab: kinds of ground made by different techniques, each a
//! patch round a point, to judge side by side (the user, 2026-10-21: new
//! districts found by trying techniques, not only by imagining districts).
//! Each technique is cheap and rough on purpose; those that survive are
//! developed into districts.
//!
//! A patch is meshes (gathered by tile) and the height of its top, on a
//! grid, for what stands on it (the player's start, the camera).
//!
//! The techniques (`build`'s names):
//! - `tilted`: the tilted-native ground (see `tilted.rs`).
//! - `canyons`: a plateau cut by a network of straight-sided canyons, deep
//!   and wide, branching into narrower and shallower ones (enclosure).
//! - `bridged`: the canyons with levels of their own (enclosure and
//!   levels): natural bridges left across them, grooves round their walls
//!   (strata), and their mesas' tops inverted, cut into pits with ribs
//!   between them (against the blocky look).
//! - `nested`: the structures' fractal rule run at the scale of the land:
//!   plateaus within plateaus, channels between them; `sunken`, the same
//!   cut downward (pits within pits, ribs between); `contrast`, most blocks
//!   left blank, a few split all the way down (knots).
//! - `stacked`: ground in layers, decks on pillars over a floor, voids
//!   between them; `floors`, the decks continuous over many pillars, a
//!   megastructure's floors.
//!
//! The axes found so far (the user, 2026-10-21): levels (several heights
//! under a point) as a district's identity, not only a landmark's; and
//! enclosure, full (canyons) or partial (nested).
//!
//! Removed: round 1's `spots` (reaction-diffusion spots: a tank farm,
//! "nothing going for it") and `penrose` (a Penrose tiling's rhombi, heights
//! nested by supertile: rubble; its cracks suggested the canyons); round 2's
//! `turing` (a reaction-diffusion labyrinth raised into walls, traced by
//! marching squares: from above its curves and sameness failed, between its
//! walls it was promising; the canyons took its place); and round 5's
//! tops for the bridged mesas, tilted (each small block tipped its mesa's
//! way) and spires (a sparse stone forest), "not convinced": the inverted
//! tops took their place.

mod canyons;
mod floors;
mod nested;
mod stacked;

use std::collections::HashMap;

use glam::{DVec2, DVec3};

use crate::tilted::{SheetMesh, Tilted, clip, face, triangulate};

/// The names `build` knows.
pub const NAMES: [&str; 8] = ["tilted", "canyons", "bridged", "nested", "sunken", "contrast", "stacked", "floors"];

/// How wide a patch is (m), and its tiles (m).
pub const SPAN: f64 = 2048.0;
const TILE: f64 = 256.0;
/// The top grid's cells (m).
const CELL: f64 = 4.0;

/// A patch of ground: its meshes, by tile, and its top.
pub struct Patch {
    pub pieces: Vec<SheetMesh>,
    top: Top,
}

impl Patch {
    /// The height of its top at a point (0 off it, or where nothing is).
    pub fn height(&self, x: f64, z: f64) -> f64 {
        self.top.at(DVec2::new(x, z))
    }
}

/// A patch of the named ground round `centre`; an error naming those there
/// are if there is no such.
pub fn build(name: &str, centre: (f64, f64), seed: u32) -> Result<Patch, String> {
    let centre = DVec2::new(centre.0, centre.1);
    let mut b = Builder::new(centre);
    match name {
        "tilted" => {
            // (Raised so its lowest sheets stand on the lab's flat ground.)
            let lift = 160.0;
            let tilted = Tilted::new(crate::WORLD_SIZE, seed);
            let pieces = tilted.patch((centre.x, centre.y), SPAN, lift, -1.0);
            b.top.fill(|p| tilted.height(p) + lift);
            return Ok(Patch { pieces, top: b.top });
        }
        "canyons" => canyons::build(&mut b, seed, &canyons::PLAIN),
        "bridged" => {
            canyons::build(&mut b, seed, &canyons::BRIDGED);
            canyons::bridges(&mut b, seed);
        }
        "nested" => nested::build(&mut b, seed, nested::RAISED),
        "sunken" => nested::sunken(&mut b, seed),
        "contrast" => nested::build(&mut b, seed, nested::CONTRAST),
        "stacked" => stacked::build(&mut b, seed),
        "floors" => floors::build(&mut b, seed),
        _ => return Err(format!("no ground called {name:?}: there are {}", NAMES.join(", "))),
    }
    Ok(b.finish())
}

/// The highest top over a grid of cells round the patch's middle.
struct Top {
    origin: DVec2,
    n: usize,
    h: Vec<f64>,
}

impl Top {
    fn new(centre: DVec2) -> Self {
        let n = (SPAN / CELL).ceil() as usize;
        Self { origin: centre - DVec2::splat(SPAN * 0.5), n, h: vec![0.0; n * n] }
    }

    fn cell(&self, p: DVec2) -> Option<usize> {
        let q = (p - self.origin) / CELL;
        let (i, j) = (q.x.floor(), q.y.floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.n && (j as usize) < self.n).then(|| j as usize * self.n + i as usize)
    }

    fn at(&self, p: DVec2) -> f64 {
        self.cell(p).map_or(0.0, |k| self.h[k])
    }

    /// Each cell's middle's height, from `f`.
    fn fill(&mut self, f: impl Fn(DVec2) -> f64) {
        for j in 0..self.n {
            for i in 0..self.n {
                let p = self.origin + (DVec2::new(i as f64, j as f64) + 0.5) * CELL;
                self.h[j * self.n + i] = f(p);
            }
        }
    }

    /// Raised to `y` over a polygon (where its cells' middles fall in it).
    fn raise(&mut self, outline: &[DVec2], y: f64) {
        let (mut lo, mut hi) = (outline[0], outline[0]);
        for p in outline {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        let a = ((lo - self.origin) / CELL).floor().max(DVec2::ZERO);
        let b = ((hi - self.origin) / CELL).ceil().min(DVec2::splat(self.n as f64));
        for j in a.y as usize..b.y as usize {
            for i in a.x as usize..b.x as usize {
                let p = self.origin + (DVec2::new(i as f64, j as f64) + 0.5) * CELL;
                if inside(outline, p) {
                    let k = j * self.n + i;
                    self.h[k] = self.h[k].max(y);
                }
            }
        }
    }
}

/// Whether a point is inside a polygon (even-odd).
fn inside(poly: &[DVec2], p: DVec2) -> bool {
    let mut odd = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            odd = !odd;
        }
    }
    odd
}

/// Gathers a patch's solids into meshes by tile, and its top; each solid
/// in the tone set when it is added (its albedo: as the plates, each its
/// own, so forms read apart).
struct Builder {
    centre: DVec2,
    tiles: HashMap<(i32, i32), SheetMesh>,
    top: Top,
    tone: f32,
}

impl Builder {
    fn new(centre: DVec2) -> Self {
        Self { centre, tiles: HashMap::new(), top: Top::new(centre), tone: 0.18 }
    }

    /// A tone round `base`, varied by `spread` with `k`'s hash.
    fn shade(&mut self, seed: u32, k: (i32, i32), base: f32, spread: f32) {
        self.tone = base + spread * (rand(seed, k.0, k.1, 99) as f32 * 2.0 - 1.0);
    }

    /// Whether a point is on the patch.
    fn on(&self, p: DVec2) -> bool {
        let d = (p - self.centre).abs();
        d.x <= SPAN * 0.5 && d.y <= SPAN * 0.5
    }

    fn mesh(&mut self, at: DVec2) -> &mut SheetMesh {
        let key = ((at.x / TILE).floor() as i32, (at.y / TILE).floor() as i32);
        self.tiles.entry(key).or_insert_with(|| SheetMesh {
            middle: (DVec2::new(key.0 as f64, key.1 as f64) + 0.5) * TILE,
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            tones: Vec::new(),
        })
    }

    /// A flat top over a polygon at `y`.
    fn top(&mut self, outline: &[DVec2], y: f64) {
        if outline.len() < 3 {
            return;
        }
        let at = |p: DVec2| DVec3::new(p.x, y, p.y);
        let tone = self.tone;
        let m = self.mesh(middle(outline));
        for t in triangulate(outline) {
            face(m, [at(outline[t[0]]), at(outline[t[1]]), at(outline[t[2]])], DVec3::Y);
        }
        m.tones.resize(m.positions.len(), tone);
        self.top.raise(outline, y);
    }

    /// An underside over a polygon at `y`, facing down.
    fn underside(&mut self, outline: &[DVec2], y: f64) {
        if outline.len() < 3 {
            return;
        }
        let at = |p: DVec2| DVec3::new(p.x, y, p.y);
        let tone = self.tone;
        let m = self.mesh(middle(outline));
        for t in triangulate(outline) {
            face(m, [at(outline[t[0]]), at(outline[t[1]]), at(outline[t[2]])], -DVec3::Y);
        }
        m.tones.resize(m.positions.len(), tone);
    }

    /// A wall along `a` to `b`, from `low` up to `high`, facing `out` (a
    /// side, in the ground's plane).
    fn wall(&mut self, a: DVec2, b: DVec2, low: f64, high: f64, out: DVec2) {
        if high - low < 1e-6 || a.distance_squared(b) < 1e-10 {
            return;
        }
        let tone = self.tone;
        let m = self.mesh((a + b) * 0.5);
        let hint = DVec3::new(out.x, 0.0, out.y);
        let (a0, b0, a1, b1) = (DVec3::new(a.x, low, a.y), DVec3::new(b.x, low, b.y), DVec3::new(a.x, high, a.y), DVec3::new(b.x, high, b.y));
        face(m, [a0, b0, b1], hint);
        face(m, [a0, b1, a1], hint);
        m.tones.resize(m.positions.len(), tone);
    }

    /// A prism: a polygon's top at `top`, its sides down to `bottom`, and
    /// its underside if it floats.
    fn prism(&mut self, outline: &[DVec2], top: f64, bottom: f64, floats: bool) {
        if outline.len() < 3 || top <= bottom {
            return;
        }
        let ccw = crate::tilted::area(outline) > 0.0;
        self.top(outline, top);
        let n = outline.len();
        for i in 0..n {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            let out = (b - a).perp().normalize_or_zero() * if ccw { -1.0 } else { 1.0 };
            self.wall(a, b, bottom, top, out);
        }
        if floats {
            self.underside(outline, bottom);
        }
    }

    fn finish(self) -> Patch {
        Patch { pieces: self.tiles.into_values().collect(), top: self.top }
    }
}

/// The Voronoi cell of site (`i`, `j`) on a grid of `spacing` m from
/// `origin`, each site strayed up to `jitter` of a cell (hashed with `key`):
/// a square cut by its neighbours' bisectors. With its site.
fn voronoi(seed: u32, key: i32, origin: DVec2, spacing: f64, jitter: f64, i: i32, j: i32) -> (DVec2, Vec<DVec2>) {
    let site = |i: i32, j: i32| {
        let r = |k: i32| rand(seed, i, j, key * 16 + k);
        origin + (DVec2::new(i as f64, j as f64) + 0.5 + DVec2::new(r(1) - 0.5, r(2) - 0.5) * jitter) * spacing
    };
    let s = site(i, j);
    let mut cell: Vec<DVec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|&(x, z)| s + DVec2::new(x, z) * spacing * 1.5).collect();
    for dj in -2..=2 {
        for di in -2..=2 {
            if (di, dj) == (0, 0) || cell.len() < 3 {
                continue;
            }
            let o = site(i + di, j + dj);
            cell = clip(&cell, (s + o) * 0.5, o - s);
        }
    }
    (s, cell)
}

/// A convex polygon (counter-clockwise) shrunk by `d` m on every side;
/// nothing if that leaves nothing.
fn inset(p: &[DVec2], d: f64) -> Vec<DVec2> {
    let mut out = p.to_vec();
    for i in 0..p.len() {
        if out.len() < 3 {
            return Vec::new();
        }
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let inward = (b - a).perp().normalize_or_zero();
        out = clip(&out, a + inward * d, -inward);
    }
    if out.len() < 3 || crate::tilted::area(&out).abs() < 1.0 { Vec::new() } else { out }
}

/// A convex polygon cut to another (convex, counter-clockwise).
fn within(p: &[DVec2], by: &[DVec2]) -> Vec<DVec2> {
    let mut out = p.to_vec();
    for i in 0..by.len() {
        if out.len() < 3 {
            return Vec::new();
        }
        let (a, b) = (by[i], by[(i + 1) % by.len()]);
        out = clip(&out, a, -(b - a).perp());
    }
    out
}

/// A polygon's middle (its points' mean).
fn middle(p: &[DVec2]) -> DVec2 {
    p.iter().copied().sum::<DVec2>() / p.len() as f64
}

/// Our own random numbers: a hash of integers to [0, 1).
fn rand(seed: u32, a: i32, b: i32, k: i32) -> f64 {
    crate::noise::hash01(a, b, k, seed) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounds_build() {
        // Every kind builds, has ground, and stands on the patch.
        let centre = DVec2::splat(8192.0);
        for name in NAMES {
            let patch = build(name, (centre.x, centre.y), 1).unwrap();
            let triangles: usize = patch.pieces.iter().map(|m| m.indices.len() / 3).sum();
            let tops = patch.top.h.iter().filter(|&&h| h > 0.5).count();
            println!("{name}: {} tiles, {triangles} triangles, top over {:.0}% of it", patch.pieces.len(), tops as f64 / patch.top.h.len() as f64 * 100.0);
            assert!(triangles > 1000, "{name}: {triangles} triangles");
            assert!(patch.height(centre.x, centre.y) >= 0.0);
        }
        assert!(build("nothing", (centre.x, centre.y), 1).is_err());
    }
}


