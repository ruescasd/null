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

/// The reactor at `center` (its base on the ground), `radius` across the
/// drum and `height` to the top of the crown: a stepped plinth with giant
/// buttresses round it, a lower drum ribbed and banded, a narrower upper
/// drum with deep vertical channels, setbacks and a lantern.
pub fn reactor(center: Vec3, radius: f32, height: f32, tone: f32) -> Vec<Solid> {
    let mut out = Vec::new();
    let n = ((TAU * radius / 14.0) as usize).clamp(32, 96);
    // A stepped plinth.
    ring(&mut out, center, radius * 1.02, radius * 0.25, (-8.0, 14.0), n, tone + 0.02);
    ring(&mut out, center, radius * 1.02, radius * 0.12, (14.0, 30.0), n, tone + 0.03);
    // Giant buttresses: wedges leaning on the lower drum, all round.
    let fins = 12;
    let lower = height * 0.5;
    for i in 0..fins {
        let a = TAU * (i as f32 + 0.5) / fins as f32;
        let run = radius * 0.6;
        let rise = lower * 0.55;
        // Tall side (+x) against the drum: +x points inwards.
        let turn = Quat::from_rotation_y(-(a + std::f32::consts::PI));
        let mid = center + Vec3::new(a.cos(), 0.0, a.sin()) * (radius + run * 0.5) + Vec3::Y * ((rise - 8.0) * 0.5);
        out.push(Solid { wedge: true, ..boxed(mid, Vec3::new(run * 0.5, (rise + 8.0) * 0.5, 6.0), turn, tone + 0.01) });
    }
    // The lower drum: ribs and bands.
    ring(&mut out, center, radius - 8.0, 8.0, (0.0, lower), n, tone - 0.02);
    for i in (0..n).step_by(4) {
        let a = TAU * i as f32 / n as f32;
        let p = center + Vec3::new(a.cos(), 0.0, a.sin()) * (radius + 3.0) + Vec3::Y * (lower * 0.5);
        out.push(boxed(p, Vec3::new(3.0, lower * 0.5, 4.0), Quat::from_rotation_y(-a), tone + 0.03));
    }
    let mut y = 60.0;
    while y < lower - 20.0 {
        ring(&mut out, center, radius, 6.0, (y, y + 6.0), n, tone + 0.05);
        y += 70.0;
    }
    // A shoulder, then the upper drum: narrower, cut by deep channels.
    ring(&mut out, center, radius * 0.72, radius * 0.3, (lower, lower + 14.0), n, tone + 0.04);
    let upper = height * 0.88;
    let ur = radius * 0.72;
    let m = (n * 3 / 4).max(24);
    for i in 0..m {
        // Every third segment left out deep: a channel.
        let deep = if i % 3 == 0 { 3.0 } else { 10.0 };
        let a = TAU * i as f32 / m as f32;
        let width = TAU * ur / m as f32 * 0.5 + 0.05;
        let p = center + Vec3::new(a.cos(), 0.0, a.sin()) * (ur - 10.0 + deep * 0.5) + Vec3::Y * ((lower + upper) * 0.5);
        out.push(boxed(p, Vec3::new(deep * 0.5, (upper - lower) * 0.5, width), Quat::from_rotation_y(-a), if i % 3 == 0 { tone - 0.06 } else { tone - 0.01 }));
    }
    // The crown: setbacks and a lantern.
    ring(&mut out, center, ur * 0.86, ur * 0.18, (upper, upper + height * 0.03), m, tone + 0.02);
    ring(&mut out, center, ur * 0.6, ur * 0.16, (upper + height * 0.03, upper + height * 0.06), m, tone + 0.03);
    ring(&mut out, center, ur * 0.25, ur * 0.06, (upper + height * 0.06, height), (m / 2).max(16), tone + 0.05);
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
