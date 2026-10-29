//! The ground's brick (the user, 2026-10-27: the flat wilderness lacks
//! near scale, next to the canals' streetlights): one unit the ground is
//! built of, the same size everywhere, shown where it stands out of a
//! plate. The shader's subdivision (`terrain.wgsl`) splits the plates on by
//! their own rule, the same at every size, so it gives speed, not size; and
//! below 8 m it read as painted. The brick breaks the rule: square where
//! the plates are Voronoi, one size where they are every size, geometry.
//!
//! Kerbs (`--opt kerbs`, off): a row of bricks along every edge of a flat
//! plate, a little proud. They looked the best of three, part of the plate,
//! but gave the least scale; kept to try as a coursed wall's top edge (see
//! the walls in `terrain.wgsl`). Tried and dropped, as objects pasted on
//! (the user: shrubs' role, but they don't fit): heaves, clusters of up to
//! 9 bricks on one world grid risen 1-3 courses out of a plate, one to 900
//! m² (dice, debris, a ruin from above); studs, a brick two courses high
//! every 8 m along every edge, 1 m in (a car park, a cemetery where edges
//! met).

use glam::{DVec2, Vec3};

use super::{Plate, PlateWorld};
use crate::mesh::ColumnMesh;

/// The brick: its side (m), the joint between two (m).
const BRICK: (f64, f64) = (1.0, 0.04);
/// Kerbs: how far proud of the plate (m), how deep in from its edge (of a
/// brick).
const KERB: (f64, f64) = (0.08, 0.5);

impl PlateWorld {
    /// A flat plate's kerbs, `poly` its outline, `h` its top.
    pub(super) fn emit_kerbs(&self, mesh: &mut ColumnMesh, poly: &[DVec2], plate: &Plate, h: f64, origin: (f64, f64)) {
        let (side, joint) = BRICK;
        let (proud, deep) = KERB;
        let n = poly.len();
        let centroid = poly.iter().copied().sum::<DVec2>() / n as f64;
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let length = (b - a).length();
            if length <= side * 2.0 {
                continue;
            }
            let along = (b - a) / length;
            let mut inward = along.perp();
            if inward.dot(centroid - (a + b) * 0.5) < 0.0 {
                inward = -inward;
            }
            // (Whole bricks only, from a brick in at each end.)
            let count = ((length - side * 2.0) / side).floor() as usize;
            let start = (length - count as f64 * side) * 0.5;
            for k in 0..count {
                let o = a + along * (start + k as f64 * side);
                let quad = [o, o + along * (side - joint), o + along * (side - joint) + inward * side * deep, o + inward * side * deep];
                brick(mesh, &quad, (h - 0.05, h + proud), plate.albedo, origin);
            }
        }
    }
}

/// A brick: its outline (four corners, either way round), from `y.0` up to
/// `y.1`; its top and four sides.
fn brick(mesh: &mut ColumnMesh, quad: &[DVec2; 4], y: (f64, f64), albedo: f32, origin: (f64, f64)) {
    let at = |p: DVec2, h: f64| Vec3::new((p.x - origin.0) as f32, h as f32, (p.y - origin.1) as f32);
    let middle = quad.iter().copied().sum::<DVec2>() * 0.25;
    let mut face = |corners: [Vec3; 4], normal: Vec3, albedo: f32, ao: f32| {
        let first = mesh.positions.len() as u32;
        for p in corners {
            mesh.positions.push(p.to_array());
            mesh.normals.push(normal.to_array());
            mesh.albedo.push(albedo);
            mesh.ao.push(ao);
        }
        // (Wound to face `normal`.)
        let (a, b, c) = (corners[0], corners[1], corners[2]);
        let i = if (b - a).cross(c - a).dot(normal) >= 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
        mesh.indices.extend(i.iter().map(|k| first + k));
    };
    face(quad.map(|p| at(p, y.1)), Vec3::Y, albedo, 1.0);
    for i in 0..4 {
        let (a, b) = (quad[i], quad[(i + 1) % 4]);
        let mut out = (b - a).perp().normalize();
        if out.dot((a + b) * 0.5 - middle) < 0.0 {
            out = -out;
        }
        face([at(a, y.1), at(b, y.1), at(b, y.0), at(a, y.0)], Vec3::new(out.x as f32, 0.0, out.y as f32), albedo * 0.85, 0.6);
    }
}
