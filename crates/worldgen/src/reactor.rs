//! A reactor: a colossal banded drum rising from a stepped plinth in the
//! middle of a city, ribbed up its height, crowned with setbacks and a
//! lantern; and the massive cables that hang from it to the city round it.

use std::f32::consts::TAU;

use glam::{Quat, Vec3};

use crate::structure::Solid;

fn boxed(center: Vec3, half: Vec3, rotation: Quat, albedo: f32) -> Solid {
    Solid { glow: 0.0, detail: false, wedge: false, round: false, center, rotation, half, albedo }
}

/// A ring of `n` boxes round `center` at `radius` (to their inner face),
/// `deep` deep, from `y0` to `y1`, shaded `albedo`; every `every`th one
/// (if any) a rib standing `proud` further out.
#[allow(clippy::too_many_arguments)]
fn ring(out: &mut Vec<Solid>, center: Vec3, radius: f32, deep: f32, (y0, y1): (f32, f32), n: usize, albedo: f32) {
    let width = TAU * (radius + deep) / n as f32 * 0.5 + 0.05;
    for i in 0..n {
        let a = TAU * i as f32 / n as f32;
        let p = center + Vec3::new(a.cos(), 0.0, a.sin()) * (radius + deep * 0.5) + Vec3::Y * ((y0 + y1) * 0.5);
        out.push(boxed(p, Vec3::new(deep * 0.5, (y1 - y0) * 0.5, width), Quat::from_rotation_y(-a), albedo));
    }
}

/// The reactor's radius at height `t` (0 at the base, 1 at the top), as a
/// share of its full radius: wide at the base, a long thin waist, wide
/// again at the top (like a tokamak's central column), eased between.
fn profile(t: f32) -> f32 {
    let ease = |a: f32, b: f32, x: f32| {
        let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    let waist = 0.42;
    1.0 - (1.0 - waist) * ease(0.12, 0.3, t) + (1.0 - waist) * ease(0.7, 0.86, t)
}

/// How far out the reactor's surface is at height `y`, for a reactor
/// `radius` at its widest and `height` tall (so cables start on it).
pub fn surface(radius: f32, height: f32, y: f32) -> f32 {
    radius * profile((y / (height * 0.94)).clamp(0.0, 1.0))
}

/// The reactor at `center` (its base on the ground), `radius` at its
/// widest and `height` to the top: a smooth spool (wide, a long thin
/// waist, wide) in many thin bands, cut by fine etchings running its whole
/// height that glow faintly, a few of them brightly; thin rings hovering
/// round the waist with nothing holding them, their inner edges lit; a
/// smooth base and a lit crown. No hoops, no rivets: not a boiler.
pub fn reactor(center: Vec3, radius: f32, height: f32, tone: f32) -> Vec<Solid> {
    let mut out = Vec::new();
    let n = 120;
    // A smooth low base, flush with the spool's foot.
    ring(&mut out, center, radius * 0.9, radius * 0.35, (-8.0, 6.0), n / 2, tone + 0.02);
    // The spool, band by band; the same segments in every band, so the
    // etchings line up into lines running the whole height.
    let top = height * 0.94;
    let bands = 180;
    let deep = 7.0;
    for b in 0..bands {
        let (t0, t1) = (b as f32 / bands as f32, (b + 1) as f32 / bands as f32);
        let r = radius * profile((t0 + t1) * 0.5);
        let (y0, y1) = (top * t0, top * t1 + 0.05);
        for i in 0..n {
            let a = TAU * i as f32 / n as f32;
            let etched = i % 4 == 0;
            let seam = i % 30 == 0;
            let set = if etched { 1.6 } else { 0.0 };
            let width = TAU * r / n as f32 * 0.5 + 0.05;
            let p = center + Vec3::new(a.cos(), 0.0, a.sin()) * (r - deep * 0.5 - set) + Vec3::Y * ((y0 + y1) * 0.5);
            let mut piece = boxed(p, Vec3::new(deep * 0.5, (y1 - y0) * 0.5, width), Quat::from_rotation_y(-a), if etched { tone - 0.08 } else { tone + 0.02 });
            if etched {
                piece.glow = if seam { 0.9 } else { 0.18 };
            }
            out.push(piece);
        }
    }
    // Rings hovering round the waist: thin, flat, held by nothing, each a
    // little further out, their inner edges lit.
    let waist = radius * profile(0.5);
    for (k, t) in [0.36f32, 0.47, 0.58, 0.66].into_iter().enumerate() {
        let y = top * t;
        let inner = waist + 45.0 + 22.0 * k as f32;
        let wide = 14.0 + 6.0 * (k % 2) as f32;
        let m = 96;
        ring(&mut out, center, inner, wide, (y - 1.6, y + 1.6), m, tone + 0.06);
        let start = out.len();
        ring(&mut out, center, inner - 0.6, 0.8, (y - 0.6, y + 0.6), m, tone + 0.1);
        for piece in &mut out[start..] {
            piece.glow = 0.7;
        }
    }
    // The crown: a smooth cap, a lit band, a slender lantern.
    let r = radius * profile(1.0);
    ring(&mut out, center, r * 0.75, r * 0.25, (top, top + height * 0.015), n / 2, tone + 0.03);
    let start = out.len();
    ring(&mut out, center, r * 0.7, r * 0.05, (top + height * 0.015, top + height * 0.02), n / 2, tone + 0.1);
    for piece in &mut out[start..] {
        piece.glow = 1.0;
    }
    ring(&mut out, center, r * 0.15, r * 0.05, (top + height * 0.02, height), 24, tone + 0.05);
    out
}

/// A massive anchor for a cable's end on a deck, at `at` (on the deck's
/// top, at its edge) facing out along `out`.
pub fn anchor(at: Vec3, out_dir: Vec3, size: f32, tone: f32) -> Vec<Solid> {
    let yaw = Quat::from_rotation_y(-out_dir.z.atan2(out_dir.x));
    let mut out = Vec::new();
    // A block set back from the edge, a lit slot where the cable enters.
    out.push(boxed(at - out_dir * (size * 0.6) + Vec3::Y * (size * 0.5), Vec3::new(size * 0.6, size * 0.5, size * 0.7), yaw, tone + 0.03));
    let mut slot = boxed(at - out_dir * 0.05 + Vec3::Y * (size * 0.5), Vec3::new(0.1, size * 0.15, size * 0.4), yaw, tone + 0.1);
    slot.glow = 0.6;
    out.push(slot);
    out
}

/// A massive cable hanging from `a` to `b`, sagging `sag` metres below the
/// straight line at its middle, `radius` thick, with collars at both ends.
pub fn cable(a: Vec3, b: Vec3, sag: f32, radius: f32, tone: f32) -> Vec<Solid> {
    let mut out = Vec::new();
    let point = |t: f32| a.lerp(b, t) - Vec3::Y * (sag * 4.0 * t * (1.0 - t));
    let segments = 18;
    for i in 0..segments {
        let (p, q) = (point(i as f32 / segments as f32), point((i + 1) as f32 / segments as f32));
        let d = q - p;
        let len = d.length();
        if len < 0.01 {
            continue;
        }
        let turn = Quat::from_rotation_arc(Vec3::X, d / len);
        out.push(Solid { round: true, ..boxed((p + q) * 0.5, Vec3::new(len * 0.5 + radius * 0.3, radius, radius), turn, tone) });
    }
    for (p, q) in [(a, point(0.05)), (b, point(0.95))] {
        let d = (q - p).normalize_or_zero();
        let turn = Quat::from_rotation_arc(Vec3::X, if d == Vec3::ZERO { Vec3::X } else { d });
        out.push(boxed(p + d * radius, Vec3::new(radius * 1.2, radius * 1.6, radius * 1.6), turn, tone + 0.04));
    }
    out
}
