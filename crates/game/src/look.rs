//! The visual treatment of an airless world: a black, starry sky; two distant
//! suns on independent orbits; no haze; shadows filled only by light bounced
//! off the sunlit ground; and a greyscale grade.

use std::f32::consts::{PI, TAU};

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    asset::RenderAssetUsages,
    camera::{Exposure, Hdr},
    core_pipeline::tonemapping::Tonemapping,
    light::{
        CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster, NotShadowReceiver,
        ShadowFilteringMethod, Skybox,
    },
    pbr::{ContactShadows, DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    render::{
        render_resource::{
            Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
            TextureViewDimension,
        },
        view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
    },
};
use worldgen::noise::hash01;

use crate::{Args, camera::FlyCam, terrain::WorldGen};

pub struct LookPlugin;

impl Plugin for LookPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<Args>().clone();
        app.insert_resource(Sky {
            time: args.time.unwrap_or(150.0),
            speed: 1.0,
            paused: args.shot.is_some() || args.opt("labshots"),
            soft_shadows: !args.opt("hard"),
        })
        .insert_resource(Tuning {
            bounce: args.num("bounce", 2.0),
            fill: args.num("fill", 6000.0),
            night_fill: args.num("night_fill", 2500.0),
            softness: [args.num("soft0", 1.0), args.num("soft1", 1.0)],
            fog_day: args.num("fog_day", 0.04),
            fog_night: args.num("fog_night", 0.01),
        })
        .init_resource::<Gloom>()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GlobalAmbientLight::NONE)
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_systems(Startup, setup)
        .add_systems(Update, (sky_controls, move_suns, darken, hud).chain())
        .add_systems(Update, flicker);
    }
}

/// Bloom flickers: on in short, irregular bursts (about 30 ms on, 80 ms off
/// on average), so bright things (the creature's chest and eyes) seem to
/// stutter like a failing light. It began as a bug (an unordered pass that
/// threw bloom away most frames) and was kept for how it looked, measured
/// from the bug. `--opt steadybloom` keeps bloom on.
fn flicker(time: Res<Time>, args: Res<Args>, mut blooms: Query<&mut Bloom>, mut state: Local<(bool, u64)>) {
    if args.opt("steadybloom") {
        return;
    }
    let (on, seed) = &mut *state;
    // A small xorshift: the flicker needs no quality, only irregularity.
    *seed = if *seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { *seed };
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    let roll = (*seed >> 11) as f32 / (1u64 << 53) as f32;
    let mean = if *on { 0.03 } else { 0.08 };
    if roll < time.delta_secs() / mean {
        *on = !*on;
    }
    for mut bloom in &mut blooms {
        bloom.intensity = if *on { 0.12 } else { 0.0001 };
    }
}

/// Average albedo of the ground, used to estimate how much sunlight it
/// bounces back up into shadows. The default `bounce` tuning of 2 on top of
/// this stands in for multiple bounces and keeps shadowed relief readable.
const GROUND_ALBEDO: f32 = 0.2;

/// A sun on a circular orbit. In greyscale the suns can only differ in
/// brightness and size: a larger sun casts softer shadows.
struct SunOrbit {
    illuminance: f32,
    /// Apparent diameter in degrees (drawn disc, and shadow softness).
    angular_size: f32,
    /// PCSS light size for this sun's shadows. For directional lights Bevy
    /// measures this against light-space depth, so the numbers are empirical.
    shadow_softness: f32,
    /// Seconds per orbit.
    period: f32,
    phase: f32,
    /// Highest elevation the sun reaches, in degrees.
    max_elevation: f32,
    /// Compass direction of the orbit's rising point, in degrees.
    heading: f32,
}

const SUNS: [SunOrbit; 2] = [
    // A small, fierce white star: hard shadows.
    SunOrbit {
        illuminance: 22_000.0,
        angular_size: 0.7,
        shadow_softness: 8.0,
        period: 900.0,
        phase: 0.0,
        max_elevation: 27.0,
        heading: 20.0,
    },
    // A large, dim companion: broad soft shadows.
    SunOrbit {
        illuminance: 7_000.0,
        angular_size: 4.5,
        shadow_softness: 3600.0,
        period: 347.0,
        phase: 1.9,
        max_elevation: 16.0,
        heading: 115.0,
    },
];

impl SunOrbit {
    /// Unit vector towards the sun at time `t`.
    fn direction(&self, t: f32) -> Vec3 {
        let angle = t / self.period * TAU + self.phase;
        // Circle in a plane through the horizon line, tilted so its top is at
        // `max_elevation`.
        let tilt = (90.0 - self.max_elevation).to_radians();
        let along = Vec3::X * angle.cos();
        let up = (Vec3::Y * tilt.cos() + Vec3::Z * tilt.sin()) * angle.sin();
        Quat::from_rotation_y(self.heading.to_radians()) * (along + up)
    }
}

#[derive(Resource)]
pub struct Sky {
    /// Simulation time driving the suns, in seconds.
    pub time: f32,
    pub speed: f32,
    pub paused: bool,
    pub soft_shadows: bool,
}

/// Multipliers that can be tweaked from the command line while tuning.
#[derive(Resource)]
struct Tuning {
    bounce: f32,
    /// Fill light in full daylight, and at night.
    fill: f32,
    night_fill: f32,
    softness: [f32; 2],
    /// Brightness of the haze by day and by night.
    fog_day: f32,
    fog_night: f32,
}

#[derive(Component)]
struct Sun(usize);

#[derive(Component)]
struct SunDisc(usize);

#[derive(Component)]
struct Fill;

#[derive(Component)]
struct Hud;

/// The suns' discs are drawn this far away (inside the far plane).
const DISC_DISTANCE: f32 = 15_000.0;

fn setup(
    mut commands: Commands,
    args: Res<Args>,
    world: Res<WorldGen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let [x, agl, z, yaw, pitch] = args.cam.unwrap_or([1200.0, 30.0, 900.0, -40.0, -6.0]);
    let ground = world.ground_height(x, z);
    let fly = FlyCam {
        yaw: yaw.to_radians(),
        pitch: pitch.to_radians(),
        speed: 25.0,
        noclip: (args.shot.is_some() && !args.opt("fight")) || args.opt("noclip") || args.opt("labshots"),
    };

    let stars = star_cubemap(&mut images);
    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: 65f32.to_radians(),
                far: 20_000.0,
                ..default()
            }),
            Transform::from_xyz(x, ground + agl, z).with_rotation(fly.rotation()),
            fly,
            Hdr,
            Msaa::Off,
            Exposure { ev100: args.num("ev", 11.2) },
            Tonemapping::AgX,
            Bloom { intensity: 0.12, ..Bloom::NATURAL },
            Skybox { image: Some(stars), brightness: 900.0, ..default() },
            GeneratedEnvironmentMapLight {
                environment_map: bounce_cubemap(&mut images),
                intensity: 0.0,
                ..default()
            },
            ColorGrading {
                global: ColorGradingGlobal { post_saturation: 0.0, ..default() },
                shadows: ColorGradingSection { contrast: args.num("contrast", 1.1), ..default() },
                midtones: ColorGradingSection { contrast: args.num("contrast", 1.1), ..default() },
                highlights: ColorGradingSection { contrast: args.num("contrast", 1.1), ..default() },
            },
        ))
        .id();
    let mut cam = commands.entity(camera);
    // Haze. There is no air, but layers fading with distance are what
    // make the scale read (drama over correctness): distant things sink
    // into darkness, faintly lit by day and glowing a little towards the
    // suns. `--set fog=0` turns it off.
    let visibility = args.num("fog", 4500.0);
    if visibility > 0.0 {
        cam.insert(DistanceFog {
            color: Color::BLACK,
            directional_light_color: Color::srgba(1.0, 1.0, 1.0, args.num("fog_glow", 0.15)),
            directional_light_exponent: 10.0,
            falloff: FogFalloff::from_visibility_squared(visibility),
        });
    }
    if !args.opt("notaa") {
        cam.insert((TemporalAntiAliasing::default(), ShadowFilteringMethod::Temporal));
    }
    if !args.opt("nocontact") {
        cam.insert(ContactShadows::default());
    }

    // Fill: a soft, shadowless light kept opposite the dominant sun, so
    // whatever faces away from the key light stays readable. Not physical
    // (the sky is black), but it keeps the darkness dramatic rather than
    // empty. Its strength follows the daylight (see `move_suns`).
    commands.spawn((
        Fill,
        DirectionalLight {
            illuminance: 0.0,
            shadow_maps_enabled: false,
            ..default()
        },
    ));

    let disc_mesh = meshes.add(Sphere::new(1.0).mesh().uv(32, 16));
    for (i, orbit) in SUNS.iter().enumerate() {
        commands.spawn((
            Sun(i),
            DirectionalLight {
                illuminance: orbit.illuminance,
                shadow_maps_enabled: true,
                contact_shadows_enabled: true,
                ..default()
            },
            // The dim sun's soft shadows get away with less resolution and range.
            if i == 0 {
                CascadeShadowConfigBuilder {
                    num_cascades: 4,
                    minimum_distance: 0.1,
                    first_cascade_far_bound: 40.0,
                    maximum_distance: 3000.0,
                    overlap_proportion: 0.2,
                }
            } else {
                CascadeShadowConfigBuilder {
                    num_cascades: 2,
                    minimum_distance: 0.1,
                    first_cascade_far_bound: 120.0,
                    maximum_distance: 1200.0,
                    overlap_proportion: 0.2,
                }
            }
            .build(),
        ));
        // Brightness relative to the main sun, so the small disc blooms more.
        let radiance = orbit.illuminance / SUNS[0].illuminance
            * (SUNS[0].angular_size / orbit.angular_size).powi(2)
            * 150.0;
        commands.spawn((
            SunDisc(i),
            Mesh3d(disc_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::linear_rgb(radiance, radiance, radiance),
                unlit: true,
                ..default()
            })),
            Transform::from_scale(Vec3::splat(
                DISC_DISTANCE * (orbit.angular_size.to_radians() * 0.5).tan(),
            )),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }

    commands.spawn((
        Hud,
        if args.shot.is_some() || args.opt("labshots") { Visibility::Hidden } else { Visibility::Visible },
        Text::new(""),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::srgb(0.75, 0.75, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            top: px(8),
            left: px(8),
            ..default()
        },
    ));
}

fn sky_controls(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut sky: ResMut<Sky>) {
    let dt = time.delta_secs();
    if keys.just_pressed(KeyCode::KeyT) {
        sky.paused = !sky.paused;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        sky.soft_shadows = !sky.soft_shadows;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        sky.speed = (sky.speed * (1.0 + 2.0 * dt)).min(200.0);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        sky.speed = (sky.speed / (1.0 + 2.0 * dt)).max(0.1);
    }
    // Scrub time by hand.
    if keys.pressed(KeyCode::ArrowRight) {
        sky.time += 60.0 * dt;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        sky.time -= 60.0 * dt;
    }
    if !sky.paused {
        sky.time += sky.speed * dt;
    }
}

/// The swarm's darkness (`--opt dark`; see `combat::gloom`): 0 clear, 1 all
/// but blind.
#[derive(Resource, Default)]
pub struct Gloom(pub f32);

/// Darkness closing in: the haze draws in to a few metres (`--set
/// gloom_near`), thickening from your feet out, and goes black; the light
/// itself dims (`--set gloom_dim`, in stops at its darkest); the stars go
/// out. What glows (the buds' lights, eyeshine) is not hazed, so it is all
/// that shows.
fn darken(args: Res<Args>, gloom: Res<Gloom>, camera: Single<(Option<&mut DistanceFog>, &mut Skybox, &mut Exposure), With<FlyCam>>) {
    let g = gloom.0;
    let (fog, mut sky, mut exposure) = camera.into_inner();
    sky.brightness = 900.0 * (1.0 - g) * (1.0 - g);
    exposure.ev100 = args.num("ev", 11.2) + args.num("gloom_dim", 2.5) * g;
    if let Some(mut fog) = fog {
        let visibility = args.num("fog", 4500.0);
        let near = args.num("gloom_near", 4.0);
        // (Closing in fast at first: a little gloom already hides the far
        // distance, a quarter of it leaves about twenty metres.)
        // (In the swarm's darkness the haze thickens steadily from your feet,
        // not with the square of the distance, which leaves the ground round
        // you clear.)
        let seen = 1.0 / (1.0 / visibility + g * (1.0 / near - 1.0 / visibility));
        fog.falloff = if g > 0.0 { FogFalloff::from_visibility(seen) } else { FogFalloff::from_visibility_squared(seen) };
        // (Darker, not thinner: the alpha is how strongly the haze applies.)
        let c = fog.color.to_linear() * (1.0 - g);
        fog.color = Color::linear_rgb(c.red, c.green, c.blue);
        // (The glow towards the suns goes first: it is lit by them, and
        // would light the dark.)
        fog.directional_light_color.set_alpha(args.num("fog_glow", 0.15) * (1.0 - g / 0.25).max(0.0));
    }
}

fn move_suns(
    sky: Res<Sky>,
    tuning: Res<Tuning>,
    // Bevy turns the generated light into an `EnvironmentMapLight` once the
    // cubemap is filtered; from then on that component carries the intensity.
    camera: Single<(&Transform, Option<&mut EnvironmentMapLight>, Option<&mut DistanceFog>), With<FlyCam>>,
    mut suns: Query<(&Sun, &mut Transform, &mut DirectionalLight), Without<FlyCam>>,
    fill: Single<(&mut Transform, &mut DirectionalLight), (With<Fill>, Without<Sun>, Without<FlyCam>)>,
    mut discs: Query<(&SunDisc, &mut Transform), (Without<Sun>, Without<FlyCam>, Without<Fill>)>,
) {
    let (cam_transform, bounce, fog) = camera.into_inner();
    let mut bounced = 0.0;
    // How much daylight there is (0 when both suns are down), and which sun
    // dominates it.
    let mut daylight = 0.0;
    let mut key = SUNS[0].direction(sky.time);
    let mut key_weight = 0.0;
    for (sun, mut transform, mut light) in &mut suns {
        let orbit = &SUNS[sun.0];
        let dir = orbit.direction(sky.time);
        *transform = Transform::IDENTITY.looking_to(-dir, Vec3::Y);
        // Below the horizon a sun is simply off.
        let up = dir.y.max(0.0);
        light.illuminance = orbit.illuminance * (up * 20.0).min(1.0);
        light.soft_shadow_size =
            sky.soft_shadows.then_some(orbit.shadow_softness * tuning.softness[sun.0]);
        bounced += orbit.illuminance * up;
        let t = ((dir.y + 0.03) / 0.15).clamp(0.0, 1.0);
        let weight = orbit.illuminance / SUNS[0].illuminance * t * t * (3.0 - 2.0 * t);
        daylight += weight;
        if weight > key_weight {
            key_weight = weight;
            key = dir;
        }
    }
    let daylight = daylight.min(1.0);
    // Sunlit ground radiance (Lambertian), which lights everything from below.
    if let Some(mut bounce) = bounce {
        bounce.intensity = bounced * GROUND_ALBEDO / PI * tuning.bounce;
    }

    // The fill follows the daylight: opposite the dominant sun's compass
    // direction, 35 degrees up, and at night a faint glow from overhead so
    // the world never goes completely blind.
    let (mut fill_transform, mut fill_light) = fill.into_inner();
    let away = Vec3::new(-key.x, 0.0, -key.z).normalize_or(Vec3::X);
    let day_from = (away * 35f32.to_radians().cos() + Vec3::Y * 35f32.to_radians().sin()).normalize();
    let from = Vec3::Y.lerp(day_from, daylight).normalize();
    *fill_transform = Transform::IDENTITY.looking_to(-from, Vec3::Y);
    fill_light.illuminance = tuning.night_fill + (tuning.fill - tuning.night_fill) * daylight;
    if let Some(mut fog) = fog {
        let luma = tuning.fog_night + (tuning.fog_day - tuning.fog_night) * daylight;
        fog.color = Color::linear_rgb(luma, luma, luma);
    }

    for (disc, mut transform) in &mut discs {
        let dir = SUNS[disc.0].direction(sky.time);
        transform.translation = cam_transform.translation + dir * DISC_DISTANCE;
    }
}

/// Cubemap face texel to direction, using the wgpu/Vulkan face order.
fn cube_dir(face: u32, u: f32, v: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    }
    .normalize()
}

fn cubemap(images: &mut Assets<Image>, size: u32, texel: impl Fn(u32, u32, u32, Vec3) -> f32) -> Handle<Image> {
    let mut values = Vec::with_capacity((size * size * 6) as usize);
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                values.push(texel(face, x, y, cube_dir(face, u, v)));
            }
        }
    }
    cubemap_image(images, size, &values)
}

/// A greyscale cube texture from per-texel values (face, then row, then
/// column), clamped to 0..1.
fn cubemap_image(images: &mut Assets<Image>, size: u32, values: &[f32]) -> Handle<Image> {
    let data = values
        .iter()
        .flat_map(|v| {
            let value = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            [value, value, value, 255]
        })
        .collect();
    let mut image = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 6 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    images.add(image)
}

/// Light arriving from each direction relative to the sunlit ground: the ground
/// below the horizon, and essentially nothing from the black sky above.
fn bounce_cubemap(images: &mut Assets<Image>) -> Handle<Image> {
    cubemap(images, 32, |_, _, _, dir| {
        let t = ((-dir.y + 0.03) / 0.06).clamp(0.0, 1.0);
        0.004 + t * t * (3.0 - 2.0 * t)
    })
}

/// A field of stars with a faint band across the sky. Each star is a small
/// soft blob a few texels wide rather than a single texel: temporal
/// anti-aliasing treats one-pixel features as noise and erases them as soon
/// as the camera moves.
fn star_cubemap(images: &mut Assets<Image>) -> Handle<Image> {
    const SIZE: u32 = 1024;
    const SIGMA: f32 = 1.0;
    let band_normal = Vec3::new(0.3, 0.55, -0.78).normalize();
    let n = SIZE as usize;
    let mut values = vec![0.0f32; n * n * 6];
    for face in 0..6u32 {
        let base = face as usize * n * n;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let u = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                let band = (-(cube_dir(face, u, v).dot(band_normal) / 0.18).powi(2)).exp();
                let i = base + y as usize * n + x as usize;
                values[i] += band * 0.012;
                let r = hash01(x as i32, y as i32, face as i32, 0x5747);
                if r >= 0.0009 * (1.0 + 5.0 * band) {
                    continue;
                }
                // Splat a small Gaussian (clipped at the face edge).
                let b = hash01(x as i32, y as i32, face as i32, 0x9157);
                let peak = 0.3 + 0.7 * b.powf(5.0);
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let (sx, sy) = (x as i32 + dx, y as i32 + dy);
                        if sx < 0 || sy < 0 || sx >= SIZE as i32 || sy >= SIZE as i32 {
                            continue;
                        }
                        let w = (-((dx * dx + dy * dy) as f32) / (2.0 * SIGMA * SIGMA)).exp();
                        values[base + sy as usize * n + sx as usize] += peak * w;
                    }
                }
            }
        }
    }
    cubemap_image(images, SIZE, &values)
}

fn hud(
    keys: Res<ButtonInput<KeyCode>>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    sky: Res<Sky>,
    camera: Single<(&Transform, &FlyCam)>,
    mut text: Single<(&mut Text, &mut Visibility), With<Hud>>,
) {
    let (text, visibility) = &mut *text;
    if keys.just_pressed(KeyCode::F1) {
        visibility.toggle_visible_hidden();
    }
    let fps = diagnostics
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let (t, fly) = *camera;
    let elevations: Vec<String> = SUNS
        .iter()
        .map(|s| format!("{:.0}", s.direction(sky.time).y.asin().to_degrees()))
        .collect();
    text.0 = format!(
        "{fps:.0} fps\npos {:.0} {:.0} {:.0}{}\ntime {:.0}s x{:.1}{}  sun elevations {}{}\n\n\
         click: capture mouse  esc: release  f1: this help  f12: screenshot\n\
         wasd: move  space or mouse 2 (hold): jump / bunny hop  mouse 1: shard shotgun\n\
         e or mouse 4/5 (hold): tether - roots where you aim and pulls you in; let go to keep the momentum\n\
         v: noclip fly (space/ctrl: up/down, shift: fast, wheel: speed)\n\
         t: pause time  left/right: scrub time  up/down: time speed  p: soft shadows",
        t.translation.x,
        t.translation.y,
        t.translation.z,
        if fly.noclip { format!("  noclip, speed {:.0}", fly.speed) } else { String::new() },
        sky.time,
        sky.speed,
        if sky.paused { " (paused)" } else { "" },
        elevations.join(" / "),
        if sky.soft_shadows { "" } else { "  (hard shadows)" },
    );
}
