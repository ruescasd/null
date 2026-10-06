//! The shard shotgun in view, a double-barrelled pump-action: a broad slab of
//! a receiver with a rail on top, a bolted plate and an ejection port (a pale shard
//! showing in it) on the side you see; two barrels side by side over a
//! magazine tube, a window in the magazine with shards lined up in it; a ribbed pump round the
//! magazine; a pistol grip; a stock running back out of view, so the gun
//! reaches your shoulder. A crown of prongs at each muzzle; the barrels fire
//! in turn. Dark metal, a pale
//! stripe along the lower edge.
//!
//! It is a heavy-duty tool, grimy from hard use, never makeshift: chamfered
//! edges, regular bolts, machined ridges; soot where the shots go out, grease
//! on the pump and the magazine, a faint mottle of use on the metal.
//!
//! Firing is excessive on purpose (a gun must not feel weak): a white star
//! bursting from the muzzle and a ring of shock flung out from it, sparks
//! spraying forward, the spent shard thrown out of the port, the view kicking,
//! shaking and punching out (FOV).
//!
//! It moves: it kicks back and up as it fires, the prongs flare open and snap
//! shut, and the pump racks back and forward in time with the reload. It lags
//! behind the view as you turn and settles back, bobs with your steps (more
//! running) and dips when you land.

use bevy::{
    asset::RenderAssetUsages,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use super::*;

/// Where the gun sits in view, from the eye.
const REST: Vec3 = Vec3::new(0.21, -0.19, -0.4);

#[derive(Component)]
pub(super) struct Viewmodel;

/// The pump (it slides back and forward along the magazine).
#[derive(Component)]
pub(super) struct Pump;

/// How far the pump racks back, metres.
const STROKE: f32 = 0.05;

/// A prong of the muzzle's crown, at this angle round the axis.
#[derive(Component)]
pub(super) struct Prong(f32);

/// Where the muzzle is in the gun's frame, and from the eye.
const MUZZLE: Vec3 = Vec3::new(0.0, 0.016, -0.39);
/// How far each barrel is to the side of the middle.
pub(super) const BARREL_X: f32 = 0.022;
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
    /// A ring's growth: its radius goes as (age / life) to this power.
    power: f32,
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
    let needle = meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 0.04), (Vec3::NEG_Y, 0.06, 0.04)]));
    // A hexagonal prism (unit sizes, along +Y): the tubes.
    let hex = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(6));
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
        // A tube along -Z: hexagonal, `radius`, from z0 back to z1, at height y.
        let tube = |radius: f32, x: f32, y: f32, z0: f32, z1: f32| {
            (
                Mesh3d(hex.clone()),
                Transform::from_xyz(x, y, (z0 + z1) * 0.5).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::new(radius, (z1 - z0).abs(), radius)),
                bevy::light::NotShadowCaster,
            )
        };
        // The receiver: a broad slab, its long edges chamfered, a rail on
        // top.
        g.spawn((block(Vec3::new(0.07, 0.072, 0.15), Vec3::ZERO, 0.0), MeshMaterial3d(metal.clone())));
        for (x, y) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            g.spawn((rolled(Vec3::new(0.012, 0.012, 0.15), Vec3::new(x * 0.033, y * 0.034, 0.0), std::f32::consts::FRAC_PI_4), MeshMaterial3d(metal.clone())));
        }
        g.spawn((block(Vec3::new(0.016, 0.008, 0.15), Vec3::new(0.0, 0.042, 0.0), 0.0), MeshMaterial3d(metal.clone())));
        // On the side you see: a plate with machined grooves along it, and
        // in front of it the ejection port, a pale shard showing in it.
        g.spawn((block(Vec3::new(0.004, 0.044, 0.09), Vec3::new(-0.0365, -0.002, 0.018), 0.0), MeshMaterial3d(metal.clone())));
        for y in [-0.014, -0.002, 0.01] {
            g.spawn((block(Vec3::new(0.0015, 0.0028, 0.078), Vec3::new(-0.0386, y, 0.018), 0.0), MeshMaterial3d(wrap.clone())));
        }
        g.spawn((block(Vec3::new(0.002, 0.02, 0.042), Vec3::new(-0.035, 0.01, -0.052), 0.0), MeshMaterial3d(wrap.clone())));
        g.spawn((
            Mesh3d(needle.clone()),
            MeshMaterial3d(pale.clone()),
            Transform::from_xyz(-0.0355, 0.01, -0.035).with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::NEG_Z)).with_scale(Vec3::new(0.12, 0.034, 0.12)),
            bevy::light::NotShadowCaster,
        ));
        // A pale stripe along the lower edge.
        g.spawn((block(Vec3::new(0.003, 0.006, 0.13), Vec3::new(-0.0355, -0.025, 0.0), 0.0), MeshMaterial3d(pale.clone())));

        // The barrel over the magazine, sooted at the muzzle; a band holding
        // them together at the front.
        for side in [-1.0, 1.0] {
            g.spawn((tube(0.019, side * BARREL_X, MUZZLE.y, -0.075, -0.33), MeshMaterial3d(metal.clone())));
            g.spawn((tube(0.02, side * BARREL_X, MUZZLE.y, -0.33, -0.385), MeshMaterial3d(soot.clone())));
        }
        g.spawn((tube(0.016, 0.0, -0.026, -0.075, -0.235), MeshMaterial3d(grease.clone())));
        g.spawn((tube(0.016, 0.0, -0.026, -0.295, -0.33), MeshMaterial3d(metal.clone())));
        g.spawn((block(Vec3::new(0.078, 0.072, 0.014), Vec3::new(0.0, -0.005, -0.315), 0.0), MeshMaterial3d(metal.clone())));
        // The magazine's window: four rails, three pale shards lined up in it.
        for k in 0..4 {
            let a = (k as f32 + 0.5) / 4.0 * std::f32::consts::TAU;
            g.spawn((block(Vec3::new(0.005, 0.005, 0.062), Vec3::new(a.cos() * 0.015, -0.026 + a.sin() * 0.015, -0.265), 0.0), MeshMaterial3d(grease.clone())));
        }
        for k in 0..3 {
            g.spawn((
                Mesh3d(needle.clone()),
                MeshMaterial3d(pale.clone()),
                Transform::from_xyz(0.0, -0.026, -0.246 - 0.018 * k as f32).with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::NEG_Z)).with_scale(Vec3::new(0.16, 0.017, 0.16)),
                bevy::light::NotShadowCaster,
            ));
        }
        // The pump: ribbed, round the magazine.
        g.spawn((Pump, Transform::default(), Visibility::default())).with_children(|p| {
            p.spawn((block(Vec3::new(0.056, 0.04, 0.11), Vec3::new(0.0, -0.03, -0.16), 0.0), MeshMaterial3d(metal.clone())));
            for k in 0..6 {
                p.spawn((block(Vec3::new(0.06, 0.044, 0.007), Vec3::new(0.0, -0.03, -0.12 - 0.016 * k as f32), 0.0), MeshMaterial3d(wrap.clone())));
            }
        });

        // The pistol grip, machined ridges on it; the trigger and its guard.
        g.spawn((block(Vec3::new(0.032, 0.11, 0.045), Vec3::new(0.0, -0.08, 0.055), -0.35), MeshMaterial3d(metal.clone())));
        let grip_axis = Quat::from_rotation_x(-0.35) * Vec3::Y;
        for k in 0..7 {
            g.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(wrap.clone()),
                Transform::from_translation(Vec3::new(0.0, -0.08, 0.055) + grip_axis * (-0.045 + 0.0135 * k as f32)).with_rotation(Quat::from_rotation_x(-0.35)).with_scale(Vec3::new(0.036, 0.006, 0.05)),
                bevy::light::NotShadowCaster,
            ));
        }
        g.spawn((block(Vec3::new(0.008, 0.008, 0.055), Vec3::new(0.0, -0.058, 0.005), 0.0), MeshMaterial3d(metal.clone())));
        g.spawn((block(Vec3::new(0.005, 0.022, 0.006), Vec3::new(0.0, -0.046, 0.012), 0.2), MeshMaterial3d(worn.clone())));
        // The stock: back from the receiver and down, out of view.
        g.spawn((block(Vec3::new(0.046, 0.058, 0.42), Vec3::new(0.0, -0.045, 0.28), 0.16), MeshMaterial3d(metal.clone())));
        g.spawn((block(Vec3::new(0.05, 0.062, 0.03), Vec3::new(0.0, -0.016, 0.09), 0.16), MeshMaterial3d(wrap.clone())));

        // A faint light of its own, reaching no further than the gun, so its
        // facets read whichever way the sun is.
        g.spawn((
            PointLight { intensity: 25000.0, range: 0.9, radius: 0.0, shadow_maps_enabled: false, ..default() },
            Transform::from_xyz(-0.12, 0.16, 0.18),
        ));
        // A crown at each muzzle.
        for side in [-1.0, 1.0] {
            for k in 0..5 {
                let a = (k as f32 + 0.5) / 5.0 * std::f32::consts::TAU;
                g.spawn((
                    Prong(a),
                    Mesh3d(needle.clone()),
                    MeshMaterial3d(soot.clone()),
                    Transform::from_xyz(side * BARREL_X + a.cos() * 0.021, MUZZLE.y + a.sin() * 0.021, MUZZLE.z + 0.012).with_scale(Vec3::new(0.18, 0.065, 0.18)),
                    bevy::light::NotShadowCaster,
                ));
            }
        }
    });
    gun
}

/// Sway, bob, recoil, the prongs' flare and the pump's stroke.
#[allow(clippy::type_complexity)]
pub(super) fn animate(
    time: Res<Time>,
    mut gun: ResMut<Gun>,
    camera: Single<(&FlyCam, &Player)>,
    mut model: Single<&mut Transform, (With<Viewmodel>, Without<Pump>, Without<Prong>)>,
    mut pump: Single<&mut Transform, (With<Pump>, Without<Viewmodel>, Without<Prong>)>,
    mut prongs: Query<(&Prong, &mut Transform), (Without<Viewmodel>, Without<Pump>)>,
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
    // The pump racks back, then forward, as the reload is heard; quick out,
    // a hard stop at each end.
    let ease = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        1.0 - (1.0 - x).powi(3)
    };
    let back = if gun.shots > 0 { ease((progress - 0.12) / 0.14) - ease((progress - 0.34) / 0.14) } else { 0.0 };
    pump.translation = Vec3::Z * STROKE * back;
}

/// Everything a shot throws, from the muzzle at `at` (in the world), along
/// `forward`. `gun` is the gun in view (the flash rides on it).
#[allow(clippy::too_many_arguments)]
pub(super) fn fire(commands: &mut Commands, fx: &Fx, gun: Entity, at: Vec3, forward: Vec3, right: Vec3, up: Vec3, shot: u32) {
    let r = |k: i32, j: i32| hash01(shot as i32, k, j, 0x9f2) - 0.5;
    // The barrels fire in turn.
    let side = if shot % 2 == 0 { 1.0 } else { -1.0 };
    let muzzle = MUZZLE + Vec3::X * side * BARREL_X;
    let at = at + right * side * BARREL_X;
    // On the gun: the star, turned a different way each shot, and a ring of
    // shock flung out.
    commands.entity(gun).with_children(|g| {
        g.spawn((
            Flash { age: 0.0, life: 0.075, grow: 0.012, size: Vec3::new(0.32, 0.32, 0.45), ring: false, power: 1.0 },
            Mesh3d(fx.star.clone()),
            MeshMaterial3d(fx.flash.clone()),
            Transform::from_translation(muzzle).with_rotation(Quat::from_rotation_z(r(0, 0) * 6.0)).with_scale(Vec3::ZERO),
            bevy::light::NotShadowCaster,
        ));
        // A ring of shock, dark, flung out; inside it a thin bright ring
        // born small and racing outwards, overtaking it as it fades.
        for (material, size, power) in [(&fx.dark, Vec3::new(0.22, 0.12, 0.22), 0.5), (&fx.flash, Vec3::new(0.24, 0.05, 0.24), 1.6)] {
            g.spawn((
                Flash { age: 0.0, life: 0.11, grow: 0.11, size, ring: true, power },
                Mesh3d(fx.ring.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(muzzle + Vec3::NEG_Z * 0.04).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(Vec3::ZERO),
                bevy::light::NotShadowCaster,
            ));
        }
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
    // The spent shard, flung out of the port to the left, tumbling.
    commands.spawn((
        Particle {
            velocity: -right * (3.0 + r(0, 14)) + up * (2.5 + r(0, 15)) - forward * 0.5,
            spin: Vec3::new(14.0 + r(0, 16) * 6.0, 4.0, 9.0),
            drag: 0.2,
            gravity: 14.0,
            age: 0.0,
            life: 1.4,
            from: Vec3::new(0.18, 0.06, 0.18),
            to: Vec3::new(0.18, 0.06, 0.18),
            streak: false,
        },
        Mesh3d(fx.needle.clone()),
        MeshMaterial3d(fx.pale.clone()),
        Transform::from_translation(at - right * side * BARREL_X - forward * 0.33 - right * 0.045 - up * 0.006),
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
            let grow = 0.2 + 0.8 * k.powf(f.power);
            Vec3::new(f.size.x * grow, f.size.y * (1.0 - k) * 0.6, f.size.z * grow)
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
