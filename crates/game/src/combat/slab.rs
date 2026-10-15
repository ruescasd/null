//! The porter's load (`--set slab N`): what it carries, unknown, holding
//! energy (the user: the plain white slab too plain; it may detonate it,
//! and its energy may be drained). Each kind built once, as a mesh or two
//! (one for each finish), in the load's own frame: across it x, along the
//! back y, out from the back -z; its size `rig::LOAD_SIZE`. Its light pulses
//! slowly (see `pulse`).
//!
//! 0. The plain slab, as it was.
//! 2. Caged: the creatures' substance (dark fragments round a glowing core,
//!    as the orb's) held in a heavy dark frame.
//! 3. Seams: a clean pale monolith, glowing lines set in a recursive
//!    pattern of grooves on its outer face; bound with straps.
//!
//! (The user: both 2 and 3 good, not yet settled. 1, carved, a fragment of
//! the architecture cut in recessed panels, a core seen through its
//! deepest cuts, was taken out: its light white on pale stone, lost.)

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use worldgen::{
    ifs::{self, Block, Keep, Rule},
    noise::hash01,
};

/// The load's size (m): across, along the back, deep.
const SIZE: Vec3 = crate::rig::LOAD_SIZE;
/// How bright its light, and how it pulses (s a beat; how much of it).
const GLOW: f32 = 6.0;
const BEAT: f32 = 1.7;
const SWELL: f32 = 0.45;

/// A kind of load: its number (`--set slab`), its parts, each a mesh and
/// its finish.
pub(super) type Kind = (usize, Vec<(Handle<Mesh>, Handle<StandardMaterial>)>);

/// The loads' glowing finish, pulsed (see `pulse`).
#[derive(Resource)]
pub(super) struct Glow(Handle<StandardMaterial>);

/// A mesh being put together from others, each moved into place.
#[derive(Default)]
struct Merged {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl Merged {
    fn add(&mut self, mesh: &Mesh, place: Transform) {
        let base = self.positions.len() as u32;
        let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()) else { return };
        let normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).and_then(|a| a.as_float3());
        let matrix = place.to_matrix();
        for (k, p) in positions.iter().enumerate() {
            self.positions.push(matrix.transform_point3(Vec3::from_array(*p)).to_array());
            let n = normals.map_or(Vec3::Y, |n| Vec3::from_array(n[k]));
            // (Normals by the inverse transpose: scaled boxes keep them square.)
            self.normals.push((place.rotation * (n / place.scale)).normalize_or(Vec3::Y).to_array());
        }
        match mesh.indices() {
            Some(Indices::U32(i)) => self.indices.extend(i.iter().map(|i| base + i)),
            Some(Indices::U16(i)) => self.indices.extend(i.iter().map(|i| base + *i as u32)),
            None => self.indices.extend(base..base + positions.len() as u32),
        }
    }

    /// A box, centred at `at`, `half` its size each way.
    fn cube(&mut self, at: Vec3, half: Vec3) {
        self.add(&Cuboid::new(2.0, 2.0, 2.0).mesh().build(), Transform::from_translation(at).with_scale(half));
    }

    fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// Dark bolted straps round it, across it, at `at` along it (each a band
/// round its four faces, standing off them, and a plate with bolts on its
/// outer face): heavy-duty, worn by use.
fn straps(m: &mut Merged, at: &[f32]) {
    let (w, t) = (0.028, 0.008);
    let h = SIZE * 0.5 + Vec3::splat(t);
    for &y in at {
        m.cube(Vec3::new(0.0, y, -h.z), Vec3::new(h.x, w, t));
        m.cube(Vec3::new(0.0, y, h.z), Vec3::new(h.x, w, t));
        m.cube(Vec3::new(-h.x, y, 0.0), Vec3::new(t, w, h.z));
        m.cube(Vec3::new(h.x, y, 0.0), Vec3::new(t, w, h.z));
        // The plate, and its four bolts.
        m.cube(Vec3::new(0.0, y, -h.z - t), Vec3::new(0.06, w * 1.6, t));
        for (bx, by) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            m.cube(Vec3::new(bx * 0.042, y + by * w * 1.05, -h.z - 2.6 * t), Vec3::splat(0.006));
        }
    }
}

/// Caged: the substance (as the orb's: dark fragments with gaps, sparser
/// within, a glowing core), filling the frame; the frame's twelve edges and
/// two bars across its outer face, dark, bolted at its corners.
fn caged(meshes: &[Mesh; 3], finishes: &mut [Merged; 3], glow: &mut Merged, frame: &mut Merged) {
    let half = SIZE * 0.5 - Vec3::splat(0.02);
    let rule = Rule { divisions: [3, 4, 2], keep: Keep::All, depth: 2, gap: 1.0, twist: Quat::IDENTITY, stop_chance: 0.0, lift: 0.0, min_size: 0.0 };
    let root = Block { center: Vec3::ZERO, rotation: Quat::IDENTITY, half, level: 0 };
    for (n, leaf) in ifs::generate(&rule, root, 0x5ac, 5000).iter().enumerate() {
        let b = leaf.block;
        let r = |k: i32| hash01(n as i32, k, 0, 0x5ad);
        // How deep in it is (0 at its surface, 1 at its middle).
        let d = 1.0 - (b.center.abs() / half).max_element();
        let (keep, glows) = if d < 0.3 { (r(1) < 0.7, false) } else { (r(1) < 0.8, r(2) < 0.8) };
        if !keep {
            continue;
        }
        let shrink = Vec3::new(0.55 + 0.4 * r(3), 0.55 + 0.4 * r(4), 0.55 + 0.4 * r(5));
        let tilt = Quat::from_euler(EulerRot::YXZ, (r(6) - 0.5) * 0.9, (r(7) - 0.5) * 0.7, (r(8) - 0.5) * 0.7);
        let place = Transform::from_translation(b.center).with_rotation(tilt).with_scale(b.half * shrink);
        let shape = &meshes[(r(9) * 3.0) as usize % 3];
        if glows {
            glow.add(shape, place);
        } else {
            finishes[(r(10) * 3.0) as usize % 3].add(shape, place);
        }
    }
    let (h, bar) = (SIZE * 0.5 + Vec3::splat(0.012), 0.016);
    for (a, b) in [(1, 2), (0, 2), (0, 1)] {
        for s in [-1.0, 1.0] {
            for t in [-1.0, 1.0] {
                let mut at = Vec3::ZERO;
                at[a] = s * h[a];
                at[b] = t * h[b];
                let mut half = Vec3::splat(bar);
                let along = 3 - a - b;
                half[along] = h[along] + bar;
                frame.cube(at, half);
            }
        }
    }
    for y in [-0.18, 0.18] {
        frame.cube(Vec3::new(0.0, y, -h.z), Vec3::new(h.x, bar, bar));
    }
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                frame.cube(Vec3::new(x * h.x, y * h.y, z * h.z), Vec3::splat(bar * 1.8));
            }
        }
    }
}

/// Seams: a pale monolith, its outer face split and split again (each part
/// across or along, unevenly), a glowing line set in each split.
fn seams(stone: &mut Merged, glow: &mut Merged) {
    let h = SIZE * 0.5;
    stone.cube(Vec3::ZERO, h);
    fn split(glow: &mut Merged, lo: Vec2, hi: Vec2, level: u32, n: i32, face: f32) {
        if level == 4 {
            return;
        }
        let r = |k: i32| hash01(n, k, level as i32, 0x5ae);
        let size = hi - lo;
        // (Along its longer side, a third to two thirds of the way.)
        let across = size.x > size.y;
        let t = 0.33 + 0.34 * r(1);
        // (Set in its face, a little proud of it.)
        let (line, w) = (0.006, 0.0015);
        if across {
            let x = lo.x + size.x * t;
            glow.cube(Vec3::new(x, (lo.y + hi.y) * 0.5, face - w), Vec3::new(line, size.y * 0.5, w));
            split(glow, lo, Vec2::new(x, hi.y), level + 1, n * 2, face);
            split(glow, Vec2::new(x, lo.y), hi, level + 1, n * 2 + 1, face);
        } else {
            let y = lo.y + size.y * t;
            glow.cube(Vec3::new((lo.x + hi.x) * 0.5, y, face - w), Vec3::new(size.x * 0.5, line, w));
            split(glow, lo, Vec2::new(hi.x, y), level + 1, n * 2, face);
            split(glow, Vec2::new(lo.x, y), hi, level + 1, n * 2 + 1, face);
        }
    }
    split(glow, Vec2::new(-h.x, -h.y), Vec2::new(h.x, h.y), 0, 1, -h.z + 0.001);
}

/// Every kind of load, built once (see the module's list); its glowing
/// finish kept to be pulsed.
pub(super) fn build(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Vec<Kind> {
    // (Clean, pale, a little sheen: the world's own matter. Grimy dark
    // straps and frame: worked metal, oiled. The core's light.)
    let stone = materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.79, 0.76), perceptual_roughness: 0.45, reflectance: 0.4, ..default() });
    let metal = materials.add(StandardMaterial { base_color: Color::srgb(0.06, 0.055, 0.05), perceptual_roughness: 0.55, metallic: 0.6, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(GLOW), ..default() });
    let finishes = [(0.22, 0.35, 0.7), (0.09, 0.55, 0.5), (0.03, 0.2, 0.9)].map(|(tone, rough, refl)| materials.add(StandardMaterial { base_color: Color::srgb(tone, tone, tone), perceptual_roughness: rough, reflectance: refl, ..default() }));
    let mut kinds = Vec::new();
    // 0: plain.
    let mut plain = Merged::default();
    plain.cube(Vec3::ZERO, SIZE * 0.5);
    kinds.push((0, vec![(meshes.add(plain.build()), materials.add(StandardMaterial { base_color: Color::srgb(0.78, 0.77, 0.74), perceptual_roughness: 0.9, ..default() }))]));
    // 2: caged.
    let shapes = [
        Cuboid::new(2.0, 2.0, 2.0).mesh().build(),
        crate::figure::faceted(&[[-1., -1., -1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.], [1., -1., 1.], [-1., 1., 1.]], &[&[0, 2, 1], &[3, 4, 5], &[0, 1, 4, 3], &[0, 3, 5, 2], &[1, 2, 5, 4]]),
        crate::figure::faceted(&[[-1., -1., -1.], [1., -1., -0.6], [-0.2, -1., 1.], [0.3, 1., 0.1]], &[&[0, 1, 2], &[0, 3, 1], &[1, 3, 2], &[2, 3, 0]]),
    ];
    let (mut fs, mut g, mut m) = ([Merged::default(), Merged::default(), Merged::default()], Merged::default(), Merged::default());
    caged(&shapes, &mut fs, &mut g, &mut m);
    let mut parts: Vec<_> = fs.into_iter().zip(finishes).map(|(f, finish)| (meshes.add(f.build()), finish)).collect();
    parts.push((meshes.add(g.build()), glow.clone()));
    parts.push((meshes.add(m.build()), metal.clone()));
    kinds.push((2, parts));
    // 3: seams.
    let (mut s, mut g, mut m) = (Merged::default(), Merged::default(), Merged::default());
    seams(&mut s, &mut g);
    straps(&mut m, &[-0.26, 0.26]);
    kinds.push((3, vec![(meshes.add(s.build()), stone), (meshes.add(g.build()), glow.clone()), (meshes.add(m.build()), metal)]));
    commands.insert_resource(Glow(glow));
    kinds
}

/// The loads' light pulses, slowly, swelling and fading.
pub(super) fn pulse(time: Res<Time>, glow: Option<Res<Glow>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let Some(glow) = glow else { return };
    let beat = (time.elapsed_secs() / BEAT * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    if let Some(mut m) = materials.get_mut(&glow.0) {
        m.emissive = LinearRgba::gray(GLOW * (1.0 - SWELL + SWELL * beat * beat));
    }
}

/// A load shown in the lab, turning slowly about its long axis.
#[derive(Component)]
pub(super) struct Shown(f32);

/// `--opt slablab`: the kinds of load (but the plain) side by side before
/// you at eye height, their outer faces to you, turning slowly to and fro.
#[allow(clippy::too_many_arguments)]
pub(super) fn lab(
    mut commands: Commands,
    args: Res<crate::Args>,
    time: Res<Time>,
    streamer: Res<crate::terrain::Streamer>,
    camera: Single<&Transform, (With<crate::camera::FlyCam>, Without<Shown>)>,
    heads: Res<super::hunter::Heads>,
    mut shown: Query<(&Shown, &mut Transform)>,
    mut settled_for: Local<f32>,
    mut spawned: Local<bool>,
) {
    if !args.opt("slablab") {
        return;
    }
    if !*spawned {
        *settled_for = if streamer.near { *settled_for + time.delta_secs() } else { 0.0 };
        if *settled_for < 1.0 {
            return;
        }
        *spawned = true;
        let eye = camera.translation;
        let ahead = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::NEG_Z);
        let across = Vec3::new(-ahead.z, 0.0, ahead.x);
        let kinds: Vec<_> = heads.loads().iter().filter(|(k, _)| *k != 0).collect();
        let n = kinds.len();
        for (j, (k, parts)) in kinds.into_iter().enumerate() {
            let at = eye + ahead * 1.7 + across * (j as f32 - (n as f32 - 1.0) * 0.5) * 0.8 - Vec3::Y * 0.1;
            let load = commands.spawn((Shown(*k as f32), Transform::from_translation(at).looking_to(-ahead, Vec3::Y), Visibility::default())).id();
            for (mesh, material) in parts {
                commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default(), ChildOf(load)));
            }
        }
        return;
    }
    let t = time.elapsed_secs();
    for (s, mut transform) in &mut shown {
        let yaw = (t * 0.5 + s.0).sin() * 0.6;
        let face = camera.translation - transform.translation;
        let face = Vec3::new(face.x, 0.0, face.z).normalize_or(Vec3::Z);
        // (Its outer face, -z, to you, turned to and fro.)
        transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, face) * Quat::from_rotation_y(yaw);
    }
}
