//! Groves (`--opt grove`): the buds grow, asleep on the ground, rooted to it
//! by cables sprawling out from under them, shut and dim whoever looks. Noise
//! wakes them: a shot within 35 m, or running (not walking, Shift) within
//! 12 m. A bud waking is thrown open, tears free of its roots (they fall
//! slack and wither) and takes to the air; and it wakes those near it, a
//! moment later, so a grove can go up all at once.

use bevy::{asset::RenderAssetUsages, audio::Volume, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology, prelude::*};
use worldgen::noise::hash01;

use super::{
    Assets3, Gun, Mode, Swarmer,
    forms::{Asleep, Gazed},
    hunter::cables::{rope, tubes},
};
use crate::{
    Args,
    player::{Player, WALK_SPEED},
    terrain::WorldGen,
};

/// How far a shot is heard, and running; running is anything faster than a
/// walk.
const SHOT_HEARD: f32 = 35.0;
const RUN_HEARD: f32 = 12.0;
/// How far a waking bud wakes others, and how soon.
const SPREAD: f32 = 7.0;
const SPREAD_DELAY: (f32, f32) = (0.25, 0.8);
/// Roots on each, how far they reach, and how long torn roots last.
const ROOTS: usize = 7;
const REACH: (f32, f32) = (1.2, 3.2);
const WITHER: f32 = 5.0;

/// A bud about to wake (seconds left).
#[derive(Component)]
pub(super) struct Waking(f32);

/// A bud's roots: each from under it to a point on the ground, its length
/// and points; torn free, they hang from the ground end and wither.
#[derive(Component)]
pub(super) struct Roots {
    bud: Entity,
    strands: Vec<(Vec3, f32, Vec<Vec3>, Vec<Vec3>)>,
    /// Seconds since it was torn free.
    torn: Option<f32>,
    mesh: Handle<Mesh>,
}

/// Where the roots leave the bud, below it.
fn base(bud: &Transform) -> Vec3 {
    bud.translation - Vec3::Y * 0.3
}

#[allow(clippy::too_many_arguments)]
pub(super) fn grove(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    world: Res<WorldGen>,
    gun: Res<Gun>,
    assets: Res<Assets3>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material: Local<Option<Handle<StandardMaterial>>>,
    mut shots: Local<u32>,
    player: Single<(&Transform, &Player), Without<Swarmer>>,
    planted: Query<(Entity, &Transform), Added<Asleep>>,
    mut buds: Query<(Entity, &Transform, &mut Swarmer, &mut Gazed, Option<&mut Waking>, Has<Asleep>)>,
    mut roots: Query<(Entity, &mut Roots)>,
) {
    if !args.opt("grove") {
        return;
    }
    let dt = time.delta_secs().clamp(0.001, 1.0 / 30.0);
    let material = material
        .get_or_insert_with(|| materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.4, reflectance: 0.5, ..default() }))
        .clone();
    let draw = |strands: &[(Vec3, f32, Vec<Vec3>, Vec<Vec3>)], mesh: &mut Mesh| {
        tubes(mesh, strands.iter().enumerate().map(|(k, (_, _, points, _))| (&points[..], (0.05, 0.012), if k % 5 == 2 { 0.45 } else { 0.035 })));
    };

    // New buds put down roots.
    for (bud, transform) in &planted {
        let seed = bud.index_u32() as i32;
        let from = base(transform);
        let mut strands = Vec::new();
        for k in 0..ROOTS {
            let r = |j: i32| hash01(seed, k as i32, j, 0x6f1);
            let a = (k as f32 + r(0) * 0.7) / ROOTS as f32 * std::f32::consts::TAU;
            let reach = REACH.0 + (REACH.1 - REACH.0) * r(1);
            let (x, z) = (from.x + a.cos() * reach, from.z + a.sin() * reach);
            let ground = Vec3::new(x, world.ground_height(x, z), z);
            let length = from.distance(ground) * 1.15;
            let (mut points, mut previous) = (Vec::new(), Vec::new());
            rope(&mut points, &mut previous, from, Some(ground), length, &world, dt);
            strands.push((ground, length, points, previous));
        }
        // (Built whole from the start: the renderer does not take an empty
        // mesh growing.)
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        draw(&strands, &mut mesh);
        let mesh = meshes.add(mesh);
        commands.spawn((
            Roots { bud, strands, torn: None, mesh: mesh.clone() },
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            Visibility::default(),
            NoFrustumCulling,
        ));
    }

    // Noise: a shot, or running.
    let (ptransform, p) = *player;
    let ear = ptransform.translation;
    let shot = gun.shots != *shots;
    *shots = gun.shots;
    let running = Vec2::new(p.velocity.x, p.velocity.z).length() > WALK_SPEED + 1.0;
    let heard = |at: Vec3| (shot && at.distance(ear) < SHOT_HEARD) || (running && at.distance(ear) < RUN_HEARD);

    // Asleep ones that hear it, or whose time has come, wake; and set those
    // near them waking.
    let mut woken: Vec<Vec3> = Vec::new();
    for (bud, transform, mut s, mut gazed, waking, asleep) in &mut buds {
        if !asleep {
            continue;
        }
        let due = match waking {
            Some(mut w) => {
                w.0 -= dt;
                w.0 <= 0.0
            }
            None => false,
        };
        if !(due || heard(transform.translation)) {
            continue;
        }
        commands.entity(bud).remove::<(Asleep, Waking)>();
        s.mode = Mode::Free;
        s.velocity = Vec3::Y * 4.0;
        gazed.startle();
        commands.spawn((
            AudioPlayer::new(assets.assemble.clone()),
            PlaybackSettings::DESPAWN.with_spatial(true).with_volume(Volume::Linear(0.8)).with_speed(1.6),
            Transform::from_translation(transform.translation),
        ));
        woken.push(transform.translation);
    }
    for (bud, transform, _, _, waking, asleep) in &buds {
        if asleep && waking.is_none() && woken.iter().any(|w| w.distance(transform.translation) < SPREAD) {
            let r = hash01(bud.index_u32() as i32, 1, 0, 0x6f2);
            commands.entity(bud).insert(Waking(SPREAD_DELAY.0 + (SPREAD_DELAY.1 - SPREAD_DELAY.0) * r));
        }
    }

    // Roots: held between bud and ground while it sleeps; torn free, they
    // hang from the ground end, fall slack and wither.
    for (entity, mut root) in &mut roots {
        let bud = buds.get(root.bud).ok();
        let attached = bud.is_some_and(|b| b.5);
        if !attached && root.torn.is_none() {
            root.torn = Some(0.0);
            // (Now held at the ground end: points from there.)
            for (_, _, points, previous) in &mut root.strands {
                points.reverse();
                previous.reverse();
            }
        }
        if let Some(t) = root.torn.as_mut() {
            *t += dt;
            if *t > WITHER {
                commands.entity(entity).despawn();
                continue;
            }
        }
        let from = bud.map(|b| base(b.1));
        let torn = root.torn.is_some();
        let Roots { strands, mesh, .. } = &mut *root;
        for (ground, length, points, previous) in strands.iter_mut() {
            if torn {
                rope(points, previous, *ground, None, *length, &world, dt);
            } else if let Some(from) = from {
                rope(points, previous, from, Some(*ground), *length, &world, dt);
            }
        }
        if let Some(mut m) = meshes.get_mut(&*mesh) {
            draw(strands, &mut m);
        }
    }
}
