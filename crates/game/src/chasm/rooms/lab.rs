//! The room lab's rooms: each a kind of space, not a box furnished (the
//! first batch, all one rectangle with a door at one end and a focus at the
//! other, read as churches and as one another; review 79). Six kinds, each
//! its own envelope and way in: a rotunda, a crossing, an overlook, a
//! cascade, a ring, a great hypostyle; then a few choices within each, a
//! scale, and detail made with geometry (stepped reveals, coffers, bands).
//! Built from the kit's units.

use std::collections::HashMap;

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::{Cache, Filler, Grid};

/// A room generated: its grid (its corner at the origin until placed), its
/// lights (where, how far they reach, how bright, in metres from its
/// corner), its glowing boxes (light seen), what it was made from, where
/// to see it from (a name, the eye, the point looked at), and how high its
/// way in is in its grid.
pub(super) struct Room {
    pub grid: Grid,
    pub lights: Vec<(Vec3, f32, f32)>,
    pub fill: Vec<(Vec3, f32, f32)>,
    pub glow: Vec<(Vec3, Vec3)>,
    pub recipe: Vec<String>,
    pub views: Vec<(&'static str, Vec3, Vec3)>,
    pub floor: f32,
}

/// Outer walls' thickness.
const T: f32 = 1.5;

/// Rounds to half metres.
fn half(x: f32) -> f32 {
    (x * 2.0).round() / 2.0
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
    detail: &'static str,
}

impl B {
    fn new(size: Vec3, detail: &'static str) -> B {
        B { g: Grid::solid(Vec3::ZERO, size), c: Cache(HashMap::new()), lights: Vec::new(), fill: Vec::new(), glow: Vec::new(), recipe: Vec::new(), views: Vec::new(), detail }
    }
    fn carve(&mut self, lo: Vec3, hi: Vec3) {
        self.g.fill(lo, hi, Filler::Empty);
    }
    fn solid(&mut self, lo: Vec3, hi: Vec3) {
        self.g.fill(lo, hi, Filler::Solid);
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
    fn column(&mut self, x: f32, z: f32, r: f32, y: (f32, f32)) {
        self.g.column(x, z, r, y, &mut self.c);
    }
    /// Light thrown back off the floor where the light falls, filling the
    /// room (no shadows): at `at`, reaching `range`.
    fn bounce(&mut self, at: Vec3, range: f32) {
        self.fill.push((at, range, strength(range)));
    }
    /// A lamp: a light and the small glowing box it is.
    fn lamp(&mut self, at: Vec3, range: f32, k: f32) {
        self.lights.push((at, range, k));
        self.glow.push((at - Vec3::splat(0.15), at + Vec3::splat(0.15)));
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
        if self.detail == "reveals" {
            for (k, d) in [(1.0, 0.5), (2.0, 0.25)] {
                self.arch(axis, x - k * 0.5, w + k, y, spring, n, d);
                self.arch(axis, x - k * 0.5, w + k, y, spring, n + depth - d, d);
            }
        }
    }
    /// Coffers in a flat ceiling at `y` over (x0..x1, z0..z1): a recess a
    /// bay across, its ribs half a metre, a second recess in it.
    fn coffers(&mut self, (x0, x1): (f32, f32), (z0, z1): (f32, f32), y: f32, bay: f32) {
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
    /// Bands round the inside of a box room's walls: a plinth at the foot,
    /// a cornice in two steps under the ceiling.
    fn bands(&mut self, (x0, x1): (f32, f32), (z0, z1): (f32, f32), (y0, y1): (f32, f32)) {
        for (d, a, b) in [(0.25, y0, y0 + 0.5), (0.25, y1 - 1.0, y1 - 0.5), (0.5, y1 - 0.5, y1)] {
            self.solid(Vec3::new(x0, a, z0), Vec3::new(x0 + d, b, z1));
            self.solid(Vec3::new(x1 - d, a, z0), Vec3::new(x1, b, z1));
            self.solid(Vec3::new(x0, a, z0), Vec3::new(x1, b, z0 + d));
            self.solid(Vec3::new(x0, a, z1 - d), Vec3::new(x1, b, z1));
        }
    }
    /// The same round a round room of radius `r`.
    fn round_bands(&mut self, (x, z): (f32, f32), r: f32, (y0, y1): (f32, f32)) {
        self.ring(x, z, (r - 0.5, r + 0.5), (y0, y0 + 0.5), true);
        self.ring(x, z, (r - 0.5, r + 0.5), (y1 - 1.0, y1 - 0.5), true);
        self.ring(x, z, (r - 1.0, r + 0.5), (y1 - 0.5, y1), true);
    }
    fn view(&mut self, name: &'static str, eye: Vec3, at: Vec3) {
        self.views.push((name, eye, at));
    }
    fn finish(self, floor: f32) -> Room {
        Room { grid: self.g, lights: self.lights, fill: self.fill, glow: self.glow, recipe: self.recipe, views: self.views, floor }
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
    let detail = ["reveals", "coffers", "bands", "none"][pick(2, 4)];
    // (Each kind's main choice in turn, room by room of that kind: two of a
    // kind side by side differ in it.)
    let alt = ((seed + 5) / 6) as usize;
    let mut room = match (seed + 5) % 6 {
        0 => rotunda(&pick, alt, scale, detail),
        1 => crossing(&pick, alt, scale, detail),
        2 => overlook(&pick, alt, scale, detail),
        3 => cascade(&pick, alt, scale, detail),
        4 => ring(&pick, alt, scale, detail),
        _ => hypostyle(&pick, alt, scale, detail),
    };
    room.recipe.push(format!("detail: {detail}"));
    room
}

/// A round room: a stepped dome, a lantern, or a flat ceiling with an
/// oculus; a round pit or a plinth in the middle; in at the floor, or onto
/// a ring gallery half way up.
fn rotunda(pick: Pick, alt: usize, s: f32, detail: &'static str) -> Room {
    let f = 4.0;
    let rad = half([6.0, 8.0, 10.0][pick(10, 3)] * s);
    let h = half(rad * [0.9, 1.4][pick(11, 2)]);
    let top = ["a stepped dome", "a lantern", "a flat ceiling with an oculus"][alt % 3];
    let floor = ["flat", "a round pit stepped down", "a round plinth"][pick(13, 3)];
    let gallery = (alt + pick(14, 2)) % 2 == 1;
    let levels = 5;
    let (dr, dh) = (half(rad * 0.7 / levels as f32).max(0.5), half(rad * 0.6 / levels as f32).max(0.5));
    let above = match top {
        "a stepped dome" => levels as f32 * dh + 3.0,
        "a lantern" => half(rad * 0.9) + 1.5,
        _ => 3.0,
    };
    let side = 2.0 * (rad + T);
    let mut b = B::new(Vec3::new(side, f + h + above + 1.5, side), detail);
    let (cx, cz) = (T + rad, T + rad);
    b.note(format!("a rotunda {:.0} m across, {h:.1} m to {top}", rad * 2.0));
    b.ring(cx, cz, (0.0, rad), (f, f + h), false);
    if detail == "bands" {
        b.round_bands((cx, cz), rad, (f, f + h));
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
            if detail == "coffers" {
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
            b.column(cx, cz, half(rad * 0.3).max(1.0), (f, f + 1.0));
            b.lamp(Vec3::new(cx, f + 2.5, cz), rad * 2.0, strength(rad * 2.0));
        }
        _ => {}
    }
    if floor != "flat" {
        b.note(format!("in the middle, {floor}"));
    }
    // The way in: at the floor through the front wall, or onto a ring
    // gallery half way up.
    let g = half((h * 0.45).clamp(3.5, 6.0));
    let (y_in, eye) = if gallery && h >= g + 3.5 {
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

/// Two halls crossing, each arm vaulted along its own way or flat, a tower
/// of light over the crossing.
fn crossing(pick: Pick, alt: usize, s: f32, detail: &'static str) -> Room {
    let f = 3.0;
    let w = half([6.0, 8.0, 10.0][pick(10, 3)] * s);
    let (south, north, arm) = (half(w * [1.0, 1.5][pick(11, 2)]), half(w * [1.0, 2.0][pick(12, 2)]), half(w * [0.75, 1.0][pick(13, 2)]));
    let vaulted = alt % 2 == 0;
    let h = half(w * [0.9, 1.3][pick(15, 2)]).max(if vaulted { 3.0 + w * 0.5 } else { 4.0 });
    let tower = ["stepped", "a lantern", "open to the sky"][(alt + pick(16, 3)) % 3];
    let th = half(w * [0.8, 1.4][pick(17, 2)]);
    let mut b = B::new(Vec3::new(w + 2.0 * (arm + T), f + h + th + 2.0, south + w + north + 2.0 * T), detail);
    let cx = T + arm + w * 0.5;
    let (z0, z1) = (T + south, T + south + w);
    let (x0, x1) = (cx - w * 0.5, cx + w * 0.5);
    b.note(format!("two halls {w:.0} m wide crossing, {h:.1} m high, {}, a tower {tower} over the crossing", if vaulted { "each arm vaulted along its way" } else { "flat ceilinged" }));
    let arms = [((x0, x1), (T, z0), 2), ((x0, x1), (z1, z1 + north), 2), ((T, x0), (z0, z1), 0), ((x1, x1 + arm), (z0, z1), 0)];
    for &((a, c), (d, e), axis) in &arms {
        if vaulted {
            if axis == 2 {
                b.vault(2, a, c - a, f, h - w * 0.5, d, e - d);
            } else {
                b.vault(0, d, e - d, f, h - w * 0.5, a, c - a);
            }
        } else {
            b.carve(Vec3::new(a, f, d), Vec3::new(c, f + h, e));
            if detail == "coffers" {
                b.coffers((a, c), (d, e), f + h, half(w / 3.0).max(2.0));
            }
        }
    }
    if detail == "bands" && !vaulted {
        for &((a, c), (d, e), _) in &arms {
            b.bands((a, c), (d, e), (f, f + h));
        }
    }
    // The crossing, and the tower over it.
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x1, f + h, z1));
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
    let top = if vaulted { h - w * 0.5 - 0.5 } else { h - 1.0 };
    let far = z1 + north;
    b.carve(Vec3::new(cx - 0.25, f + 0.5, far), Vec3::new(cx + 0.25, f + top, far + T));
    b.beyond(Vec3::new(cx, f + top * 0.5, far + T + 1.0), (Vec3::new(cx - 0.75, f, far + T + 1.8), Vec3::new(cx + 0.75, f + top + 0.5, far + T + 2.0)), w + north, strength(w + north));
    let east = x1 + arm;
    for (a, lit, panel) in [(0.0, -1.0, -2.0), (east, east + T + 1.0, east + T + 1.8)] {
        b.carve(Vec3::new(a, f + 0.5, cz - 0.25), Vec3::new(a + T, f + top, cz + 0.25));
        b.beyond(Vec3::new(lit, f + top * 0.5, cz), (Vec3::new(panel, f, cz - 0.75), Vec3::new(panel + 0.2, f + top + 0.5, cz + 0.75)), w + arm, strength(w + arm));
    }
    b.door(2, cx - 1.0, 2.0, f, 2.5, 0.0, T);
    b.view("entry", Vec3::new(cx, f + 1.7, T + 0.5), Vec3::new(cx, f + h * 0.6, z1));
    b.bounce(Vec3::new(cx, f + 1.0, cz), 2.0 * w + h);
    b.view("crossing", Vec3::new(cx, f + 1.7, z0 - w * 0.4), Vec3::new(cx, f + h + th * 0.6, cz));
    b.finish(f)
}

/// In high on a balcony over a deep hall: across it a bridge to a door in
/// the far wall, or a forest of giant columns rising from far below, or a
/// flight down along a wall.
fn overlook(pick: Pick, alt: usize, s: f32, detail: &'static str) -> Room {
    let f = 3.0;
    // (Narrower, a hall this deep is a shaft.)
    let w = half([12.0, 16.0, 20.0][pick(10, 3)] * s).max(14.0);
    let l = half(w * [1.0, 1.4][pick(11, 2)]);
    let mode = ["a bridge", "giant columns", "a flight down a wall"][alt % 3];
    let bd = 3.0;
    let mut depth = half([6.0, 10.0, 14.0][pick(13, 3)] * s).max(5.0);
    if mode == "a flight down a wall" {
        // (Its run, twice its drop, within the hall.)
        depth = depth.min(half((l - bd - 2.0) * 0.5)).max(3.0);
    }
    let above = half([4.0, 6.0, 8.0][pick(14, 3)] * s.max(1.0));
    let light = ["from below", "from the far door", "through oculi"][(alt + 1) % 3];
    let piers = pick(16, 2) == 0;
    let e = f + depth;
    let mut b = B::new(Vec3::new(w + 2.0 * T, e + above + 2.0, l + 2.0 * T), detail);
    let (x0, x1, z0, z1) = (T, T + w, T, T + l);
    let cx = T + w * 0.5;
    b.note(format!("in high on a balcony over a hall {w:.0} by {l:.0} m, {depth:.1} m deep, {above:.1} m above; {mode}; light {light}"));
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x1, e + above, z1));
    if detail == "bands" {
        b.bands((x0, x1), (z0, z1), (f, e + above));
    }
    if detail == "coffers" {
        b.coffers((x0, x1), (z0, z1), e + above, half(w / 4.0).max(2.0));
    }
    // The balcony, on a corbel stepping back down into the wall; its
    // parapet, open where the way goes on.
    b.solid(Vec3::new(x0, e - 0.75, z0), Vec3::new(x1, e, z0 + bd));
    b.solid(Vec3::new(x0, e - 1.5, z0), Vec3::new(x1, e - 0.75, z0 + bd - 0.75));
    b.solid(Vec3::new(x0, e - 2.25, z0), Vec3::new(x1, e - 1.5, z0 + bd - 1.5));
    let gap = match mode {
        "a bridge" => (cx - 1.5, cx + 1.5),
        "a flight down a wall" => (x0, x0 + 2.0),
        _ => (cx, cx),
    };
    b.solid(Vec3::new(gap.1, e, z0 + bd - 0.5), Vec3::new(x1, e + 1.0, z0 + bd));
    if gap.0 > x0 {
        b.solid(Vec3::new(x0, e, z0 + bd - 0.5), Vec3::new(gap.0, e + 1.0, z0 + bd));
    }
    match mode {
        "a bridge" => {
            b.solid(Vec3::new(cx - 1.5, e - 1.0, z0 + bd), Vec3::new(cx + 1.5, e, z1));
            b.door(2, cx - 1.0, 2.0, e, 2.5, z1, T);
            if piers {
                for z in [z0 + bd + (l - bd) / 3.0, z0 + bd + 2.0 * (l - bd) / 3.0] {
                    let z = half(z);
                    b.solid(Vec3::new(cx - 1.0, f, z - 1.0), Vec3::new(cx + 1.0, e - 1.0, z + 1.0));
                }
                b.note("the bridge on two piers");
            }
        }
        "giant columns" => {
            let bay = half(w / 3.0).max(4.0);
            let mut x = x0 + bay;
            while x < x1 - 1.0 {
                let mut z = z0 + bd + bay;
                while z < z1 - 1.0 {
                    b.column(x, z, 0.75, (f, e + above));
                    z += bay;
                }
                x += bay;
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
    b.view("entry", Vec3::new(cx + 0.75, e + 1.7, z0 + bd - 0.75), Vec3::new(cx, f + depth * 0.2, z0 + l * 0.65));
    b.bounce(Vec3::new(cx, f + 1.0, z0 + l * 0.6), depth + above + w);
    b.bounce(Vec3::new(cx, e + 1.0, z0 + l * 0.5), above + w);
    b.view("below", Vec3::new(x1 - 2.0, f + 1.7, z1 - 2.0), Vec3::new(cx, e + 1.0, z0 + bd));
    b.finish(e)
}

/// A broad room whose floor steps down in terraces away from the way in,
/// so it grows taller (or keeps its headroom, its ceiling stepping down
/// too); at the far end, an opening full of light.
fn cascade(pick: Pick, alt: usize, s: f32, detail: &'static str) -> Room {
    let f = 3.0;
    let w = half([12.0, 16.0, 20.0][pick(10, 3)] * s);
    let n = 3 + pick(11, 3);
    let drop = [1.0, 1.5][pick(12, 2)];
    let tread = half([4.0, 6.0][pick(13, 2)] * s);
    let headroom = half([4.0, 6.0][pick(14, 2)] * s.max(1.0));
    let stairs = ["full width", "in the middle", "at the sides, alternating", "ramps in the middle"][(alt + pick(15, 2) * 2) % 4];
    let ceiling = ["flat (taller as you go down)", "stepping down with the floor"][alt % 2];
    let flat = ceiling.starts_with("flat");
    let l = 3.0 + n as f32 * tread;
    let e = f + n as f32 * drop;
    let mut b = B::new(Vec3::new(w + 2.0 * T, e + headroom + 2.0, l + 2.0 * T), detail);
    let (x0, x1, z0) = (T, T + w, T);
    let cx = T + w * 0.5;
    b.note(format!("a room {w:.0} m wide stepping down in {n} terraces of {drop:.1} m, {tread:.0} m deep; stairs {stairs}; ceiling {ceiling}"));
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x1, e + headroom, z0 + l));
    if detail == "bands" {
        b.bands((x0, x1), (z0, z0 + l), (e, e + headroom));
    }
    if detail == "coffers" && flat {
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
        // The stair down onto it, from the terrace above.
        let steps = (drop / 0.25) as u32;
        let run = steps as f32 * 0.5;
        let (x, width) = match stairs {
            "full width" => (x0, w),
            "in the middle" | "ramps in the middle" => (cx - 2.0, 4.0),
            _ if k % 2 == 1 => (x0, 3.0),
            _ => (x1 - 3.0, 3.0),
        };
        if stairs.starts_with("ramps") {
            b.g.slope(2, -1.0, x, width, y, za + run, (2, 1), drop, &mut b.c);
        } else {
            b.g.stair(2, -1.0, x, width, y, za + run, steps, 0.25, 0.5);
        }
    }
    // The far wall: an opening as wide as a third of the room, full of light.
    let bottom = e - n as f32 * drop;
    let top = if flat { e + headroom - 1.0 } else { bottom + headroom };
    let ow = half(w / 3.0).max(2.0);
    b.door(2, cx - ow * 0.5, ow, bottom, (top - bottom - ow * 0.5).max(1.0), z0 + l, T);
    b.beyond(Vec3::new(cx, bottom + 2.0, z0 + l + T + 2.0), (Vec3::new(cx - ow, bottom, z0 + l + T + 3.0), Vec3::new(cx + ow, top + 1.0, z0 + l + T + 3.2)), l + 10.0, strength(l + 10.0) * 2.0);
    b.door(2, cx - 1.0, 2.0, e, 2.5, 0.0, T);
    b.view("entry", Vec3::new(cx, e + 1.7, z0 + 0.5), Vec3::new(cx, bottom + 1.0, z0 + l));
    b.bounce(Vec3::new(cx, bottom + 1.0, z0 + l * 0.7), l + headroom);
    b.view("below", Vec3::new(cx + w * 0.3, bottom + 1.7, z0 + l - 1.0), Vec3::new(cx - w * 0.1, e + 1.5, z0 + 1.0));
    b.finish(e)
}

/// A corridor all round a core: a massive one, or a well open to the sky
/// behind a colonnade, a pit below; square or round.
fn ring(pick: Pick, alt: usize, s: f32, detail: &'static str) -> Room {
    let f = 6.0;
    let outer = half([16.0, 22.0, 28.0][pick(10, 3)] * s);
    let c = half([3.0, 4.0, 6.0][pick(11, 3)] * s).min(outer / 3.5).max(2.5);
    let h = half(c * [1.2, 2.0][pick(12, 2)]).max(3.5);
    let (round, well) = [(true, true), (false, false), (false, true), (true, false)][(alt + 3) % 4];
    let mut b = B::new(Vec3::new(outer + 2.0 * T, f + h + 4.0, outer + 2.0 * T), detail);
    let (cx, cz) = (T + outer * 0.5, T + outer * 0.5);
    let core = outer * 0.5 - c;
    b.note(format!(
        "a {} corridor {c:.1} m wide, {h:.1} m high, all round {}",
        if round { "round" } else { "square" },
        if well { "a well open to the sky behind a colonnade, a pit below" } else { "a massive core" }
    ));
    if round {
        b.ring(cx, cz, (0.0, outer * 0.5), (f, f + h), false);
        if detail == "bands" {
            b.round_bands((cx, cz), outer * 0.5, (f, f + h));
        }
        if !well {
            b.ring(cx, cz, (0.0, core), (f, f + h), true);
        }
    } else {
        b.carve(Vec3::new(T, f, T), Vec3::new(T + outer, f + h, T + outer));
        if detail == "bands" {
            b.bands((T, T + outer), (T, T + outer), (f, f + h));
        }
        if detail == "coffers" {
            // (Along each side of the corridor.)
            let o = T + outer;
            for (xs, zs) in [((T, o), (T, T + c)), ((T, o), (o - c, o)), ((T, T + c), (T + c, o - c)), ((o - c, o), (T + c, o - c))] {
                b.coffers(xs, zs, f + h, c.min(3.0));
            }
        }
        if !well {
            b.solid(Vec3::new(cx - core, f, cz - core), Vec3::new(cx + core, f + h, cz + core));
        }
    }
    if well {
        // The well: open from the pit to the sky; round its edge a parapet
        // and a colonnade carrying the corridor's ceiling.
        let pit = 4.0;
        if round {
            b.ring(cx, cz, (0.0, core), (f + h, f + h + 4.0), false);
            b.ring(cx, cz, (0.0, core - 0.5), (f - pit, f), false);
            b.ring(cx, cz, (core - 0.5, core), (f, f + 1.0), true);
            let n = ((core * std::f32::consts::TAU / 3.0).round() as i32).max(6);
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU + 0.3;
                let (x, z) = (cx + a.cos() * (core - 0.25), cz + a.sin() * (core - 0.25));
                b.column((x * 4.0).round() / 4.0, (z * 4.0).round() / 4.0, 0.5, (f + 1.0, f + h));
            }
        } else {
            b.carve(Vec3::new(cx - core, f + h, cz - core), Vec3::new(cx + core, f + h + 4.0, cz + core));
            b.carve(Vec3::new(cx - core + 0.5, f - pit, cz - core + 0.5), Vec3::new(cx + core - 0.5, f, cz + core - 0.5));
            // (The parapet: the well's edge, half a metre wide, a metre up.)
            let (a, z_) = (cx - core, cz - core);
            let o = 2.0 * core;
            for (lo, hi) in [((0.0, 0.0), (o, 0.5)), ((0.0, o - 0.5), (o, o)), ((0.0, 0.0), (0.5, o)), ((o - 0.5, 0.0), (o, o))] {
                b.solid(Vec3::new(a + lo.0, f, z_ + lo.1), Vec3::new(a + hi.0, f + 1.0, z_ + hi.1));
            }
            let k = ((o / 3.0).round() as i32).max(2);
            for i in 0..=k {
                let t = ((0.25 + i as f32 * (o - 0.5) / k as f32) * 4.0).round() / 4.0;
                for (x, z) in [(a + t, z_ + 0.25), (a + t, z_ + o - 0.25), (a + 0.25, z_ + t), (a + o - 0.25, z_ + t)] {
                    b.column(x, z, 0.5, (f + 1.0, f + h));
                }
            }
        }
        b.sky(Vec3::new(cx, f + h + 4.0, cz), core, h + 14.0, strength(h + 14.0) * 5.0);
        b.lamp(Vec3::new(cx, f - pit + 0.5, cz), h + pit + 6.0, strength(h + pit + 6.0));
    } else {
        // (Lamps at the corridor's corners.)
        let d = (outer * 0.5 - c * 0.5) * if round { 0.7071 } else { 1.0 };
        for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            b.lamp(Vec3::new(cx + sx * d, f + h - 1.0, cz + sz * d), c * 4.0 + 6.0, strength(c * 4.0 + 6.0));
        }
    }
    b.door(2, cx - 1.0, 2.0, f, 2.5, 0.0, T + 0.5);
    // (Seen along the corridor from the door; and across the well, or
    // round the corridor from its far side.)
    let mid = T + c * 0.5;
    let bend = if round { outer * 0.18 } else { 0.0 };
    b.view("entry", Vec3::new(cx, f + 1.7, mid), Vec3::new(cx + outer * 0.45, f + h * 0.45, mid + bend));
    if well {
        b.bounce(Vec3::new(cx, f - 3.0, cz), h + outer * 0.5 + 4.0);
        b.view("well", Vec3::new(cx + 1.0, f + 1.7, T + c - 0.5), Vec3::new(cx, f - 1.0, cz + core));
    } else {
        for z in [mid, T + outer - c * 0.5] {
            b.bounce(Vec3::new(cx, f + 0.5, z), c * 4.0 + 6.0);
        }
        let far = T + outer - c * 0.5;
        b.view("round", Vec3::new(cx, f + 1.7, far), Vec3::new(cx - outer * 0.45, f + h * 0.45, far - bend));
    }
    b.finish(f)
}

/// A great hall of columns on plinths, a clearing in its middle under
/// raised light; the clearing sunk, or a dais across the far side.
fn hypostyle(pick: Pick, alt: usize, s: f32, detail: &'static str) -> Room {
    let f = 4.0;
    let bay = [5.0, 6.0][pick(10, 2)];
    let n = [5, 7][pick(11, 2)];
    let side = n as f32 * bay;
    let h = half([6.0, 8.0, 10.0][pick(12, 3)] * s.max(0.9));
    let clearing = [1, 3][pick(13, 2)];
    let lantern = half(bay * [0.6, 1.2][pick(14, 2)]);
    let floor = ["flat", "the clearing sunk a metre", "a dais across the far side"][alt % 3];
    let radius = [0.5, 0.75][pick(16, 2)];
    let mut b = B::new(Vec3::new(side + 2.0 * T, f + h + lantern + 2.5, side + 2.0 * T), detail);
    let (x0, z0) = (T, T);
    let mid = side * 0.5;
    let (c0, c1) = (mid - clearing as f32 * bay * 0.5, mid + clearing as f32 * bay * 0.5);
    b.note(format!(
        "a hall {side:.0} m square, {h:.1} m high, columns {:.1} m thick on plinths every {bay:.0} m; a clearing {clearing} bay{} across in the middle under a lantern {lantern:.1} m higher; {floor}",
        radius * 2.0,
        if clearing > 1 { "s" } else { "" }
    ));
    b.carve(Vec3::new(x0, f, z0), Vec3::new(x0 + side, f + h, z0 + side));
    if detail == "coffers" {
        b.coffers((x0, x0 + side), (z0, z0 + side), f + h, bay);
    }
    if detail == "bands" {
        b.bands((x0, x0 + side), (z0, z0 + side), (f, f + h));
    }
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
    // Columns on square plinths at every crossing of the bays but in the
    // clearing.
    for i in 1..n {
        for j in 1..n {
            let (x, z) = (i as f32 * bay, j as f32 * bay);
            if x > c0 && x < c1 && z > c0 && z < c1 {
                continue;
            }
            let base = if floor == "a dais across the far side" && z > side - bay { f + 1.0 } else { f };
            b.solid(Vec3::new(x0 + x - radius - 0.25, base, z0 + z - radius - 0.25), Vec3::new(x0 + x + radius + 0.25, base + 0.5, z0 + z + radius + 0.25));
            b.column(x0 + x, z0 + z, radius, (base + 0.5, f + h));
        }
    }
    b.lamp(Vec3::new(cx, f + h + lantern - 1.0, z0 + mid), h + lantern + side * 0.5, strength(h + lantern + side * 0.5) * 3.0);
    b.door(2, cx - 1.0, 2.0, f, 2.5, 0.0, T);
    b.view("entry", Vec3::new(cx, f + 1.7, z0 + 0.5), Vec3::new(cx, f + h * 0.5, z0 + side * 0.6));
    b.bounce(Vec3::new(cx, f + 1.0, z0 + mid), side * 0.7);
    b.view("clearing", Vec3::new(x0 + c0 + 0.5, f + 1.7, z0 + c0 + 0.5), Vec3::new(x0 + side, f + h * 0.6, z0 + side * 0.7));
    b.finish(f)
}
