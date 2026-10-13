//! Cloth: a grid of points swinging under gravity (Verlet), kept at the
//! lengths between them (round and down, and across the diagonals and two
//! apart, more softly, so it resists shearing and folds rather than
//! crumpling), some held where they are put, pushed out of bodies (capsules)
//! and kept above a floor; drawn as one mesh, rebuilt every frame. The rope's
//! way of doing things (see `rope.rs`), in two dimensions.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// How much of its speed a point keeps a step (the air's drag), and how
/// many times a step the lengths are put right (more: stiffer, less
/// stretching in a sudden jerk).
const DAMPING: f32 = 0.985;
const ITERATIONS: usize = 16;
/// How far a point may go in a step (m): a body jerked hard flung its robe
/// up over its head (the user: mangled).
const FASTEST: f32 = 0.12;

pub struct Cloth {
    pub points: Vec<Vec3>,
    previous: Vec<Vec3>,
    /// Two points, their length apart, and how firmly it is kept (0..1).
    links: Vec<(u32, u32, f32, f32)>,
    triangles: Vec<u32>,
}

impl Cloth {
    /// A tube `cols` points round and `rows` down, each starting where `at`
    /// puts it (its lengths those it starts with), its folds kept by `bend`
    /// (by row, 0..1: how firmly it keeps the shape it starts in; 0.3 soft),
    /// slit down its seam (between its last column and its first) in the
    /// rows `slit` names: a hood's opening.
    pub fn tube(cols: usize, rows: usize, at: impl Fn(usize, usize) -> Vec3, bend: impl Fn(usize) -> f32, slit: impl Fn(usize) -> bool) -> Cloth {
        let idx = |c: usize, r: usize| (r * cols + c % cols) as u32;
        let points: Vec<Vec3> = (0..rows).flat_map(|r| (0..cols).map(move |c| (c, r))).map(|(c, r)| at(c, r)).collect();
        // (Across the seam, in a slit row: not joined.)
        let cut = |c: usize, r: usize, by: usize| c + by >= cols && slit(r);
        let mut links = Vec::new();
        {
            let mut link = |a: u32, b: u32, firm: f32| links.push((a, b, points[a as usize].distance(points[b as usize]), firm));
            for r in 0..rows {
                for c in 0..cols {
                    let p = idx(c, r);
                    if !cut(c, r, 1) {
                        link(p, idx(c + 1, r), 1.0);
                    }
                    if !cut(c, r, 2) {
                        link(p, idx(c + 2, r), bend(r));
                    }
                    if r + 1 < rows {
                        link(p, idx(c, r + 1), 1.0);
                        if !cut(c, r, 1) && !cut(c, r + 1, 1) {
                            link(p, idx(c + 1, r + 1), 0.5);
                            link(idx(c + 1, r), idx(c, r + 1), 0.5);
                        }
                    }
                    if r + 2 < rows {
                        link(p, idx(c, r + 2), bend(r));
                    }
                }
            }
        }
        let mut triangles = Vec::new();
        for r in 0..rows - 1 {
            for c in 0..cols {
                if cut(c, r, 1) || cut(c, r + 1, 1) {
                    continue;
                }
                let (a, b, d, e) = (idx(c, r), idx(c + 1, r), idx(c, r + 1), idx(c + 1, r + 1));
                triangles.extend([a, d, b, b, d, e]);
            }
        }
        Cloth { previous: points.clone(), points, links, triangles }
    }

    /// One step: under gravity, kept at its lengths, the `held` points where
    /// they are put, out of the `bodies` (capsules: two ends and a radius),
    /// above `floor` and below `ceiling` (a robe never over its shoulders:
    /// swung far aside, again and again, one turned up over its head). Held
    /// points far from where they were (put somewhere new) take the whole
    /// cloth with them, still.
    pub fn step(&mut self, held: &[(u32, Vec3)], bodies: &[(Vec3, Vec3, f32)], floor: f32, ceiling: f32, dt: f32) {
        if let Some(&(i, at)) = held.first() {
            let jump = at - self.points[i as usize];
            if jump.length() > 3.0 {
                for (p, q) in self.points.iter_mut().zip(self.previous.iter_mut()) {
                    *p += jump;
                    *q = *p;
                }
            }
        }
        let gravity = Vec3::NEG_Y * 9.8 * dt * dt;
        for (p, q) in self.points.iter_mut().zip(self.previous.iter_mut()) {
            let v = ((*p - *q) * DAMPING).clamp_length_max(FASTEST);
            *q = *p;
            *p += v + gravity;
        }
        for _ in 0..ITERATIONS {
            for &(i, at) in held {
                self.points[i as usize] = at;
            }
            for &(a, b, rest, firm) in &self.links {
                let (pa, pb) = (self.points[a as usize], self.points[b as usize]);
                let d = pb - pa;
                let l = d.length().max(1e-6);
                let fix = d * ((l - rest) / l) * 0.5 * firm;
                self.points[a as usize] += fix;
                self.points[b as usize] -= fix;
            }
            for p in &mut self.points {
                for &(a, b, r) in bodies {
                    let ab = b - a;
                    let t = ((*p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                    let c = a + ab * t;
                    let d = *p - c;
                    if d.length_squared() < r * r {
                        *p = c + d.normalize_or(Vec3::X) * r;
                    }
                }
                p.y = p.y.clamp(floor, ceiling.max(floor));
            }
        }
        for &(i, at) in held {
            self.points[i as usize] = at;
        }
    }

    /// Its mesh (to be kept up with `draw`).
    pub fn mesh(&self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_indices(Indices::U32(self.triangles.clone()));
        self.draw(&mut mesh);
        mesh
    }

    /// Where its points are now, into its mesh, with smooth normals.
    pub fn draw(&self, mesh: &mut Mesh) {
        let mut normals = vec![Vec3::ZERO; self.points.len()];
        for t in self.triangles.chunks(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (self.points[b] - self.points[a]).cross(self.points[c] - self.points[a]);
            normals[a] += n;
            normals[b] += n;
            normals[c] += n;
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.points.iter().map(|p| p.to_array()).collect::<Vec<_>>());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.iter().map(|n| n.normalize_or(Vec3::Y).to_array()).collect::<Vec<_>>());
    }
}
