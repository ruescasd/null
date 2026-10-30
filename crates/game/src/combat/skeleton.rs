//! A body of bones (a creature's `body=1` option, the harrow's so far; the
//! user, 2026-10-29: its fragments a jumble, no outline; muscles tried,
//! "just another block of black"; then: restrict it to a skeletal
//! composition). Its torso and neck are a spine of vertebrae, each with a
//! blade standing from it, tall over its shoulders where it hauls, low at
//! its waist; an open ribcage, no breastbone, the ribs ending free; shoulder
//! blades and a pelvis, plates from its spine to its legs' joints. The gaps
//! between them make the shape: what is behind it is seen through it. Hard
//! faceted black, as its legs are; each piece fixed to its bone.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use super::hunter::{Part, RIGID};
use crate::rig::Bone;

/// The bones' meshes: vertebrae (their blades low, middling, tall), a left
/// and a right rib, and a plate. All but the plate in units of the radius of
/// the bone they hang on, its middle line along +Y; the plate along +Y from
/// 0 to 1.
pub(super) struct Kit {
    vertebrae: [Handle<Mesh>; 3],
    /// The ribs, left and right, for each thickness of `RIBS`.
    ribs: Vec<[Handle<Mesh>; 2]>,
    plate: Handle<Mesh>,
    /// A strut, a spike, a hoop, a tile (see `rod`, `spike`, `hoop`, `tile`).
    rod: Handle<Mesh>,
    spike: Handle<Mesh>,
    hoop: Handle<Mesh>,
    tile: Handle<Mesh>,
    /// The carapace's Voronoi plates (see `carapace`).
    plates: Vec<Plate>,
}

/// A plate of the carapace: where its middle lies on the torso as made (as
/// a bone's frame, x right, y along it from the rump, z down; see
/// `shell_radius`), how it is turned there (its x along the body, z out),
/// its mesh in that frame.
struct Plate {
    at: Vec3,
    turn: Quat,
    mesh: Handle<Mesh>,
}

/// The ribs' thicknesses (the `ribs` option; the user: bigger ribcage
/// bones, the same skeleton otherwise), as the option takes them.
use crate::traits::RIB_SIZES as RIBS;

/// How far up from a bone's middle line its spine runs (radii).
const SPINE: f32 = 0.75;

pub(super) fn build(meshes: &mut Assets<Mesh>) -> Kit {
    Kit {
        vertebrae: [0.45, 0.9, 1.5].map(|blade| meshes.add(vertebra(blade))),
        ribs: RIBS.iter().map(|&thick| [-1.0, 1.0].map(|side| meshes.add(rib(side, thick)))).collect(),
        plate: meshes.add(plate()),
        rod: meshes.add(rod()),
        spike: meshes.add(spike()),
        hoop: meshes.add(hoop()),
        tile: meshes.add(tile()),
        plates: carapace().into_iter().map(|(at, turn, mesh)| Plate { at, turn, mesh: meshes.add(mesh) }).collect(),
    }
}

/// A harrow's torso as it is made (every one alike): its three bones,
/// rump to chest, their lengths and radii (m).
const TORSO: [(f32, f32); 3] = [(0.6565, 0.2706), (0.6565, 0.2376), (0.6565, 0.3696)];

/// How far out the shell lies (radii); how far it rounds over the torso's
/// ends (m: behind the rump, before the chest); how near its plates'
/// middles come at most, how wide the gaps between them are, how thick they
/// are (m).
const SHELL: f32 = 1.1;
const SHELL_ENDS: (f32, f32) = (0.3, 0.22);
const SHELL_CELL: f32 = 0.22;
const SHELL_GAP: f32 = 0.05;
const SHELL_THICK: f32 = 0.035;

/// How clear a plate's middle is of a leg's upper bone and of the neck, or
/// it is left out (m, for a harrow as made): their ways out of the shell.
const SHELL_CLEAR: f32 = 0.2;

/// The torso's radius `s` along it, eased from bone to bone (the bones'
/// own change at their joints).
fn torso_radius(s: f32) -> f32 {
    let x = (s * 3.0 - 0.5).clamp(0.0, 2.0);
    let k = (x.floor() as usize).min(1);
    TORSO[k].1 + (TORSO[k + 1].1 - TORSO[k].1) * (x - k as f32)
}

fn torso_length() -> f32 {
    TORSO.iter().map(|t| t.0).sum()
}

/// The shell's radius `y` along the torso as made (m from the rump): round
/// over its ends, none past them.
fn shell_radius(y: f32) -> f32 {
    let (length, (back, front)) = (torso_length(), SHELL_ENDS);
    let end = |r: f32, t: f32| r * (1.0 - t * t).max(0.0).sqrt();
    if y < 0.0 {
        end(SHELL * torso_radius(0.0), y / back)
    } else if y > length {
        end(SHELL * torso_radius(1.0), (y - length) / front)
    } else {
        SHELL * torso_radius(y / length)
    }
}

/// How far `p` is out of the shell (m, near it; below 0 within).
fn shell_out(p: Vec3) -> f32 {
    let (length, (back, front)) = (torso_length(), SHELL_ENDS);
    let r = Vec2::new(p.x, p.z).length();
    // (Past its ends' tips, from the tips.)
    if p.y < -back {
        Vec2::new(r, p.y + back).length()
    } else if p.y > length + front {
        Vec2::new(r, p.y - length - front).length()
    } else {
        r - shell_radius(p.y)
    }
}

/// The shell's way out at `p`, on it.
fn shell_normal(p: Vec3) -> Vec3 {
    let d = |e: Vec3| shell_out(p + e * 0.002) - shell_out(p - e * 0.002);
    Vec3::new(d(Vec3::X), d(Vec3::Y), d(Vec3::Z)).normalize_or(Vec3::NEG_Z)
}

/// `p` (within 0.15 m of the shell) moved along `way` onto it; straight out
/// from its middle line where `way` does not cross it there.
fn onto_shell(p: Vec3, way: Vec3) -> Vec3 {
    let (mut lo, mut hi) = (-0.15f32, 0.15f32);
    if shell_out(p + way * lo) > 0.0 || shell_out(p + way * hi) < 0.0 {
        let k = shell_radius(p.y) / Vec2::new(p.x, p.z).length().max(1e-4);
        return Vec3::new(p.x * k, p.y, p.z * k);
    }
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        if shell_out(p + way * mid) > 0.0 { hi = mid } else { lo = mid }
    }
    p + way * (lo + hi) * 0.5
}

/// The carapace (the user: Voronoi plates, gaps between them; closed, its
/// underside and ends too): a shell round the torso as made, rounded over
/// its ends. Seeds on it by chance in proportion to its girth, each kept
/// only if no nearer than `SHELL_CELL` to one kept before (evenly spread,
/// never in rows); each seed's cell in the plane touching the shell there,
/// cut by the planes halfway to its neighbours, drawn in from its edges by
/// half the gap, laid back onto the shell, curving with it, its edge
/// bevelled, `SHELL_THICK` thick. Each plate's middle, turn and mesh (see
/// `Plate`).
fn carapace() -> Vec<(Vec3, Quat, Mesh)> {
    let (length, (back, front)) = (torso_length(), SHELL_ENDS);
    let span = length + back + front;
    let widest = (0..=40).map(|k| shell_radius(-back + span * k as f32 / 40.0)).fold(0.0, f32::max);
    let mut seeds: Vec<Vec3> = Vec::new();
    let mut since = 0;
    for k in 0..60_000 {
        // (Until 4000 tries in a row found no room.)
        if since > 4000 {
            break;
        }
        since += 1;
        let h = |n: i32| worldgen::noise::hash01(k, n, 0, 0x5e11);
        let y = -back + h(1) * span;
        let r = shell_radius(y);
        if h(3) * widest > r {
            continue;
        }
        let a = (h(2) * 2.0 - 1.0) * std::f32::consts::PI;
        let p = Vec3::new(a.sin() * r, y, -a.cos() * r);
        if seeds.iter().all(|s| s.distance(p) >= SHELL_CELL) {
            seeds.push(p);
            since = 0;
        }
    }
    let mut plates = Vec::new();
    for (i, &seed) in seeds.iter().enumerate() {
        let out = shell_normal(seed);
        let along = (Vec3::Y - out * out.y).normalize_or(Vec3::X);
        let across = out.cross(along);
        let mut cell = vec![Vec2::new(-0.5, -0.5), Vec2::new(0.5, -0.5), Vec2::new(0.5, 0.5), Vec2::new(-0.5, 0.5)];
        for (j, &other) in seeds.iter().enumerate() {
            let d = other - seed;
            let away = Vec2::new(d.dot(along), d.dot(across));
            if i == j || d.length() > SHELL_CELL * 3.0 || away.length() < 1e-4 {
                continue;
            }
            // (The plane halfway, where it cuts this one.)
            cell = clip(&cell, away * (d.length_squared() * 0.5 / away.length_squared()), away);
        }
        let Some(cell) = inset(&cell, SHELL_GAP * 0.5) else { continue };
        let middle = cell.iter().sum::<Vec2>() / cell.len() as f32;
        let turn = Mat3::from_cols(along, across, out);
        let on = |q: Vec2| onto_shell(seed + along * q.x + across * q.y, out);
        let local = |p: Vec3| turn.transpose() * (p - seed);
        // (Three rings, face to back: the face drawn in a little, the edge a
        // third of the way in, the back; then the face's middle and the
        // back's.)
        let rings = [(0.12, 0.0), (0.0, SHELL_THICK * 0.35), (0.0, SHELL_THICK)].map(|(drawn, depth)| {
            cell.iter()
                .map(|&q| {
                    let p = on(middle + (q - middle) * (1.0 - drawn));
                    local(p - shell_normal(p) * depth)
                })
                .collect::<Vec<_>>()
        });
        let face = on(middle);
        plates.push((seed, Quat::from_mat3(&turn), plate_mesh(rings, local(face), local(face - shell_normal(face) * SHELL_THICK))));
    }
    plates
}

/// `polygon` (convex, in order) cut to the side of the line through `at`
/// square to `away` that `away` points from.
fn clip(polygon: &[Vec2], at: Vec2, away: Vec2) -> Vec<Vec2> {
    let side = |p: Vec2| (p - at).dot(away);
    let mut out = Vec::new();
    for k in 0..polygon.len() {
        let (p, q) = (polygon[k], polygon[(k + 1) % polygon.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp <= 0.0 {
            out.push(p);
        }
        if (sp < 0.0) != (sq < 0.0) {
            out.push(p + (q - p) * (sp / (sp - sq)));
        }
    }
    out
}

/// `polygon` (convex, in order) drawn in from every edge by `by`; none when
/// nothing is left of it.
fn inset(polygon: &[Vec2], by: f32) -> Option<Vec<Vec2>> {
    let n = polygon.len();
    let middle = polygon.iter().sum::<Vec2>() / n as f32;
    // (Each edge's line moved in: a point on it and its way.)
    let lines: Vec<(Vec2, Vec2)> = (0..n)
        .map(|k| {
            let (p, q) = (polygon[k], polygon[(k + 1) % n]);
            let way = (q - p).normalize_or_zero();
            let mut inward = way.perp();
            if inward.dot(middle - p) < 0.0 {
                inward = -inward;
            }
            (p + inward * by, way)
        })
        .collect();
    let mut out = Vec::new();
    for k in 0..n {
        let ((p, u), (q, v)) = (lines[(k + n - 1) % n], lines[k]);
        let cross = u.perp_dot(v);
        if cross.abs() < 1e-6 {
            continue;
        }
        out.push(p + u * ((q - p).perp_dot(v) / cross));
    }
    // (Too small, or turned inside out by being drawn in past its middle.)
    let area = (0..out.len()).map(|k| out[k].perp_dot(out[(k + 1) % out.len()])).sum::<f32>() * 0.5;
    let before = (0..n).map(|k| polygon[k].perp_dot(polygon[(k + 1) % n])).sum::<f32>() * 0.5;
    (out.len() >= 3 && area * before > 0.0 && area.abs() > SHELL_CELL * SHELL_CELL * 0.08).then_some(out)
}

/// A plate's mesh from its rings, face to back (see `carapace`), and the
/// middles of its face and back; its face (+Z) wound outwards.
fn plate_mesh(rings: [Vec<Vec3>; 3], face: Vec3, back: Vec3) -> Mesh {
    let mut solid = Solid::default();
    let n = rings[0].len() as u32;
    for ring in &rings {
        solid.positions.extend(ring.iter().map(|p| p.to_array()));
    }
    let (f, b) = (3 * n, 3 * n + 1);
    solid.positions.push(face.to_array());
    solid.positions.push(back.to_array());
    for ring in 0..2 {
        for k in 0..n {
            let (a0, a1) = (ring * n + k, ring * n + (k + 1) % n);
            indices_quad(&mut solid.indices, a0, a0 + n, a1, a1 + n);
        }
    }
    for k in 0..n {
        solid.indices.extend([f, (k + 1) % n, k]);
        solid.indices.extend([b, 2 * n + k, 2 * n + (k + 1) % n]);
    }
    // (Whichever way round its outline went.)
    let [p, q, r] = [f, 1 % n, 0].map(|i| Vec3::from_array(solid.positions[i as usize]));
    if (q - p).cross(r - p).z < 0.0 {
        for t in solid.indices.chunks_mut(3) {
            t.swap(1, 2);
        }
    }
    solid.mesh()
}

/// Faceted solids built up piece by piece, flat shaded.
#[derive(Default)]
struct Solid {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl Solid {
    /// A tube through `points`, its section an ellipse (`sides` facets) of
    /// half sizes `size` at each point (x across the way `across` lies, y
    /// square to both); a size of 0 a point.
    fn tube(&mut self, points: &[Vec3], size: &[Vec2], sides: usize, across: Vec3) {
        let base = self.positions.len() as u32;
        for (i, &p) in points.iter().enumerate() {
            let along = if i + 1 < points.len() { points[i + 1] - p } else { p - points[i - 1] }.normalize_or(Vec3::Y);
            let u = (across - along * across.dot(along)).normalize_or(along.any_orthonormal_vector());
            let v = along.cross(u);
            for s in 0..sides {
                let a = s as f32 / sides as f32 * std::f32::consts::TAU;
                self.positions.push((p + u * a.cos() * size[i].x + v * a.sin() * size[i].y).to_array());
            }
        }
        let n = sides as u32;
        for i in 0..points.len() as u32 - 1 {
            for s in 0..n {
                let (a0, a1) = (base + i * n + s, base + i * n + (s + 1) % n);
                indices_quad(&mut self.indices, a0, a0 + n, a1, a1 + n);
            }
        }
        // (Closed at an end that is not a point: a fan of the ring.)
        for (ring, flip) in [(0u32, true), (points.len() as u32 - 1, false)] {
            if size[ring as usize].length() > 1e-4 {
                let centre = self.positions.len() as u32;
                self.positions.push(points[ring as usize].to_array());
                for s in 0..n {
                    let (a, b) = (base + ring * n + s, base + ring * n + (s + 1) % n);
                    if flip { self.indices.extend([centre, b, a]) } else { self.indices.extend([centre, a, b]) }
                }
            }
        }
    }

    fn mesh(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions).with_inserted_indices(Indices::U32(self.indices));
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
        mesh
    }
}

/// A quad between two rings (`a` the nearer start, `b` the next), facing
/// out.
fn indices_quad(indices: &mut Vec<u32>, a0: u32, b0: u32, a1: u32, b1: u32) {
    indices.extend([a0, a1, b0, a1, b1, b0]);
}

/// A vertebra on the spine's line (`SPINE` up from the middle): its body,
/// a blade standing up from it and leaning back, `blade` radii tall, thin
/// across and long along; short processes either side.
fn vertebra(blade: f32) -> Mesh {
    let mut s = Solid::default();
    let at = Vec3::new(0.0, 0.0, -SPINE);
    s.tube(&[at - Vec3::Y * 0.17, at, at + Vec3::Y * 0.17], &[Vec2::splat(0.1), Vec2::splat(0.15), Vec2::splat(0.1)], 5, Vec3::X);
    s.tube(&[at, at + Vec3::new(0.0, -0.08, -blade * 0.55), at + Vec3::new(0.0, -0.2, -blade)], &[Vec2::new(0.035, 0.13), Vec2::new(0.03, 0.09), Vec2::ZERO], 4, Vec3::X);
    for side in [-1.0, 1.0] {
        s.tube(&[at + Vec3::X * side * 0.1, at + Vec3::new(side * 0.4, 0.02, 0.06)], &[Vec2::new(0.06, 0.035), Vec2::ZERO], 4, Vec3::Z);
    }
    s.mesh()
}

/// A rib, `side` the body's (-1 left): from the spine out over the flank
/// and down, curling in under the belly, sloping back as it goes, its end
/// free; a flat band, thinner and narrower towards its end, `thick` times
/// as thick and broad as the first.
fn rib(side: f32, thick: f32) -> Mesh {
    let mut s = Solid::default();
    let points = [
        Vec3::new(side * 0.08, 0.0, -SPINE),
        Vec3::new(side * 0.75, -0.05, -0.55),
        Vec3::new(side * 1.12, -0.13, 0.0),
        Vec3::new(side * 1.0, -0.2, 0.55),
        Vec3::new(side * 0.55, -0.24, 0.92),
        Vec3::new(side * 0.25, -0.22, 1.0),
    ];
    let size = [Vec2::new(0.04, 0.09), Vec2::new(0.035, 0.08), Vec2::new(0.03, 0.075), Vec2::new(0.028, 0.06), Vec2::new(0.02, 0.04), Vec2::ZERO].map(|v| v * thick);
    s.tube(&points, &size, 4, Vec3::Y);
    s.mesh()
}

/// A plate along +Y from 0 to 1: broad at its root, narrowing to its end, a
/// ridge down its middle, thin.
fn plate() -> Mesh {
    let mut s = Solid::default();
    s.tube(&[Vec3::ZERO, Vec3::Y * 0.35, Vec3::Y * 0.8, Vec3::Y], &[Vec2::new(0.5, 0.05), Vec2::new(0.45, 0.06), Vec2::new(0.2, 0.05), Vec2::new(0.08, 0.03)], 6, Vec3::X);
    s.mesh()
}

/// A rod along +Y from 0 to 1, a unit thick: six-sided, a little narrower
/// at its ends (a strut of a frame).
fn rod() -> Mesh {
    let mut s = Solid::default();
    s.tube(&[Vec3::ZERO, Vec3::Y * 0.15, Vec3::Y * 0.85, Vec3::Y], &[Vec2::splat(0.75), Vec2::splat(1.0), Vec2::splat(1.0), Vec2::splat(0.75)], 6, Vec3::X);
    s.mesh()
}

/// A spike along +Y from 0 to 1, a unit thick at its root, to a point.
fn spike() -> Mesh {
    let mut s = Solid::default();
    s.tube(&[Vec3::ZERO, Vec3::Y * 0.5, Vec3::Y], &[Vec2::splat(1.0), Vec2::splat(0.6), Vec2::ZERO], 5, Vec3::X);
    s.mesh()
}

/// A tile along +Y from 0 to 1, a unit wide: its corners cut, a ridge down
/// its middle, thin (a plate of a carapace).
fn tile() -> Mesh {
    let mut s = Solid::default();
    s.tube(&[Vec3::ZERO, Vec3::Y * 0.15, Vec3::Y * 0.85, Vec3::Y], &[Vec2::new(0.38, 0.04), Vec2::new(0.5, 0.06), Vec2::new(0.5, 0.06), Vec2::new(0.38, 0.04)], 4, Vec3::X);
    s.mesh()
}

/// A hoop round +Y, a unit across, its band 0.07 thick (and twice that
/// along +Y).
fn hoop() -> Mesh {
    let mut s = Solid::default();
    let n = 16;
    let points: Vec<Vec3> = (0..=n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            Vec3::new(a.cos(), 0.0, a.sin())
        })
        .collect();
    s.tube(&points, &vec![Vec2::new(0.14, 0.07); points.len()], 4, Vec3::Y);
    s.mesh()
}

/// A harrow's pieces as they are put on it (see `spawn_harrow`).
struct Build<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    kit: &'a Kit,
    material: Handle<StandardMaterial>,
    hunter: Entity,
    bones: &'a [Bone],
}

impl Build<'_, '_, '_> {
    /// A point by bone `bone`: `along` it, out from its line (in its frame,
    /// in its radii: x right, z down).
    fn at(&self, bone: usize, along: f32, x: f32, z: f32) -> Vec3 {
        let b = &self.bones[bone];
        b.a.lerp(b.b, along) + b.rotation() * Vec3::new(x, 0.0, z) * b.radius
    }

    /// `mesh` fixed to bone `bone`, `along` it, as it lies in the bone's
    /// frame.
    fn piece(&mut self, mesh: &Handle<Mesh>, bone: usize, along: f32, offset: Vec3, rotation: Quat, scale: Vec3) {
        self.commands.spawn((
            Part { hunter: self.hunter, bone, along, offset, rotation, velocity: Vec3::ZERO, stiffness: RIGID },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(self.material.clone()),
            Transform::from_scale(scale),
        ));
    }

    /// `mesh` (along +Y from 0 to 1) from `from` to `to`, fixed to bone
    /// `on`, `width` across (x, and z through), its face (+Z) towards `out`.
    fn span(&mut self, mesh: &Handle<Mesh>, on: usize, from: Vec3, to: Vec3, width: Vec2, out: Vec3) {
        let b = self.bones[on];
        let turn = b.rotation();
        let length = from.distance(to).max(0.01);
        let way = (to - from) / length;
        let face = (out - way * out.dot(way)).normalize_or(way.any_orthonormal_vector());
        let world = Quat::from_mat3(&Mat3::from_cols(way.cross(face), way, face));
        self.piece(mesh, on, 0.0, turn.inverse() * (from - b.a), turn.inverse() * world, Vec3::new(width.x, length, width.y));
    }

    /// A strut (a rod) from `from` to `to`, `thick` (m) through, fixed to
    /// bone `on`.
    fn strut(&mut self, on: usize, from: Vec3, to: Vec3, thick: f32) {
        let rod = self.kit.rod.clone();
        self.span(&rod, on, from, to, Vec2::splat(thick * 0.5), Vec3::Y);
    }

    /// A spike from `from` to its point at `to`, `thick` (m) at its root.
    fn spike(&mut self, on: usize, from: Vec3, to: Vec3, thick: f32) {
        let spike = self.kit.spike.clone();
        self.span(&spike, on, from, to, Vec2::splat(thick * 0.5), Vec3::Y);
    }

    /// The spine's vertebrae, rump to chest (`s` 0 to 1 along it), their
    /// blades rising over the shoulders, low at the waist.
    fn spine(&mut self) {
        for bone in 0..3 {
            let r = self.bones[bone].radius;
            for k in 0..4 {
                let along = (k as f32 + 0.5) / 4.0;
                let s = (bone as f32 + along) / 3.0;
                let tall = if s > 0.6 { 2 } else if s > 0.3 { 0 } else { 1 };
                let mesh = self.kit.vertebrae[tall].clone();
                self.piece(&mesh, bone, along, Vec3::ZERO, Quat::IDENTITY, Vec3::splat(r));
            }
        }
    }

    /// What every body has: vertebrae up the neck; shoulder blades and the
    /// pelvis's plates, from the spine to the legs' joints (what carries
    /// them).
    fn common(&mut self) {
        let neck = self.bones[3].radius;
        let small = self.kit.vertebrae[0].clone();
        for k in 0..4 {
            self.piece(&small, 3, 0.12 + 0.24 * k as f32, Vec3::new(0.0, 0.0, neck * 0.35), Quat::IDENTITY, Vec3::splat(neck * 0.75));
        }
        let plate = self.kit.plate.clone();
        for (on, along, leg, width, side) in [(2, 0.5, 5, 0.5, -1.0), (2, 0.5, 7, 0.5, 1.0), (0, 0.35, 9, 0.42, -1.0), (0, 0.35, 11, 0.42, 1.0)] {
            let r = self.bones[on].radius;
            let from = self.at(on, along, side * 0.3, -SPINE);
            let out = self.bones[on].rotation() * Vec3::X * side;
            let to = self.bones[leg].a;
            self.span(&plate, on, from, to, Vec2::splat(width * r / 0.5), out);
        }
    }

    /// An arch from `from` to `to`, bowing `rise` (m) out towards `out`,
    /// of struts, thinner towards its end.
    #[allow(clippy::too_many_arguments)]
    fn arch(&mut self, on: usize, from: Vec3, to: Vec3, out: Vec3, rise: f32, thick: f32) {
        let n = 8;
        let point = |k: usize| {
            let t = k as f32 / n as f32;
            from.lerp(to, t) + out * (std::f32::consts::PI * t).sin() * rise
        };
        for k in 0..n {
            self.strut(on, point(k), point(k + 1), thick * (1.0 - 0.4 * k as f32 / n as f32));
        }
    }

    /// A branch from `root` along `dir`, `length` long and `thick` through,
    /// forking `depth` times more: its two children either side of it, in a
    /// plane turned about it by the golden angle from its parent's (as
    /// leaves round a stem: so the forks fill a volume, not a plane, the
    /// user), shorter and thinner. The last ones' ends go in `tips` (where,
    /// how thick, on which bone), to be joined to other trees'.
    #[allow(clippy::too_many_arguments)]
    fn branch(&mut self, on: usize, root: Vec3, dir: Vec3, ahead: Vec3, length: f32, thick: f32, depth: u32, seed: i32, tips: &mut Vec<(Vec3, f32, usize)>) {
        let end = root + dir * length;
        self.strut(on, root, end, thick);
        if depth == 0 {
            tips.push((end, thick, on));
            return;
        }
        let square = dir.cross(ahead).normalize_or(dir.any_orthonormal_vector());
        let turn = 2.399_963 * depth as f32 + worldgen::noise::hash01(seed, depth as i32, 9, 0x5b1) * 0.6;
        let axis = Quat::from_axis_angle(dir, turn) * square;
        for (k, way) in [-1.0f32, 1.0].into_iter().enumerate() {
            let spread = 0.5 + 0.2 * worldgen::noise::hash01(seed, depth as i32, k as i32, 0x5b1);
            let child = Quat::from_axis_angle(axis, way * spread) * dir;
            self.branch(on, end, child, ahead, length * 0.65, thick * 0.62, depth - 1, seed * 7 + k as i32, tips);
        }
    }
}

/// The harrow's body on `hunter` as `body` says (its option; see
/// `traits::OPTIONS`): 1 a skeleton; 2 hoops; 3 a spiral; 4 a truss; 5
/// arches; 6 a keel and a fan; 7 branches forking (a fractal, sparse), from
/// a trunk, their twigs joined; 9 plates in bands; 10 Voronoi plates (8 is
/// not built here: its own shards, sparse, see `hunter`). All
/// leave the void that shows their shape (the user: dense, the detail could
/// not be seen; the skeleton's gaps let the background draw its outline).
/// `ribs`, a skeleton's ribs' thickness. Bones: 0-2 its torso from the rump
/// to the chest, 3 its neck, 4 its head, 5-6 the left foreleg, 7-8 the
/// right, 9-10 the left hind leg, 11-12 the right.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_harrow(commands: &mut Commands, kit: &Kit, material: &Handle<StandardMaterial>, hunter: Entity, bones: &[Bone], body: usize, ribs: f32) {
    let mut b = Build { commands, kit, material: material.clone(), hunter, bones };
    b.common();
    // (Along its torso, `s` from the rump to the chest: which bone that is
    // on, and how far along it.)
    let on = |s: f32| ((s.min(0.999) * 3.0).floor() as usize).min(2);
    let along = |s: f32| s.min(0.999) * 3.0 - on(s) as f32;
    match body {
        // A skeleton: the spine; an open ribcage, from the waist's back
        // half to the chest, growing, no breastbone, the ribs' ends free.
        1 => {
            b.spine();
            // (Only those it takes get this far: see `traits::OPTIONS`.)
            let thick = RIBS.iter().position(|&t| t == ribs).unwrap_or(0);
            for (bone, at, k) in [(1, 0.55, 0.85), (1, 0.85, 0.92), (2, 0.15, 1.0), (2, 0.4, 1.05), (2, 0.65, 1.05), (2, 0.9, 0.95)] {
                let scale = Vec3::splat(bones[bone].radius * k);
                for side in 0..2 {
                    let rib = kit.ribs[thick][side].clone();
                    b.piece(&rib, bone, at, Vec3::ZERO, Quat::IDENTITY, scale);
                }
            }
        }
        // Hoops: rings round it, its girth, tilted by turns; joined along
        // the top by a rail.
        2 => {
            let stations = [0.08, 0.24, 0.42, 0.6, 0.76, 0.92];
            let mut tops = Vec::new();
            for (k, &s) in stations.iter().enumerate() {
                let (bone, at) = (on(s), along(s));
                let r = bones[bone].radius * 1.05;
                let tilt = Quat::from_rotation_x(if k % 2 == 0 { 0.25 } else { -0.1 });
                let hoop = kit.hoop.clone();
                b.piece(&hoop, bone, at, Vec3::ZERO, tilt, Vec3::splat(r));
                tops.push((bone, b.at(bone, at, 0.0, -1.05)));
            }
            for w in tops.windows(2) {
                b.strut(w[0].0, w[0].1, w[1].1, 0.07);
            }
        }
        // A spiral: one band wound round it two turns and a half from the
        // rump to the chest, a strut a twelfth of a turn.
        3 => {
            let steps = 30;
            let points: Vec<Vec3> = (0..=steps)
                .map(|k| {
                    let s = k as f32 / steps as f32;
                    let a = s * 2.5 * std::f32::consts::TAU;
                    b.at(on(s), along(s), a.cos() * 1.08, a.sin() * 1.08)
                })
                .collect();
            for k in 0..steps {
                b.strut(on(k as f32 / steps as f32), points[k], points[k + 1], 0.09);
            }
        }
        // A truss: three rails, two along its back's sides and one under
        // its belly; a triangle of struts round it at each station; a brace
        // across each bay's sides, by turns one way and the other.
        4 => {
            let corners = [(-0.7, -0.75), (0.7, -0.75), (0.0, 1.0)];
            let stations = 7;
            let s_of = |k: usize| k as f32 / (stations - 1) as f32;
            let point = |b: &Build, k: usize, c: (f32, f32)| b.at(on(s_of(k)), along(s_of(k)), c.0, c.1);
            for k in 0..stations {
                let bone = on(s_of(k));
                for i in 0..3 {
                    let (p, q) = (point(&b, k, corners[i]), point(&b, k, corners[(i + 1) % 3]));
                    b.strut(bone, p, q, 0.05);
                }
                if k + 1 < stations {
                    for i in 0..3 {
                        let j = (i + 1) % 3;
                        let (p, q) = (point(&b, k, corners[i]), point(&b, k + 1, corners[i]));
                        b.strut(bone, p, q, 0.08);
                        let (p, q) = if k % 2 == 0 { (point(&b, k, corners[i]), point(&b, k + 1, corners[j])) } else { (point(&b, k, corners[j]), point(&b, k + 1, corners[i])) };
                        b.strut(bone, p, q, 0.035);
                    }
                }
            }
        }
        // Arches: the spine, and great arches springing from it out over
        // each leg's joint down beside it (flying buttresses), and a pair
        // over the waist.
        5 => {
            b.spine();
            for (bone, at, leg, side) in [(2, 0.7, 5, -1.0), (2, 0.7, 7, 1.0), (0, 0.5, 9, -1.0), (0, 0.5, 11, 1.0)] {
                let from = b.at(bone, at, side * 0.2, -0.9);
                let to = bones[leg].a - Vec3::Y * 0.05;
                let out = bones[bone].rotation() * Vec3::new(side, 0.0, -0.6).normalize();
                b.arch(bone, from, to, out, 0.35, 0.09);
            }
            for side in [-1.0, 1.0] {
                let (from, to) = (b.at(1, 0.15, side * 0.2, -0.9), b.at(1, 0.85, side * 1.0, 0.8));
                let out = bones[1].rotation() * Vec3::X * side;
                b.arch(1, from, to, out, 0.4, 0.07);
            }
        }
        // A keel and a fan: the spine; a deep blade under the chest and the
        // waist; spines fanning out and up from its lower edge round the
        // flanks, their points free.
        6 => {
            b.spine();
            let (from, to) = (b.at(2, 0.95, 0.0, 0.55), b.at(1, 0.2, 0.0, 0.55));
            let side = bones[2].rotation() * Vec3::X;
            let plate = kit.plate.clone();
            let deep = bones[2].radius * 1.4;
            b.span(&plate, 2, from, to, Vec2::splat(deep / 0.5), side);
            for (bone, at, reach) in [(1, 0.35, 1.0), (1, 0.7, 1.15), (2, 0.05, 1.3), (2, 0.35, 1.4), (2, 0.65, 1.35), (2, 0.9, 1.1)] {
                let root = b.at(bone, at, 0.0, 1.2);
                for side in [-1.0, 1.0] {
                    let tip = b.at(bone, at - 0.15, side * reach * 1.25, -0.25 * reach);
                    b.spike(bone, root, tip, 0.07);
                }
            }
        }
        // Branches: a trunk along its line; from it, out round the body,
        // trees each forking in two, three times, shorter and thinner each
        // time (a fractal, sparse: the void between them kept; the user);
        // each last twig grown into the nearest of another tree's (the user:
        // each strand hanging on its own was not needed; one whole, joined).
        7 => {
            // (Three trees a station, round it at a third of a turn apart,
            // the whole turned on by the golden angle from one station to
            // the next: up, out and down, all round.)
            let mut trunk = Vec::new();
            let mut tips: Vec<Vec<(Vec3, f32, usize)>> = Vec::new();
            for (k, s) in [0.06, 0.27, 0.48, 0.69, 0.9].into_iter().enumerate() {
                let (bone, at) = (on(s), along(s));
                let r = bones[bone].radius;
                let turn = bones[bone].rotation();
                let hub = b.at(bone, at, 0.0, 0.0);
                trunk.push((bone, hub, r));
                for j in 0..3 {
                    let a = k as f32 * 2.399_963 + j as f32 * std::f32::consts::TAU / 3.0;
                    let out = Vec3::new(a.cos(), 0.0, a.sin());
                    let root = b.at(bone, at, out.x * 0.35, out.z * 0.35);
                    b.strut(bone, hub, root, r * 0.18);
                    let dir = turn * (out + Vec3::Y * 0.25).normalize();
                    let mut ends = Vec::new();
                    b.branch(bone, root, dir, turn * Vec3::Y, r * 1.3, r * 0.18, 2, k as i32 * 3 + j as i32, &mut ends);
                    tips.push(ends);
                }
            }
            for w in trunk.windows(2) {
                b.strut(w[0].0, w[0].1, w[1].1, w[0].2 * 0.22);
            }
            // (Each tip to the nearest tip of another tree, each pair once.)
            let all: Vec<(usize, Vec3, f32, usize)> = tips.iter().enumerate().flat_map(|(tree, ends)| ends.iter().map(move |&(p, thick, bone)| (tree, p, thick, bone))).collect();
            let mut joined: Vec<(usize, usize)> = Vec::new();
            for (i, &(tree, p, thick, bone)) in all.iter().enumerate() {
                let nearest = all.iter().enumerate().filter(|(_, t)| t.0 != tree).min_by(|x, y| x.1.1.distance(p).total_cmp(&y.1.1.distance(p)));
                if let Some((j, &(_, q, _, _))) = nearest {
                    let pair = (i.min(j), i.max(j));
                    if !joined.contains(&pair) {
                        joined.push(pair);
                        b.strut(bone, p, q, thick);
                    }
                }
            }
        }
        // Plates: a carapace in bands along its back and flanks, six tiles
        // a band, each shingled (its back edge proud); gaps between the
        // tiles and between the bands, none under its belly (the user: the
        // plates' solidity, the background still showing through).
        9 => {
            let bands = 7;
            let tile = kit.tile.clone();
            for k in 0..bands {
                let (s0, s1) = ((k as f32 + 0.15) / bands as f32, (k as f32 + 0.85) / bands as f32);
                let bone = on((s0 + s1) * 0.5);
                let r = bones[bone].radius;
                for i in 0..6 {
                    let a = (-100.0 + 40.0 * i as f32).to_radians();
                    let (x, z) = (a.sin(), -a.cos());
                    let from = b.at(on(s0), along(s0), x * 1.05, z * 1.05);
                    let to = b.at(on(s1), along(s1), x * 1.2, z * 1.2);
                    let out = bones[bone].rotation() * Vec3::new(x, 0.0, z);
                    // (Seven tenths of its share of the girth.)
                    b.span(&tile, bone, from, to, Vec2::new(r * 1.1 * 40f32.to_radians() * 0.7, 0.5), out);
                }
            }
        }
        // Voronoi plates: the carapace (see `carapace`), each plate on the
        // bone under its middle, as it lies on the torso as made; none
        // where the legs and the neck come out.
        10 => {
            let scale = bones[1].radius / TORSO[1].1;
            let near = |p: Vec3, a: Vec3, c: Vec3| p.distance(a + (c - a) * ((p - a).dot(c - a) / (c - a).length_squared().max(1e-6)).clamp(0.0, 1.0));
            for plate in &kit.plates {
                let bone = ((plate.at.y / TORSO[0].0).max(0.0) as usize).min(2);
                let local = (plate.at - Vec3::Y * TORSO[0].0 * bone as f32) * scale;
                let world = bones[bone].a + bones[bone].rotation() * local;
                if [3, 5, 7, 9, 11].iter().any(|&k| near(world, bones[k].a, bones[k].b) < SHELL_CLEAR * scale) {
                    continue;
                }
                b.piece(&plate.mesh, bone, 0.0, local, plate.turn, Vec3::splat(scale));
            }
        }
        _ => {}
    }
}
