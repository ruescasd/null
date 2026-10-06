//! The shard shotgun in view: an angular receiver and grip; an open cage of
//! six rails and three rings for a barrel, a bundle of pale shards (the
//! ammunition) lying in it; a crown of prongs at the muzzle. Dark metal, a
//! thin pale stripe along each side.
//!
//! It moves: it kicks back and up as it fires, the prongs flare open and snap
//! shut, and the cage turns a sixth of a turn to bring the next shard round,
//! stopping with a jolt, in time with the reload. It lags behind the view as
//! you turn and settles back, bobs with your steps (more running) and dips
//! when you land.

use super::*;

/// Where the gun sits in view, from the eye.
const REST: Vec3 = Vec3::new(0.2, -0.17, -0.42);

#[derive(Component)]
pub(super) struct Viewmodel;

/// The barrel's cage (it turns about the barrel's axis).
#[derive(Component)]
pub(super) struct Cage;

/// A prong of the muzzle's crown, at this angle round the axis.
#[derive(Component)]
pub(super) struct Prong(f32);

/// Builds the gun, in the eye's frame (-Z forward, +Y up), and returns it.
pub(super) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, pale: Handle<StandardMaterial>) -> Entity {
    // Dark, but matte enough that its facets catch the light (a glossy
    // black reflects the black sky and reads as a hole).
    let metal = materials.add(StandardMaterial { base_color: Color::srgb(0.07, 0.07, 0.07), perceptual_roughness: 0.5, reflectance: 0.45, ..default() });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let ring = meshes.add(Torus::new(0.039, 0.051).mesh().major_resolution(6).minor_resolution(4));
    let needle = meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 0.04), (Vec3::NEG_Y, 0.06, 0.04)]));
    let block = |size: Vec3, at: Vec3, tilt: f32| (Mesh3d(cube.clone()), Transform::from_translation(at).with_rotation(Quat::from_rotation_x(tilt)).with_scale(size), bevy::light::NotShadowCaster);
    let rolled = |size: Vec3, at: Vec3, roll: f32| (Mesh3d(cube.clone()), Transform::from_translation(at).with_rotation(Quat::from_rotation_z(roll)).with_scale(size), bevy::light::NotShadowCaster);
    let gun = commands.spawn((Viewmodel, Transform::from_translation(REST), Visibility::default())).id();
    commands.entity(gun).with_children(|g| {
        // The receiver, a bevelled top, a stub of stock behind, the grip
        // angled down and back, a trigger guard.
        g.spawn((block(Vec3::new(0.075, 0.085, 0.21), Vec3::new(0.0, 0.0, 0.02), 0.0), MeshMaterial3d(metal.clone())));
        g.spawn((block(Vec3::new(0.05, 0.03, 0.17), Vec3::new(0.0, 0.05, 0.04), 0.05), MeshMaterial3d(metal.clone())));
        g.spawn((block(Vec3::new(0.06, 0.07, 0.09), Vec3::new(0.0, -0.012, 0.16), -0.15), MeshMaterial3d(metal.clone())));
        g.spawn((block(Vec3::new(0.035, 0.13, 0.05), Vec3::new(0.0, -0.09, 0.08), -0.35), MeshMaterial3d(metal.clone())));
        g.spawn((block(Vec3::new(0.012, 0.012, 0.07), Vec3::new(0.0, -0.06, 0.0), 0.0), MeshMaterial3d(metal.clone())));
        // Plates angled on its flanks, and a fin along the top, so it is
        // faceted rather than a block.
        for side in [-1.0, 1.0] {
            g.spawn((rolled(Vec3::new(0.012, 0.06, 0.15), Vec3::new(side * 0.042, -0.01, 0.03), side * 0.22), MeshMaterial3d(metal.clone())));
        }
        g.spawn((block(Vec3::new(0.012, 0.02, 0.13), Vec3::new(0.0, 0.07, 0.05), 0.12), MeshMaterial3d(metal.clone())));
        // A thin pale stripe along each side, on the plates.
        for side in [-1.0, 1.0] {
            g.spawn((rolled(Vec3::new(0.003, 0.007, 0.15), Vec3::new(side * 0.0505, 0.0, 0.03), side * 0.22), MeshMaterial3d(pale.clone())));
        }
        // A faint light of its own, reaching no further than the gun, so its
        // facets read whichever way the sun is.
        g.spawn((
            PointLight { intensity: 25000.0, range: 0.9, radius: 0.0, shadow_maps_enabled: false, ..default() },
            Transform::from_xyz(-0.12, 0.16, 0.18),
        ));
        // The barrel: a cage that turns, the ammunition lying in it.
        g.spawn((Cage, Transform::from_xyz(0.0, 0.005, -0.08), Visibility::default())).with_children(|c| {
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                c.spawn((block(Vec3::new(0.008, 0.008, 0.39), Vec3::new(a.cos() * 0.045, a.sin() * 0.045, -0.19), 0.0), MeshMaterial3d(metal.clone())));
                // A shard of ammunition, pointing forward.
                let b = a + std::f32::consts::PI / 6.0;
                c.spawn((
                    Mesh3d(needle.clone()),
                    MeshMaterial3d(pale.clone()),
                    Transform::from_xyz(b.cos() * 0.02, b.sin() * 0.02, -0.04)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::NEG_Z))
                        .with_scale(Vec3::new(0.18, 0.3, 0.18)),
                    bevy::light::NotShadowCaster,
                ));
            }
            for z in [-0.01, -0.2, -0.385] {
                c.spawn((
                    Mesh3d(ring.clone()),
                    MeshMaterial3d(metal.clone()),
                    Transform::from_xyz(0.0, 0.0, z).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    bevy::light::NotShadowCaster,
                ));
            }
        });
        // The muzzle's crown.
        for k in 0..6 {
            let a = (k as f32 + 0.5) / 6.0 * std::f32::consts::TAU;
            g.spawn((
                Prong(a),
                Mesh3d(needle.clone()),
                MeshMaterial3d(metal.clone()),
                Transform::from_xyz(a.cos() * 0.04, 0.005 + a.sin() * 0.04, -0.46).with_scale(Vec3::new(0.25, 0.09, 0.25)),
                bevy::light::NotShadowCaster,
            ));
        }
    });
    gun
}

/// Sway, bob, recoil, the prongs' flare and the cage's turn.
#[allow(clippy::type_complexity)]
pub(super) fn animate(
    time: Res<Time>,
    mut gun: ResMut<Gun>,
    camera: Single<(&FlyCam, &Player)>,
    mut model: Single<&mut Transform, (With<Viewmodel>, Without<Cage>, Without<Prong>)>,
    mut cage: Single<&mut Transform, (With<Cage>, Without<Viewmodel>, Without<Prong>)>,
    mut prongs: Query<(&Prong, &mut Transform), (Without<Viewmodel>, Without<Cage>)>,
    // Sway (and its velocity), the last view angles, the step phase, the
    // landing dip, and whether it was on the ground.
    mut state: Local<(Vec2, Vec2, Option<Vec2>, f32, f32, bool)>,
) {
    let dt = time.delta_secs().max(1e-4);
    let (fly, player) = *camera;
    let (sway, sway_v, last, phase, dip, was_grounded) = &mut *state;

    // Sway: it lags behind the view as you turn, on a spring.
    let view = Vec2::new(fly.yaw, fly.pitch);
    let turn = last.map_or(Vec2::ZERO, |l| (view - l) / dt);
    *last = Some(view);
    let want = Vec2::new(turn.x * 0.012, -turn.y * 0.01).clamp(Vec2::splat(-0.035), Vec2::splat(0.035));
    *sway_v += ((want - *sway) * 120.0 - *sway_v * 16.0) * dt;
    *sway += *sway_v * dt;

    // Bob with the steps when on the ground; a dip on landing.
    let speed = Vec2::new(player.velocity.x, player.velocity.z).length();
    let step = (speed / 10.0).min(1.3) * player.grounded as i32 as f32;
    *phase += speed.max(0.0) * dt * 0.75;
    let bob = Vec2::new((*phase).sin() * 0.008, -((*phase).sin()).abs() * 0.009) * step;
    if player.grounded && !*was_grounded {
        *dip = 0.03;
    }
    *was_grounded = player.grounded;
    *dip *= (-dt * 9.0).exp();

    // Recoil.
    gun.recoil *= (-dt * 10.0).exp();
    let r = gun.recoil;
    model.translation = REST + Vec3::new(sway.x + bob.x, -sway.y + bob.y - *dip - r * 0.3, r);
    model.rotation = Quat::from_rotation_x(r * 2.0 + *dip * 2.0) * Quat::from_rotation_y(sway.x * 2.0) * Quat::from_rotation_z(-sway.x * 3.0);

    // Since the last shot, 0..1 of the reload.
    let since = (RELOAD - gun.cooldown).max(0.0);
    let progress = (since / RELOAD).clamp(0.0, 1.0);
    // The prongs flare open as it fires and snap shut.
    let flare = (-since * 14.0).exp() * (gun.shots > 0) as i32 as f32;
    for (prong, mut t) in &mut prongs {
        let out = 0.22 + 0.9 * flare;
        let dir = Vec3::new(prong.0.cos() * out, prong.0.sin() * out, -1.0).normalize();
        t.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
    }
    // The cage turns a sixth to the next shard, quickly, through the middle
    // of the reload, with a little overshoot as it stops.
    let s = ((progress - 0.25) / 0.35).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - s).powi(3) + 0.08 * (s * std::f32::consts::PI).sin() * (1.0 - s);
    let turns = gun.shots.saturating_sub(1) as f32 + if gun.shots > 0 { ease } else { 0.0 };
    cage.rotation = Quat::from_rotation_z(turns * std::f32::consts::TAU / 6.0);
}
