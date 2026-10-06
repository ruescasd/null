//! The shard shotgun in view: an angular receiver and grip; a barrel whose
//! middle is an open cage of six rails, a bundle of pale shards (the
//! ammunition) lying in it, its front and back shrouded; a crown of prongs at
//! the muzzle. Dark metal, a thin pale stripe along each side.
//!
//! It is a heavy-duty tool, grimy from hard use, never makeshift: an armour
//! plate bolted on its flank, regular bolts, machined ridges on the grip, an
//! armoured hose clamped under the barrel; soot where the shots go out,
//! grease on the moving cage, a faint mottle of use on the metal.
//!
//! Firing is excessive on purpose (a gun must not feel weak): a white star
//! bursting from the muzzle and a ring of shock flung out from it, sparks
//! spraying forward, the spent shard thrown out of the cage's window, the view kicking, shaking and punching out (FOV).
//!
//! It moves: it kicks back and up as it fires, the prongs flare open and snap
//! shut, and the cage turns a sixth of a turn to bring the next shard round,
//! stopping with a jolt, in time with the reload. It lags behind the view as
//! you turn and settles back, bobs with your steps (more running) and dips
//! when you land.

use bevy::{
    asset::RenderAssetUsages,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

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

/// Where the muzzle is in the gun's frame, and from the eye.
const MUZZLE: Vec3 = Vec3::new(0.0, 0.005, -0.33);
pub(super) const MUZZLE_VIEW: Vec3 = Vec3::new(REST.x + MUZZLE.x, REST.y + MUZZLE.y, REST.z + MUZZLE.z);

/// What firing throws.
#[derive(Resource)]
pub(super) struct Fx {
    star: Handle<Mesh>,
    ring: Handle<Mesh>,
    needle: Handle<Mesh>,
    flash: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
    /// Dark sparks (half of them), which read against a pale ground.
    dark: Handle<StandardMaterial>,
    pale: Handle<StandardMaterial>,
}

/// A flash on the gun: grows to `size` in `grow` seconds, then shrinks away
/// by `life`. A ring grows instead, and thins.
#[derive(Component)]
pub(super) struct Flash {
    age: f32,
    life: f32,
    grow: f32,
    size: Vec3,
    ring: bool,
}

/// A particle in the world: sparks, the spent shard.
#[derive(Component)]
pub(super) struct Particle {
    velocity: Vec3,
    spin: Vec3,
    /// Fraction of its speed lost per second, and gravity (m/s²).
    drag: f32,
    gravity: f32,
    age: f32,
    life: f32,
    /// Its size at birth and at death (it grows or shrinks between).
    from: Vec3,
    to: Vec3,
    /// Stretched along its flight (sparks).
    streak: bool,
}

/// Builds the gun, in the eye's frame (-Z forward, +Y up), and returns it.
pub(super) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, pale: Handle<StandardMaterial>) -> Entity {
    // Dark, but matte enough that its facets catch the light (a glossy
    // black reflects the black sky and reads as a hole); stained and
    // scratched.
    let grime = images.add(grime_image());
    let metal = materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.12, 0.12), base_color_texture: Some(grime), perceptual_roughness: 0.62, reflectance: 0.4, ..default() });
    // Cord and rubber: matte, nearly black.
    let wrap = materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.95, reflectance: 0.2, ..default() });
    // Grease: dark and glossy.
    let grease = materials.add(StandardMaterial { base_color: Color::srgb(0.035, 0.035, 0.035), perceptual_roughness: 0.12, reflectance: 0.7, ..default() });
    // Bolt heads: bare, a little brighter than the finish.
    let worn = materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.15, 0.15), perceptual_roughness: 0.75, reflectance: 0.35, ..default() });
    // Soot, where the shots go out: matte, darker than the finish.
    let soot = materials.add(StandardMaterial { base_color: Color::srgb(0.025, 0.025, 0.025), perceptual_roughness: 0.9, reflectance: 0.2, ..default() });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let ring = meshes.add(Torus::new(0.039, 0.051).mesh().major_resolution(6).minor_resolution(4));
    let needle = meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 0.04), (Vec3::NEG_Y, 0.06, 0.04)]));
    let shroud = meshes.add(Cylinder::new(0.056, 1.0).mesh().resolution(6));
    // What firing throws: a star of rays mostly forward, a ring, and the
    // materials (white-hot, bright).
    let star: Vec<(Vec3, f32, f32)> = (0..14)
        .map(|k| {
            let r = |j: i32| hash01(k, j, 4, 0x9f1) - 0.5;
            let a = k as f32 / 14.0 * std::f32::consts::TAU;
            let forward = k % 3 != 0;
            let d = if forward { Vec3::new(a.cos() * 0.35, a.sin() * 0.35, -1.0) } else { Vec3::new(a.cos(), a.sin(), -0.15) };
            (d.normalize(), if forward { 0.7 + 0.5 * (r(0) + 0.5) } else { 0.35 + 0.3 * (r(0) + 0.5) }, 0.06)
        })
        .collect();
    commands.insert_resource(Fx {
        star: meshes.add(shard_mesh(&star)),
        ring: meshes.add(Torus::new(0.85, 1.0).mesh().major_resolution(12).minor_resolution(3)),
        needle: needle.clone(),
        flash: materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(4000.0, 4000.0, 4000.0), ..default() }),
        spark: materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(600.0, 600.0, 600.0), ..default() }),
        dark: materials.add(StandardMaterial { base_color: Color::BLACK, perceptual_roughness: 1.0, reflectance: 0.0, ..default() }),
        pale: pale.clone(),
    });
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
        // An armour plate bolted square on the left flank, bolts at its
        // corners and along its middle.
        let plate = Quat::from_rotation_z(-0.22);
        let plate_at = Vec3::new(-0.05, -0.006, 0.07);
        g.spawn((Mesh3d(cube.clone()), MeshMaterial3d(metal.clone()), Transform::from_translation(plate_at).with_rotation(plate).with_scale(Vec3::new(0.006, 0.048, 0.1)), bevy::light::NotShadowCaster));
        for y in [-0.018, 0.018] {
            for z in [-0.04, 0.0, 0.04] {
                g.spawn((Mesh3d(cube.clone()), MeshMaterial3d(worn.clone()), Transform::from_translation(plate_at + plate * Vec3::new(-0.0045, y, z)).with_rotation(plate).with_scale(Vec3::splat(0.0055)), bevy::light::NotShadowCaster));
            }
        }
        // Bolts along the top of the receiver, evenly spaced.
        for k in 0..5 {
            let z = -0.06 + 0.042 * k as f32;
            for side in [-1.0, 1.0] {
                g.spawn((block(Vec3::splat(0.0055), Vec3::new(side * 0.03, 0.044, z), 0.0), MeshMaterial3d(worn.clone())));
            }
        }
        // Machined ridges on the grip.
        let grip_axis = Quat::from_rotation_x(-0.35) * Vec3::Y;
        let grip_at = Vec3::new(0.0, -0.09, 0.08);
        for k in 0..8 {
            let t = -0.05 + 0.0135 * k as f32;
            g.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(wrap.clone()),
                Transform::from_translation(grip_at + grip_axis * t).with_rotation(Quat::from_rotation_x(-0.35)).with_scale(Vec3::new(0.039, 0.006, 0.055)),
                bevy::light::NotShadowCaster,
            ));
        }
        // An armoured hose run straight under the barrel, in two clamps.
        g.spawn((block(Vec3::new(0.011, 0.011, 0.26), Vec3::new(0.0, -0.058, -0.17), 0.0), MeshMaterial3d(wrap.clone())));
        for z in [-0.1, -0.27] {
            g.spawn((block(Vec3::new(0.02, 0.022, 0.012), Vec3::new(0.0, -0.054, z), 0.0), MeshMaterial3d(metal.clone())));
        }
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
        // The barrel: a cage that turns, the ammunition lying in it; only
        // its middle shows, between two shrouds.
        let shrouded = |length: f32, z: f32| {
            (
                Mesh3d(shroud.clone()),
                Transform::from_xyz(0.0, 0.005, z).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(1.0, length, 1.0)),
                bevy::light::NotShadowCaster,
            )
        };
        g.spawn((shrouded(0.08, -0.12), MeshMaterial3d(metal.clone())));
        // (Sooted at the front, where the shots go out.)
        g.spawn((shrouded(0.09, -0.28), MeshMaterial3d(soot.clone())));
        // (A collar at each shroud's open end.)
        // (Greasy round the cage, where it turns.)
        for z in [-0.162, -0.235] {
            g.spawn((
                Mesh3d(ring.clone()),
                MeshMaterial3d(grease.clone()),
                Transform::from_xyz(0.0, 0.005, z).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::splat(1.18)),
                bevy::light::NotShadowCaster,
            ));
        }
        g.spawn((Cage, Transform::from_xyz(0.0, 0.005, -0.08), Visibility::default())).with_children(|c| {
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                c.spawn((block(Vec3::new(0.008, 0.008, 0.25), Vec3::new(a.cos() * 0.045, a.sin() * 0.045, -0.12), 0.0), MeshMaterial3d(grease.clone())));
                // A shard of ammunition, pointing forward, in the window.
                let b = a + std::f32::consts::PI / 6.0;
                c.spawn((
                    Mesh3d(needle.clone()),
                    MeshMaterial3d(pale.clone()),
                    Transform::from_xyz(b.cos() * 0.02, b.sin() * 0.02, -0.1)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::NEG_Z))
                        .with_scale(Vec3::new(0.18, 0.08, 0.18)),
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
                MeshMaterial3d(soot.clone()),
                Transform::from_xyz(a.cos() * 0.045, 0.005 + a.sin() * 0.045, MUZZLE.z + 0.01).with_scale(Vec3::new(0.25, 0.09, 0.25)),
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

/// Everything a shot throws, from the muzzle at `at` (in the world), along
/// `forward`. `gun` is the gun in view (the flash rides on it).
#[allow(clippy::too_many_arguments)]
pub(super) fn fire(commands: &mut Commands, fx: &Fx, gun: Entity, at: Vec3, forward: Vec3, right: Vec3, up: Vec3, shot: u32) {
    let r = |k: i32, j: i32| hash01(shot as i32, k, j, 0x9f2) - 0.5;
    // On the gun: the star, turned a different way each shot, and a ring of
    // shock flung out.
    commands.entity(gun).with_children(|g| {
        g.spawn((
            Flash { age: 0.0, life: 0.075, grow: 0.012, size: Vec3::new(0.32, 0.32, 0.45), ring: false },
            Mesh3d(fx.star.clone()),
            MeshMaterial3d(fx.flash.clone()),
            Transform::from_translation(MUZZLE).with_rotation(Quat::from_rotation_z(r(0, 0) * 6.0)).with_scale(Vec3::ZERO),
            bevy::light::NotShadowCaster,
        ));
        g.spawn((
            // (Half the effect it had: smaller, thinner, briefer.)
            Flash { age: 0.0, life: 0.11, grow: 0.11, size: Vec3::new(0.22, 0.12, 0.22), ring: true },
            Mesh3d(fx.ring.clone()),
            MeshMaterial3d(fx.flash.clone()),
            Transform::from_translation(MUZZLE + Vec3::NEG_Z * 0.04).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::ZERO),
            bevy::light::NotShadowCaster,
        ));
    });
    // Sparks spraying forward in a tight cone, half pale and half dark,
    // each leaving a fading trail of ghosts.
    for k in 0..26 {
        let dir = (forward + right * r(k, 1) * 0.35 + up * r(k, 2) * 0.35).normalize();
        commands.spawn((
            Particle { velocity: dir * (22.0 + 30.0 * (r(k, 3) + 0.5)), spin: Vec3::ZERO, drag: 4.0, gravity: 6.0, age: 0.0, life: 0.12 + 0.2 * (r(k, 4) + 0.5), from: Vec3::new(0.25, 0.5, 0.25), to: Vec3::new(0.05, 0.1, 0.05), streak: true },
            Mesh3d(fx.needle.clone()),
            MeshMaterial3d(if k % 2 == 0 { fx.spark.clone() } else { fx.dark.clone() }),
            Transform::from_translation(at).with_scale(Vec3::ZERO),
            bevy::light::NotShadowCaster,
        ));
    }
    // The spent shard, flung out of the cage's window to the right,
    // tumbling.
    commands.spawn((
        Particle {
            velocity: right * (3.0 + r(0, 14)) + up * (2.5 + r(0, 15)) - forward * 0.5,
            spin: Vec3::new(14.0 + r(0, 16) * 6.0, 4.0, 9.0),
            drag: 0.2,
            gravity: 14.0,
            age: 0.0,
            life: 1.4,
            from: Vec3::new(0.3, 0.14, 0.3),
            to: Vec3::new(0.3, 0.14, 0.3),
            streak: false,
        },
        Mesh3d(fx.needle.clone()),
        MeshMaterial3d(fx.pale.clone()),
        Transform::from_translation(at - forward * 0.2 + right * 0.03),
        bevy::light::NotShadowCaster,
    ));
}

/// Flashes grow and shrink away; particles fly.
pub(super) fn effects(mut commands: Commands, time: Res<Time>, mut flashes: Query<(Entity, &mut Flash, &mut Transform), Without<Particle>>, mut particles: Query<(Entity, &mut Particle, &mut Transform, &Mesh3d, &MeshMaterial3d<StandardMaterial>), Without<Flash>>) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut f, mut t) in &mut flashes {
        f.age += dt;
        if f.age >= f.life {
            commands.entity(e).despawn();
            continue;
        }
        t.scale = if f.ring {
            // A ring flung out: growing, thinning.
            let k = f.age / f.life;
            Vec3::new(f.size.x * (0.2 + 0.8 * k.sqrt()), f.size.y * (1.0 - k) * 0.6, f.size.z * (0.2 + 0.8 * k.sqrt()))
        } else if f.age < f.grow {
            f.size * (f.age / f.grow)
        } else {
            f.size * (1.0 - ((f.age - f.grow) / (f.life - f.grow)).powi(2))
        };
    }
    for (e, mut p, mut t, mesh, material) in &mut particles {
        p.age += dt;
        if p.age >= p.life {
            commands.entity(e).despawn();
            continue;
        }
        let drag = (-p.drag * dt).exp();
        p.velocity *= drag;
        p.velocity.y -= p.gravity * dt;
        t.translation += p.velocity * dt;
        let k = p.age / p.life;
        let size = p.from.lerp(p.to, k);
        if p.streak {
            let speed = p.velocity.length();
            t.rotation = Quat::from_rotation_arc(Vec3::Y, p.velocity / speed.max(1e-3));
            t.scale = Vec3::new(size.x, size.y * (1.0 + speed * 0.09), size.z);
            // A ghost left where it is, fading fast: a trail.
            commands.spawn((
                Particle { velocity: Vec3::ZERO, spin: Vec3::ZERO, drag: 0.0, gravity: 0.0, age: 0.0, life: 0.06, from: t.scale * 0.8, to: t.scale * 0.2, streak: false },
                mesh.clone(),
                material.clone(),
                *t,
                bevy::light::NotShadowCaster,
            ));
        } else {
            let spin = p.spin * dt;
            t.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
            t.scale = size * (1.0 - k.powi(4));
        }
    }
}

/// The metal's mottle of use: faint, uneven stains (multiplied into the dark
/// finish).
fn grime_image() -> Image {
    const N: u32 = 96;
    let noise = |x: f32, y: f32, cells: f32, salt: i32| {
        let (fx, fy) = (x * cells, y * cells);
        let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
        let (tx, ty) = (fx - ix as f32, fy - iy as f32);
        let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let h = |a: i32, b: i32| hash01(a.rem_euclid(cells as i32), b.rem_euclid(cells as i32), salt, 0x9f6);
        let top = h(ix, iy) + (h(ix + 1, iy) - h(ix, iy)) * sx;
        let bottom = h(ix, iy + 1) + (h(ix + 1, iy + 1) - h(ix, iy + 1)) * sx;
        top + (bottom - top) * sy
    };
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (u, v) = (x as f32 / N as f32, y as f32 / N as f32);
            let stain = 0.55 * noise(u, v, 4.0, 1) + 0.3 * noise(u, v, 9.0, 2) + 0.15 * noise(u, v, 23.0, 3);
            let value = 0.82 + 0.14 * stain;
            let byte = (value.clamp(0.0, 1.0) * 255.0) as u8;
            data.extend_from_slice(&[byte, byte, byte, 255]);
        }
    }
    Image::new(Extent3d { width: N, height: N, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)
}
