//! The bishop (the user's idea, after Hexen's Dark Bishops; a lab for now,
//! `--opt bishop` with `--opt peace`): a robed thing that floats, its body
//! never seen, only the cloth round it. A robe hangs from hidden shoulders
//! over a hidden torso, its hem ragged; a hood, stiff, open at the front onto
//! darkness and two eyes. What it is shows only in how the cloth moves: the
//! hem lagging a sudden move, swinging back and settling as it stops.
//!
//! The lab: it hangs before you, facing you, and moves in turn, eight
//! seconds each (`--set bishop_mode=N` holds one): 0 still, 1 drifting from
//! side to side, 2 jerking aside, dead still between, 3 turning sharply.

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};

use super::*;
use crate::cloth::Cloth;

/// Points round the robe and down it; its hidden shoulders' height over
/// what it floats above, and the robe's length from them.
const ROUND: usize = 28;
const DOWN: usize = 20;
const SHOULDERS: f32 = 2.35;
const LENGTH: f32 = 1.95;

#[derive(Component)]
pub(super) struct Bishop {
    cloth: Cloth,
    mesh: Handle<Mesh>,
    /// Where it hangs from (the lab's middle), the lab's sideways, how far
    /// aside it is and wants to be, which way it faces and wants to, the
    /// lab's clock and when it next moves.
    home: Vec3,
    side: Vec3,
    aside: f32,
    want_aside: f32,
    heading: f32,
    want_heading: f32,
    time: f32,
    next: f32,
}

/// The hood and what is in it, carried with the bishop's head.
#[derive(Component)]
pub(super) struct Cowl {
    offset: Vec3,
}

/// The robe's points where it starts: the neck ring, the shoulders, then
/// flaring down to the hem, the hem ragged (each column of it a little
/// longer or shorter); in its own frame (x across, z forward), from its
/// shoulders.
fn robe_at(c: usize, r: usize) -> Vec3 {
    let a = c as f32 / ROUND as f32 * std::f32::consts::TAU;
    let (across, depth, y) = match r {
        0 => (0.17, 0.15, 0.08),
        1 => (0.36, 0.22, -0.05),
        _ => {
            let t = (r - 1) as f32 / (DOWN - 2) as f32;
            let ragged = if r + 4 > DOWN { (hash01(c as i32, 0, 0, 0x8b1) - 0.3) * 0.5 * (r + 4 - DOWN) as f32 / 4.0 } else { 0.0 };
            (0.36 + 0.42 * t, 0.24 + 0.36 * t, -0.05 - LENGTH * t - ragged)
        }
    };
    Vec3::new(a.sin() * across, y, a.cos() * depth)
}

/// A cowl: a deep hood, open at the front, its crown drawn back to a point;
/// in its frame (y up, z forward), from the neck.
fn cowl_mesh() -> Mesh {
    const ARC: usize = 14;
    // (height, half width, half depth, how far back)
    let rings = [(0.0, 0.2, 0.2, 0.0), (0.14, 0.2, 0.21, -0.02), (0.3, 0.17, 0.19, -0.04), (0.42, 0.11, 0.13, -0.07), (0.5, 0.03, 0.04, -0.12)];
    let open = 0.75_f32;
    let mut positions = Vec::new();
    for &(y, w, d, back) in &rings {
        for k in 0..=ARC {
            let a = open + (std::f32::consts::TAU - 2.0 * open) * k as f32 / ARC as f32;
            positions.push([a.sin() * w, y, a.cos() * d + back]);
        }
    }
    let mut indices = Vec::new();
    for r in 0..rings.len() as u32 - 1 {
        for k in 0..ARC as u32 {
            let (a, b) = (r * (ARC as u32 + 1) + k, (r + 1) * (ARC as u32 + 1) + k);
            indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh.compute_smooth_normals();
    mesh
}

/// `--opt bishop`: one, hanging before you, moving in turn.
#[allow(clippy::too_many_arguments)]
pub(super) fn lab(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    ground: crate::ground::Ground,
    camera: Single<&Transform, (With<FlyCam>, Without<Cowl>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bishops: Query<&mut Bishop>,
    mut cowls: Query<(&Cowl, &mut Transform), Without<FlyCam>>,
    mut started: Local<bool>,
) {
    if !args.opt("bishop") {
        return;
    }
    let dt = time.delta_secs().min(1.0 / 30.0);
    if !*started {
        *started = true;
        let forward = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::Z);
        let home = camera.translation + forward * 6.0;
        let home = Vec3::new(home.x, ground.at(home), home.z);
        let to = camera.translation - home;
        let heading = to.x.atan2(to.z);
        let place = Quat::from_rotation_y(heading);
        let top = home + Vec3::Y * SHOULDERS;
        let cloth = Cloth::tube(ROUND, DOWN, |c, r| top + place * robe_at(c, r));
        let mesh = meshes.add(cloth.mesh());
        // Heavy dark cloth, a faint sheen on it (black, its folds were not
        // seen), seen from both sides.
        let robe = materials.add(StandardMaterial { base_color: Color::srgb(0.07, 0.065, 0.06), perceptual_roughness: 0.75, double_sided: true, cull_mode: None, ..default() });
        commands.spawn((
            Bishop { cloth, mesh: mesh.clone(), home, side: Vec3::new(forward.z, 0.0, -forward.x), aside: 0.0, want_aside: 0.0, heading, want_heading: heading, time: 0.0, next: 1.0 },
            Mesh3d(mesh),
            MeshMaterial3d(robe.clone()),
            Transform::default(),
            bevy::camera::visibility::NoFrustumCulling,
        ));
        // The hood; in it darkness (unlit, no light gets in), and two eyes.
        let void = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, ..default() });
        let eye = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(1.0, 1.0, 1.0) * 400.0, ..default() });
        commands.spawn((Cowl { offset: Vec3::ZERO }, Mesh3d(meshes.add(cowl_mesh())), MeshMaterial3d(robe), Transform::default()));
        commands.spawn((Cowl { offset: Vec3::new(0.0, 0.22, 0.02) }, Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap())), MeshMaterial3d(void), Transform::from_scale(Vec3::new(0.16, 0.19, 0.15))));
        let glint = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
        for side in [-1.0, 1.0] {
            commands.spawn((Cowl { offset: Vec3::new(side * 0.055, 0.24, 0.15) }, Mesh3d(glint.clone()), MeshMaterial3d(eye.clone()), Transform::from_scale(Vec3::new(0.022, 0.009, 0.01))));
        }
        return;
    }
    let Ok(mut b) = bishops.single_mut() else { return };
    let b = &mut *b;
    b.time += dt;
    let mode = if args.num("bishop_mode", -1.0) >= 0.0 { args.num("bishop_mode", 0.0) as u32 } else { (b.time / 8.0) as u32 % 4 };
    let r = |k: i32| hash01((b.time * 10.0) as i32, k, 0, 0x8b2);
    let to = camera.translation - b.home;
    let facing = to.x.atan2(to.z);
    match mode {
        1 => b.want_aside = 2.2 * (b.time * 0.55).sin(),
        2 if b.time >= b.next => {
            b.next = b.time + 0.6 + 1.1 * r(0);
            b.want_aside = (r(1) - 0.5) * 6.0;
        }
        3 if b.time >= b.next => {
            b.next = b.time + 0.9 + 0.8 * r(2);
            b.want_heading = facing + (r(3) - 0.5) * 2.6;
        }
        0 => b.want_aside = 0.0,
        _ => {}
    }
    if mode != 3 {
        b.want_heading = facing;
    }
    // Drifting, it follows slowly; jerked aside, it is there in a blink, and
    // still; turning, it snaps round.
    let rate = if mode == 2 { 22.0 } else { 3.0 };
    b.aside += (b.want_aside - b.aside) * (1.0 - (-dt * rate).exp());
    let turn = (b.want_heading - b.heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    b.heading += turn * (1.0 - (-dt * if mode == 3 { 14.0 } else { 4.0 }).exp());
    let place = Quat::from_rotation_y(b.heading);
    let base = b.home + b.side * b.aside;
    let floor = ground.at(base);
    // It floats, rising and falling a little.
    let top = Vec3::new(base.x, floor + SHOULDERS + 0.04 * (b.time * 1.3).sin(), base.z);
    // Held: the neck ring and the shoulders. Bodies: the hidden torso, its
    // shoulders, its hips.
    let held: Vec<(u32, Vec3)> = (0..2 * ROUND).map(|i| (i as u32, top + place * robe_at(i % ROUND, i / ROUND))).collect();
    let bodies = [
        (top + Vec3::Y * -0.12, top + Vec3::Y * -0.95, 0.24),
        (top + place * Vec3::new(-0.3, -0.12, 0.0), top + place * Vec3::new(0.3, -0.12, 0.0), 0.12),
        (top + Vec3::Y * -1.0, top + Vec3::Y * -1.45, 0.2),
    ];
    // (Two half steps: steadier in a sudden move.)
    for _ in 0..2 {
        b.cloth.step(&held, &bodies, floor + 0.02, dt * 0.5);
    }
    if let Some(mut mesh) = meshes.get_mut(&b.mesh) {
        b.cloth.draw(&mut mesh);
    }
    let neck = top + Vec3::Y * 0.06;
    for (cowl, mut transform) in &mut cowls {
        transform.translation = neck + place * cowl.offset;
        transform.rotation = place;
    }
}
