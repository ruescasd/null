//! The bishop (the user's idea, after Hexen's Dark Bishops; a lab for now,
//! `--opt bishop` with `--opt peace`): a robed thing that floats, its body
//! never seen, only the cloth round it and the stiff things on it. After the
//! Dark Bishop: a tall pointed hood, a black void for a face; a broad stiff
//! mantle flaring off the shoulders, horns curving up from it; a stole down
//! its front; hands clasped at the chest round a dim light; a robe narrow at
//! the waist, belled out to a heavy hem. Hexen's gold and blue trims are pale
//! markings on dark here (the creatures' way). What it is shows only in how
//! the cloth moves: the hem lagging a sudden move, swinging back, settling.
//!
//! The lab: it hangs before you, facing you, and moves in turn, eight
//! seconds each (`--set bishop_mode=N` holds one): 0 still, 1 drifting from
//! side to side, 2 jerking aside, dead still between, 3 turning sharply.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
};

use super::*;
use crate::cloth::Cloth;

/// Points round the robe and down it; its hidden shoulders' height over
/// what it floats above, and the robe's length from them.
const ROUND: usize = 28;
const DOWN: usize = 20;
const SHOULDERS: f32 = 2.35;
const LENGTH: f32 = 1.95;
/// The cloth's shade and its markings' (sRGB).
const DARK: f32 = 0.035;
const PALE: f32 = 0.5;

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

/// A stiff part (the hood, the mantle, horns, hands) and what is in the
/// hood: carried with its shoulders, where it is from them and turned how.
#[derive(Component)]
pub(super) struct Stiff {
    offset: Vec3,
    turn: Quat,
}

/// The robe's points where it starts, in its own frame (x across, z
/// forward), from its shoulders: the neck ring, the shoulders, then narrow
/// at the waist and belled out to the hem, the hem ragged a little (each
/// column of it a little longer or shorter).
fn robe_at(c: usize, r: usize) -> Vec3 {
    let a = c as f32 / ROUND as f32 * std::f32::consts::TAU;
    let (across, depth, y) = match r {
        0 => (0.17, 0.15, 0.08),
        1 => (0.36, 0.22, -0.05),
        _ => {
            let t = (r - 1) as f32 / (DOWN - 2) as f32;
            let ragged = if r + 3 > DOWN { (hash01(c as i32, 0, 0, 0x8b1) - 0.5) * 0.12 * (r + 3 - DOWN) as f32 / 3.0 } else { 0.0 };
            (0.36 + 0.04 * t + 0.5 * t.powi(3), 0.24 + 0.03 * t + 0.4 * t.powi(3), -0.05 - LENGTH * t - ragged)
        }
    };
    Vec3::new(a.sin() * across, y, a.cos() * depth)
}

/// What shade a point of the robe is: pale on the stole (two bands down its
/// front) and the hem; dark else. (A belt across the stole made a cross.)
fn robe_shade(c: usize, r: usize) -> f32 {
    let stole = (c == 2 || c == ROUND - 2) && r >= 2;
    let hem = r + 1 == DOWN;
    if stole || hem { PALE } else { DARK }
}

/// A stiff shell: rings (height, half width, half depth, how far forward)
/// round all but an opening at its front (`open`, radians each side), with
/// pale edges (the opening's, and the rings `trim` names); its vertices'
/// colours carry the shade. Its frame: y up, z forward.
fn shell(rings: &[(f32, f32, f32, f32)], open: f32, trim: &[usize]) -> Mesh {
    const ARC: usize = 18;
    let (mut positions, mut colors) = (Vec::new(), Vec::new());
    for (r, &(y, w, d, fwd)) in rings.iter().enumerate() {
        for k in 0..=ARC {
            let a = open + (std::f32::consts::TAU - 2.0 * open) * k as f32 / ARC as f32;
            positions.push([a.sin() * w, y, a.cos() * d + fwd]);
            let pale = (open > 0.0 && (k == 0 || k == ARC)) || trim.contains(&r);
            let v = if pale { PALE } else { DARK };
            colors.push(Color::srgb(v, v, v).to_linear().to_f32_array());
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_smooth_normals();
    mesh
}

/// `--opt bishop`: one, hanging before you, moving in turn.
#[allow(clippy::too_many_arguments)]
pub(super) fn lab(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    assets: Res<Assets3>,
    ground: crate::ground::Ground,
    camera: Single<&Transform, (With<FlyCam>, Without<Stiff>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bishops: Query<&mut Bishop>,
    mut stiff: Query<(&Stiff, &mut Transform), Without<FlyCam>>,
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
        // (The hem's last rows keep their bell; the rest folds softly.)
        let cloth = Cloth::tube(ROUND, DOWN, |c, r| top + place * robe_at(c, r), |r| if r + 5 > DOWN { 0.7 } else { 0.3 });
        let mut robe_mesh = cloth.mesh();
        let shades: Vec<[f32; 4]> = (0..DOWN).flat_map(|r| (0..ROUND).map(move |c| robe_shade(c, r)).map(|v| Color::srgb(v, v, v).to_linear().to_f32_array())).collect();
        robe_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, shades);
        let mesh = meshes.add(robe_mesh);
        // Heavy cloth, matte (a sheen off the bright ground made it grey),
        // seen from both sides; its shade in its vertices (the markings).
        let cloth_material = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, reflectance: 0.25, double_sided: true, cull_mode: None, ..default() });
        commands.spawn((
            Bishop { cloth, mesh: mesh.clone(), home, side: Vec3::new(forward.z, 0.0, -forward.x), aside: 0.0, want_aside: 0.0, heading, want_heading: heading, time: 0.0, next: 1.0 },
            Mesh3d(mesh),
            MeshMaterial3d(cloth_material.clone()),
            Transform::default(),
            bevy::camera::visibility::NoFrustumCulling,
        ));
        let mut stiff_part = |mesh: Handle<Mesh>, material: Handle<StandardMaterial>, offset: Vec3, turn: Quat, scale: Vec3| {
            commands.spawn((Stiff { offset, turn }, Mesh3d(mesh), MeshMaterial3d(material), Transform::from_scale(scale)));
        };
        // The hood: deep, drawn back to a point (a narrow spike read as a
        // chess bishop's mitre), its opening's edge pale.
        let hood = shell(&[(0.0, 0.25, 0.25, 0.02), (0.15, 0.26, 0.27, 0.0), (0.32, 0.23, 0.25, -0.04), (0.46, 0.16, 0.18, -0.1), (0.56, 0.08, 0.09, -0.17), (0.62, 0.01, 0.01, -0.24)], 0.75, &[]);
        stiff_part(meshes.add(hood), cloth_material.clone(), Vec3::Y * 0.02, Quat::IDENTITY, Vec3::ONE);
        // In the hood: darkness (unlit: no light gets in), no eyes (a void
        // for a face, as the Dark Bishop's).
        let void = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, ..default() });
        stiff_part(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), void, Vec3::new(0.0, 0.22, 0.03), Quat::IDENTITY, Vec3::new(0.19, 0.22, 0.17));
        // The mantle: stiff, broad, falling over the shoulders like a cape's
        // collar (stuck out flat, it was a hat's brim), its edge pale, a V
        // open down its front.
        let mantle = shell(&[(0.06, 0.22, 0.19, 0.0), (-0.04, 0.4, 0.28, 0.0), (-0.2, 0.56, 0.34, 0.02), (-0.36, 0.62, 0.37, 0.03), (-0.42, 0.64, 0.38, 0.03)], 0.45, &[3, 4]);
        stiff_part(meshes.add(mantle), cloth_material.clone(), Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
        // Horns curving up off it, two to a shoulder.
        let horn = materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.6, ..default() });
        for side in [-1.0f32, 1.0] {
            for (at, dir, length) in [(Vec3::new(0.48, -0.24, -0.06), Vec3::new(0.3, 1.0, -0.12), 0.6), (Vec3::new(0.6, -0.34, 0.05), Vec3::new(0.6, 0.8, 0.0), 0.45)] {
                let at = Vec3::new(at.x * side, at.y, at.z);
                let dir = Vec3::new(dir.x * side, dir.y, dir.z).normalize();
                stiff_part(assets.shard.clone(), horn.clone(), at, Quat::from_rotation_arc(Vec3::Y, dir), Vec3::new(5.0, length / 0.7, 5.0));
            }
            // A hand: hard claws reaching in to the light at the chest.
            for k in 0..3 {
                let kk = k as f32 - 1.0;
                let dir = Vec3::new(-side * 0.8, 0.15 + 0.15 * kk, 0.3).normalize();
                stiff_part(assets.shard.clone(), horn.clone(), Vec3::new(side * 0.2, -0.55 + 0.04 * kk, 0.3), Quat::from_rotation_arc(Vec3::Y, dir), Vec3::new(2.2, 0.26 / 0.7, 2.2));
            }
        }
        // What it holds: a dim light (the glow kept for the rare ones; a
        // caster is).
        let light = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(1.0, 1.0, 1.0) * 40.0, ..default() });
        stiff_part(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), light, Vec3::new(0.0, -0.52, 0.34), Quat::IDENTITY, Vec3::splat(0.07));
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
        // (Its forearms, folded in to its hands: the robe falls over them.)
        (top + place * Vec3::new(-0.3, -0.35, 0.05), top + place * Vec3::new(0.0, -0.55, 0.28), 0.1),
        (top + place * Vec3::new(0.3, -0.35, 0.05), top + place * Vec3::new(0.0, -0.55, 0.28), 0.1),
    ];
    // (Two half steps: steadier in a sudden move.)
    for _ in 0..2 {
        b.cloth.step(&held, &bodies, floor + 0.02, dt * 0.5);
    }
    if let Some(mut mesh) = meshes.get_mut(&b.mesh) {
        b.cloth.draw(&mut mesh);
    }
    let neck = top + Vec3::Y * 0.06;
    for (part, mut transform) in &mut stiff {
        transform.translation = neck + place * part.offset;
        transform.rotation = place * part.turn;
    }
}
