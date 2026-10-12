//! A painting that bleeds (the user's intrusion): drops of ichor, the
//! creatures' blood, glossy black as in `combat/ichor.rs`, welling from the
//! picture's bottom edge (`B::lay_bleed`), falling onto the frame's ledge
//! under it, where its marks lie. Each drop swells, falls, and leaves a new
//! mark, the oldest going. (A trickle down the picture and the frame to
//! the floor, beads sliding down it, did not work: the user.)

use std::collections::VecDeque;

use bevy::prelude::*;
use worldgen::noise::hash01;

/// A drop's radius; how long it swells, and how large it gets.
const BEAD: f32 = 0.016;
const SWELL: f32 = 1.4;
const GRAVITY: f32 = 9.8;
/// Marks at most (the first laid ones counted).
const MARKS: usize = 18;
/// Mark shapes (`combat`'s splats).
const SHAPES: u32 = 4;

/// Where its drops hang (in metres from the room's corner), which way the
/// wall faces, the height of what they fall on, its seed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bleed {
    pub lip: Vec3,
    pub facing: Vec3,
    pub floor: f32,
    pub seed: u32,
}

#[derive(Component)]
pub(crate) struct Bleeder {
    bleed: Bleed,
    /// Seconds to the next drop, and how many have come.
    next: f32,
    count: u32,
    marks: VecDeque<Entity>,
    looks: Looks,
}

/// What its drops and marks are made of.
#[derive(Clone)]
struct Looks {
    bead: Handle<Mesh>,
    wet: Handle<StandardMaterial>,
    quad: Handle<Mesh>,
    marks: Vec<Handle<StandardMaterial>>,
}

/// A drop: swelling at the lip (seconds), then falling (its speed).
#[derive(Component)]
pub(crate) struct Drop {
    owner: Entity,
    swell: f32,
    fall: Option<f32>,
}

pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, bleeds: &[Bleed]) {
    if bleeds.is_empty() {
        return;
    }
    let wet = materials.add(StandardMaterial { base_color: Color::srgb(0.01, 0.01, 0.01), perceptual_roughness: 0.12, reflectance: 0.7, ..default() });
    let marks = (0..SHAPES)
        .map(|k| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.01, 0.01, 0.01),
                base_color_texture: Some(images.add(crate::combat::ichor::splat_image(k + 11))),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.15,
                reflectance: 0.6,
                depth_bias: 50.0,
                ..default()
            })
        })
        .collect();
    let looks = Looks { bead: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()), wet, quad: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)).mesh()), marks };
    for b in bleeds {
        let r = |k: i32| hash01(b.seed as i32, k, 1, 0xb1e);
        // Marks already there, as if it had bled a while: small ones (a
        // ledge is a quarter metre deep), scattered along it and a little
        // out from the wall, one larger.
        let under = Vec3::new(b.lip.x, b.floor + 0.004, b.lip.z) + b.facing * 0.05;
        let side = Vec3::Y.cross(b.facing).normalize();
        let mut marks = VecDeque::new();
        for k in 0..7 {
            let a = r(10 + k) * std::f32::consts::PI;
            let d = 0.02 + 0.06 * r(20 + k);
            let size = if k == 0 { 0.12 } else { 0.04 + 0.05 * r(30 + k) };
            marks.push_back(mark(commands, &looks, under + (side * a.cos() + b.facing * a.sin()) * d, size, b.seed + k as u32));
        }
        commands.spawn(Bleeder { bleed: *b, next: 1.0 + 2.0 * r(3), count: 0, marks, looks: looks.clone() });
    }
}

/// A mark on the floor, `size` across, turned by its seed.
fn mark(commands: &mut Commands, looks: &Looks, at: Vec3, size: f32, seed: u32) -> Entity {
    let r = |k: i32| hash01(seed as i32, k, 2, 0xb1f);
    commands
        .spawn((
            Mesh3d(looks.quad.clone()),
            MeshMaterial3d(looks.marks[(r(1) * SHAPES as f32) as usize % SHAPES as usize].clone()),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(r(0) * std::f32::consts::TAU)).with_scale(Vec3::new(size, 1.0, size * (1.0 + 0.3 * r(2)))),
            bevy::light::NotShadowCaster,
        ))
        .id()
}

/// Drops come, swell at the lip, fall, leave a mark.
pub(crate) fn animate(mut commands: Commands, time: Res<Time>, mut bleeders: Query<(Entity, &mut Bleeder)>, mut drops: Query<(Entity, &mut Drop, &mut Transform)>) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut b) in &mut bleeders {
        b.next -= dt;
        if b.next <= 0.0 {
            b.count += 1;
            b.next = 2.5 + 3.5 * hash01(b.bleed.seed as i32, b.count as i32, 3, 0xb20);
            commands.spawn((Drop { owner: entity, swell: 0.0, fall: None }, Mesh3d(b.looks.bead.clone()), MeshMaterial3d(b.looks.wet.clone()), Transform::from_translation(b.bleed.lip).with_scale(Vec3::ZERO), bevy::light::NotShadowCaster));
        }
    }
    for (entity, mut drop, mut transform) in &mut drops {
        let Ok((_, mut b)) = bleeders.get_mut(drop.owner) else { continue };
        let Some(v) = drop.fall else {
            // Swelling, hanging under the lip, a hair off the face.
            drop.swell += dt;
            let k = 0.3 + 1.3 * (drop.swell / SWELL).min(1.0);
            transform.translation = b.bleed.lip + b.bleed.facing * BEAD * 1.2 - Vec3::Y * BEAD * k * 1.1;
            transform.scale = Vec3::new(BEAD * k, BEAD * k * 1.3, BEAD * k);
            if drop.swell >= SWELL {
                drop.fall = Some(0.0);
            }
            continue;
        };
        // Falling, stretched along its fall; where it lands, a mark.
        let v = v + GRAVITY * dt;
        drop.fall = Some(v);
        transform.translation.y -= v * dt;
        transform.scale = Vec3::new(BEAD * 1.6, BEAD * 1.6 * (1.0 + v * 0.25), BEAD * 1.6);
        if transform.translation.y <= b.bleed.floor {
            let r = |k: i32| hash01(b.bleed.seed as i32, b.count as i32 * 17 + k, 4, 0xb21) - 0.5;
            let side = Vec3::Y.cross(b.bleed.facing).normalize();
            let at = Vec3::new(transform.translation.x, b.bleed.floor + 0.004, transform.translation.z) + side * r(0) * 0.08 + b.bleed.facing * (r(1) + 0.5) * 0.04;
            let seed = b.bleed.seed.wrapping_add(b.count * 131 + entity.index_u32());
            let looks = b.looks.clone();
            let e = mark(&mut commands, &looks, at, 0.04 + 0.05 * (r(2) + 0.5), seed);
            b.marks.push_back(e);
            while b.marks.len() > MARKS {
                if let Some(old) = b.marks.pop_front() {
                    commands.entity(old).try_despawn();
                }
            }
            commands.entity(entity).despawn();
        }
    }
}
