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
const SHOULDERS: f32 = 2.1;
const LENGTH: f32 = 1.75;
/// The cloth's shade and its markings' (sRGB).
const DARK: f32 = 0.035;
/// How far its head is bowed (radians).
const BOW: f32 = 0.85;
const PALE: f32 = 0.5;

/// Points round a sleeve and along it; where its hidden arm runs (from its
/// shoulders' middle, its frame: x across, z forward): shoulder, elbow,
/// wrist, the right arm's (the left's mirrored).
const SLEEVE_ROUND: usize = 10;
const SLEEVE_ALONG: usize = 9;
const ARM: [Vec3; 3] = [Vec3::new(0.36, -0.14, -0.02), Vec3::new(0.42, -0.62, 0.04), Vec3::new(0.12, -0.66, 0.25)];

#[derive(Component)]
pub(super) struct Bishop {
    cloth: Cloth,
    mesh: Handle<Mesh>,
    /// Its sleeves (left, right), and their meshes.
    sleeves: Vec<(Cloth, Handle<Mesh>)>,
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
    /// How fast it is moving aside (m/s), and where its shoulders were.
    pace: f32,
    last_top: Vec3,
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
        0 => (0.13, 0.12, 0.08),
        1 => (0.27, 0.17, -0.05),
        _ => {
            let t = (r - 1) as f32 / (DOWN - 2) as f32;
            let ragged = if r + 3 > DOWN { (hash01(c as i32, 0, 0, 0x8b1) - 0.5) * 0.12 * (r + 3 - DOWN) as f32 / 3.0 } else { 0.0 };
            (0.27 + 0.02 * t + 0.32 * t.powi(3), 0.18 + 0.02 * t + 0.27 * t.powi(3), -0.05 - LENGTH * t - ragged)
        }
    };
    Vec3::new(a.sin() * across, y, a.cos() * depth)
}

/// Where along the hidden arm (0 shoulder, 1 wrist), on side `side` (-1
/// left, 1 right).
fn arm_at(s: f32, side: f32) -> Vec3 {
    let p = if s < 0.5 { ARM[0].lerp(ARM[1], s * 2.0) } else { ARM[1].lerp(ARM[2], s * 2.0 - 1.0) };
    Vec3::new(p.x * side, p.y, p.z)
}

/// A sleeve's points where it starts (its frame as the robe's): rings round
/// the arm, widening from the shoulder to the wrist, and past the wrist a
/// bell, wide, the cuff.
fn sleeve_at(c: usize, r: usize, side: f32) -> Vec3 {
    let cuff = r + 1 == SLEEVE_ALONG;
    let s = (r.min(SLEEVE_ALONG - 2)) as f32 / (SLEEVE_ALONG - 2) as f32;
    let along = (arm_at((s + 0.02).min(1.0), side) - arm_at((s - 0.02).max(0.0), side)).normalize_or(Vec3::NEG_Y);
    let across = along.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = across.cross(along);
    let a = c as f32 / SLEEVE_ROUND as f32 * std::f32::consts::TAU;
    let radius = if cuff { 0.19 } else { 0.09 + 0.06 * s };
    let centre = arm_at(s, side) + if cuff { along * 0.14 - Vec3::Y * 0.05 } else { Vec3::ZERO };
    centre + (across * a.cos() + up * a.sin()) * radius
}

/// What shade a point of the robe is: pale on the stole (two bands down its
/// front) and the hem; dark else. (A belt across the stole made a cross.)
fn robe_shade(c: usize, r: usize) -> f32 {
    let stole = (c == 2 || c == ROUND - 2) && r >= 2;
    let hem = r + 1 == DOWN;
    if stole || hem { PALE } else { DARK }
}

/// Where the hood bows from (its frame: from the neck, y up, z forward).
const PIVOT: Vec3 = Vec3::new(0.0, -0.02, 0.04);

/// The hood and the mantle, one piece, as a hooded cape is: from the hood's
/// crown down round a head, its opening narrowing to close under the chin,
/// on round the throat and out over the shoulders, sloping to their
/// corners, longer at the front and back over the chest; the hood bowed at
/// the neck (`BOW`), easing into the mantle (a stiff hood bowed on its own
/// did not show it, its point read as a triangle: hoods drape, the user).
/// Pale at the hood's opening and the mantle's lip. Its frame: from the
/// neck, y up, z forward.
fn capelet() -> Mesh {
    const ARC: usize = 22;
    // (height, half width, half depth, how far forward, the opening each
    // side of the front, radians)
    let rings: [(f32, f32, f32, f32, f32); 14] = [
        (0.42, 0.005, 0.01, 0.015, 0.75),
        (0.41, 0.045, 0.06, 0.02, 0.75),
        (0.385, 0.095, 0.115, 0.03, 0.75),
        (0.34, 0.14, 0.165, 0.04, 0.75),
        (0.28, 0.175, 0.2, 0.05, 0.75),
        (0.2, 0.2, 0.22, 0.06, 0.75),
        (0.1, 0.21, 0.22, 0.07, 0.72),
        (0.02, 0.2, 0.2, 0.07, 0.55),
        (-0.04, 0.18, 0.17, 0.05, 0.25),
        (-0.08, 0.2, 0.17, 0.02, 0.0),
        (-0.12, 0.33, 0.23, 0.0, 0.0),
        (-0.2, 0.5, 0.28, 0.0, 0.0),
        (-0.27, 0.52, 0.29, 0.0, 0.0),
        (-0.3, 0.52, 0.29, 0.0, 0.0),
    ];
    let lip = rings.len() - 1;
    let (mut positions, mut colors) = (Vec::new(), Vec::new());
    for (r, &(y, w, d, fwd, open)) in rings.iter().enumerate() {
        // The mantle longer at its front and back.
        let droop = [0.0, 0.0, 0.5, 0.9, 1.0][r.saturating_sub(9).min(4)];
        // The hood bowed, easing in from the throat.
        let t = ((y + 0.06) / 0.14).clamp(0.0, 1.0);
        let bow = Quat::from_rotation_x(BOW * t * t * (3.0 - 2.0 * t));
        for k in 0..=ARC {
            let a = open + (std::f32::consts::TAU - 2.0 * open) * k as f32 / ARC as f32;
            let p = Vec3::new(a.sin() * w, y - 0.14 * droop * a.cos().powi(2), a.cos() * d + fwd);
            positions.push((PIVOT + bow * (p - PIVOT)).to_array());
            let pale = (open > 0.0 && (k == 0 || k == ARC)) || r == lip;
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
        // (`--opt bishopgrey`: all of it one flat grey, to see its outline.)
        let grey = args.opt("bishopgrey");
        let mut robe_mesh = cloth.mesh();
        let shades: Vec<[f32; 4]> = (0..DOWN).flat_map(|r| (0..ROUND).map(move |c| robe_shade(c, r)).map(|v| Color::srgb(v, v, v).to_linear().to_f32_array())).collect();
        if !grey {
            robe_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, shades);
        }
        let mesh = meshes.add(robe_mesh);
        // Heavy cloth, matte (a sheen off the bright ground made it grey),
        // seen from both sides; its shade in its vertices (the markings).
        let cloth_material = materials.add(StandardMaterial { base_color: if grey { Color::srgb(0.4, 0.4, 0.4) } else { Color::WHITE }, perceptual_roughness: 0.9, reflectance: 0.25, double_sided: true, cull_mode: None, ..default() });
        let flat = |mut mesh: Mesh| {
            if grey {
                mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
            }
            mesh
        };
        // Its sleeves, hanging off hidden arms bent in to its hands, their
        // cuffs drooping (the user: what makes the Dark Bishop's outline is
        // its arms).
        let mut sleeves = Vec::new();
        for side in [-1.0f32, 1.0] {
            let sleeve = Cloth::tube(SLEEVE_ROUND, SLEEVE_ALONG, |c, r| top + place * sleeve_at(c, r, side), |_| 0.35);
            let mut sleeve_mesh = sleeve.mesh();
            if !grey {
                let shades: Vec<[f32; 4]> = (0..SLEEVE_ALONG).flat_map(|r| (0..SLEEVE_ROUND).map(move |_| if r + 1 == SLEEVE_ALONG { PALE } else { DARK })).map(|v| Color::srgb(v, v, v).to_linear().to_f32_array()).collect();
                sleeve_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, shades);
            }
            let handle = meshes.add(sleeve_mesh);
            commands.spawn((Mesh3d(handle.clone()), MeshMaterial3d(cloth_material.clone()), Transform::default(), bevy::camera::visibility::NoFrustumCulling));
            sleeves.push((sleeve, handle));
        }
        commands.spawn((
            Bishop { cloth, mesh: mesh.clone(), sleeves, home, side: Vec3::new(forward.z, 0.0, -forward.x), aside: 0.0, want_aside: 0.0, heading, want_heading: heading, time: 0.0, next: 1.0, pace: 0.0, last_top: top },
            Mesh3d(mesh),
            MeshMaterial3d(cloth_material.clone()),
            Transform::default(),
            bevy::camera::visibility::NoFrustumCulling,
        ));
        let mut stiff_part = |mesh: Handle<Mesh>, material: Handle<StandardMaterial>, offset: Vec3, turn: Quat, scale: Vec3| {
            commands.spawn((Stiff { offset, turn }, Mesh3d(mesh), MeshMaterial3d(material), Transform::from_scale(scale)));
        };
        // The hood and the mantle: one piece (the user, with a hooded cape
        // for reference), the hood bowed (see `capelet`); in the hood,
        // darkness (unlit: no light gets in), no eyes (a void for a face, as
        // the Dark Bishop's).
        stiff_part(meshes.add(flat(capelet())), cloth_material.clone(), Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
        let void = if grey { cloth_material.clone() } else { materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, ..default() }) };
        let bow = Quat::from_rotation_x(BOW);
        stiff_part(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), void, PIVOT + bow * (Vec3::new(0.0, 0.19, 0.07) - PIVOT), bow, Vec3::new(0.15, 0.18, 0.13));
        // Horns curving up off it, two to a shoulder.
        let horn = if grey { cloth_material.clone() } else { materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.6, ..default() }) };
        for side in [-1.0f32, 1.0] {
            // (At the mantle's shoulder corners.)
            for (at, dir, length) in [(Vec3::new(0.42, -0.05, -0.06), Vec3::new(0.2, 1.0, -0.15), 0.55), (Vec3::new(0.52, -0.1, 0.03), Vec3::new(0.45, 0.9, 0.0), 0.38)] {
                let at = Vec3::new(at.x * side, at.y, at.z);
                let dir = Vec3::new(dir.x * side, dir.y, dir.z).normalize();
                stiff_part(assets.shard.clone(), horn.clone(), at, Quat::from_rotation_arc(Vec3::Y, dir), Vec3::new(5.0, length / 0.7, 5.0));
            }
            // A hand: hard claws reaching in to the light at the chest.
            for k in 0..3 {
                let kk = k as f32 - 1.0;
                let dir = Vec3::new(-side * 0.8, 0.15 + 0.15 * kk, 0.3).normalize();
                stiff_part(assets.shard.clone(), horn.clone(), Vec3::new(side * 0.15, -0.63 + 0.04 * kk, 0.25), Quat::from_rotation_arc(Vec3::Y, dir), Vec3::new(2.2, 0.26 / 0.7, 2.2));
            }
        }
        // What it holds: a dim light (the glow kept for the rare ones; a
        // caster is).
        let light = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(1.0, 1.0, 1.0) * 40.0, ..default() });
        stiff_part(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), light, Vec3::new(0.0, -0.6, 0.29), Quat::IDENTITY, Vec3::splat(0.06));
        return;
    }
    let Ok(mut b) = bishops.single_mut() else { return };
    let b = &mut *b;
    b.time += dt;
    let mode = if args.num("bishop_mode", -1.0) >= 0.0 { args.num("bishop_mode", 0.0) as u32 } else { (b.time / 8.0) as u32 % 4 };
    let r = |k: i32| hash01((b.time * 10.0) as i32, k, 0, 0x8b2);
    let to = camera.translation - b.home;
    // (`--set bishop_yaw=D`: it faces D degrees off you, to see it from
    // the side.)
    let facing = to.x.atan2(to.z) + args.num("bishop_yaw", 0.0).to_radians();
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
    // (Jerked, a lunge at up to 18 m/s, sudden, braked hard: snapped there
    // at once, 66 m/s at first, it flung its robe up over its head.)
    if mode == 2 {
        let to = b.want_aside - b.aside;
        let want = (to * 12.0).clamp(-18.0, 18.0);
        b.pace += (want - b.pace).clamp(-200.0 * dt, 200.0 * dt);
        b.aside += b.pace * dt;
    } else {
        b.pace = 0.0;
        b.aside += (b.want_aside - b.aside) * (1.0 - (-dt * 3.0).exp());
    }
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
        (top + Vec3::Y * -0.12, top + Vec3::Y * -0.95, 0.17),
        (top + place * Vec3::new(-0.22, -0.1, 0.0), top + place * Vec3::new(0.22, -0.1, 0.0), 0.1),
        (top + Vec3::Y * -1.0, top + Vec3::Y * -1.45, 0.14),
        // (Not its forearms: the robe at its sides, pushed out of them
        // through their middles, flipped and tangled, and in the end turned
        // up over its head, the user. The sleeves hide where they meet.)
    ];
    // (Steps enough that its shoulders go no more than 3 cm a step: in a
    // sudden move it keeps up.)
    let steps = ((top.distance(b.last_top) / 0.03).ceil() as usize).clamp(2, 10);
    b.last_top = top;
    for _ in 0..steps {
        b.cloth.step(&held, &bodies, floor + 0.02, top.y + 0.1, dt / steps as f32);
    }
    if let Some(mut mesh) = meshes.get_mut(&b.mesh) {
        b.cloth.draw(&mut mesh);
    }
    // The sleeves: held at the shoulder all round and at the wrist along
    // its top (the cuff hangs), over the hidden arm.
    for (k, (sleeve, handle)) in b.sleeves.iter_mut().enumerate() {
        let side = if k == 0 { -1.0 } else { 1.0 };
        let wrist = SLEEVE_ALONG - 2;
        let held: Vec<(u32, Vec3)> = (0..SLEEVE_ROUND)
            .map(|c| (c, 0))
            .chain((0..SLEEVE_ROUND).filter(|&c| (c as f32 / SLEEVE_ROUND as f32 * std::f32::consts::TAU).sin() > 0.0).map(|c| (c, wrist)))
            .map(|(c, r)| ((r * SLEEVE_ROUND + c) as u32, top + place * sleeve_at(c, r, side)))
            .collect();
        let arm = [
            (top + place * arm_at(0.0, side), top + place * arm_at(0.5, side), 0.09),
            (top + place * arm_at(0.5, side), top + place * arm_at(1.0, side), 0.08),
        ];
        for _ in 0..steps {
            sleeve.step(&held, &arm, floor + 0.02, top.y + 0.05, dt / steps as f32);
        }
        if let Some(mut mesh) = meshes.get_mut(&*handle) {
            sleeve.draw(&mut mesh);
        }
    }
    let neck = top + Vec3::Y * 0.06;
    for (part, mut transform) in &mut stiff {
        transform.translation = neck + place * part.offset;
        transform.rotation = place * part.turn;
    }
}
