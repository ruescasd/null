//! Clusters: masses made of cylinders. Drums of very different sizes,
//! packed and fused on a footprint, taller towards its middle, some
//! stepping in as they rise; a few lying across between them; and a tangle
//! of small tubes sagging from drum to drum or winding down their sides and
//! out over the ground. No frames, no fittings: the mass is the cylinders.

use glam::{Quat, Vec2, Vec3};

use crate::forms::inward_normals;
use crate::noise::hash01;
use crate::structure::Solid;

/// A round solid from `a` to `b`.
fn tube(a: Vec3, b: Vec3, radius: f32, albedo: f32) -> Option<Solid> {
    let d = b - a;
    let dir = d.try_normalize()?;
    Some(Solid {
        wedge: false,
        round: true,
        center: (a + b) * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::X, dir),
        half: Vec3::new(d.length() * 0.5, radius, radius),
        albedo,
    })
}

/// A curved tube through `points`, its segments overlapping a little so
/// the joints do not show.
fn curve(out: &mut Vec<Solid>, points: &[Vec3], radius: f32, albedo: f32) {
    for w in points.windows(2) {
        let dir = (w[1] - w[0]).normalize_or_zero();
        out.extend(tube(w[0] - dir * radius * 0.6, w[1] + dir * radius * 0.6, radius, albedo));
    }
}

struct Drum {
    center: Vec2,
    radius: f32,
    height: f32,
}

/// Builds a cluster on `poly` (convex) from `floor`, about `height` tall at
/// its middle.
pub fn build(poly: &[Vec2], floor: f32, height: f32, tone: f32, seed: u32) -> Vec<Solid> {
    let mut out = Vec::new();
    if poly.len() < 3 {
        return out;
    }
    let inward = inward_normals(poly);
    let inside = |p: Vec2, margin: f32| inward.iter().enumerate().all(|(i, n)| n.dot(p - poly[i]) >= margin);
    let center = poly.iter().copied().sum::<Vec2>() / poly.len() as f32;
    let reach = inward.iter().enumerate().map(|(i, n)| n.dot(center - poly[i])).fold(f32::MAX, f32::min).max(1.0);
    let (lo, hi) = poly.iter().fold((poly[0], poly[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let r = |a: i32, b: i32| hash01(a, b, 0xd0d, seed);

    // Drums: few huge, many small; they may overlap a little, fusing.
    let mut drums: Vec<Drum> = Vec::new();
    let biggest = reach * 0.45;
    for k in 0..3000 {
        if drums.len() >= 220 {
            break;
        }
        let p = lo + (hi - lo) * Vec2::new(r(k, 0), r(k, 1));
        let radius = (biggest * r(k, 2).powf(2.0)).max(0.4);
        if !inside(p, radius * 0.6) {
            continue;
        }
        // Heavily overlapping: fused rather than standing side by side.
        if drums.iter().any(|d| d.center.distance(p) < (d.radius + radius) * 0.5 || d.center.distance(p) < d.radius.max(radius) * 0.9) {
            continue;
        }
        let middle = (1.0 - p.distance(center) / reach).clamp(0.0, 1.0);
        let tall = height * (0.25 + 0.75 * middle) * (0.55 + 0.45 * r(k, 3)) * (0.6 + 0.6 * (radius / biggest).sqrt());
        drums.push(Drum { center: p, radius, height: tall.max(radius * 1.5) });
    }
    for (i, d) in drums.iter().enumerate() {
        let i = i as i32;
        let shade = (tone + (r(i, 10) - 0.4) * 0.06).clamp(0.04, 0.3);
        // Stepping in as it rises: one to three sections, now and then a
        // narrow neck between them.
        let sections = 1 + (r(i, 11) * 3.0) as i32;
        let mut y = 0.0;
        let mut radius = d.radius;
        for s in 0..sections {
            let h = if s + 1 == sections { d.height - y } else { d.height * (0.3 + 0.3 * r(i, 12 + s)) };
            if h <= 0.2 {
                break;
            }
            let base = Vec3::new(d.center.x, floor + y - if s == 0 { 2.0 } else { 0.0 }, d.center.y);
            out.extend(tube(base, Vec3::new(d.center.x, floor + y + h, d.center.y), radius, shade));
            y += h;
            if s + 1 < sections && r(i, 20 + s) < 0.5 {
                let neck = 0.6 + 1.2 * r(i, 30 + s);
                let a = Vec3::new(d.center.x, floor + y, d.center.y);
                out.extend(tube(a, a + Vec3::Y * neck, radius * 0.6, (shade - 0.03).max(0.03)));
                y += neck;
            }
            radius *= 0.7 + 0.22 * r(i, 40 + s);
        }
    }

    // A few drums lying across between tall neighbours.
    let mut spans = 0;
    for (i, a) in drums.iter().enumerate() {
        for (j, b) in drums.iter().enumerate().skip(i + 1) {
            if spans >= 4 || r(i as i32 * 97 + j as i32, 50) > 0.08 {
                continue;
            }
            let gap = a.center.distance(b.center) - a.radius - b.radius;
            if gap < 2.0 || gap > reach * 0.6 || a.height < 12.0 || b.height < 12.0 {
                continue;
            }
            let y = floor + a.height.min(b.height) * (0.4 + 0.4 * r(i as i32 * 97 + j as i32, 51));
            let radius = (a.radius.min(b.radius) * 0.5).clamp(0.6, 3.0);
            let dir = (b.center - a.center).normalize_or_zero();
            let pa = a.center + dir * a.radius * 0.5;
            let pb = b.center - dir * b.radius * 0.5;
            out.extend(tube(Vec3::new(pa.x, y, pa.y), Vec3::new(pb.x, y, pb.y), radius, tone + 0.02));
            spans += 1;
        }
    }

    // The tangle.
    let thin = |k: i32, j: i32| 0.08 + 0.4 * r(k, j).powf(2.5);
    for k in 0..700 {
        let shade = (tone + (r(k, 60) - 0.5) * 0.12).clamp(0.03, 0.35);
        let Some(a) = drums.get((r(k, 61) * drums.len() as f32) as usize) else { break };
        if r(k, 62) < 0.3 {
            // Sagging from this drum to a neighbour.
            let near: Vec<&Drum> = drums
                .iter()
                .filter(|b| {
                    let d = b.center.distance(a.center);
                    d > 0.1 && d < (a.radius + b.radius) * 3.0 + 6.0
                })
                .collect();
            let Some(b) = near.get((r(k, 63) * near.len() as f32) as usize) else { continue };
            let dir = (b.center - a.center).normalize_or_zero();
            let side = Vec2::new(-dir.y, dir.x) * (r(k, 64) - 0.5) * a.radius.min(b.radius);
            let radius = thin(k, 65);
            let ya = floor + a.height * (0.2 + 0.75 * r(k, 66));
            let yb = floor + b.height * (0.2 + 0.75 * r(k, 67));
            let pa = a.center + dir * (a.radius + radius) + side;
            let pb = b.center - dir * (b.radius + radius) + side;
            let (pa, pb) = (Vec3::new(pa.x, ya, pa.y), Vec3::new(pb.x, yb, pb.y));
            let sag = pa.distance(pb) * (0.1 + 0.3 * r(k, 68));
            let points: Vec<Vec3> = (0..=8)
                .map(|s| {
                    let t = s as f32 / 8.0;
                    pa.lerp(pb, t) - Vec3::Y * sag * 4.0 * t * (1.0 - t)
                })
                .collect();
            curve(&mut out, &points, radius, shade);
        } else {
            // A bundle winding down the drum's side and out over the ground.
            let count = 1 + (r(k, 70) * 4.0) as i32;
            let top = a.height * (0.3 + 0.7 * r(k, 71));
            let turn = (r(k, 72) - 0.5) * 2.5;
            let start = r(k, 73) * std::f32::consts::TAU;
            for m in 0..count {
                let radius = thin(k * 8 + m, 74).min(a.radius * 0.25);
                let around = a.radius + radius * 1.1;
                let offset = m as f32 * radius * 2.3 / around;
                let mut points: Vec<Vec3> = (0..=10)
                    .map(|s| {
                        let t = s as f32 / 10.0;
                        let angle = start + offset + turn * t;
                        let p = a.center + Vec2::new(angle.cos(), angle.sin()) * around;
                        Vec3::new(p.x, floor + top * (1.0 - t) + radius * 0.5, p.y)
                    })
                    .collect();
                // Out over the ground, sinking into it.
                let last = *points.last().unwrap();
                let out_dir = (Vec2::new(last.x, last.z) - a.center).normalize_or_zero();
                let run = 2.0 + 6.0 * r(k * 8 + m, 75);
                let end = Vec2::new(last.x, last.z) + out_dir * run;
                points.push(Vec3::new(end.x, floor + radius * 0.2, end.y));
                points.push(Vec3::new(end.x + out_dir.x * 1.5, floor - radius * 2.0, end.y + out_dir.y * 1.5));
                curve(&mut out, &points, radius, shade);
            }
        }
    }
    out
}
