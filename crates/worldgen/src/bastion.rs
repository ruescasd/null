//! Bastions: round towers of vast bare wall, given scale by a few things
//! at human size, after BLAME!'s megastructures. The wall is thick; at a
//! few levels a band of it opens into a row of deep arches (or narrow
//! slots) with a gallery behind and a dark core beyond; a door at the foot;
//! a stair spiralling up the outside from the door to each arcade in turn;
//! ledges along some arcades; and arched bridges with parapets between
//! neighbouring towers, entering through openings. Nothing else: the bare
//! wall is the point, the stair is what measures it.

use glam::{Quat, Vec2, Vec3};

use crate::forms::{centroid, inward_normals, Prism};
use crate::noise::hash01;
use crate::structure::Solid;

/// The most a step rises (metres): walkable.
const RISE: f32 = 0.42;
/// Wall and gallery depth (metres).
const WALL: f32 = 3.0;
const GALLERY: f32 = 3.5;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Arches,
    Slots,
    Door,
}

/// A band of the wall that opens: from `y0` to `y1` above the floor; bays
/// open in every bay, every other one, or (door) only one; `force` is an
/// angle whose bay always opens (a bridge or a stair arrives there).
#[derive(Clone)]
struct Band {
    y0: f32,
    y1: f32,
    kind: Kind,
    every: usize,
    force: Vec<f32>,
    ledge: bool,
}

struct Tower {
    center: Vec2,
    radius: f32,
    height: f32,
    /// Bridges arriving: (height of the deck, angle towards the other tower).
    arrivals: Vec<(f32, f32)>,
}

fn solid(center: Vec3, yaw: f32, half: Vec3, albedo: f32) -> Solid {
    Solid { glow: 0.0, detail: false, wedge: false, round: false, center, rotation: Quat::from_rotation_y(yaw), half, albedo }
}

/// The yaw that turns a box's x along the tangent at angle `a` (its z then
/// points outwards).
fn tangent_yaw(a: f32) -> f32 {
    -(a + std::f32::consts::FRAC_PI_2)
}

/// Whether angle `a` falls within `half` of angle `b`.
fn near(a: f32, b: f32, half: f32) -> bool {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d) <= half
}

/// Builds bastions on `poly` (convex) from `floor`: up to `count` towers
/// about `height` tall, joined by bridges.
pub fn build(poly: &[Vec2], floor: f32, height: f32, count: u32, tone: f32, seed: u32) -> (Vec<Prism>, Vec<Solid>) {
    let mut prisms = Vec::new();
    let mut out = Vec::new();
    if poly.len() < 3 {
        return (prisms, out);
    }
    let r = |a: i32, b: i32| hash01(a, b, 0xba5, seed);
    let inward = inward_normals(poly);
    let room = |p: Vec2| inward.iter().enumerate().map(|(i, n)| n.dot(p - poly[i])).fold(f32::MAX, f32::min);
    let c = centroid(poly);
    let reach = room(c).max(5.0);

    // Towers: the first in the middle, the rest where they fit, with room
    // for a bridge between.
    let mut towers: Vec<Tower> = Vec::new();
    let (lo, hi) = poly.iter().fold((poly[0], poly[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    for k in 0..400 {
        if towers.len() as u32 >= count.max(1) {
            break;
        }
        let first = towers.is_empty();
        let radius = if count <= 1 { reach * 0.9 } else { reach * (0.22 + 0.12 * r(k, 0)) };
        let p = if first && count <= 1 { c } else { lo + (hi - lo) * Vec2::new(r(k, 1), r(k, 2)) };
        if room(p) < radius {
            continue;
        }
        if towers.iter().any(|t| t.center.distance(p) < t.radius + radius + 25.0) {
            continue;
        }
        let tall = height * (0.65 + 0.35 * r(k, 3));
        towers.push(Tower { center: p, radius: radius.max(6.0), height: tall, arrivals: Vec::new() });
    }

    // Bridges between each tower and its nearest neighbour.
    let mut bridges: Vec<(usize, usize, f32)> = Vec::new();
    for i in 0..towers.len() {
        let Some(j) = (0..towers.len())
            .filter(|&j| j != i)
            .min_by(|&a, &b| towers[a].center.distance(towers[i].center).total_cmp(&towers[b].center.distance(towers[i].center)))
        else {
            continue;
        };
        if bridges.iter().any(|&(a, b, _)| (a, b) == (i.min(j), i.max(j))) {
            continue;
        }
        let top = towers[i].height.min(towers[j].height);
        let y = (top * (0.3 + 0.35 * r(i as i32 * 7 + j as i32, 4))).max(12.0);
        bridges.push((i.min(j), i.max(j), y));
    }
    for &(i, j, y) in &bridges {
        let d = towers[j].center - towers[i].center;
        let a_ij = d.y.atan2(d.x);
        towers[i].arrivals.push((y, a_ij));
        towers[j].arrivals.push((y, a_ij + std::f32::consts::PI));
    }

    for (t, tower) in towers.iter().enumerate() {
        build_tower(&mut prisms, &mut out, tower, floor, tone, seed ^ (t as u32 + 1).wrapping_mul(0x9e37_79b9));
    }
    for &(i, j, y) in &bridges {
        bridge(&mut out, &towers[i], &towers[j], floor + y, tone);
    }
    (prisms, out)
}

fn build_tower(prisms: &mut Vec<Prism>, out: &mut Vec<Solid>, t: &Tower, floor: f32, tone: f32, seed: u32) {
    let r = |a: i32, b: i32| hash01(a, b, 0x70e, seed);
    let tau = std::f32::consts::TAU;
    let sides = ((t.radius * tau / 6.0).round() as usize).clamp(16, 96);
    let da = tau / sides as f32;
    let chord = 2.0 * t.radius * (da * 0.5).sin();
    let wall_tone = (tone + 0.04).clamp(0.05, 0.4);
    let dark = (tone * 0.35).max(0.02);

    // The bands: a door at the foot, the bridges' arrivals, then arcades and
    // slots in between, with long stretches of bare wall.
    let door = r(0, 0) * tau;
    let mut bands = vec![Band { y0: 0.0, y1: 8.0 + 4.0 * r(0, 1), kind: Kind::Door, every: 1, force: vec![door], ledge: false }];
    for &(y, a) in &t.arrivals {
        let h = 4.5 + 2.0 * r((y * 10.0) as i32, 2);
        bands.push(Band { y0: y, y1: y + h, kind: Kind::Arches, every: 1 + (r((y * 10.0) as i32, 3) * 2.0) as usize, force: vec![a], ledge: false });
    }
    let mut y = 30.0 + 40.0 * r(1, 0);
    let mut k = 0;
    while y < t.height - 14.0 && k < 12 {
        let h = if r(k, 10) < 0.6 { 4.5 + 2.5 * r(k, 11) } else { 2.5 + 1.0 * r(k, 11) };
        let clash = bands.iter().any(|b| y < b.y1 + 8.0 && y + h > b.y0 - 8.0);
        if !clash && y + h < t.height - 6.0 {
            let kind = if h > 4.0 { Kind::Arches } else { Kind::Slots };
            bands.push(Band { y0: y, y1: y + h, kind, every: 1 + (r(k, 12) * 2.0) as usize, force: vec![], ledge: r(k, 13) < 0.4 });
        }
        y += h + 25.0 + 70.0 * r(k, 14);
        k += 1;
    }
    bands.sort_by(|a, b| a.y0.total_cmp(&b.y0));

    // The stair: from the door, round the outside, up to each arch band in
    // turn, arriving at a bay it opens.
    let flights: Vec<f32> = bands.iter().filter(|b| b.kind == Kind::Arches).map(|b| b.y0).collect();
    let turn = if r(2, 0) < 0.5 { 1.0 } else { -1.0 };
    let width = 2.2;
    let mid = t.radius + width * 0.5;
    let step_angle = 0.55 / mid * turn;
    let mut angle = door + turn * da * 1.2;
    let mut level = 0.0;
    let mut arrivals: Vec<(f32, f32)> = Vec::new();
    for &target in &flights {
        let steps = ((target - level) / RISE).ceil() as i32;
        if steps < 1 {
            continue;
        }
        let rise = (target - level) / steps as f32;
        for s in 0..steps {
            let a = angle + step_angle * s as f32;
            let top = level + rise * (s + 1) as f32;
            let p = t.center + Vec2::new(a.cos(), a.sin()) * mid;
            out.push(solid(
                Vec3::new(p.x, floor + top - 0.35, p.y),
                tangent_yaw(a),
                Vec3::new(0.32, 0.35, width * 0.5),
                wall_tone,
            ));
        }
        angle += step_angle * steps as f32;
        // A landing, and the bay there opens.
        let p = t.center + Vec2::new(angle.cos(), angle.sin()) * mid;
        out.push(solid(Vec3::new(p.x, floor + target - 0.35, p.y), tangent_yaw(angle), Vec3::new(1.8, 0.35, width * 0.5), wall_tone));
        arrivals.push((target, angle));
        angle += step_angle * 4.0;
        level = target;
    }
    for band in &mut bands {
        for &(y, a) in &arrivals {
            if (band.y0 - y).abs() < 0.01 {
                band.force.push(a);
            }
        }
    }

    // The wall, segment by segment.
    for i in 0..sides {
        let a = (i as f32 + 0.5) * da;
        let out_dir = Vec2::new(a.cos(), a.sin());
        let at = |radius: f32, y: f32| {
            let p = t.center + out_dir * radius;
            Vec3::new(p.x, floor + y, p.y)
        };
        let face = |off: f32, y: f32| at(t.radius + off, y);
        let yaw = tangent_yaw(a);
        let along = Vec3::new(-a.sin(), 0.0, a.cos());
        let mut solid_from = -2.0;
        for band in &bands {
            let open = band.force.iter().any(|&f| near(a, f, da * 0.5))
                || (band.kind != Kind::Door && i % band.every == 0);
            if !open || band.y0 < solid_from {
                continue;
            }
            // Wall up to the band.
            if band.y0 > solid_from {
                let (y0, y1) = (solid_from, band.y0);
                out.push(solid(at(t.radius - WALL * 0.5, (y0 + y1) * 0.5), yaw, Vec3::new(chord * 0.5 + 0.05, (y1 - y0) * 0.5, WALL * 0.5), wall_tone));
            }
            bay(out, band, &face, along, yaw, chord, wall_tone, dark);
            solid_from = band.y1;
        }
        if t.height > solid_from {
            let (y0, y1) = (solid_from, t.height);
            out.push(solid(at(t.radius - WALL * 0.5, (y0 + y1) * 0.5), yaw, Vec3::new(chord * 0.5 + 0.05, (y1 - y0) * 0.5, WALL * 0.5), wall_tone));
        }
        // The roof over wall and gallery, a parapet round it.
        let inner = 2.0 * (t.radius - WALL - GALLERY).max(1.0) * (da * 0.5).sin();
        out.push(solid(at(t.radius - (WALL + GALLERY) * 0.5, t.height - 0.5), yaw, Vec3::new((chord + inner) * 0.25 + 0.05, 0.5, (WALL + GALLERY) * 0.5), wall_tone));
        out.push(solid(at(t.radius - 0.4, t.height + 0.6), yaw, Vec3::new(chord * 0.5, 0.6, 0.4), wall_tone));
        // Ledges along some arcades.
        for band in bands.iter().filter(|b| b.ledge) {
            out.push(solid(at(t.radius + 0.9, band.y0 - 0.3), yaw, Vec3::new(chord * 0.5 + 0.05, 0.3, 0.9), wall_tone));
        }
    }
    // The dark core.
    let core_r = (t.radius - WALL - GALLERY).max(1.0);
    let core: Vec<Vec2> = (0..sides).map(|i| t.center + Vec2::from_angle(i as f32 * da) * core_r).collect();
    prisms.push(Prism { points: core, y0: floor - 2.0, y1: floor + t.height, top_scale: 1.0, lean: Vec2::ZERO, albedo: dark });
}

/// One open bay of a band: piers, the opening's head, and the gallery's
/// floor and ceiling behind it. `at(radial, y)` is a point on the bay's
/// middle line, `radial` metres out from the wall's outer face.
#[allow(clippy::too_many_arguments)]
fn bay(out: &mut Vec<Solid>, band: &Band, at: &dyn Fn(f32, f32) -> Vec3, along: Vec3, yaw: f32, chord: f32, tone: f32, dark: f32) {
    let (y0, y1) = (band.y0, band.y1);
    let h = y1 - y0;
    // A piece of the wall's thickness, `x0..x1` along the bay.
    let piece = |out: &mut Vec<Solid>, x0: f32, x1: f32, ya: f32, yb: f32| {
        if x1 - x0 < 0.01 || yb - ya < 0.01 {
            return;
        }
        let center = at(-WALL * 0.5, (ya + yb) * 0.5) + along * ((x0 + x1) * 0.5);
        out.push(solid(center, yaw, Vec3::new((x1 - x0) * 0.5, (yb - ya) * 0.5, WALL * 0.5), tone));
    };
    let half = chord * 0.5 + 0.05;
    match band.kind {
        Kind::Slots => {
            // Two or three narrow slots between piers.
            let n = if chord > 5.0 { 3 } else { 2 };
            let pitch = chord / n as f32;
            let slot = (pitch * 0.35).max(0.6);
            let mut x = -half;
            for s in 0..n {
                let c = -chord * 0.5 + pitch * (s as f32 + 0.5);
                piece(out, x, c - slot * 0.5, y0, y1);
                x = c + slot * 0.5;
            }
            piece(out, x, half, y0, y1);
        }
        Kind::Arches | Kind::Door => {
            // A door is one arch; an arcade, small arches about as wide as
            // a door, as many as the segment holds.
            let n = if band.kind == Kind::Door { 1 } else { ((chord / 3.2).round() as usize).max(1) };
            let pitch = chord / n as f32;
            let pier = (pitch * 0.3).max(0.5);
            let w = pitch - pier;
            let rad = w * 0.5;
            let lintel = (h * 0.12).max(0.5);
            let spring = (y1 - lintel - rad).max(y0 + 1.5);
            let mut x = -half;
            for k in 0..n {
                let c = -chord * 0.5 + pitch * (k as f32 + 0.5);
                piece(out, x, c - w * 0.5, y0, y1);
                // The head: strips from the arc up to the band's top.
                let strips = 8;
                for s in 0..strips {
                    let xa = -w * 0.5 + w * s as f32 / strips as f32;
                    let xb = xa + w / strips as f32;
                    let x_in = if xa.abs() < xb.abs() { xa } else { xb };
                    let arc = spring + (rad * rad - x_in * x_in).max(0.0).sqrt();
                    piece(out, c + xa, c + xb, arc.min(y1), y1);
                }
                x = c + w * 0.5;
            }
            piece(out, x, half, y0, y1);
        }
    }
    // The gallery behind: a dark floor and ceiling.
    for (y, dy) in [(y0, -0.25), (y1, 0.25)] {
        out.push(solid(at(-(WALL + GALLERY * 0.5), y + dy), yaw, Vec3::new(chord * 0.45, 0.25, GALLERY * 0.5 + 0.1), dark));
    }
}

/// An arched bridge with parapets between two towers, its deck at `y`.
fn bridge(out: &mut Vec<Solid>, a: &Tower, b: &Tower, y: f32, tone: f32) {
    let d = b.center - a.center;
    let dist = d.length();
    let dir = d / dist.max(1e-3);
    let start = a.center + dir * (a.radius - WALL * 0.5);
    let span = dist - a.radius - b.radius + WALL;
    if span < 2.0 {
        return;
    }
    let yaw = -dir.y.atan2(dir.x);
    let width = 3.2;
    let deck = 1.0;
    let shade = (tone + 0.04).clamp(0.05, 0.4);
    let at = |x: f32, yy: f32| {
        let p = start + dir * x;
        Vec3::new(p.x, yy, p.y)
    };
    out.push(solid(at(span * 0.5, y - deck * 0.5), yaw, Vec3::new(span * 0.5, deck * 0.5, width * 0.5), shade));
    // Parapets.
    let side = Vec3::new(-dir.y, 0.0, dir.x) * (width * 0.5 - 0.15);
    for s in [-1.0, 1.0] {
        out.push(solid(at(span * 0.5, y + 0.55) + side * s, yaw, Vec3::new(span * 0.5, 0.55, 0.15), shade));
    }
    // The arch beneath: strips from the deck down to a soffit that falls
    // towards the towers.
    let rise = span * 0.22;
    let strips = 18;
    for s in 0..strips {
        let x0 = span * s as f32 / strips as f32;
        let x1 = x0 + span / strips as f32;
        let xm = (x0 + x1) * 0.5;
        let u = (xm / span) * 2.0 - 1.0;
        let soffit = y - deck - rise * u * u - 0.6;
        let top = y - deck;
        out.push(solid(at(xm, (soffit + top) * 0.5), yaw, Vec3::new((x1 - x0) * 0.5 + 0.02, (top - soffit) * 0.5, width * 0.4), shade));
    }
}
