//! Curtains: wiring as an element in its own right, after the Citadel in
//! Half-Life 2. A broken ring of tall walls stands round an open shaft, and
//! sheaves of long cables (tens in each) cross the void between them on
//! smooth sagging curves, fanning apart and drawing together, or hang down
//! the walls' inner faces to the floor. Thin against the space, but many.
//! The walls carry the pipe network sunk into their faces.

use glam::{Vec2, Vec3};

use crate::dressing::Tube;
use crate::forms::{inward_normals, Prism};
use crate::noise::hash01;
use crate::rack::{network, Rack, Surface};
use crate::structure::Solid;

/// A quadratic curve from `a` through control `c` to `b`, as a tube.
fn cable(out: &mut Vec<Solid>, a: Vec3, c: Vec3, b: Vec3, radius: f32, albedo: f32) {
    const STEPS: usize = 14;
    let point = |t: f32| a * (1.0 - t) * (1.0 - t) + c * 2.0 * t * (1.0 - t) + b * t * t;
    let points: Vec<Vec3> = (0..=STEPS).map(|s| point(s as f32 / STEPS as f32)).collect();
    crate::cluster::curve(out, &points, radius, albedo);
}

/// The walls (as prisms), the cables, and the walls' sunk pipes, on `poly`
/// (convex) from `floor`, `height` tall.
pub fn build(poly: &[Vec2], floor: f32, height: f32, tone: f32, seed: u32) -> (Vec<Prism>, Vec<Solid>, Vec<Tube>) {
    let mut walls = Vec::new();
    let mut out = Vec::new();
    let mut pipes = Rack::default();
    let r = |a: i32, b: i32| hash01(a, b, 0xc17, seed);
    let n = poly.len();
    if n < 3 {
        return (walls, out, Vec::new());
    }
    let inward = inward_normals(poly);
    let thick = 4.0;
    // The walls: most edges get one, leaving gaps into the shaft; each with
    // its inner face (a line at the wall's foot, facing in) and height.
    let mut faces: Vec<(Vec2, Vec2, Vec2, f32)> = Vec::new();
    for i in 0..n {
        if r(i as i32, 0) < 0.25 && faces.len() + (n - i) > 3 {
            continue;
        }
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let inn = inward[i];
        let h = height * (0.75 + 0.25 * r(i as i32, 1));
        walls.push(Prism {
            points: vec![a, b, b + inn * thick, a + inn * thick],
            y0: floor - 2.0,
            y1: floor + h,
            top_scale: 1.0,
            lean: Vec2::ZERO,
            albedo: (tone - 0.02 + 0.03 * r(i as i32, 2)).clamp(0.03, 0.3),
        });
        faces.push((a + inn * thick, b + inn * thick, inn, h));
        // The pipe network sunk into both faces of the wall.
        let length = (b - a).length();
        let along = (b - a) / length.max(1e-3);
        for (start, dir, out_dir, salt) in [(a + inn * thick, along, inn, 1u32), (b, -along, -inn, 2)] {
            let face = Surface::wall(start, dir, out_dir, length, floor, h).sunk();
            network(&mut pipes, &face, 2.5, tone, seed ^ (i as u32 * 31 + salt).wrapping_mul(0x9e37_79b9));
        }
    }
    if faces.len() < 2 {
        return (walls, out, pipes.tubes);
    }
    // A point on a face's inner side: `t` along it, `y` up, a little off it.
    let on = |f: &(Vec2, Vec2, Vec2, f32), t: f32, y: f32| {
        let p = f.0.lerp(f.1, t) + f.2 * 0.3;
        Vec3::new(p.x, floor + y, p.y)
    };
    let sheaves = 10 + (r(9, 0) * 7.0) as i32;
    for k in 0..sheaves {
        let count = 20 + (r(k, 10) * 40.0) as i32;
        let radius = 0.07 + 0.12 * r(k, 11);
        let shade = (tone + 0.02 + (r(k, 12) - 0.5) * 0.08).clamp(0.03, 0.35);
        let i = (r(k, 13) * faces.len() as f32) as usize % faces.len();
        let a_face = &faces[i];
        // Where the sheaf leaves its wall: a band along the face, high up.
        let (t0, width_a) = (0.15 + 0.5 * r(k, 14), 0.1 + 0.25 * r(k, 15));
        let ya = a_face.3 * (0.55 + 0.4 * r(k, 16));
        if r(k, 17) < 0.75 {
            // Across the shaft to another wall: lower down, or (now and
            // then) high up and nearly level, criss-crossing overhead.
            let j = (i + 1 + (r(k, 18) * (faces.len() - 1) as f32) as usize) % faces.len();
            let b_face = &faces[j];
            let (u0, width_b) = (0.15 + 0.5 * r(k, 19), 0.05 + 0.35 * r(k, 20));
            let level = r(k, 24) < 0.35;
            let yb = if level { b_face.3 * (0.5 + 0.45 * r(k, 21)) } else { b_face.3 * (0.15 + 0.6 * r(k, 21)) };
            for m in 0..count {
                let s = m as f32 / (count - 1).max(1) as f32;
                let jitter = |q: i32| (r(k * 128 + m, q) - 0.5) * 0.04;
                let a = on(a_face, t0 + width_a * s + jitter(30), ya + jitter(31) * 20.0);
                let b = on(b_face, u0 + width_b * (1.0 - s) + jitter(32), yb + jitter(33) * 20.0);
                let span = a.distance(b);
                let sag = span * (0.12 + 0.1 * r(k, 22)) + jitter(34) * span;
                let c = (a + b) * 0.5 - Vec3::Y * sag * 2.0;
                let thick = radius * (0.7 + 0.6 * r(k * 128 + m, 35));
                cable(&mut out, a, c, b, thick, shade);
            }
        } else {
            // Hanging down the wall's inner face to the floor, bowing out.
            let reach = 3.0 + 10.0 * r(k, 23);
            for m in 0..count {
                let s = m as f32 / (count - 1).max(1) as f32;
                let jitter = |q: i32| (r(k * 128 + m, q) - 0.5) * 0.03;
                let a = on(a_face, t0 + width_a * s + jitter(40), ya);
                let foot = on(a_face, t0 + width_a * (0.5 + (s - 0.5) * 1.6) + jitter(41), 0.0);
                let out_dir = Vec3::new(a_face.2.x, 0.0, a_face.2.y);
                let b = foot + out_dir * (reach * (0.6 + 0.4 * r(k * 128 + m, 42))) - Vec3::Y * 0.3;
                let c = Vec3::new(a.x, floor + ya * 0.35, a.z) + out_dir * reach * (0.3 + jitter(43) * 10.0);
                let thick = radius * (0.7 + 0.6 * r(k * 128 + m, 44));
                cable(&mut out, a, c, b, thick, shade);
            }
        }
    }
    out.extend(pipes.solids);
    (walls, out, pipes.tubes)
}
