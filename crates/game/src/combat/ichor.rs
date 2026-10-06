//! Ichor: what spills from a body when it is hit (black, like everything
//! else). Each hit throws glossy droplets, stretched along their flight so
//! they read as liquid: most spray out of the far side along the shot, some
//! splash back. Where a droplet lands it leaves a splat, an irregular pool
//! with satellite drops; on a wall the splat slowly runs down. The last few
//! hundred splats stay, so a fight leaves its marks.
//!
//! Black on a dark body would not show, so the moment of impact is a white
//! burst with the ichor thrown against it in silhouette: a crown of liquid
//! blades flung out along the shot, which breaks into the droplets.

use std::collections::VecDeque;

use bevy::{
    asset::RenderAssetUsages,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use super::*;

/// Splats kept at most; the oldest go first.
const MAX_SPLATS: usize = 600;
const GRAVITY: f32 = 18.0;
/// Shapes of splat (generated textures).
const SHAPES: usize = 6;

#[derive(Resource)]
pub(super) struct Ichor {
    drop: Handle<Mesh>,
    wet: Handle<StandardMaterial>,
    quad: Handle<Mesh>,
    splats: Vec<Handle<StandardMaterial>>,
    /// The impact: a white star, and a crown of liquid blades along +Y.
    star: Handle<Mesh>,
    white: Handle<StandardMaterial>,
    crowns: Vec<Handle<Mesh>>,
}

/// A burst at the moment of impact, growing fast then shrinking away.
#[derive(Component)]
pub(super) struct Burst {
    age: f32,
    life: f32,
    /// Seconds to reach full size.
    grow: f32,
    size: Vec3,
}

#[derive(Resource, Default)]
pub(super) struct Splats(VecDeque<Entity>);

#[derive(Component)]
pub(super) struct Droplet {
    velocity: Vec3,
    size: f32,
    life: f32,
}

/// A splat on a wall runs down for a while.
#[derive(Component)]
pub(super) struct Run {
    /// Metres per second, slowing.
    speed: f32,
    age: f32,
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    // Glossy black: it catches the sun.
    let wet = materials.add(StandardMaterial {
        base_color: Color::srgb(0.01, 0.01, 0.01),
        perceptual_roughness: 0.12,
        reflectance: 0.7,
        ..default()
    });
    let splats = (0..SHAPES)
        .map(|k| {
            let texture = images.add(splat_image(k as u32));
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.01, 0.01, 0.01),
                base_color_texture: Some(texture),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.15,
                reflectance: 0.6,
                depth_bias: 50.0,
                ..default()
            })
        })
        .collect();
    // A star of short rays, the same from every side.
    let star: Vec<(Vec3, f32, f32)> = (0..10)
        .map(|k| {
            let d = Vec3::new(hash01(k, 0, 2, 0x1c6) - 0.5, hash01(k, 1, 2, 0x1c6) - 0.5, hash01(k, 2, 2, 0x1c6) - 0.5);
            (d.normalize_or(Vec3::Y), 0.5 + 0.5 * hash01(k, 3, 2, 0x1c6), 0.08)
        })
        .collect();
    // Crowns: blades of liquid fanning out in a cone round +Y, ragged.
    let crowns = (0..4)
        .map(|c| {
            let blades: Vec<(Vec3, f32, f32)> = (0..9)
                .map(|k| {
                    let r = |j: i32| hash01(c, k, j, 0x1c7);
                    let a = (k as f32 + r(0) * 0.6) / 9.0 * std::f32::consts::TAU;
                    let open = 0.35 + 0.5 * r(1);
                    (Vec3::new(a.cos() * open, 1.0, a.sin() * open), 0.5 + 0.6 * r(2), 0.035 + 0.03 * r(3))
                })
                .collect();
            meshes.add(shard_mesh(&blades))
        })
        .collect();
    let white = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(120.0, 120.0, 120.0), ..default() });
    commands.insert_resource(Ichor {
        drop: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        wet,
        quad: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(0.5)).mesh()),
        splats,
        star: meshes.add(shard_mesh(&star)),
        white,
        crowns,
    });
    commands.init_resource::<Splats>();
}

/// An irregular pool: a wobbly blob, a few lobes reaching out, and drops
/// scattered round it. White, its shape in the alpha.
fn splat_image(seed: u32) -> Image {
    const N: u32 = 128;
    let r = |k: i32| hash01(seed as i32, k, 3, 0x1c0);
    let lobes: Vec<(f32, f32, f32)> = (0..7).map(|k| (r(k) * std::f32::consts::TAU, 0.08 + 0.12 * r(k + 20), 0.12 + 0.2 * r(k + 40))).collect();
    let drops: Vec<(f32, f32, f32)> = (0..14)
        .map(|k| {
            let a = r(k + 60) * std::f32::consts::TAU;
            let d = 0.3 + 0.17 * r(k + 80);
            (0.5 + a.cos() * d, 0.5 + a.sin() * d, 0.008 + 0.025 * r(k + 100))
        })
        .collect();
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (u, v) = ((x as f32 + 0.5) / N as f32, (y as f32 + 0.5) / N as f32);
            let (dx, dy) = (u - 0.5, v - 0.5);
            let d = (dx * dx + dy * dy).sqrt();
            let a = dy.atan2(dx);
            // The blob's edge: a base radius, wobbling, pushed out by lobes.
            let mut edge = 0.2 + 0.025 * (a * 5.0 + r(1) * 6.0).sin() + 0.015 * (a * 11.0 + r(2) * 6.0).sin();
            for &(la, width, reach) in &lobes {
                let diff = ((a - la + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs();
                edge += reach * (1.0 - diff / width).max(0.0).powi(2);
            }
            let mut alpha = ((edge - d) * N as f32 * 0.6).clamp(0.0, 1.0);
            for &(cx, cy, cr) in &drops {
                let dd = ((u - cx).powi(2) + (v - cy).powi(2)).sqrt();
                alpha = alpha.max(((cr - dd) * N as f32 * 0.8).clamp(0.0, 1.0));
            }
            // Fade out before the quad's edge.
            alpha *= (1.0 - ((d - 0.46) * 25.0).clamp(0.0, 1.0)) * 0.95;
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d { width: N, height: N, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Ichor bursting from a hit at `at` by a shot flying along `dir`;
/// `strength` 1 for one shard, more for a death.
pub(super) fn spray(commands: &mut Commands, ichor: &Ichor, at: Vec3, dir: Vec3, strength: f32, seed: u32) {
    // The impact: a white star, and the crown thrown out against it.
    let r = |j: i32| hash01(seed as i32, j, 9, 0x1c8);
    let size = 0.45 * strength.sqrt();
    commands.spawn((
        Burst { age: 0.0, life: 0.09, grow: 0.015, size: Vec3::splat(size * 1.2) },
        Mesh3d(ichor.star.clone()),
        MeshMaterial3d(ichor.white.clone()),
        Transform::from_translation(at + dir * 0.3).with_scale(Vec3::ZERO),
        bevy::light::NotShadowCaster,
    ));
    let twist = Quat::from_rotation_y(r(0) * std::f32::consts::TAU);
    commands.spawn((
        Burst { age: 0.0, life: 0.22, grow: 0.06, size: Vec3::new(size * 1.6, size * 2.2, size * 1.6) },
        Mesh3d(ichor.crowns[(r(1) * ichor.crowns.len() as f32) as usize % ichor.crowns.len()].clone()),
        MeshMaterial3d(ichor.wet.clone()),
        Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir) * twist).with_scale(Vec3::ZERO),
        bevy::light::NotShadowCaster,
    ));
    let count = (7.0 * strength).round() as i32;
    for k in 0..count {
        let r = |j: i32| hash01(seed as i32, k, j, 0x1c4) - 0.5;
        let back = k % 4 == 3;
        let glob = k % 6 == 0;
        // Out of the far side along the shot, or splashing back; always a
        // little up, and spread.
        let main = if back { -dir * 0.6 } else { dir };
        let spread = Vec3::new(r(0), r(1) + 0.35, r(2)) * if back { 1.4 } else { 0.9 };
        let speed = if glob { 5.0 + 4.0 * (r(3) + 0.5) } else { 8.0 + 12.0 * (r(3) + 0.5) } * strength.sqrt().min(1.6);
        let size = if glob { 0.045 + 0.025 * (r(4) + 0.5) } else { 0.014 + 0.016 * (r(4) + 0.5) };
        commands.spawn((
            Droplet { velocity: (main + spread).normalize_or(Vec3::Y) * speed, size, life: 3.0 },
            Mesh3d(ichor.drop.clone()),
            MeshMaterial3d(ichor.wet.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(size)),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// Bursts grow fast to full size, then shrink away.
pub(super) fn burst(mut commands: Commands, time: Res<Time>, mut bursts: Query<(Entity, &mut Burst, &mut Transform)>) {
    let dt = time.delta_secs();
    for (entity, mut b, mut transform) in &mut bursts {
        b.age += dt;
        if b.age >= b.life {
            commands.entity(entity).despawn();
            continue;
        }
        let k = if b.age < b.grow {
            let t = b.age / b.grow;
            1.0 - (1.0 - t) * (1.0 - t)
        } else {
            1.0 - ((b.age - b.grow) / (b.life - b.grow)).powi(2)
        };
        transform.scale = b.size * k;
    }
}

/// Droplets fly and fall; where one strikes something it leaves a splat.
#[allow(clippy::too_many_arguments)]
pub(super) fn fly(
    mut commands: Commands,
    time: Res<Time>,
    spatial: SpatialQuery,
    ichor: Res<Ichor>,
    mut splats: ResMut<Splats>,
    mut drops: Query<(Entity, &mut Droplet, &mut Transform), Without<Run>>,
    mut runs: Query<(Entity, &mut Run, &mut Transform), Without<Droplet>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut d, mut transform) in &mut drops {
        d.life -= dt;
        d.velocity.y -= GRAVITY * dt;
        let step = d.velocity * dt;
        let from = transform.translation;
        let length = step.length();
        let hit = Dir3::new(step).ok().and_then(|dir| spatial.cast_ray(from, dir, length, true, &default()));
        if let Some(hit) = hit {
            let at = from + step.normalize_or_zero() * hit.distance;
            // (About as many splats as when there were fewer droplets.)
            if hash01(entity.index_u32() as i32, 3, 3, 0x1c9) < 0.57 {
                splat(&mut commands, &ichor, &mut splats, at, hit.normal, d.size, d.velocity, entity.index_u32());
            }
            commands.entity(entity).despawn();
            continue;
        }
        if d.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = from + step;
        // Stretched along its flight: a streak, not a dot.
        let speed = d.velocity.length();
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, d.velocity / speed.max(1e-3));
        transform.scale = Vec3::new(d.size, d.size * (1.0 + speed * 0.35), d.size);
    }
    // Splats on walls run down, slowing.
    for (entity, mut run, mut transform) in &mut runs {
        run.age += dt;
        let v = run.speed * (-run.age * 1.2).exp();
        if v < 0.01 {
            commands.entity(entity).remove::<Run>();
            continue;
        }
        // Local +Z points down the wall: grow that way, keeping the top.
        let down = transform.rotation * Vec3::Z;
        transform.scale.z += v * dt;
        transform.translation += down * v * dt * 0.5;
    }
}

#[allow(clippy::too_many_arguments)]
fn splat(commands: &mut Commands, ichor: &Ichor, splats: &mut Splats, at: Vec3, normal: Vec3, size: f32, velocity: Vec3, seed: u32) {
    let r = |j: i32| hash01(seed as i32, j, 7, 0x1c5);
    // Bigger and longer the faster it struck, along its flight.
    // (Sized as when droplets were smaller: big splats were too much.)
    let width = size / 1.35 * (14.0 + 10.0 * r(0));
    let along = velocity - normal * velocity.dot(normal);
    let wall = normal.y.abs() < 0.6;
    // Local Y is the surface's normal; local Z runs down a wall, or along
    // the flight on the ground.
    let z = if wall {
        (Vec3::NEG_Y - normal * -normal.y).normalize_or(Vec3::Z)
    } else {
        along.normalize_or(normal.any_orthonormal_vector())
    };
    let x = normal.cross(z).normalize_or(Vec3::X);
    let z = x.cross(normal);
    let rotation = Quat::from_mat3(&Mat3::from_cols(x, normal, z));
    let stretch = 1.0 + (along.length() * 0.06).min(1.2);
    let mut e = commands.spawn((
        Mesh3d(ichor.quad.clone()),
        MeshMaterial3d(ichor.splats[(r(1) * SHAPES as f32) as usize % SHAPES].clone()),
        Transform::from_translation(at + normal * 0.02).with_rotation(rotation).with_scale(Vec3::new(width, 1.0, width * stretch)),
        bevy::light::NotShadowCaster,
    ));
    if wall {
        e.insert(Run { speed: 0.15 + 0.35 * r(2), age: 0.0 });
    }
    splats.0.push_back(e.id());
    while splats.0.len() > MAX_SPLATS {
        if let Some(old) = splats.0.pop_front() {
            commands.entity(old).try_despawn();
        }
    }
}
