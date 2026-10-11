//! The room lab's rooms: each generated from a recipe, a few discrete
//! choices (a plan, a ceiling, a proportion, a change of level, a focus at
//! the far end, where the light comes from, how you come in), so what makes
//! a room good can be traced to its choices. Built from the kit's units.

use std::collections::HashMap;

use bevy::prelude::*;
use worldgen::noise::hash01;

use super::{Cache, Filler, Grid};

/// A room generated: its grid (its corner at the origin until placed), its
/// lights (where, how far they reach, how bright, in metres from its
/// corner), its glowing boxes (light seen), what it was made from, and
/// where to see it from (eye, looking at).
pub(super) struct Room {
    pub grid: Grid,
    pub lights: Vec<(Vec3, f32, f32)>,
    pub glow: Vec<(Vec3, Vec3)>,
    pub recipe: Vec<String>,
    pub views: Vec<(&'static str, Vec3, Vec3)>,
}

/// The side walls' thickness, and the back wall's (deep enough for an apse
/// or a lit chamber behind it).
const T: f32 = 1.0;
const BACK: f32 = 4.0;

/// The floor's height in the grid: rock beneath it for a sunken floor.
pub(super) const FLOOR: f32 = 3.0;

/// Rounds to whole cells (a quarter of a metre), or to half metres.
fn q(x: f32) -> f32 {
    (x * 4.0).round() / 4.0
}
fn half(x: f32) -> f32 {
    (x * 2.0).round() / 2.0
}

pub(super) fn generate(seed: u32) -> Room {
    let r = |k: i32| hash01(seed as i32, k, 0, 0x51a);
    let pick = |k: i32, n: usize| ((r(k) * n as f32) as usize).min(n - 1);
    let mut cache = Cache(HashMap::new());
    let f = FLOOR;

    let plan = ["single", "aisled", "gallery", "hypostyle"][pick(1, 4)];
    let mut ceiling = ["flat", "vault", "transverse", "stepped", "shaft"][pick(2, 5)];
    let proportion = ["low", "even", "tall"][pick(3, 3)];
    let mut level = ["none", "dais", "sunken", "split"][pick(4, 4)];
    let focal = ["none", "apse", "portal", "slot"][pick(5, 4)];
    let light = ["axis", "above", "focal", "clerestory"][pick(6, 4)];
    let entry = ["door", "vestibule"][pick(7, 2)];
    let bay = [3.0, 4.0, 5.0][pick(8, 3)];
    let bays = 4 + pick(9, 4) as i32;
    let tall = [0.6, 1.0, 1.6][pick(3, 3)];

    // The nave's width, and the plan across it: from x = T, the nave
    // between `xa` and `xb`.
    let aisle = [2.5, 3.0, 4.0][pick(10, 3)];
    let strip = 3.0;
    let (wn, width) = match plan {
        "single" => {
            let w = [8.0, 10.0, 12.0, 14.0, 16.0][pick(11, 5)];
            (w, w)
        }
        "aisled" => {
            let w = [6.0, 8.0, 10.0][pick(11, 3)];
            (w, w + 2.0 * (aisle + 1.0))
        }
        "gallery" => {
            let w = [12.0, 14.0, 16.0][pick(11, 3)];
            (w, w)
        }
        _ => {
            // (The hypostyle: aisles side by side, a metre of wall or
            // column between them.)
            let n = 3 + pick(11, 3) as i32;
            let w = n as f32 * bay - 1.0;
            (w, w)
        }
    };
    let (xa, xb) = if plan == "aisled" { (T + aisle + 1.0, T + aisle + 1.0 + wn) } else { (T, T + width) };
    // (The axis: the nave's middle; in the hypostyle, its middle aisle's,
    // between the lines of columns.)
    let xc = if plan == "hypostyle" {
        let n = ((width + 1.0) / bay).round() as i32;
        T + (bay - 1.0) * 0.5 + bay * ((n - 1) / 2) as f32
    } else {
        (xa + xb) * 0.5
    };
    let len = bays as f32 * bay - 1.0;
    let front = if entry == "vestibule" { 6.0 } else { T };
    let (z0, z1) = (front, front + len);

    // How high: by the nave's width (the hypostyle's by its aisles').
    let mut h = half(if plan == "hypostyle" { bay * (0.9 + tall) } else { wn * tall }).clamp(4.5, 18.0);
    if plan == "hypostyle" && ceiling != "vault" {
        ceiling = "flat";
    }
    let span = if plan == "hypostyle" { bay - 1.0 } else { wn };
    match ceiling {
        "vault" => h = h.max(3.0 + span * 0.5),
        "transverse" => h = h.max(3.0 + (bay - 1.0) * 0.5),
        "stepped" => h = h.max(4.5),
        _ => {}
    }
    let gallery = q((h * 0.4).clamp(3.5, 5.0));
    if plan == "gallery" {
        // (Headroom on the gallery, under the ceiling's curve.)
        h = h.max(gallery + 3.5);
        match ceiling {
            "vault" => h = h.max(gallery + 2.5 + span * 0.5),
            "transverse" => h = h.max(gallery + 2.5 + (bay - 1.0) * 0.5),
            _ => {}
        }
    }
    let aisle_h = q((h * 0.5).clamp(3.5, (h - 2.0).max(3.5)));
    if plan == "hypostyle" && level != "dais" {
        level = "none";
    }
    let shaft = if ceiling == "shaft" { half((h * 0.7).clamp(3.0, 8.0)) } else { 0.0 };
    let top = f + h + shaft + 1.5;
    let mut g = Grid::solid(Vec3::ZERO, Vec3::new(width + 2.0 * T, top, front + len + BACK));
    let mut recipe = vec![format!("{plan}, {wn:.0} m wide, {len:.0} m long, {h:.1} m high ({proportion})")];
    let mut lights = Vec::new();
    let mut glow = Vec::new();
    let carve = |g: &mut Grid, lo: Vec3, hi: Vec3| g.fill(lo, hi, Filler::Empty);
    let solid = |g: &mut Grid, lo: Vec3, hi: Vec3| g.fill(lo, hi, Filler::Solid);

    // The volumes, by plan.
    let mut supports = String::new();
    match plan {
        "aisled" => {
            for (a, b) in [(T, T + aisle), (xb + 1.0, xb + 1.0 + aisle)] {
                carve(&mut g, Vec3::new(a, f, z0), Vec3::new(b, f + aisle_h, z1));
            }
            nave(&mut g, ceiling, (xa, xb), (z0, z1), h, bay, &mut cache);
            // The walls between: an arcade where its arches fit under
            // the aisles' ceiling, or a colonnade.
            let arcade = r(12) < 0.6 && aisle_h - 0.75 - (bay - 1.0) * 0.5 >= 2.25;
            for x in [xa - 1.0, xb] {
                if arcade {
                    for k in 0..bays {
                        g.arch(0, z0 + k as f32 * bay, bay - 1.0, f, aisle_h - 0.75 - (bay - 1.0) * 0.5, x, 1.0, &mut cache);
                    }
                } else {
                    carve(&mut g, Vec3::new(x, f, z0), Vec3::new(x + 1.0, f + aisle_h - 0.75, z1));
                    for k in 1..bays {
                        g.column(x + 0.5, z0 + k as f32 * bay - 0.5, 0.5, (f, f + aisle_h - 0.75), &mut cache);
                    }
                }
            }
            supports = format!("aisles {aisle:.1} m wide, {aisle_h:.1} m high, behind {}", if arcade { "arcades" } else { "colonnades" });
        }
        "hypostyle" => {
            let n = ((width + 1.0) / bay).round() as i32;
            if ceiling == "vault" {
                // Arcades between aisles, each aisle vaulted.
                let spring = h - (bay - 1.0) * 0.5;
                let arch_spring = (spring - 0.75 - (bay - 1.0) * 0.5).max(2.0);
                for j in 0..n {
                    let a = T + j as f32 * bay;
                    g.vault(2, a, bay - 1.0, f, spring, z0, len, &mut cache);
                    if j > 0 {
                        for k in 0..bays {
                            g.arch(0, z0 + k as f32 * bay, bay - 1.0, f, arch_spring, a - 1.0, 1.0, &mut cache);
                        }
                    }
                }
                supports = format!("{n} vaulted aisles between arcades");
            } else {
                carve(&mut g, Vec3::new(T, f, z0), Vec3::new(T + width, f + h, z1));
                let round = r(12) < 0.6;
                for j in 1..n {
                    for k in 1..bays {
                        let (x, z) = (T + j as f32 * bay - 0.5, z0 + k as f32 * bay - 0.5);
                        if round {
                            g.column(x, z, 0.5, (f, f + h), &mut cache);
                        } else {
                            solid(&mut g, Vec3::new(x - 0.5, f, z - 0.5), Vec3::new(x + 0.5, f + h, z + 0.5));
                        }
                    }
                }
                supports = format!("{} {} columns under a flat ceiling", (n - 1) * (bays - 1), if round { "round" } else { "square" });
            }
        }
        _ => nave(&mut g, ceiling, (xa, xb), (z0, z1), h, bay, &mut cache),
    }
    // The gallery: round the sides and the back at `gallery`, on columns,
    // a parapet along its edge, a stair up the left wall to it.
    let mut free = (xa, xb, z0, z1);
    if plan == "gallery" {
        let zg = z0 + 0.5 + gallery * 2.0;
        let slab = |g: &mut Grid, x: (f32, f32), z: (f32, f32)| g.fill(Vec3::new(x.0, f + gallery - 0.5, z.0), Vec3::new(x.1, f + gallery, z.1), Filler::Solid);
        slab(&mut g, (T, T + strip), (zg, z1));
        slab(&mut g, (T + width - strip, T + width), (zg, z1));
        slab(&mut g, (T, T + width), (z1 - strip, z1));
        // (Parapets, a metre high, a quarter thick.)
        let wall = |g: &mut Grid, lo: Vec3, hi: Vec3| g.fill(lo, hi, Filler::Solid);
        let (p0, p1) = (f + gallery, f + gallery + 1.0);
        wall(&mut g, Vec3::new(T + strip - 0.25, p0, zg), Vec3::new(T + strip, p1, z1 - strip));
        wall(&mut g, Vec3::new(T + width - strip, p0, zg), Vec3::new(T + width - strip + 0.25, p1, z1 - strip));
        wall(&mut g, Vec3::new(T + strip - 0.25, p0, z1 - strip), Vec3::new(T + width - strip + 0.25, p1, z1 - strip + 0.25));
        wall(&mut g, Vec3::new(T + 1.5, p0, zg), Vec3::new(T + strip, p1, zg + 0.25));
        wall(&mut g, Vec3::new(T + width - strip, p0, zg), Vec3::new(T + width, p1, zg + 0.25));
        // (Columns under its edge, every bay.)
        let mut z = zg + 1.5;
        while z < z1 - strip - 1.0 {
            g.column(T + strip - 0.5, q(z), 0.5, (f, f + gallery - 0.5), &mut cache);
            g.column(T + width - strip + 0.5, q(z), 0.5, (f, f + gallery - 0.5), &mut cache);
            z += bay;
        }
        let mut x = T + strip + 1.0;
        while x < T + width - strip - 0.5 {
            g.column(q(x), z1 - strip + 0.5, 0.5, (f, f + gallery - 0.5), &mut cache);
            x += bay;
        }
        g.stair(2, 1.0, T, 1.5, f, z0 + 0.5, (gallery * 4.0) as u32, 0.25, 0.5);
        supports = format!("a gallery round three sides at {gallery:.2} m on columns, a stair up to it");
        free = (T + strip, T + width - strip, z0, z1 - strip);
    }
    if !supports.is_empty() {
        recipe.push(supports);
    }
    recipe.push(format!("ceiling: {ceiling}"));

    // A change of level in the free floor.
    let (fa, fb, fz0, fz1) = free;
    let mut back = f;
    match level {
        "dais" => {
            solid(&mut g, Vec3::new(fa, f, z1 - 5.0), Vec3::new(fb, f + 1.0, z1 + BACK - 1.0));
            g.stair(2, 1.0, fa, fb - fa, f, z1 - 7.0, 4, 0.25, 0.5);
            back = f + 1.0;
            recipe.push("a dais at the far end, a metre up".into());
        }
        "sunken" => {
            let steps = if fb - fa >= 10.0 { 4 } else { 2 };
            for i in 0..steps {
                let inset = 1.0 + i as f32 * 0.5;
                carve(&mut g, Vec3::new(fa + inset, f - (i + 1) as f32 * 0.25, fz0 + inset + 1.0), Vec3::new(fb - inset, f - i as f32 * 0.25, fz1 - inset - 1.0));
            }
            recipe.push(format!("its middle sunk {:.2} m by steps all round", steps as f32 * 0.25));
        }
        "split" => {
            let zm = half(z0 + len * 0.55);
            solid(&mut g, Vec3::new(fa, f, zm), Vec3::new(fb, f + 2.0, z1 + BACK - 1.0));
            let w = if r(13) < 0.5 { 3.0 } else { fb - fa };
            if r(14) < 0.35 {
                g.slope(2, 1.0, xc - w * 0.5, w, f, zm - 4.0, (2, 1), 2.0, &mut cache);
                recipe.push(format!("the far half 2 m up, a ramp {w:.0} m wide"));
            } else {
                g.stair(2, 1.0, xc - w * 0.5, w, f, zm - 4.0, 8, 0.25, 0.5);
                recipe.push(format!("the far half 2 m up, a stair {w:.0} m wide"));
            }
            back = f + 2.0;
        }
        _ => {}
    }

    // The focus at the far end.
    // (Where the ceiling begins to curve over, if it does.)
    let room_top = |h: f32| match ceiling {
        "vault" => h - span * 0.5,
        "transverse" => h - (bay - 1.0) * 0.5,
        _ => h,
    };
    let clear = room_top(h) - (back - f);
    let mut focus = Vec3::new(xc, back + 2.0, z1 - 2.0);
    match focal {
        "apse" => {
            let w = ((fb - fa - 2.0).min(8.0).min((clear - 3.0) * 2.0) / 2.0).floor().max(1.0) * 2.0;
            let spring = q((clear - w * 0.5 - 0.5).max(2.5));
            g.arch(2, xc - w * 0.5, w, back, spring, z1, 3.0, &mut cache);
            focus = Vec3::new(xc, back + spring * 0.6, z1 + 1.5);
            recipe.push(format!("an apse {w:.0} m wide in the far wall"));
        }
        "portal" => {
            let spring = q((clear * 0.5).clamp(2.5, (clear - 2.0).max(2.5)));
            g.arch(2, xc - 1.0, 2.0, back, spring, z1, 1.0, &mut cache);
            carve(&mut g, Vec3::new(xc - 2.0, back, z1 + 1.0), Vec3::new(xc + 2.0, back + spring + 2.0, z1 + 3.5));
            focus = Vec3::new(xc, back + spring * 0.7, z1 + 2.5);
            lights.push((focus, 14.0, if light == "focal" { 0.002 } else { 0.0006 }));
            recipe.push("a tall portal in the far wall, lit from beyond".into());
        }
        "slot" => {
            let top = (clear - 1.0).max(3.0);
            carve(&mut g, Vec3::new(xc - 0.25, back + 0.5, z1), Vec3::new(xc + 0.25, back + top, z1 + BACK));
            focus = Vec3::new(xc, back + top * 0.5, z1 + BACK + 1.0);
            glow.push((Vec3::new(xc - 1.0, back, z1 + BACK + 2.0), Vec3::new(xc + 1.0, back + top + 0.5, z1 + BACK + 2.2)));
            lights.push((focus, 18.0 + h, if light == "focal" { 0.003 } else { 0.001 }));
            recipe.push("a slot of light up the far wall".into());
        }
        _ => {}
    }

    // How you come in: a door in the front wall, or a long low passage.
    match entry {
        "vestibule" => {
            g.arch(2, xc - 1.25, 2.5, f, 2.0, 0.0, front, &mut cache);
            recipe.push("in by a passage 6 m long, 3.25 m high".into());
        }
        _ => g.arch(2, xc - 1.0, 2.0, f, 2.25, 0.0, front, &mut cache),
    }

    // The light.
    let lamp = |h: f32| 0.00025 * (h / 6.0).powi(2);
    match light {
        "axis" => {
            let y = f + (h - 1.5).min(h * 0.7);
            let mut z = z0 + bay * 0.5;
            while z < z1 {
                lights.push((Vec3::new(xc, y, z), h * 2.0 + 8.0, lamp(h)));
                glow.push((Vec3::new(xc - 0.15, y - 0.15, z - 0.15), Vec3::new(xc + 0.15, y + 0.15, z + 0.15)));
                z += bay * 2.0;
            }
            recipe.push("light: lamps along the axis".into());
        }
        "above" => {
            // (Over the middle of a bay: clear of any columns.)
            let zc = z0 + bay * (bays / 2) as f32 + (bay - 1.0) * 0.5;
            if ceiling == "shaft" {
                lights.push((Vec3::new(xc, f + h + shaft - 0.5, zc), h + shaft + 10.0, 0.003));
                glow.push((Vec3::new(xc - 1.5, f + h + shaft - 0.1, zc - 1.5), Vec3::new(xc + 1.5, f + h + shaft, zc + 1.5)));
                recipe.push("light: from the top of the shaft".into());
            } else {
                // An oculus through the roof, lit from above.
                carve(&mut g, Vec3::new(xc - 1.0, f + h - 0.5, zc - 1.0), Vec3::new(xc + 1.0, top, zc + 1.0));
                lights.push((Vec3::new(xc, top + 3.0, zc), h + 14.0, 0.004));
                glow.push((Vec3::new(xc - 2.0, top + 4.0, zc - 2.0), Vec3::new(xc + 2.0, top + 4.2, zc + 2.0)));
                recipe.push("light: an oculus in the roof".into());
            }
        }
        "focal" => {
            if focal == "none" || focal == "apse" {
                lights.push((focus, h + 10.0, lamp(h) * 2.0));
                glow.push((focus - Vec3::splat(0.15), focus + Vec3::splat(0.15)));
            }
            recipe.push("light: at the focus alone".into());
        }
        _ => {
            // Slits high in the side walls, a bay apart, lit from outside.
            let base = match plan {
                "aisled" => aisle_h + 0.75,
                "gallery" => gallery + 2.0,
                _ => h * 0.5,
            };
            let top_ = room_top(h) - 0.5;
            if top_ - base >= 1.0 {
                for k in 0..bays {
                    let z = z0 + k as f32 * bay + (bay - 1.0) * 0.5;
                    for (x0, x1, out) in [(0.0, xa, -1.5), (xb, width + 2.0 * T, width + 2.0 * T + 1.5)] {
                        carve(&mut g, Vec3::new(x0, f + base, z - 0.25), Vec3::new(x1, f + top_, z + 0.25));
                        if k % 2 == 0 {
                            lights.push((Vec3::new(out, f + (base + top_) * 0.5, z), h + 16.0, lamp(h) * 2.0));
                        }
                        let gx = if out < 0.0 { -2.2 } else { width + 2.0 * T + 2.0 };
                        glow.push((Vec3::new(gx, f + base - 0.5, z - 1.0), Vec3::new(gx + 0.2, f + top_ + 0.5, z + 1.0)));
                    }
                }
                recipe.push("light: slits high in the side walls".into());
            } else {
                lights.push((Vec3::new(xc, f + h * 0.7, (z0 + z1) * 0.5), h * 2.0 + 8.0, lamp(h)));
                recipe.push("light: a lamp (no room for slits)".into());
            }
        }
    }
    // (Where the floor is at the door, to see from.)
    let eye = Vec3::new(xc, f + 1.7, if entry == "vestibule" { z0 - 3.0 } else { z0 + 0.5 });
    let views = vec![
        ("entry", eye, Vec3::new(xc, f + h * 0.4, z1)),
        ("corner", Vec3::new(xa + 0.75, f + h * 0.55, z0 + 0.75), Vec3::new(xb - 1.0, f + h * 0.3, z1 - 1.0)),
    ];
    Room { grid: g, lights, glow, recipe, views }
}

/// The nave between `xa` and `xb`, under its ceiling.
fn nave(g: &mut Grid, ceiling: &str, (xa, xb): (f32, f32), (z0, z1): (f32, f32), h: f32, bay: f32, cache: &mut Cache) {
    let f = FLOOR;
    let w = xb - xa;
    let len = z1 - z0;
    match ceiling {
        "vault" => g.vault(2, xa, w, f, h - w * 0.5, z0, len, cache),
        "transverse" => {
            let r = (bay - 1.0) * 0.5;
            g.fill(Vec3::new(xa, f, z0), Vec3::new(xb, f + h - r, z1), Filler::Empty);
            let n = ((len + 1.0) / bay).round() as i32;
            for k in 0..n {
                g.arch(0, z0 + k as f32 * bay, bay - 1.0, f, h - r, xa, w, cache);
            }
        }
        "stepped" => {
            g.fill(Vec3::new(xa, f, z0), Vec3::new(xb, f + h - 1.5, z1), Filler::Empty);
            g.fill(Vec3::new(xa + 1.0, f, z0 + 1.0), Vec3::new(xb - 1.0, f + h - 0.75, z1 - 1.0), Filler::Empty);
            g.fill(Vec3::new(xa + 2.0, f, z0 + 2.0), Vec3::new(xb - 2.0, f + h, z1 - 2.0), Filler::Empty);
        }
        "shaft" => {
            g.fill(Vec3::new(xa, f, z0), Vec3::new(xb, f + h, z1), Filler::Empty);
            let s = (w - 2.0).min(4.0);
            let (xc, zc) = ((xa + xb) * 0.5, half((z0 + z1) * 0.5));
            g.fill(Vec3::new(xc - s * 0.5, f + h - 1.0, zc - s * 0.5), Vec3::new(xc + s * 0.5, f + h + half((h * 0.7).clamp(3.0, 8.0)), zc + s * 0.5), Filler::Empty);
        }
        _ => g.fill(Vec3::new(xa, f, z0), Vec3::new(xb, f + h, z1), Filler::Empty),
    }
}
