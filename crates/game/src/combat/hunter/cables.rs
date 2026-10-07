//! Cables on a hunter (`--set cables=N`): strands hung on its bones, all of a
//! hunter's drawn as one mesh of tubes rebuilt every frame.
//!
//! 1. Muscles: cables strung between bones (body to thigh, across each knee,
//!    up the neck), taut when the limb is stretched, sagging when it folds.
//! 2. Fibres: loose strands hanging from the underside of the body, the neck
//!    and the legs, swinging as it moves, trailing when it runs.
//! 3. Woven: strands spiralling round every bone in place of the shards (the
//!    head keeps its own), the limbs and body bundles of cable, each strand
//!    running the length of a chain of bones (back and neck, a leg, the tail).
//! 4. A woven body: the same round the body only, the rest as before.
//!
//! Strung and hanging strands are chains of points kept at their length
//! (Verlet); woven ones follow their place on the bones on springs, a little
//! behind.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use worldgen::noise::hash01;

use super::Hunter;
use crate::{Args, terrain::WorldGen};

/// Where on a bone: how far along it (0..1) and out from it, in its frame
/// (x to the body's right, y along the bone, z its other side: below, for
/// the body).
#[derive(Clone, Copy)]
struct Anchor {
    bone: usize,
    along: f32,
    offset: Vec3,
}

impl Anchor {
    fn at(&self, bones: &[crate::rig::Bone]) -> Option<Vec3> {
        let b = bones.get(self.bone)?;
        Some(b.a.lerp(b.b, self.along) + b.rotation() * (self.offset * b.radius))
    }
}

enum Kind {
    /// Between two anchors, `length` long.
    Strung { from: Anchor, to: Anchor, length: f32 },
    /// From an anchor, its other end free.
    Hanging { from: Anchor, length: f32 },
    /// Round the bones `first..=last` (each joined to the next): `turns`
    /// times along each, starting at `phase`, `out` of their radius away.
    Wound { first: usize, last: usize, phase: f32, turns: f32, out: f32 },
}

struct Strand {
    kind: Kind,
    points: Vec<Vec3>,
    previous: Vec<Vec3>,
    /// Thickness at the root and the tip, and the shade.
    width: (f32, f32),
    shade: f32,
}

/// A hunter's cables.
#[derive(Component)]
pub(in crate::combat) struct Cables {
    hunter: Entity,
    strands: Vec<Strand>,
    mesh: Handle<Mesh>,
}

/// Points along a strung or hanging strand.
const NODES: usize = 9;
/// Points along a wound strand, per bone, and sides of every tube.
const WOUND_NODES: usize = 10;
const SIDES: usize = 5;

#[allow(clippy::too_many_arguments)]
pub(in crate::combat) fn run(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    world: Res<WorldGen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material: Local<Option<Handle<StandardMaterial>>>,
    added: Query<(Entity, &Hunter), Added<Hunter>>,
    hunters: Query<&Hunter>,
    mut cables: Query<(Entity, &mut Cables)>,
) {
    let variant = args.num("cables", 0.0) as u32;
    if variant == 0 {
        return;
    }
    let material = material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.35, reflectance: 0.5, ..default() })
        })
        .clone();
    for (entity, hunter) in &added {
        let mut strands = strands(variant, hunter, entity.index_u32());
        for s in &mut strands {
            step(s, &hunter.rig.bones, &world, 1.0 / 60.0);
        }
        // (Built whole from the start: the renderer does not take an empty
        // mesh growing.)
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        tubes(&mut mesh, &strands);
        let mesh = meshes.add(mesh);
        commands.spawn((
            Cables { hunter: entity, strands, mesh: mesh.clone() },
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            Visibility::default(),
            NoFrustumCulling,
        ));
    }

    let dt = time.delta_secs().clamp(0.001, 1.0 / 30.0);
    for (entity, mut c) in &mut cables {
        let Ok(hunter) = hunters.get(c.hunter) else {
            commands.entity(entity).despawn();
            continue;
        };
        let bones = &hunter.rig.bones;
        for s in &mut c.strands {
            step(s, bones, &world, dt);
        }
        if let Some(mut mesh) = meshes.get_mut(&c.mesh) {
            tubes(&mut mesh, &c.strands);
        }
    }
}

/// The strands for a variant, laid out on the hunter's bones as they are.
fn strands(variant: u32, hunter: &Hunter, seed: u32) -> Vec<Strand> {
    let plan = &hunter.rig.plan;
    let bones = &hunter.rig.bones;
    let limbs = plan.legs.len() + plan.arms.len();
    let tail = 5 + 2 * limbs;
    let r = |k: usize, j: i32| hash01(seed as i32, k as i32, j, 0x6c1) - 0.5;
    let mut out = Vec::new();
    let mut k = 0;
    let mut add = |kind: Kind, width: (f32, f32), shade: f32| {
        out.push(Strand { kind, points: Vec::new(), previous: Vec::new(), width, shade });
    };
    let a = |bone: usize, along: f32, offset: Vec3| Anchor { bone, along, offset };
    match variant {
        1 => {
            for limb in 0..limbs {
                let (upper, lower) = (5 + 2 * limb, 6 + 2 * limb);
                // Body to thigh: from the end of the body it hangs from.
                let chest = limb < plan.legs.len() && plan.legs[limb].root == crate::rig::Root::Chest || limb >= plan.legs.len();
                let body = if chest { 2 } else { 0 };
                let side = if bones[upper].a.distance(bones[body].b) < bones[upper].a.distance(bones[body].a) { 0.85 } else { 0.15 };
                for j in 0..3 {
                    k += 1;
                    let around = Vec3::new((j as f32 - 1.0) * 0.7, 0.0, -0.6 + r(k, 0) * 0.3);
                    add(Kind::Strung { from: a(body, side + r(k, 1) * 0.1, around), to: a(upper, 0.55 + 0.1 * j as f32, Vec3::new((j as f32 - 1.0) * 0.6, 0.0, 0.9)), length: 0.0 }, (0.045, 0.03), 0.07);
                }
                // Across the knee, front and back.
                for (j, z) in [(0, 1.0), (1, -1.0), (2, 0.0)] {
                    k += 1;
                    add(Kind::Strung { from: a(upper, 0.15 + 0.1 * j as f32, Vec3::new(r(k, 0) * 0.8, 0.0, z * 1.1)), to: a(lower, 0.45 + 0.1 * j as f32, Vec3::new(r(k, 1) * 0.6, 0.0, z * 0.9)), length: 0.0 }, (0.035, 0.025), if j == 2 { 0.55 } else { 0.08 });
                }
            }
            // Up the neck to the skull.
            for j in 0..5 {
                let t = j as f32 / 5.0 * std::f32::consts::TAU;
                add(Kind::Strung { from: a(2, 0.4, Vec3::new(t.cos() * 0.7, 0.0, t.sin() * 0.7)), to: a(4, 0.15, Vec3::new(t.cos() * 0.8, 0.0, t.sin() * 0.8)), length: 0.0 }, (0.05, 0.035), if j == 2 { 0.5 } else { 0.06 });
            }
        }
        2 => {
            // Under the body and neck: a fringe.
            for bone in 0..4 {
                for j in 0..9 {
                    k += 1;
                    let x = (j as f32 / 8.0 - 0.5) * 1.6;
                    add(Kind::Hanging { from: a(bone, (j as f32 + 0.5 + r(k, 0)) / 9.0, Vec3::new(x, 0.0, 0.9)), length: 0.35 + (r(k, 1) + 0.5) * 0.6 }, (0.03, 0.008), if k % 4 == 0 { 0.6 } else { 0.06 });
                }
            }
            // From the backs of the upper legs.
            for limb in 0..limbs {
                for j in 0..4 {
                    k += 1;
                    add(Kind::Hanging { from: a(5 + 2 * limb, 0.2 + 0.2 * j as f32, Vec3::new(r(k, 0), 0.0, -1.0)), length: 0.3 + (r(k, 1) + 0.5) * 0.4 }, (0.025, 0.006), 0.06);
                }
            }
            // A few long ones from the tail.
            for bone in tail..bones.len() {
                k += 1;
                add(Kind::Hanging { from: a(bone, 0.5, Vec3::new(0.0, 0.0, 1.0)), length: 0.4 + (r(k, 1) + 0.5) * 0.5 }, (0.02, 0.005), 0.06);
            }
        }
        _ => {
            // (first, last, strands): the back and neck, each leg, the tail;
            // or the body alone.
            let mut chains = vec![(0, if variant == 4 { 2 } else { 3 }, 16)];
            if variant != 4 {
                chains.extend((0..limbs).map(|l| (5 + 2 * l, 6 + 2 * l, 8)));
                if bones.len() > tail {
                    chains.push((tail, bones.len() - 1, 5));
                }
            }
            for (first, last, count) in chains {
                for j in 0..count {
                    k += 1;
                    let turns = if j % 2 == 0 { 0.45 } else { -0.45 } * (1.0 + r(k, 0) * 0.5);
                    let pale = j % 7 == 3;
                    add(
                        Kind::Wound { first, last, phase: j as f32 / count as f32 * std::f32::consts::TAU + r(k, 1), turns, out: 0.8 + r(k, 2) * 0.3 },
                        (0.0, 0.0),
                        if pale { 0.55 } else { 0.05 + (r(k, 3) + 0.5) * 0.06 },
                    );
                }
            }
        }
    }
    // Lay each out where it is now, and size the wound ones to their bones.
    for s in &mut out {
        match s.kind {
            Kind::Strung { from, to, ref mut length } => {
                let (p, q) = (from.at(bones).unwrap_or_default(), to.at(bones).unwrap_or_default());
                *length = p.distance(q) * 1.03;
                s.points = (0..NODES).map(|i| p.lerp(q, i as f32 / (NODES - 1) as f32)).collect();
            }
            Kind::Hanging { from, length } => {
                let p = from.at(bones).unwrap_or_default();
                s.points = (0..NODES).map(|i| p - Vec3::Y * length * i as f32 / (NODES - 1) as f32).collect();
            }
            Kind::Wound { first, last, .. } => {
                // (Thinning along the chain, as its bones do.)
                s.width = (bones[first].radius * 0.2, bones[last].radius * 0.25);
            }
        }
        s.previous = s.points.clone();
    }
    out
}

/// Whether `--set cables` replaces a bone's shards: woven, every bone but the
/// head; a woven body, the body's.
pub(in crate::combat) fn replaces(args: &Args, bone: usize) -> bool {
    match args.num("cables", 0.0) as u32 {
        3 => bone != 4,
        4 => bone <= 2,
        _ => false,
    }
}

/// One step: wound strands follow their bones; the others swing under
/// gravity, kept at their length, their ends held.
fn step(s: &mut Strand, bones: &[crate::rig::Bone], world: &WorldGen, dt: f32) {
    let (from, to, length) = match s.kind {
        Kind::Wound { first, last, phase, turns, out } => {
            if last >= bones.len() {
                return;
            }
            let chain = &bones[first..=last];
            let m = chain.len();
            let n = WOUND_NODES * m + 1;
            let target = |i: usize| {
                let u = i as f32 / (n - 1) as f32 * m as f32;
                let k = (u as usize).min(m - 1);
                let t = u - k as f32;
                let b = &chain[k];
                // Radius and frame eased into the next bone's over each joint.
                let (mut radius, mut rotation) = (b.radius, b.rotation());
                if t < 0.5 && k > 0 {
                    let w = 0.5 - t;
                    radius += (chain[k - 1].radius - radius) * w;
                    rotation = rotation.slerp(chain[k - 1].rotation(), w);
                } else if t > 0.5 && k + 1 < m {
                    let w = t - 0.5;
                    radius += (chain[k + 1].radius - radius) * w;
                    rotation = rotation.slerp(chain[k + 1].rotation(), w);
                }
                let a = phase + turns * std::f32::consts::TAU * u;
                // (Fuller in the middle of the chain.)
                let swell = 0.8 + 0.2 * (std::f32::consts::PI * u / m as f32).sin();
                b.a.lerp(b.b, t) + rotation * Vec3::new(a.cos(), 0.0, a.sin()) * radius * out * swell
            };
            // On springs: a little behind where they belong, settling
            // (`previous` holds their velocities).
            if s.points.len() != n || s.points[0].distance(target(0)) > 3.0 {
                s.points = (0..n).map(target).collect();
                s.previous = vec![Vec3::ZERO; n];
                return;
            }
            for i in 0..n {
                let to = target(i) - s.points[i];
                let v = s.previous[i] + (to * 300.0 - s.previous[i] * 24.0) * dt;
                s.previous[i] = v;
                s.points[i] += v * dt;
            }
            return;
        }
        Kind::Strung { from, to, length } => (from, Some(to), length),
        Kind::Hanging { from, length } => (from, None, length),
    };
    let Some(start) = from.at(bones) else { return };
    let end = to.and_then(|t| t.at(bones));
    // Far from where it was (a new hunter, a jump): start again, still.
    if s.points[0].distance(start) > 3.0 {
        let last = end.unwrap_or(start - Vec3::Y * length);
        s.points = (0..NODES).map(|i| start.lerp(last, i as f32 / (NODES - 1) as f32)).collect();
        s.previous = s.points.clone();
    }
    let n = s.points.len();
    let gravity = Vec3::NEG_Y * 9.8 * dt * dt;
    for i in 1..n {
        let p = s.points[i];
        let v = (p - s.previous[i]) * 0.97;
        s.previous[i] = p;
        s.points[i] = p + v + gravity;
    }
    let segment = length / (n - 1) as f32;
    for _ in 0..8 {
        s.points[0] = start;
        if let Some(end) = end {
            s.points[n - 1] = end;
        }
        for i in 0..n - 1 {
            let d = s.points[i + 1] - s.points[i];
            let l = d.length().max(1e-5);
            let fix = d * ((l - segment) / l) * 0.5;
            s.points[i] += fix;
            s.points[i + 1] -= fix;
        }
    }
    s.points[0] = start;
    if let Some(end) = end {
        s.points[n - 1] = end;
    }
    // A hanging one not through the ground (taken as level under it).
    if end.is_none() {
        let floor = world.ground_height(start.x, start.z) + 0.02;
        for p in s.points.iter_mut().skip(1) {
            p.y = p.y.max(floor);
        }
    }
}

/// All the strands as tubes, into one mesh.
fn tubes(mesh: &mut Mesh, strands: &[Strand]) {
    let (mut positions, mut normals, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for s in strands {
        let n = s.points.len();
        if n < 2 {
            continue;
        }
        let shade = [s.shade, s.shade, s.shade, 1.0];
        let mut normal = (s.points[1] - s.points[0]).normalize_or(Vec3::Y).any_orthonormal_vector();
        let base = positions.len() as u32;
        for i in 0..n {
            let tangent = (s.points[(i + 1).min(n - 1)] - s.points[i.saturating_sub(1)]).normalize_or(Vec3::Y);
            // (Carried along so the tube does not twist.)
            normal = (normal - tangent * normal.dot(tangent)).normalize_or(tangent.any_orthonormal_vector());
            let binormal = tangent.cross(normal);
            let t = i as f32 / (n - 1) as f32;
            let width = s.width.0 + (s.width.1 - s.width.0) * t;
            for k in 0..SIDES {
                let a = k as f32 / SIDES as f32 * std::f32::consts::TAU;
                let out = normal * a.cos() + binormal * a.sin();
                positions.push((s.points[i] + out * width).to_array());
                normals.push(out.to_array());
                colors.push(shade);
            }
        }
        for i in 0..n as u32 - 1 {
            for k in 0..SIDES as u32 {
                let (a, b) = (base + i * SIDES as u32 + k, base + i * SIDES as u32 + (k + 1) % SIDES as u32);
                let (c, d) = (a + SIDES as u32, b + SIDES as u32);
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}
