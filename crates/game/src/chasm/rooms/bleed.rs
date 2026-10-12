//! A painting that bleeds (the user's intrusion): ichor seeping out of the
//! picture, the creatures' blood, glossy black as in `combat/ichor.rs`. Its
//! way is traced when the room is built (`B::lay_bleed`): down the picture,
//! out along each ledge of the frame to its lip, down the next face; to the
//! floor, a stream ending in a puddle at the wall's foot, or off an
//! overhang, a drip. Drawn as a thin ribbon along its way, a blot where it
//! seeps, splats on the floor (as if it had bled a while). Beads slide down
//! it, slowly; where it drips each swells at the lip, falls, and leaves a
//! new splat, the oldest going.

use std::collections::VecDeque;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use worldgen::noise::hash01;

/// The ribbon's width, and how fast beads slide (m/s): thick, slow.
const WIDTH: f32 = 0.05;
const SLIDE: f32 = 0.09;
/// A bead's radius; how long it swells at a lip, and how large it gets.
const BEAD: f32 = 0.016;
const SWELL: f32 = 1.4;
const GRAVITY: f32 = 9.8;
/// Splats on the floor under a drip at most (the first laid ones counted).
const SPLATS: usize = 22;
/// Splat shapes (`combat`'s).
const SHAPES: u32 = 4;

/// Its way, in metres from the room's corner (each point just off the
/// surface it runs on), which way the wall faces, the floor's height under
/// its lip if it drips (else it runs to the floor, its last point there),
/// and its seed.
#[derive(Clone, Debug)]
pub(crate) struct Bleed {
    pub path: Vec<Vec3>,
    pub facing: Vec3,
    pub drip: Option<f32>,
    pub seed: u32,
}

#[derive(Component)]
pub(crate) struct Bleeder {
    path: Vec<Vec3>,
    /// How far along its way each point is.
    along: Vec<f32>,
    facing: Vec3,
    drip: Option<f32>,
    seed: u32,
    /// Seconds to the next bead, and how many have come.
    next: f32,
    count: u32,
    splats: VecDeque<Entity>,
    looks: Looks,
}

/// What its beads and splats are made of.
#[derive(Clone)]
struct Looks {
    bead: Handle<Mesh>,
    wet: Handle<StandardMaterial>,
    quad: Handle<Mesh>,
    splats: Vec<Handle<StandardMaterial>>,
    /// Smooth ones, for what welled and spread slowly (a seep, a puddle).
    pools: Vec<Handle<StandardMaterial>>,
}

#[derive(Component)]
pub(crate) struct Bead {
    owner: Entity,
    /// How far along the way; then swelling at the lip (seconds); then
    /// falling (its speed).
    s: f32,
    swell: f32,
    fall: Option<f32>,
}

pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, bleeds: &[Bleed]) {
    if bleeds.is_empty() {
        return;
    }
    let wet = materials.add(StandardMaterial { base_color: Color::srgb(0.01, 0.01, 0.01), perceptual_roughness: 0.12, reflectance: 0.7, cull_mode: None, ..default() });
    let mut shapes = |image: Image| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.01, 0.01, 0.01),
            base_color_texture: Some(images.add(image)),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.15,
            reflectance: 0.6,
            depth_bias: 50.0,
            ..default()
        })
    };
    let splats: Vec<_> = (0..SHAPES).map(|k| shapes(crate::combat::ichor::splat_image(k + 11))).collect();
    let pools: Vec<_> = (0..SHAPES).map(|k| shapes(pool_image(k))).collect();
    let looks = Looks { bead: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), wet: wet.clone(), quad: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)).mesh()), splats, pools };
    for b in bleeds {
        commands.spawn((Mesh3d(meshes.add(ribbon(b))), MeshMaterial3d(wet.clone()), Transform::IDENTITY, bevy::light::NotShadowCaster));
        let r = |k: i32| hash01(b.seed as i32, k, 1, 0xb1e);
        // Where it seeps: a blot on the picture.
        splat(commands, &looks, b.path[0] + b.facing * 0.002, b.facing, 0.09 + 0.05 * r(0), b.seed, true);
        let mut splats = VecDeque::new();
        let end = *b.path.last().unwrap();
        match b.drip {
            // Under a drip, small splats scattered, one larger.
            Some(floor) => {
                // (Scattered out from the wall only: back under it, they lay
                // hidden under a plinth.)
                let under = Vec3::new(end.x, floor + 0.004, end.z);
                let side = Vec3::Y.cross(b.facing).normalize();
                for k in 0..8 {
                    let a = r(10 + k) * std::f32::consts::PI;
                    let d = 0.03 + 0.12 * r(20 + k);
                    let size = if k == 0 { 0.26 } else { 0.08 + 0.09 * r(30 + k) };
                    splats.push_back(splat(commands, &looks, under + (side * a.cos() + b.facing * a.sin()) * d, Vec3::Y, size, b.seed + k as u32, false));
                }
            }
            // At the foot of a stream, a puddle reaching out from the wall,
            // drops round it.
            None => {
                let at = end + b.facing * 0.14;
                splats.push_back(splat(commands, &looks, at, Vec3::Y, 0.32 + 0.12 * r(2), b.seed, true));
                for k in 0..3 {
                    let a = r(40 + k) * std::f32::consts::TAU;
                    splats.push_back(splat(commands, &looks, at + Vec3::new(a.cos(), 0.0, a.sin()) * (0.2 + 0.08 * r(50 + k)), Vec3::Y, 0.05 + 0.04 * r(60 + k), b.seed + 7 + k as u32, true));
                }
            }
        }
        let mut along = vec![0.0];
        for w in b.path.windows(2) {
            along.push(along.last().unwrap() + w[0].distance(w[1]));
        }
        commands.spawn(Bleeder { path: b.path.clone(), along, facing: b.facing, drip: b.drip, seed: b.seed, next: 1.0 + 2.0 * r(3), count: 0, splats, looks: looks.clone() });
    }
}

/// A splat lying on a surface (its normal `up`), `size` across, turned by
/// its seed: a drop's (`combat`'s, spattered), or `smooth`.
fn splat(commands: &mut Commands, looks: &Looks, at: Vec3, up: Vec3, size: f32, seed: u32, smooth: bool) -> Entity {
    let r = |k: i32| hash01(seed as i32, k, 2, 0xb1f);
    let rotation = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(r(0) * std::f32::consts::TAU);
    commands
        .spawn((
            Mesh3d(looks.quad.clone()),
            MeshMaterial3d((if smooth { &looks.pools } else { &looks.splats })[(r(1) * SHAPES as f32) as usize % SHAPES as usize].clone()),
            Transform::from_translation(at).with_rotation(rotation).with_scale(Vec3::new(size, 1.0, size * (1.0 + 0.3 * r(2)))),
            bevy::light::NotShadowCaster,
        ))
        .id()
}

/// A smooth pool, white, its shape in the alpha: a wobbling round edge
/// (what welled and spread slowly, not what struck).
fn pool_image(seed: u32) -> Image {
    const N: u32 = 128;
    let r = |k: i32| hash01(seed as i32, k, 6, 0xb23);
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = ((x as f32 + 0.5) / N as f32 - 0.5, (y as f32 + 0.5) / N as f32 - 0.5);
            let (d, a) = ((dx * dx + dy * dy).sqrt(), dy.atan2(dx));
            let edge = 0.36 + 0.05 * (a * 2.0 + r(0) * 6.0).sin() + 0.03 * (a * 3.0 + r(1) * 6.0).sin() + 0.015 * (a * 7.0 + r(2) * 6.0).sin();
            let alpha = ((edge - d) * N as f32 * 0.5).clamp(0.0, 1.0) * 0.97;
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    Image::new(Extent3d { width: N, height: N, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)
}

/// Beads come, slide down the way, swell at the lip, fall, splat.
pub(crate) fn animate(mut commands: Commands, time: Res<Time>, mut bleeders: Query<(Entity, &mut Bleeder)>, mut beads: Query<(Entity, &mut Bead, &mut Transform)>) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut b) in &mut bleeders {
        b.next -= dt;
        if b.next <= 0.0 {
            b.count += 1;
            let r = hash01(b.seed as i32, b.count as i32, 3, 0xb20);
            b.next = 2.5 + 3.5 * r;
            commands.spawn((Bead { owner: entity, s: 0.0, swell: 0.0, fall: None }, Mesh3d(b.looks.bead.clone()), MeshMaterial3d(b.looks.wet.clone()), Transform::from_translation(b.path[0]).with_scale(Vec3::ZERO), bevy::light::NotShadowCaster));
        }
    }
    for (entity, mut bead, mut transform) in &mut beads {
        let Ok((_, mut b)) = bleeders.get_mut(bead.owner) else { continue };
        let total = *b.along.last().unwrap();
        if let Some(v) = bead.fall {
            // Falling: stretched along its fall; on the floor, a splat.
            let v = v + GRAVITY * dt;
            bead.fall = Some(v);
            transform.translation.y -= v * dt;
            transform.scale = Vec3::new(BEAD * 1.3, BEAD * 1.3 * (1.0 + v * 0.25), BEAD * 1.3);
            transform.rotation = Quat::IDENTITY;
            let floor = b.drip.unwrap_or(f32::MIN);
            if transform.translation.y <= floor {
                let r = |k: i32| hash01(b.seed as i32, b.count as i32 * 17 + k, 4, 0xb21) - 0.5;
                let at = Vec3::new(transform.translation.x, floor + 0.004, transform.translation.z) + (Vec3::Y.cross(b.facing).normalize() * r(0) + b.facing * (r(1) + 0.5)) * 0.12;
                let seed = b.seed.wrapping_add(b.count * 131 + entity.index_u32());
                let looks = b.looks.clone();
                let e = splat(&mut commands, &looks, at, Vec3::Y, 0.06 + 0.07 * (r(2) + 0.5), seed, false);
                b.splats.push_back(e);
                while b.splats.len() > SPLATS {
                    if let Some(old) = b.splats.pop_front() {
                        commands.entity(old).try_despawn();
                    }
                }
                commands.entity(entity).despawn();
            }
            continue;
        }
        if bead.s >= total {
            // At the end: into the puddle, or swelling at the lip, hanging.
            let Some(_) = b.drip else {
                commands.entity(entity).despawn();
                continue;
            };
            bead.swell += dt;
            let k = 1.0 + 0.6 * (bead.swell / SWELL).min(1.0);
            let lip = *b.path.last().unwrap();
            transform.translation = lip - Vec3::Y * BEAD * k * 1.1;
            transform.scale = Vec3::new(BEAD * k, BEAD * k * 1.3, BEAD * k);
            transform.rotation = Quat::IDENTITY;
            if bead.swell >= SWELL {
                bead.fall = Some(0.0);
            }
            continue;
        }
        // Sliding: where along the way, stretched along it.
        bead.s += SLIDE * dt;
        let i = b.along.partition_point(|&a| a <= bead.s).clamp(1, b.path.len() - 1);
        let (p, q) = (b.path[i - 1], b.path[i]);
        let t = ((bead.s - b.along[i - 1]) / (b.along[i] - b.along[i - 1]).max(1e-4)).clamp(0.0, 1.0);
        let dir = (q - p).normalize_or(Vec3::NEG_Y);
        let up = if dir.y.abs() > 0.5 { b.facing } else { Vec3::Y };
        // (Grown in over its first few centimetres, out of the picture.)
        let grow = (bead.s / 0.05).min(1.0);
        transform.translation = p.lerp(q, t) + up * BEAD * 0.4;
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
        transform.scale = Vec3::new(BEAD, BEAD * 1.8, BEAD * 0.5) * grow;
    }
}

/// The ribbon along a bleed's way, lying on each surface it runs on (a
/// face, or a ledge's top): narrow where it seeps, wandering a little from
/// side to side down a face.
fn ribbon(b: &Bleed) -> Mesh {
    let r = |k: i32| hash01(b.seed as i32, k, 5, 0xb22) - 0.5;
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    let mut s = 0.0;
    for (j, w) in b.path.windows(2).enumerate() {
        let (p, q) = (w[0], w[1]);
        let length = p.distance(q);
        if length < 1e-4 {
            continue;
        }
        let dir = (q - p) / length;
        let up = if dir.y.abs() > 0.5 { b.facing } else { Vec3::Y };
        let side = dir.cross(up).normalize_or(Vec3::X);
        let steps = (length / 0.05).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let at = s + length * t;
            // (Wandering only down a face, not at its corners.)
            let wander = if dir.y.abs() > 0.5 { 0.008 * (at * 9.0 + r(j as i32) * 6.0).sin() * (t * (1.0 - t) * 4.0).min(1.0) } else { 0.0 };
            let width = WIDTH * (0.3 + 0.7 * (at / 0.12).min(1.0)) * (1.0 + 0.25 * (at * 13.0 + r(100) * 6.0).sin());
            let c = p.lerp(q, t) + side * wander;
            let base = positions.len() as u32;
            positions.push((c - side * width * 0.5).to_array());
            positions.push((c + side * width * 0.5).to_array());
            normals.push(up.to_array());
            normals.push(up.to_array());
            if k > 0 {
                indices.extend_from_slice(&[base - 2, base - 1, base, base - 1, base + 1, base]);
            }
        }
        s += length;
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let uvs = vec![[0.0f32, 0.0]; positions.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
