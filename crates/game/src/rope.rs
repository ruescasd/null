//! Ropes and tubes: a chain of points that swings under gravity, kept at its
//! length (Verlet), and strands drawn as tubes into one mesh. Cables on
//! creatures, the web, leeches, hair, roots, and the chasm's cables.

use bevy::{mesh::Indices, prelude::*};

use crate::terrain::WorldGen;

/// Points along a rope at least (a long one gets one every half metre or
/// so), and sides of every tube.
const NODES: usize = 9;
const SIDES: usize = 5;

/// One step of a rope: a chain of points swinging under gravity, kept at
/// its length, held at `start` and (if given) `end`; a hanging one kept off
/// the ground. Far from where it was (new, or after a jump), it starts again,
/// still.
pub fn rope(points: &mut Vec<Vec3>, previous: &mut Vec<Vec3>, start: Vec3, end: Option<Vec3>, length: f32, world: &WorldGen, dt: f32) {
    if points.is_empty() || points[0].distance(start) > 3.0 {
        // (A hanging one a little off the vertical, by where it hangs from:
        // dead straight down onto the ground it would stand up in a column.)
        let lean = Vec3::new((start.x * 12.9898).sin(), 0.0, (start.z * 78.233).sin()) * 0.2;
        let last = end.unwrap_or(start + (lean - Vec3::Y).normalize() * length);
        // (A point every half metre or so on a long one.)
        let nodes = ((length / 0.5).ceil() as usize).clamp(NODES, 32);
        *points = (0..nodes).map(|i| start.lerp(last, i as f32 / (nodes - 1) as f32)).collect();
        *previous = points.clone();
    }
    let n = points.len();
    let gravity = Vec3::NEG_Y * 9.8 * dt * dt;
    for i in 1..n {
        let p = points[i];
        let v = (p - previous[i]) * 0.97;
        previous[i] = p;
        points[i] = p + v + gravity;
    }
    let segment = length / (n - 1) as f32;
    for _ in 0..(n + 4).max(8) {
        points[0] = start;
        if let Some(end) = end {
            points[n - 1] = end;
        }
        for i in 0..n - 1 {
            let d = points[i + 1] - points[i];
            let l = d.length().max(1e-5);
            let fix = d * ((l - segment) / l) * 0.5;
            points[i] += fix;
            points[i + 1] -= fix;
        }
    }
    points[0] = start;
    if let Some(end) = end {
        points[n - 1] = end;
    }
    // Not through the ground (taken as running straight between the
    // ground under its ends, or level under a hanging one).
    let floor = world.ground_height(start.x, start.z) + 0.02;
    let far = end.map_or(floor, |e| world.ground_height(e.x, e.z) + 0.02);
    for (i, p) in points.iter_mut().enumerate().skip(1) {
        p.y = p.y.max(floor + (far - floor) * i as f32 / (n - 1) as f32);
    }
}

/// Strands as tubes, into one mesh: each its points, its thickness at the
/// start and the end, and its shade.
pub fn tubes<'a>(mesh: &mut Mesh, strands: impl IntoIterator<Item = (&'a [Vec3], (f32, f32), f32)>) {
    let (mut positions, mut normals, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (points, width, shade) in strands {
        let n = points.len();
        if n < 2 {
            continue;
        }
        let shade = [shade, shade, shade, 1.0];
        let mut normal = (points[1] - points[0]).normalize_or(Vec3::Y).any_orthonormal_vector();
        let base = positions.len() as u32;
        for i in 0..n {
            let tangent = (points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)]).normalize_or(Vec3::Y);
            // (Carried along so the tube does not twist.)
            normal = (normal - tangent * normal.dot(tangent)).normalize_or(tangent.any_orthonormal_vector());
            let binormal = tangent.cross(normal);
            let t = i as f32 / (n - 1) as f32;
            let width = width.0 + (width.1 - width.0) * t;
            for k in 0..SIDES {
                let a = k as f32 / SIDES as f32 * std::f32::consts::TAU;
                let out = normal * a.cos() + binormal * a.sin();
                positions.push((points[i] + out * width).to_array());
                normals.push(out.to_array());
                colors.push(shade);
            }
        }
        for i in 0..n as u32 - 1 {
            for k in 0..SIDES as u32 {
                let (a, b) = (base + i * SIDES as u32 + k, base + i * SIDES as u32 + (k + 1) % SIDES as u32);
                let (c, d) = (a + SIDES as u32, b + SIDES as u32);
                // (Wound to face outwards: seen from outside.)
                indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}
