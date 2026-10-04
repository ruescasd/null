//! Sunk relief: faces carved recursively into themselves. A core prism is
//! wrapped in a skin `depth` thick; each face of the skin is divided like a
//! fractal: a rectangle stays solid, splits in two, or becomes a recess (a
//! rim at its depth around a deeper panel, carved again in turn), down to
//! about half a metre. Nothing stands out of the surface: every detail is a
//! cut into it.

use glam::{Quat, Vec2, Vec3};

use crate::forms::{inset, inward_normals};
use crate::noise::hash01;
use crate::structure::Solid;

/// A rectangle of a face being carved: `u` along it, `y` up (from, to),
/// and how deep it already lies under the skin's surface.
#[derive(Clone, Copy)]
struct Patch {
    u0: f32,
    u1: f32,
    y0: f32,
    y1: f32,
    sunk: f32,
    level: u32,
    path: u32,
}

/// The core prism's outline (the polygon inset by `depth`) and the skin's
/// solids, from `floor` to `floor + height`.
pub fn build(poly: &[Vec2], floor: f32, height: f32, depth: f32, tone: f32, seed: u32) -> (Vec<Vec2>, Vec<Solid>) {
    let mut out = Vec::new();
    let core = inset(poly, depth);
    if core.len() < 3 {
        return (poly.to_vec(), out);
    }
    let inward = inward_normals(&core);
    let n = core.len();
    for i in 0..n {
        let (a, b) = (core[i], core[(i + 1) % n]);
        let length = (b - a).length();
        if length < 1.0 {
            continue;
        }
        let e = (b - a) / length;
        let out_dir = -inward[i];
        let yaw = Quat::from_rotation_y((-e.y).atan2(e.x));
        let seed = seed ^ (i as u32).wrapping_mul(0x9e37_79b9);
        let r = |path: u32, k: i32| hash01(path as i32, k, 0x5e1, seed);
        // A box on the face: u and y ranges, standing out from the core
        // face to `depth - sunk`.
        let slab = |out: &mut Vec<Solid>, u0: f32, u1: f32, y0: f32, y1: f32, sunk: f32, shade: f32| {
            let thick = depth - sunk;
            if thick < 0.02 || u1 - u0 < 0.01 || y1 - y0 < 0.01 {
                return;
            }
            let p = a + e * ((u0 + u1) * 0.5) + out_dir * (thick * 0.5);
            out.push(Solid {
                wedge: false,
                round: false,
                center: Vec3::new(p.x, floor + (y0 + y1) * 0.5, p.y),
                rotation: yaw,
                half: Vec3::new((u1 - u0) * 0.5, (y1 - y0) * 0.5, thick * 0.5),
                albedo: shade,
            });
        };
        let smallest = (height / 80.0).max(0.5);
        let mut stack = vec![Patch { u0: 0.0, u1: length, y0: 0.0, y1: height, sunk: 0.0, level: 0, path: 1 }];
        while let Some(p) = stack.pop() {
            if out.len() > 40_000 {
                break;
            }
            let (w, h) = (p.u1 - p.u0, p.y1 - p.y0);
            let shade = (tone + (r(p.path, 0) - 0.5) * 0.03 - p.sunk * 0.01).clamp(0.03, 0.4);
            let action = r(p.path, 1);
            if w.min(h) < smallest * 2.0 || p.level > 7 || (p.level > 1 && action < 0.15) {
                // Solid at its depth.
                slab(&mut out, p.u0, p.u1, p.y0, p.y1, p.sunk, shade);
            } else if action < 0.55 {
                // Split across the longer side, now and then into a row of
                // equal pieces (a rhythm), otherwise unevenly in two.
                let along = w >= h;
                let count = if r(p.path, 2) < 0.3 { 3 + (r(p.path, 3) * 4.0) as u32 } else { 2 };
                let cuts: Vec<f32> = if count == 2 {
                    vec![0.0, 0.3 + 0.4 * r(p.path, 4), 1.0]
                } else {
                    (0..=count).map(|k| k as f32 / count as f32).collect()
                };
                for k in 0..count as usize {
                    let (t0, t1) = (cuts[k], cuts[k + 1]);
                    let child = Patch {
                        u0: if along { p.u0 + w * t0 } else { p.u0 },
                        u1: if along { p.u0 + w * t1 } else { p.u1 },
                        y0: if along { p.y0 } else { p.y0 + h * t0 },
                        y1: if along { p.y1 } else { p.y0 + h * t1 },
                        level: p.level + 1,
                        path: p.path.wrapping_mul(8).wrapping_add(k as u32 + 1),
                        ..p
                    };
                    stack.push(child);
                }
            } else {
                // A recess: a rim at this depth, a deeper panel inside it.
                let rim = (w.min(h) * (0.06 + 0.12 * r(p.path, 5))).max(smallest * 0.5);
                let deeper = (p.sunk + (depth - p.sunk) * (0.25 + 0.4 * r(p.path, 6))).min(depth);
                slab(&mut out, p.u0, p.u1, p.y0, p.y0 + rim, p.sunk, shade);
                slab(&mut out, p.u0, p.u1, p.y1 - rim, p.y1, p.sunk, shade);
                slab(&mut out, p.u0, p.u0 + rim, p.y0 + rim, p.y1 - rim, p.sunk, shade);
                slab(&mut out, p.u1 - rim, p.u1, p.y0 + rim, p.y1 - rim, p.sunk, shade);
                stack.push(Patch {
                    u0: p.u0 + rim,
                    u1: p.u1 - rim,
                    y0: p.y0 + rim,
                    y1: p.y1 - rim,
                    sunk: deeper,
                    level: p.level + 1,
                    path: p.path.wrapping_mul(8).wrapping_add(7),
                });
            }
        }
    }
    (core, out)
}
