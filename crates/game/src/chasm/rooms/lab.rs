//! The room lab's rooms: each a kind of space, not a box furnished (the
//! first batch, all one rectangle with a door at one end and a focus at the
//! other, read as churches and as one another; review 79). Six kinds, each
//! its own envelope and way in: a rotunda, a crossing, an overlook, a
//! cascade, a ring, a great hypostyle; a few choices within each.
//!
//! Proportions come from rules, not draws (review 80: several rooms' were
//! off, and columns must suit the room): plans in Palladio's ratios, heights
//! from their sides, a rotunda as high as it is wide, columns by the orders
//! (as thick as a ninth or a tenth of their height, four or five thicknesses
//! apart). And no room bare (review 80's main complaint): each has a rhythm,
//! its bay, and several layers of detail made with geometry on it
//! (pilasters with niches between, coffers, plinth and cornice bands,
//! stepped reveals round openings; columns with bases and capitals).

use std::collections::HashMap;

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::{CELL, Cache, Filler, Form, Grid, Turn};

/// A room generated: its grid (its corner at the origin until placed), its
/// lights (where, how far they reach, how bright, in metres from its
/// corner), light standing for light thrown back, its glowing boxes (light
/// seen), what it was made from, where to see it from (a name, the eye, the
/// point looked at), and how high its way in is in its grid.
pub(super) struct Room {
    pub grid: Grid,
    pub lights: Vec<(Vec3, f32, f32)>,
    pub fill: Vec<(Vec3, f32, f32)>,
    pub glow: Vec<(Vec3, Vec3)>,
    pub recipe: Vec<String>,
    pub views: Vec<(&'static str, Vec3, Vec3)>,
    pub floor: f32,
    /// An orb hanging in it (see `orb.rs`): its centre and radius.
    pub orb: Option<(Vec3, f32)>,
}

/// Outer walls' thickness.
const T: f32 = 1.5;

/// Rounds to half metres.
fn half(x: f32) -> f32 {
    (x * 2.0).round() / 2.0
}

/// Palladio's proportions for a room's plan: the square, 4:3, 3:2, the
/// square's diagonal, 5:3, 2:1.
const RATIOS: [f32; 6] = [1.0, 4.0 / 3.0, 1.5, std::f32::consts::SQRT_2, 5.0 / 3.0, 2.0];

/// A column's thickness for its height, by the orders (a ninth or a tenth
/// of it), on whole half metres.
fn thickness(height: f32, order: f32) -> f32 {
    half(height / order).max(0.5)
}

/// The layers of detail a room carries.
#[derive(Clone, Copy, Default)]
struct Detail {
    reveals: bool,
    coffers: bool,
    bands: bool,
    pilasters: bool,
}

impl Detail {
    fn names(&self) -> String {
        let mut out = Vec::new();
        for (on, name) in [(self.pilasters, "pilasters with niches between"), (self.coffers, "coffers"), (self.bands, "plinth and cornice bands"), (self.reveals, "stepped reveals")] {
            if on {
                out.push(name);
            }
        }
        out.join(", ")
    }
}

/// A room being built.
struct B {
    g: Grid,
    c: Cache,
    lights: Vec<(Vec3, f32, f32)>,
    fill: Vec<(Vec3, f32, f32)>,
    glow: Vec<(Vec3, Vec3)>,
    recipe: Vec<String>,
    views: Vec<(&'static str, Vec3, Vec3)>,
    d: Detail,
    /// The walls' rhythms, laid last (see `rhythm`).
    rhythms: Vec<Rhythm>,
}

/// A box room's walls' rhythm (see `B::rhythm`): its plan, from where to
/// where up, its bay, where the first stands, whether niches go between.
type Rhythm = ((f32, f32), (f32, f32), (f32, f32), f32, Option<f32>, bool);

impl B {
    fn new(size: Vec3, d: Detail) -> B {
        B { g: Grid::solid(Vec3::ZERO, size), c: Cache(HashMap::new()), lights: Vec::new(), fill: Vec::new(), glow: Vec::new(), recipe: Vec::new(), views: Vec::new(), d, rhythms: Vec::new() }
    }
    fn carve(&mut self, lo: Vec3, hi: Vec3) {
        self.g.fill(lo, hi, Filler::Empty);
    }
    fn solid(&mut self, lo: Vec3, hi: Vec3) {
        self.g.fill(lo, hi, Filler::Solid);
    }
    /// The filler of the cell a point lies in.
    fn at(&self, p: Vec3) -> Filler {
        self.g.get((p / CELL).floor().as_ivec3().to_array())
    }
    fn is_solid(&self, p: Vec3) -> bool {
        self.at(p) == Filler::Solid
    }
    fn is_empty(&self, p: Vec3) -> bool {
        self.at(p) == Filler::Empty
    }
    fn note(&mut self, s: impl Into<String>) {
        self.recipe.push(s.into());
    }
    fn arch(&mut self, axis: usize, x: f32, w: f32, y: f32, spring: f32, n: f32, depth: f32) {
        self.g.arch(axis, x, w, y, spring, n, depth, &mut self.c);
    }
    fn vault(&mut self, axis: usize, x: f32, w: f32, y: f32, spring: f32, n: f32, len: f32) {
        self.g.vault(axis, x, w, y, spring, n, len, &mut self.c);
    }
    fn ring(&mut self, x: f32, z: f32, r: (f32, f32), y: (f32, f32), add: bool) {
        self.g.ring(x, z, r, y, add, &mut self.c);
    }
    /// A column of thickness `d` from `y0` to `y1`: a square base and a
    /// capital (a slab on a narrower one) round a round shaft.
    fn column(&mut self, x: f32, z: f32, d: f32, (y0, y1): (f32, f32)) {
        let r = d * 0.5;
        let tall = y1 - y0 >= 3.0;
        let (base, cap) = if tall { (0.5, 0.5) } else { (0.0, 0.25) };
        if tall {
            self.solid(Vec3::new(x - r - 0.25, y0, z - r - 0.25), Vec3::new(x + r + 0.25, y0 + base, z + r + 0.25));
            self.solid(Vec3::new(x - r - 0.25, y1 - 0.25, z - r - 0.25), Vec3::new(x + r + 0.25, y1, z + r + 0.25));
        }
        self.g.column(x, z, r, (y0 + base, y1 - cap), &mut self.c);
        self.solid(Vec3::new(x - r, y1 - cap, z - r), Vec3::new(x + r, y1 - if tall { 0.25 } else { 0.0 }, z + r));
    }
    /// A lamp: a light and the small glowing box it is.
    fn lamp(&mut self, at: Vec3, range: f32, k: f32) {
        self.lights.push((at, range, k));
        self.glow.push((at - Vec3::splat(0.15), at + Vec3::splat(0.15)));
    }
    /// Light thrown back off the floor where the light falls, filling the
    /// room (no shadows): at `at`, reaching `range`.
    fn bounce(&mut self, at: Vec3, range: f32) {
        self.fill.push((at, range, strength(range)));
    }
    /// The sky over an opening whose top is at `at`: a light above it, and
    /// a glowing panel over that (the opening seen from below).
    fn sky(&mut self, at: Vec3, size: f32, range: f32, k: f32) {
        self.lights.push((at + Vec3::Y * 1.5, range, k));
        self.glow.push((at + Vec3::new(-size, 3.0, -size), at + Vec3::new(size, 3.2, size)));
    }
    /// A light hidden behind an opening, `at`, and a glowing panel behind
    /// it (the opening seen lit).
    fn beyond(&mut self, at: Vec3, panel: (Vec3, Vec3), range: f32, k: f32) {
        self.lights.push((at, range, k));
        self.glow.push(panel);
    }
    /// A doorway through a wall across `axis` (from `n`, `depth` thick):
    /// `w` wide from `x`, its arch springing `spring` above `y`. With
    /// stepped reveals, two wider, shallower arches round it on each face.
    fn door(&mut self, axis: usize, x: f32, w: f32, y: f32, spring: f32, n: f32, depth: f32) {
        self.arch(axis, x, w, y, spring, n, depth);
        if self.d.reveals {
            for (k, d) in [(1.0, 0.5), (2.0, 0.25)] {
                self.arch(axis, x - k * 0.5, w + k, y, spring, n, d);
                self.arch(axis, x - k * 0.5, w + k, y, spring, n + depth - d, d);
            }
        }
    }
    /// Coffers in a flat ceiling at `y` over (x0..x1, z0..z1): a recess a
    /// bay across, its ribs half a metre, a second recess in it.
    fn coffers(&mut self, (x0, x1): (f32, f32), (z0, z1): (f32, f32), y: f32, bay: f32) {
        if !self.d.coffers {
            return;
        }
        let (nx, nz) = (((x1 - x0) / bay).floor() as i32, ((z1 - z0) / bay).floor() as i32);
        let (ox, oz) = (half((x1 - x0 - nx as f32 * bay) * 0.5), half((z1 - z0 - nz as f32 * bay) * 0.5));
        for i in 0..nx {
            for j in 0..nz {
                let (a, b) = (x0 + ox + i as f32 * bay, z0 + oz + j as f32 * bay);
                self.carve(Vec3::new(a + 0.25, y, b + 0.25), Vec3::new(a + bay - 0.25, y + 0.5, b + bay - 0.25));
                self.carve(Vec3::new(a + 0.75, y + 0.5, b + 0.75), Vec3::new(a + bay - 0.75, y + 0.75, b + bay - 0.75));
            }
        }
    }
    /// Bands round the inside of a box room's walls where there is wall: a
    /// plinth at the foot, a cornice in two steps under the ceiling.
    fn bands(&mut self, (x0, x1): (f32, f32), (z0, z1): (f32, f32), (y0, y1): (f32, f32)) {
        if !self.d.bands {
            return;
        }
        for (d, a, b) in [(0.25, y0, y0 + 0.5), (0.25, y1 - 1.0, y1 - 0.5), (0.5, y1 - 0.5, y1)] {
            for (lo, hi) in [
                (Vec3::new(x0, a, z0), Vec3::new(x0 + d, b, z1)),
                (Vec3::new(x1 - d, a, z0), Vec3::new(x1, b, z1)),
                (Vec3::new(x0, a, z0), Vec3::new(x1, b, z0 + d)),
                (Vec3::new(x0, a, z1 - d), Vec3::new(x1, b, z1)),
            ] {
                self.solid(lo, hi);
            }
        }
    }
    /// The same round a round room of radius `r`.
    fn round_bands(&mut self, (x, z): (f32, f32), r: f32, (y0, y1): (f32, f32)) {
        if !self.d.bands {
            return;
        }
        self.ring(x, z, (r - 0.5, r + 0.5), (y0, y0 + 0.5), true);
        self.ring(x, z, (r - 0.5, r + 0.5), (y1 - 1.0, y1 - 0.5), true);
        self.ring(x, z, (r - 1.0, r + 0.5), (y1 - 0.5, y1), true);
    }
    /// The walls' rhythm in a box room: pilasters a bay apart along each
    /// wall (from `y0` up to `y1`), an arched niche between each two, where
    /// there is wall behind all the way up (not across an opening) and room
    /// before. `phase`: where along each wall the first stands (from its
    /// start), so they answer the columns. Laid when the room is done, so
    /// its openings are there to be kept clear.
    fn rhythm(&mut self, x: (f32, f32), z: (f32, f32), y: (f32, f32), bay: f32, phase: Option<f32>, niches: bool) {
        if self.d.pilasters {
            self.rhythms.push((x, z, y, bay, phase, niches));
        }
    }
    fn lay_rhythm(&mut self, ((x0, x1), (z0, z1), (y0, y1), bay, phase, niches): Rhythm) {
        let pw = half(bay * 0.15).clamp(0.5, 1.0);
        let mid = (y0 + y1) * 0.5;
        // Each wall: where along it, from where to where, which way is out
        // of the room, the face's position.
        for (along, (a, b), out, face) in [(0usize, (x0, x1), -1.0, z0), (0, (x0, x1), 1.0, z1), (2, (z0, z1), -1.0, x0), (2, (z0, z1), 1.0, x1)] {
            let n = ((b - a) / bay).floor() as i32;
            let start = phase.unwrap_or(half(((b - a) - n as f32 * bay) * 0.5));
            let at = |u: f32, n_: f32, y: f32| if along == 0 { Vec3::new(u, y, face + out * n_) } else { Vec3::new(face + out * n_, y, u) };
            let mut posts = Vec::new();
            for k in 0..=n {
                let u = a + start + k as f32 * bay;
                if u - pw * 0.5 < a + 0.5 || u + pw * 0.5 > b - 0.5 {
                    continue;
                }
                // (Wall behind all the way up, across its width; room
                // before half way and under the cornice, if any.)
                let ys: Vec<f32> = (0..=((y1 - y0) as i32)).map(|i| (y0 + 0.1 + i as f32).min(y1 - 0.1)).collect();
                let behind = ys.iter().all(|&y| [-0.4, 0.0, 0.4].iter().all(|&du| self.is_solid(at(u + du * pw, 0.1, y))));
                let clear = behind && [mid, y1 - 1.5].iter().all(|&y| self.is_empty(at(u, -0.6, y)));
                if clear {
                    posts.push(u);
                    let (lo, hi) = (at(u - pw * 0.5, -0.5, y0), at(u + pw * 0.5, 0.0, y1));
                    self.solid(lo.min(hi), lo.max(hi));
                }
            }
            if !niches {
                continue;
            }
            for w in posts.windows(2) {
                let (u0, u1) = (w[0] + pw * 0.5 + 0.75, w[1] - pw * 0.5 - 0.75);
                let nw = ((u1 - u0) * 2.0).floor() / 2.0;
                if nw < 1.5 || w[1] - w[0] > bay + 0.01 {
                    continue;
                }
                let u0 = ((u0 + u1 - nw) * 0.5 * 4.0).round() / 4.0;
                let sill = y0 + 0.75;
                let spring = half(((y1 - y0) * 0.5 - nw * 0.5).clamp(1.0, nw * 1.5));
                let deep = (0..=4).all(|i| self.is_solid(at(u0 + nw * i as f32 / 4.0, 0.9, sill + 0.1)) && self.is_solid(at(u0 + nw * i as f32 / 4.0, 0.9, sill + spring + nw * 0.5)));
                if deep && sill + spring + nw * 0.5 < y1 - 0.5 && self.is_empty(at((u0 + u1) * 0.5, -0.1, mid)) {
                    let n0 = if out > 0.0 { face } else { face - 0.75 };
                    self.arch(if along == 0 { 2 } else { 0 }, u0, nw, sill, spring, n0, 0.75);
                }
            }
        }
    }
    /// A sloping corbel under a slab whose underside is at `y`, standing
    /// out from a wall face at `z0` by `size` (toward +z), from `x0` to
    /// `x1`: solid in the corner, its face at half a right angle.
    fn corbel(&mut self, (x0, x1): (f32, f32), z0: f32, y: f32, size: f32) {
        let n = (size / CELL).round() as i32;
        for k in 0..n {
            let row = y - (k + 1) as f32 * CELL;
            let reach = size - (k + 1) as f32 * CELL;
            self.solid(Vec3::new(x0, row, z0), Vec3::new(x1, row + CELL, z0 + reach));
            self.g.block(0, Form::Ramp { run: 1, rise: 1 }, Turn::default(), Vec3::new(x0, row, z0 + reach), x1 - x0, &mut self.c);
        }
    }
    fn view(&mut self, name: &'static str, eye: Vec3, at: Vec3) {
        self.views.push((name, eye, at));
    }
    fn finish(mut self, floor: f32) -> Room {
        for r in std::mem::take(&mut self.rhythms) {
            self.lay_rhythm(r);
        }
        let names = self.d.names();
        self.note(format!("detail: {}", if names.is_empty() { "none".into() } else { names }));
        Room { grid: self.g, lights: self.lights, fill: self.fill, glow: self.glow, recipe: self.recipe, views: self.views, floor, orb: None }
    }
}

/// A light's strength for how far it has to reach.
fn strength(range: f32) -> f32 {
    0.0006 * (range / 14.0).powi(2)
}

type Pick<'a> = &'a dyn Fn(i32, usize) -> usize;

/// The room for a seed: its kind by the seed (each kind in turn), the rest
/// drawn.
pub(super) fn generate(seed: u32) -> Room {
    let pick = move |k: i32, n: usize| ((hash01(seed as i32, k, 0, 0x52b) * n as f32) as usize).min(n - 1);
    let scale = [0.75, 1.0, 1.4][pick(1, 3)];
    // (At least two layers of detail.)
    let mut on = [0, 1, 2, 3].map(|k| pick(20 + k, 10) < 6);
    let mut k = pick(30, 4);
    while on.iter().filter(|o| **o).count() < 2 {
        on[k] = true;
        k = (k + 1) % 4;
    }
    let detail = Detail { reveals: on[0], coffers: on[1], bands: on[2], pilasters: on[3] };
    // (Each kind's main choice in turn, room by room of that kind: two of a
    // kind side by side differ in it.)
    let alt = ((seed + 5) / 6) as usize;
    match (seed + 5) % 6 {
        0 => rotunda(&pick, alt, scale, detail),
        1 => crossing(&pick, alt, scale, detail),
        2 => overlook(&pick, alt, scale, detail),
        3 => cascade(&pick, alt, scale, detail),
        4 => ring(&pick, alt, scale, detail),
        _ => hypostyle(&pick, alt, scale, detail),
    }
}

/// A round room as high as its radius to the dome (the Pantheon's
/// cylinder): a stepped dome, a lantern, or a flat ceiling with an oculus;
/// a round pit or a plinth in the middle; in at the floor, or onto a ring
/// gallery half way up; niches in the wall at the three other quarters.
fn rotunda(pick: Pick, alt: usize, s: f32, d: Detail) -> Room {
    let f = 4.0;
    let rad = half([6.0, 8.0, 10.0][pick(10, 3)] * s);
    let h = rad;
    let top = ["a stepped dome", "a lantern", "a flat ceiling with an oculus"][alt % 3];
    let floor = ["flat", "a round pit stepped down", "a round plinth"][pick(13, 3)];
    let gallery = (alt + pick(14, 2)) % 2 == 1;
    let levels = 6;
    let (dr, dh) = (half(rad * 0.75 / levels as f32).max(0.5), half(rad * 0.8 / levels as f32).max(0.5));
    let above = match top {
        "a stepped dome" => levels as f32 * dh + 3.0,
        "a lantern" => half(rad * 0.9) + 1.5,
        _ => 3.0,
    };
    let side = 2.0 * (rad + T);
    let mut b = B::new(Vec3::new(side, f + h + above + 1.5, side), d);
    let (cx, cz) = (T + rad, T + rad);
    b.note(format!("a rotunda {:.0} m across, its wall as high as its radius, to {top}", rad * 2.0));
    b.ring(cx, cz, (0.0, rad), (f, f + h), false);
    b.round_bands((cx, cz), rad, (f, f + h));
    // (Niches at the three quarters but the way in: carved from where the
    // wall is furthest, so nothing stands before its curve.)
    let g = half(h * 0.5).max(3.5);
    let gallery = gallery && h >= g + 3.5;
    if d.pilasters {
        let w = half(rad * 0.4).max(2.0);
        let spring = half(h * 0.45).min(if gallery { g - 1.5 - w * 0.5 } else { h }).max(1.5);
        b.arch(2, cx - w * 0.5, w, f + 0.5, spring, cz + rad, 1.0);
        b.arch(0, cz - w * 0.5, w, f + 0.5, spring, cx + rad, 1.0);
        b.arch(0, cz - w * 0.5, w, f + 0.5, spring, cx - rad - 1.0, 1.0);
    }
    let top_y = match top {
        "a stepped dome" => {
            for k in 1..=levels {
                let y = f + h + (k - 1) as f32 * dh;
                b.ring(cx, cz, (0.0, rad - k as f32 * dr), (y, y + dh), false);
            }
            let oc = (rad - (levels + 1) as f32 * dr).max(1.0);
            let y = f + h + levels as f32 * dh;
            b.ring(cx, cz, (0.0, oc), (y, f + h + above + 1.5), false);
            b.sky(Vec3::new(cx, f + h + above + 1.5, cz), oc + 1.0, h + above + 12.0, strength(h + above + 12.0) * 6.0);
            f + h + above
        }
        "a lantern" => {
            let lr = half(rad * 0.4).max(1.5);
            b.ring(cx, cz, (0.0, lr), (f + h, f + h + above - 1.5), false);
            b.lamp(Vec3::new(cx, f + h + above - 2.0, cz), h + above + 6.0, strength(h + above + 6.0) * 4.0);
            f + h + above - 1.5
        }
        _ => {
            let oc = half(rad * 0.15).max(1.0);
            b.ring(cx, cz, (0.0, oc), (f + h, f + h + above + 1.5), false);
            b.sky(Vec3::new(cx, f + h + above + 1.5, cz), oc + 1.0, h + 14.0, strength(h + 14.0) * 6.0);
            if d.coffers {
                // (Coffers in rings round the oculus.)
                let step = half(rad * 0.25).max(1.0);
                for k in 0..3 {
                    let r0 = oc + 1.0 + k as f32 * step;
                    b.ring(cx, cz, (r0, r0 + step - 0.5), (f + h, f + h + 0.5), false);
                }
            }
            f + h
        }
    };
    match floor {
        "a round pit stepped down" => {
            let rp = half(rad * 0.55);
            for i in 0..3 {
                b.ring(cx, cz, (0.0, rp - i as f32), (f - (i + 1) as f32 * 0.5, f - i as f32 * 0.5), false);
            }
            b.lamp(Vec3::new(cx, f - 1.0, cz), rad * 2.0, strength(rad * 2.0) * 0.5);
        }
        "a round plinth" => {
            b.ring(cx, cz, (0.0, half(rad * 0.3).max(1.0)), (f, f + 1.0), true);
            b.lamp(Vec3::new(cx, f + 2.5, cz), rad * 2.0, strength(rad * 2.0));
        }
        _ => {}
    }
    if floor != "flat" {
        b.note(format!("in the middle, {floor}"));
    }
    // The way in: at the floor through the front wall, or onto a ring
    // gallery half way up.
    let (y_in, eye) = if gallery {
        b.ring(cx, cz, (rad - 2.5, rad + 0.5), (f + g - 0.75, f + g), true);
        b.ring(cx, cz, (rad - 2.5, rad - 2.0), (f + g, f + g + 1.0), true);
        b.note(format!("in onto a ring gallery {g:.1} m up"));
        (f + g, Vec3::new(cx, f + g + 1.7, T + 1.0))
    } else {
        (f, Vec3::new(cx, f + 1.7, T + 0.5))
    };
    b.door(2, cx - 1.0, 2.0, y_in, 2.5, 0.0, T + 0.5);
    b.bounce(Vec3::new(cx, f + 1.0, cz), 2.0 * rad + h);
    b.view("entry", eye, Vec3::new(cx, f + h * 0.5, cz + rad));
    b.view("up", Vec3::new(cx + rad * 0.5, f + 1.7, cz + rad * 0.5), Vec3::new(cx - rad * 0.3, (f + h + top_y) * 0.5, cz - rad * 0.3));
    b.finish(f)
}

/// A shrine (`--opt orb`): a rotunda, a colonnade in a ring inside it on a
/// stepped base carrying a ring of stone, a dark round pit in the middle
/// stepping down into a shaft, and hanging over the pit in the colonnade's
/// ring, the orb. Its only light is the orb's (and a little sky through
/// the oculus).
pub(super) fn shrine() -> Room {
    let f = 4.0;
    let rad = 11.0;
    let h = rad;
    let d = Detail { reveals: true, coffers: false, bands: true, pilasters: true };
    let levels = 6;
    let (dr, dh) = (half(rad * 0.75 / levels as f32).max(0.5), half(rad * 0.8 / levels as f32).max(0.5));
    let above = levels as f32 * dh + 3.0;
    let side = 2.0 * (rad + T);
    let mut b = B::new(Vec3::new(side, f + h + above + 1.5, side), d);
    let (cx, cz) = (T + rad, T + rad);
    b.note("a shrine: a rotunda 22 m across, a ring of twelve columns inside it, a dark pit in the middle, the orb hanging over it");
    b.ring(cx, cz, (0.0, rad), (f, f + h), false);
    b.round_bands((cx, cz), rad, (f, f + h));
    let w = 4.0;
    let spring = half(h * 0.45);
    b.arch(2, cx - w * 0.5, w, f + 0.5, spring, cz + rad, 1.0);
    b.arch(0, cz - w * 0.5, w, f + 0.5, spring, cx + rad, 1.0);
    b.arch(0, cz - w * 0.5, w, f + 0.5, spring, cx - rad - 1.0, 1.0);
    for k in 1..=levels {
        let y = f + h + (k - 1) as f32 * dh;
        b.ring(cx, cz, (0.0, rad - k as f32 * dr), (y, y + dh), false);
    }
    let oc = (rad - (levels + 1) as f32 * dr).max(1.0);
    b.ring(cx, cz, (0.0, oc), (f + h + levels as f32 * dh, f + h + above + 1.5), false);
    b.sky(Vec3::new(cx, f + h + above + 1.5, cz), oc + 1.0, h + above + 12.0, strength(h + above + 12.0));
    // The colonnade on its stepped base, the ring of stone it carries.
    let (rc, hc, dd) = (7.0, 7.0, 1.0);
    b.ring(cx, cz, (rc - 2.0, rc + 2.0), (f, f + 0.25), true);
    b.ring(cx, cz, (rc - 1.5, rc + 1.5), (f + 0.25, f + 0.5), true);
    let n = 12;
    for k in 0..n {
        let a = (k as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
        let (x, z) = (((cx + a.cos() * rc) * 4.0).round() / 4.0, ((cz + a.sin() * rc) * 4.0).round() / 4.0);
        b.column(x, z, dd, (f + 0.5, f + 0.5 + hc));
    }
    let top = f + 0.5 + hc;
    b.ring(cx, cz, (rc - 1.0, rc + 1.0), (top, top + 1.0), true);
    b.ring(cx, cz, (rc - 1.25, rc + 1.25), (top + 1.0, top + 1.25), true);
    // The pit: three steps down, then a shaft into the dark.
    let rp = 4.5;
    for i in 0..3 {
        b.ring(cx, cz, (0.0, rp - i as f32), (f - (i + 1) as f32 * 0.5, f - i as f32 * 0.5), false);
    }
    b.ring(cx, cz, (0.0, rp - 3.0), (0.5, f - 1.5), false);
    b.door(2, cx - 1.25, 2.5, f, 3.0, 0.0, T + 0.5);
    let at = Vec3::new(cx, f + 6.5, cz);
    b.view("entry", Vec3::new(cx, f + 1.7, T + 0.5), at);
    b.view("beneath", Vec3::new(cx + 4.0, f + 1.0, cz - 3.0), at + Vec3::Y * 1.0);
    b.view("dome", Vec3::new(cx - 8.5, f + 1.7, cz + 2.0), Vec3::new(cx + 2.0, f + h + 3.0, cz - 1.0));
    let mut room = b.finish(f);
    room.orb = Some((at, 2.5));
    room
}

/// Two halls crossing, each arm vaulted along its own way (the vault
/// springing as high as the arm is wide) or flat (a fifth higher than
/// wide), a tower of light over the crossing.
fn crossing(pick: Pick, alt: usize, s: f32, d: Detail) -> Room {
    let f = 3.0;
    let w = half([6.0, 8.0, 10.0][pick(10, 3)] * s);
    let (south, north, arm) = (half(w * RATIOS[pick(11, 3)]), half(w * RATIOS[2 + pick(12, 4)]), half(w * [0.75, 1.0][pick(13, 2)]));
    let vaulted = alt % 2 == 0;
    let h = if vaulted { w * 1.5 } else { half(w * 1.2) };
    let tower = ["stepped", "a lantern", "open to the sky"][(alt + pick(16, 3)) % 3];
    let th = half(w * [0.8, 1.4][pick(17, 2)]);
    let mut b = B::new(Vec3::new(w + 2.0 * (arm + T), f + h + th + 2.0, south + w + north + 2.0 * T), d);
    let cx = T + arm + w * 0.5;
    let (z0, z1) = (T + south, T + south + w);
    let (x0, x1) = (cx - w * 0.5, cx + w * 0.5);
    b.note(format!("two halls {w:.0} m wide crossing, {h:.1} m high, {}, a tower {tower} over the crossing", if vaulted { "each arm vaulted along its way" } else { "flat ceilinged" }));
    let arms = [((x0, x1), (T, z0), 2), ((x0, x1), (z1, z1 + north), 2), ((T, x0), (z0, z1), 0), ((x1, x1 + arm), (z0, z1), 0)];
    let wall = if vaulted { h - w * 0.5 } else { h };
    let bay = half(w * 0.75).max(3.0);
    for &((a, c), (dd, e), axis) in &arms {
        if vaulted {
            if axis == 2 {
                b.vault(2, a, c - a, f, h - w * 0.5, dd, e - dd);
            } else {
                b.vault(0, dd, e - dd, f, h - w * 0.5, a, c - a);
            }
        } else {
            b.carve(Vec3::new(a, f, dd), Vec3::new(c, f + h, e));
            b.coffers((a, c), (dd, e), f + h, half(w / 3.0).max(2.0));
        }
    }
    // The crossing, and the tower over it.
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x1, f + h, z1));
    for &((a, c), (dd, e), _) in &arms {
        b.rhythm((a, c), (dd, e), (f, f + wall), bay, None, true);
        if !vaulted {
            b.bands((a, c), (dd, e), (f, f + h));
        }
    }
    let cz = (z0 + z1) * 0.5;
    match tower {
        "stepped" => {
            let n = 4;
            for k in 0..n {
                let inset = 0.75 * (k + 1) as f32;
                let y = f + h + th * k as f32 / n as f32;
                b.carve(Vec3::new(x0 + inset, y, z0 + inset), Vec3::new(x1 - inset, y + th / n as f32, z1 - inset));
            }
            b.lamp(Vec3::new(cx, f + h + th - 1.0, cz), h + th + 4.0, strength(h + th + 4.0) * 3.0);
        }
        "a lantern" => {
            b.carve(Vec3::new(x0 + 1.0, f + h, z0 + 1.0), Vec3::new(x1 - 1.0, f + h + th, z1 - 1.0));
            b.lamp(Vec3::new(cx, f + h + th - 1.0, cz), h + th + 4.0, strength(h + th + 4.0) * 3.0);
        }
        _ => {
            b.carve(Vec3::new(x0 + 1.0, f + h, z0 + 1.0), Vec3::new(x1 - 1.0, f + h + th + 2.0, z1 - 1.0));
            b.sky(Vec3::new(cx, f + h + th + 2.0, cz), w * 0.5, h + th + 10.0, strength(h + th + 10.0) * 6.0);
        }
    }
    // Light at the far ends of the arms: slots lit from beyond.
    let top = wall - 0.5;
    let far = z1 + north;
    b.carve(Vec3::new(cx - 0.25, f + 0.5, far), Vec3::new(cx + 0.25, f + top, far + T));
    b.beyond(Vec3::new(cx, f + top * 0.5, far + T + 1.0), (Vec3::new(cx - 0.75, f, far + T + 1.8), Vec3::new(cx + 0.75, f + top + 0.5, far + T + 2.0)), w + north, strength(w + north));
    let east = x1 + arm;
    for (a, lit, panel) in [(0.0, -1.0, -2.0), (east, east + T + 1.0, east + T + 1.8)] {
        b.carve(Vec3::new(a, f + 0.5, cz - 0.25), Vec3::new(a + T, f + top, cz + 0.25));
        b.beyond(Vec3::new(lit, f + top * 0.5, cz), (Vec3::new(panel, f, cz - 0.75), Vec3::new(panel + 0.2, f + top + 0.5, cz + 0.75)), w + arm, strength(w + arm));
    }
    b.door(2, cx - 1.0, 2.0, f, 2.5, 0.0, T);
    b.bounce(Vec3::new(cx, f + 1.0, cz), 2.0 * w + h);
    b.view("entry", Vec3::new(cx, f + 1.7, T + 0.5), Vec3::new(cx, f + h * 0.6, z1));
    b.view("crossing", Vec3::new(cx, f + 1.7, z0 - w * 0.4), Vec3::new(cx, f + h + th * 0.6, cz));
    b.finish(f)
}

/// In high on a balcony over a deep hall: across it a bridge to a door in
/// the far wall, or a forest of giant columns rising from far below, or a
/// flight down along a wall. The hall in Palladio's proportions, as deep
/// and high together as the mean of its sides.
fn overlook(pick: Pick, alt: usize, s: f32, d: Detail) -> Room {
    let f = 3.0;
    // (Narrower, a hall this deep is a shaft.)
    let w = half([14.0, 16.0, 20.0][pick(10, 3)] * s).max(14.0);
    let l = half(w * RATIOS[1 + pick(11, 4)]);
    let mode = ["a bridge", "giant columns", "a flight down a wall"][alt % 3];
    let bd = 3.0;
    let total = half((w + l) * 0.5);
    let mut depth = half(total * 0.6);
    if mode == "a flight down a wall" {
        // (Its run, twice its drop, within the hall.)
        depth = depth.min(half((l - bd - 2.0) * 0.5)).max(3.0);
    }
    let above = half(total - depth).max(4.0);
    let light = ["from below", "from the far door", "through oculi"][(alt + 1) % 3];
    let sloped = pick(16, 2) == 0;
    let e = f + depth;
    let mut b = B::new(Vec3::new(w + 2.0 * T, e + above + 2.0, l + 2.0 * T), d);
    let (x0, x1, z0, z1) = (T, T + w, T, T + l);
    let cx = T + w * 0.5;
    b.note(format!("in high on a balcony over a hall {w:.0} by {l:.0} m, {depth:.1} m deep, {above:.1} m above; {mode}; light {light}"));
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x1, e + above, z1));
    b.coffers((x0, x1), (z0, z1), e + above, half(w / 4.0).max(2.0));
    // The balcony, on a corbel (sloping, or stepping back down into the
    // wall); its parapet, open where the way goes on.
    b.solid(Vec3::new(x0, e - 0.75, z0), Vec3::new(x1, e, z0 + bd));
    if sloped {
        b.corbel((x0, x1), z0, e - 0.75, bd - 0.5);
    } else {
        b.solid(Vec3::new(x0, e - 1.5, z0), Vec3::new(x1, e - 0.75, z0 + bd - 0.75));
        b.solid(Vec3::new(x0, e - 2.25, z0), Vec3::new(x1, e - 1.5, z0 + bd - 1.5));
    }
    let gap = match mode {
        "a bridge" => (cx - 1.5, cx + 1.5),
        "a flight down a wall" => (x0, x0 + 2.0),
        _ => (cx, cx),
    };
    b.solid(Vec3::new(gap.1, e, z0 + bd - 0.5), Vec3::new(x1, e + 1.0, z0 + bd));
    if gap.0 > x0 {
        b.solid(Vec3::new(x0, e, z0 + bd - 0.5), Vec3::new(gap.0, e + 1.0, z0 + bd));
    }
    // A giant order up the side and far walls, from the bottom to the
    // ceiling, niches at the foot between.
    let bay = half(w / 3.0).max(4.0);
    b.rhythm((x0, x1), (z0 + bd, z1), (f, e + above), bay, None, true);
    b.bands((x0, x1), (z0, z1), (f, e + above));
    match mode {
        "a bridge" => {
            b.solid(Vec3::new(cx - 1.5, e - 1.0, z0 + bd), Vec3::new(cx + 1.5, e, z1));
            b.door(2, cx - 1.0, 2.0, e, 2.5, z1, T);
            if pick(17, 2) == 0 {
                for z in [z0 + bd + (l - bd) / 3.0, z0 + bd + 2.0 * (l - bd) / 3.0] {
                    let z = half(z);
                    b.solid(Vec3::new(cx - 1.0, f, z - 1.0), Vec3::new(cx + 1.0, e - 1.0, z + 1.0));
                }
                b.note("the bridge on two piers");
            }
        }
        "giant columns" => {
            // (By the orders: a tenth of their height thick, five
            // thicknesses apart.)
            let dd = thickness(e + above - f, 10.0);
            let span = half(dd * 5.0).max(4.0);
            let (nx, nz) = (((w - 2.0) / span).floor() as i32, ((l - bd - 2.0) / span).floor() as i32);
            for i in 1..=nx.max(1) {
                for j in 1..=nz.max(1) {
                    let x = x0 + half(w * i as f32 / (nx + 1) as f32);
                    let z = z0 + bd + half((l - bd) * j as f32 / (nz + 1) as f32);
                    b.column(x, z, dd, (f, e + above));
                }
            }
        }
        _ => {
            // (From the balcony's open end down along the left wall: a
            // stair rising to it.)
            let steps = (depth / 0.25) as u32;
            b.g.stair(2, -1.0, x0, 2.0, f, z0 + bd + depth * 2.0, steps, 0.25, 0.5);
        }
    }
    match light {
        "from below" => {
            for (x, z) in [(x0 + w * 0.25, z0 + l * 0.55), (x1 - w * 0.25, z0 + l * 0.8)] {
                b.lamp(Vec3::new(x, f + 0.5, z), depth + above + 8.0, strength(depth + above + 8.0) * 1.5);
            }
        }
        "from the far door" => {
            let y = if mode == "a bridge" { e } else { f };
            if mode != "a bridge" {
                b.door(2, cx - 1.5, 3.0, f, half(depth.min(8.0) * 0.6).max(2.5), z1, T);
            }
            b.beyond(Vec3::new(cx, y + 2.5, z1 + T + 1.0), (Vec3::new(cx - 2.0, y, z1 + T + 1.8), Vec3::new(cx + 2.0, y + 6.0, z1 + T + 2.0)), l + depth + 6.0, strength(l + depth + 6.0) * 2.0);
        }
        _ => {
            for z in [z0 + l * 0.4, z0 + l * 0.8] {
                let z = half(z);
                b.carve(Vec3::new(cx - 1.0, e + above, z - 1.0), Vec3::new(cx + 1.0, e + above + 2.0, z + 1.0));
                b.sky(Vec3::new(cx, e + above + 2.0, z), 1.5, depth + above + 10.0, strength(depth + above + 10.0) * 8.0);
            }
        }
    }
    b.door(2, cx - 1.0, 2.0, e, 2.5, 0.0, T);
    b.bounce(Vec3::new(cx, f + 1.0, z0 + l * 0.6), depth + above + w);
    b.bounce(Vec3::new(cx, e + 1.0, z0 + l * 0.5), above + w);
    b.view("entry", Vec3::new(cx + 0.75, e + 1.7, z0 + bd - 0.75), Vec3::new(cx, f + depth * 0.2, z0 + l * 0.65));
    b.view("below", Vec3::new(x1 - 2.0, f + 1.7, z1 - 2.0), Vec3::new(cx, e + 1.0, z0 + bd));
    b.finish(e)
}

/// A broad room whose floor steps down in terraces away from the way in,
/// by stairs, so it grows taller (or keeps its headroom, its ceiling
/// stepping down too); at the far end, an opening full of light. Its plan
/// in Palladio's proportions; as high at the bottom as the mean of its
/// sides, under a flat ceiling.
fn cascade(pick: Pick, alt: usize, s: f32, d: Detail) -> Room {
    let f = 3.0;
    let w = half([12.0, 16.0, 20.0][pick(10, 3)] * s);
    let l = half(w * RATIOS[1 + pick(11, 5)]);
    let n = 3 + pick(12, 2);
    let drop = [1.0, 1.5][pick(13, 2)];
    let tread = ((l - 3.0) / n as f32 * 2.0).floor() / 2.0;
    let l = 3.0 + n as f32 * tread;
    let ceiling = ["flat (taller as you go down)", "stepping down with the floor"][alt % 2];
    let flat = ceiling.starts_with("flat");
    let headroom = if flat { half((w * l).sqrt() - n as f32 * drop).max(4.0) } else { half(w * 0.5).max(4.0) };
    let stairs = ["full width", "in the middle", "at the sides, alternating"][pick(15, 3)];
    let e = f + n as f32 * drop;
    let mut b = B::new(Vec3::new(w + 2.0 * T, e + headroom + 2.0, l + 2.0 * T), d);
    let (x0, x1, z0) = (T, T + w, T);
    let cx = T + w * 0.5;
    b.note(format!("a room {w:.0} by {l:.0} m stepping down in {n} terraces of {drop:.1} m, {tread:.1} m deep; stairs {stairs}; ceiling {ceiling}"));
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x1, e + headroom, z0 + l));
    if flat {
        b.coffers((x0, x1), (z0, z0 + l), e + headroom, half(w / 4.0).max(2.0));
    }
    // The landing at the door, then each terrace, solid beneath; under a
    // ceiling stepping down, solid above as well.
    b.solid(Vec3::new(x0, f, z0), Vec3::new(x1, e, z0 + 3.0));
    for k in 1..=n {
        let y = e - k as f32 * drop;
        let (za, zb) = (z0 + 3.0 + (k - 1) as f32 * tread, z0 + 3.0 + k as f32 * tread);
        b.solid(Vec3::new(x0, f, za), Vec3::new(x1, y, zb));
        if !flat {
            b.solid(Vec3::new(x0, y + headroom + drop, za), Vec3::new(x1, e + headroom, zb));
        }
    }
    // Pilasters along the walls, a terrace apart, standing on the terraces.
    b.rhythm((x0, x1), (z0, z0 + l), (f, e + headroom), tread, Some(3.0), false);
    b.bands((x0, x1), (z0, z0 + l), (e, e + headroom));
    for k in 1..=n {
        let y = e - k as f32 * drop;
        let za = z0 + 3.0 + (k - 1) as f32 * tread;
        // The stair down onto it, from the terrace above.
        let steps = (drop / 0.25) as u32;
        let run = steps as f32 * 0.5;
        let (x, width) = match stairs {
            "full width" => (x0, w),
            "in the middle" => (cx - 2.0, 4.0),
            _ if k % 2 == 1 => (x0 + 0.5, 3.0),
            _ => (x1 - 3.5, 3.0),
        };
        b.g.stair(2, -1.0, x, width, y, za + run, steps, 0.25, 0.5);
    }
    // The far wall: an opening as wide as a third of the room, full of light.
    let bottom = e - n as f32 * drop;
    let top = if flat { e + headroom - 1.0 } else { bottom + headroom };
    let ow = half(w / 3.0).max(2.0);
    b.door(2, cx - ow * 0.5, ow, bottom, (top - bottom - ow * 0.5).max(1.0), z0 + l, T);
    b.beyond(Vec3::new(cx, bottom + 2.0, z0 + l + T + 2.0), (Vec3::new(cx - ow, bottom, z0 + l + T + 3.0), Vec3::new(cx + ow, top + 1.0, z0 + l + T + 3.2)), l + 10.0, strength(l + 10.0) * 2.0);
    b.door(2, cx - 1.0, 2.0, e, 2.5, 0.0, T);
    b.bounce(Vec3::new(cx, bottom + 1.0, z0 + l * 0.7), l + headroom);
    b.view("entry", Vec3::new(cx, e + 1.7, z0 + 0.5), Vec3::new(cx, bottom + 1.0, z0 + l));
    b.view("below", Vec3::new(cx + w * 0.3, bottom + 1.7, z0 + l - 1.0), Vec3::new(cx - w * 0.1, e + 1.5, z0 + 1.0));
    b.finish(e)
}

/// A corridor all round a core: a massive one with a chamber inside it,
/// its door on the far side; or a well open to the sky behind a colonnade
/// on a ring of stone, a pit below; square or round.
fn ring(pick: Pick, alt: usize, s: f32, d: Detail) -> Room {
    let f = 6.0;
    let outer = half([18.0, 22.0, 28.0][pick(10, 3)] * s).max(16.0);
    let c = half([3.0, 4.0, 5.0][pick(11, 3)] * s).min(outer / 3.5).max(3.0);
    let h = half(c * std::f32::consts::SQRT_2 * [1.0, 1.5][pick(12, 2)]).max(4.0);
    let (round, well) = [(true, true), (false, false), (false, true), (true, false)][(alt + 3) % 4];
    let mut b = B::new(Vec3::new(outer + 2.0 * T, f + h + 4.0, outer + 2.0 * T), d);
    let (cx, cz) = (T + outer * 0.5, T + outer * 0.5);
    let core = outer * 0.5 - c;
    b.note(format!(
        "a {} corridor {c:.1} m wide, {h:.1} m high, all round {}",
        if round { "round" } else { "square" },
        if well { "a well open to the sky behind a colonnade on a ring of stone, a pit below" } else { "a massive core, a chamber inside it, its door on the far side" }
    ));
    if round {
        b.ring(cx, cz, (0.0, outer * 0.5), (f, f + h), false);
        b.round_bands((cx, cz), outer * 0.5, (f, f + h));
        if !well {
            b.ring(cx, cz, (0.0, core), (f, f + h), true);
        }
    } else {
        b.carve(Vec3::new(T, f, T), Vec3::new(T + outer, f + h, T + outer));
        let o = T + outer;
        for (xs, zs) in [((T, o), (T, T + c)), ((T, o), (o - c, o)), ((T, T + c), (T + c, o - c)), ((o - c, o), (T + c, o - c))] {
            b.coffers(xs, zs, f + h, c);
        }
        if !well {
            b.solid(Vec3::new(cx - core, f, cz - core), Vec3::new(cx + core, f + h, cz + core));
        }
        b.rhythm((T, T + outer), (T, T + outer), (f, f + h), c + 1.0, None, true);
        b.bands((T, T + outer), (T, T + outer), (f, f + h));
    }
    if well {
        // The well: open from the pit to the sky; round its edge a ring of
        // stone a metre wide and a metre up, a colonnade on it carrying the
        // corridor's ceiling.
        let pit = 4.0;
        let dd = thickness(h - 1.0, 9.0).min(1.0);
        let sw = dd + 0.5;
        if round {
            b.ring(cx, cz, (0.0, core), (f + h, f + h + 4.0), false);
            b.ring(cx, cz, (0.0, core - sw), (f - pit, f), false);
            b.ring(cx, cz, (core - sw, core), (f, f + 1.0), true);
            let n = ((core * std::f32::consts::TAU / (dd * 5.0)).round() as i32).max(6);
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU + 0.3;
                let (x, z) = (cx + a.cos() * (core - sw * 0.5), cz + a.sin() * (core - sw * 0.5));
                b.g.column((x * 4.0).round() / 4.0, (z * 4.0).round() / 4.0, dd * 0.5, (f + 1.0, f + h), &mut b.c);
            }
        } else {
            b.carve(Vec3::new(cx - core, f + h, cz - core), Vec3::new(cx + core, f + h + 4.0, cz + core));
            b.carve(Vec3::new(cx - core + sw, f - pit, cz - core + sw), Vec3::new(cx + core - sw, f, cz + core - sw));
            let (a, z_) = (cx - core, cz - core);
            let o = 2.0 * core;
            for (lo, hi) in [((0.0, 0.0), (o, sw)), ((0.0, o - sw), (o, o)), ((0.0, 0.0), (sw, o)), ((o - sw, 0.0), (o, o))] {
                b.solid(Vec3::new(a + lo.0, f, z_ + lo.1), Vec3::new(a + hi.0, f + 1.0, z_ + hi.1));
            }
            let k = ((o / (dd * 5.0)).round() as i32).max(2);
            let m = sw * 0.5;
            for i in 0..=k {
                let t = ((m + i as f32 * (o - sw) / k as f32) * 4.0).round() / 4.0;
                for (x, z) in [(a + t, z_ + m), (a + t, z_ + o - m), (a + m, z_ + t), (a + o - m, z_ + t)] {
                    b.column(x, z, dd, (f + 1.0, f + h));
                }
            }
        }
        b.sky(Vec3::new(cx, f + h + 4.0, cz), core, h + 14.0, strength(h + 14.0) * 5.0);
        b.lamp(Vec3::new(cx, f - pit + 0.5, cz), h + pit + 6.0, strength(h + pit + 6.0));
        b.bounce(Vec3::new(cx, f - 3.0, cz), h + outer * 0.5 + 4.0);
    } else {
        // The chamber in the core, taller than the corridor, a lamp in it;
        // its door on the side away from the way in.
        let inner = core - 1.5;
        if round {
            b.ring(cx, cz, (0.0, inner), (f, f + h + 2.0), false);
        } else {
            b.carve(Vec3::new(cx - inner, f, cz - inner), Vec3::new(cx + inner, f + h + 2.0, cz + inner));
        }
        if round {
            b.carve(Vec3::new(cx - 1.0, f, cz + inner - 0.25), Vec3::new(cx + 1.0, f + 3.0, cz + core + 0.25));
        } else {
            b.door(2, cx - 1.0, 2.0, f, 2.0, cz + inner, core - inner);
        }
        b.lamp(Vec3::new(cx, f + h + 1.0, cz), inner * 2.0 + h, strength(inner * 2.0 + h));
        // (Lamps at the corridor's corners.)
        let dd = (outer * 0.5 - c * 0.5) * if round { 0.7071 } else { 1.0 };
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            b.lamp(Vec3::new(cx + sx * dd, f + h - 1.0, cz + sz * dd), c * 4.0 + 6.0, strength(c * 4.0 + 6.0));
        }
        for z in [T + c * 0.5, T + outer - c * 0.5] {
            b.bounce(Vec3::new(cx, f + 0.5, z), c * 4.0 + 6.0);
        }
    }
    b.door(2, cx - 1.0, 2.0, f, 2.5, 0.0, T + 0.5);
    // (Seen along the corridor from the door; and across the well, or into
    // the chamber from the corridor on the far side.)
    let mid = T + c * 0.5;
    let bend = if round { outer * 0.18 } else { 0.0 };
    b.view("entry", Vec3::new(cx, f + 1.7, mid), Vec3::new(cx + outer * 0.45, f + h * 0.45, mid + bend));
    if well {
        b.view("well", Vec3::new(cx + 1.0, f + 1.7, T + c - 0.5), Vec3::new(cx, f - 1.0, cz + core));
    } else {
        b.view("chamber", Vec3::new(cx + 1.5, f + 1.7, T + outer - c * 0.5), Vec3::new(cx - 0.5, f + h * 0.5, cz));
    }
    b.finish(f)
}

/// A great hall of columns by the orders (a tenth of their height thick,
/// five or six thicknesses apart), on plinths, with capitals; a clearing in
/// its middle under a lantern; the clearing sunk, or a dais across the far
/// side; pilasters on the walls answering the columns.
fn hypostyle(pick: Pick, alt: usize, s: f32, d: Detail) -> Room {
    let f = 4.0;
    let h = half([6.0, 8.0, 10.0][pick(12, 3)] * s.max(0.9));
    let dd = thickness(h, [9.0, 10.0][pick(16, 2)]);
    let bay = half(dd * [5.5, 6.5][pick(10, 2)]).max(4.0);
    let n = [5, 7][pick(11, 2)];
    let side = n as f32 * bay;
    let clearing = [1, 3][pick(13, 2)];
    let lantern = half(bay * [0.6, 1.2][pick(14, 2)]);
    let floor = ["flat", "the clearing sunk a metre", "a dais across the far side"][alt % 3];
    let mut b = B::new(Vec3::new(side + 2.0 * T, f + h + lantern + 2.5, side + 2.0 * T), d);
    let (x0, z0) = (T, T);
    let mid = side * 0.5;
    let (c0, c1) = (mid - clearing as f32 * bay * 0.5, mid + clearing as f32 * bay * 0.5);
    b.note(format!(
        "a hall {side:.0} m square, {h:.1} m high, columns {dd:.1} m thick every {bay:.1} m; a clearing {clearing} bay{} across in the middle under a lantern {lantern:.1} m higher; {floor}",
        if clearing > 1 { "s" } else { "" }
    ));
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x0 + side, f + h, z0 + side));
    b.coffers((x0, x0 + side), (z0, z0 + side), f + h, bay);
    // The lantern over the clearing, stepped in once.
    b.carve(Vec3::new(x0 + c0, f + h, z0 + c0), Vec3::new(x0 + c1, f + h + lantern * 0.5, z0 + c1));
    b.carve(Vec3::new(x0 + c0 + 0.75, f + h, z0 + c0 + 0.75), Vec3::new(x0 + c1 - 0.75, f + h + lantern, z0 + c1 - 0.75));
    let cx = x0 + mid;
    match floor {
        "the clearing sunk a metre" => {
            for i in 0..4 {
                let inset = i as f32 * 0.5;
                b.carve(Vec3::new(x0 + c0 + 0.5 + inset, f - (i + 1) as f32 * 0.25, z0 + c0 + 0.5 + inset), Vec3::new(x0 + c1 - 0.5 - inset, f - i as f32 * 0.25, z0 + c1 - 0.5 - inset));
            }
        }
        "a dais across the far side" => {
            b.solid(Vec3::new(x0, f, z0 + side - bay), Vec3::new(x0 + side, f + 1.0, z0 + side));
            b.g.stair(2, 1.0, x0 + c0, c1 - c0, f, z0 + side - bay - 2.0, 4, 0.25, 0.5);
        }
        _ => {}
    }
    // Pilasters answering the columns, niches between; bands.
    b.rhythm((x0, x0 + side), (z0, z0 + side), (f, f + h), bay, Some(bay), true);
    b.bands((x0, x0 + side), (z0, z0 + side), (f, f + h));
    // Columns at every crossing of the bays but in the clearing.
    for i in 1..n {
        for j in 1..n {
            let (x, z) = (i as f32 * bay, j as f32 * bay);
            if x > c0 && x < c1 && z > c0 && z < c1 {
                continue;
            }
            let base = if floor == "a dais across the far side" && z > side - bay { f + 1.0 } else { f };
            b.column(x0 + x, z0 + z, dd, (base, f + h));
        }
    }
    b.lamp(Vec3::new(cx, f + h + lantern - 1.0, z0 + mid), h + lantern + side * 0.5, strength(h + lantern + side * 0.5) * 3.0);
    b.door(2, cx - 1.0, 2.0, f, 2.5, 0.0, T);
    b.bounce(Vec3::new(cx, f + 1.0, z0 + mid), side * 0.7);
    b.view("entry", Vec3::new(cx, f + 1.7, z0 + 0.5), Vec3::new(cx, f + h * 0.5, z0 + side * 0.6));
    b.view("clearing", Vec3::new(x0 + c0 + 0.5, f + 1.7, z0 + c0 + 0.5), Vec3::new(x0 + side, f + h * 0.6, z0 + side * 0.7));
    b.finish(f)
}
