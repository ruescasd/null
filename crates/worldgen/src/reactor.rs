//! A reactor: a colossal banded drum rising from a stepped plinth in the
//! middle of a city, ribbed up its height, crowned with setbacks and a
//! lantern; and the massive cables that hang from it to the city round it.

use std::f32::consts::TAU;

use glam::{Quat, Vec3};

use crate::structure::Solid;

fn boxed(center: Vec3, half: Vec3, rotation: Quat, albedo: f32) -> Solid {
    Solid { detail: false, wedge: false, round: false, center, rotation, half, albedo }
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

/// The reactor at `center` (its base on the ground), `radius` at its
/// widest and `height` to the top of the crown: a stepped plinth, then a
/// spool (wide, a long thin waist, wide) in many thin bands, grooved by
/// fine vertical etchings that run its whole height, joints every so
/// often, a crown and a lantern.
pub fn reactor(center: Vec3, radius: f32, height: f32, tone: f32) -> Vec<Solid> {
    let mut out = Vec::new();
    // A stepped plinth.
    let n = 120;
    ring(&mut out, center, radius * 1.05, radius * 0.3, (-8.0, 16.0), n / 2, tone + 0.02);
    ring(&mut out, center, radius * 1.05, radius * 0.14, (16.0, 34.0), n / 2, tone + 0.03);
    // The spool, band by band; the same segments in every band, so their
    // grooves line up into etchings running the whole height.
    let top = height * 0.94;
    let bands = 90;
    let deep = 7.0;
    for b in 0..bands {
        let (t0, t1) = (b as f32 / bands as f32, (b + 1) as f32 / bands as f32);
        let r = radius * profile((t0 + t1) * 0.5);
        let (y0, y1) = (top * t0, top * t1 + 0.05);
        let joint = b % 9 == 8;
        for i in 0..n {
            let a = TAU * i as f32 / n as f32;
            // Every fourth segment a groove, set back; joints a little proud.
            let groove = i % 4 == 0 && !joint;
            let set = if groove { 2.5 } else if joint { -1.2 } else { 0.0 };
            let width = TAU * r / n as f32 * 0.5 + 0.05;
            let p = center + Vec3::new(a.cos(), 0.0, a.sin()) * (r - deep * 0.5 - set) + Vec3::Y * ((y0 + y1) * 0.5);
            let shade = if groove { tone - 0.07 } else if joint { tone + 0.04 } else { tone - 0.01 };
            out.push(boxed(p, Vec3::new(deep * 0.5, (y1 - y0) * 0.5, width), Quat::from_rotation_y(-a), shade));
        }
    }
    // The crown: setbacks and a lantern.
    let r = radius * profile(1.0);
    ring(&mut out, center, r * 0.8, r * 0.2, (top, top + height * 0.02), n / 2, tone + 0.02);
    ring(&mut out, center, r * 0.55, r * 0.2, (top + height * 0.02, top + height * 0.035), n / 2, tone + 0.03);
    ring(&mut out, center, r * 0.18, r * 0.06, (top + height * 0.035, height), 24, tone + 0.05);
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
