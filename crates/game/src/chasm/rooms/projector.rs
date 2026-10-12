//! A star projector (the user's: a starfield on a rotunda's walls): in the
//! middle of a dark rotunda, over an empty pedestal, nothing to be seen but
//! its throw, a starfield (a light texture: a cube of stars, in the light's
//! own frame, Bevy's `PointLightTexture`). (It had a body, two dark spheres
//! on an axis, lenses lit on them: the stars best come from nowhere, the
//! user.) It turns slowly about a pole tilted as a sky's is, and the stars
//! wheel over the walls, the dome, the floor; what stands in their way (the
//! pedestal, the room's columns, whoever walks in) casts its shadow across
//! them.

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::CubemapLayout,
    light::PointLightTexture,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use worldgen::noise::hash01;

/// Pixels on a side of each face of the stars' cube.
const FACE: u32 = 1024;
/// Stars, and the brightest's light (lumens; a star full on the texture)
/// for a light reaching `REACH` (as the square of its reach beyond).
const STARS: i32 = 5000;
const LIGHT: f32 = 2.0e7;
const REACH: f32 = 22.0;
/// How fast it turns (radians a second: a turn in six minutes), and how far
/// its pole leans from upright.
const TURN: f32 = std::f32::consts::TAU / 360.0;
const LEAN: f32 = 0.6;
/// The light it lets out all round (of a star's): the room just seen.
const LEAK: f32 = 0.06;
/// A projector: how it leans (from upright to its pole).
#[derive(Component)]
pub(crate) struct Projector {
    lean: Quat,
}

/// How far from the projector its shadows are cast (beyond, it is out of
/// sight, or nearly: casting, it cost as much as a lamp's shadows, a
/// millisecond and more, wherever you were).
const SHADOWS: f32 = 60.0;

/// Puts a projector's light at `at`, reaching `reach`.
pub(crate) fn spawn(commands: &mut Commands, images: &mut Assets<Image>, at: Vec3, reach: f32) {
    let lean = Quat::from_rotation_z(LEAN);
    commands.spawn((
        Projector { lean },
        PointLight { intensity: LIGHT * (reach / REACH).powi(2).max(1.0), range: reach, shadow_maps_enabled: false, ..default() },
        PointLightTexture { image: images.add(stars()), cubemap_layout: CubemapLayout::SequenceVertical },
        Transform::from_translation(at).with_rotation(lean),
    ));
}

/// It turns about its pole; its shadows cast only from near.
pub(crate) fn animate(time: Res<Time>, camera: Single<&Transform, (With<crate::camera::FlyCam>, Without<Projector>)>, mut projectors: Query<(&Projector, &mut Transform, &mut PointLight)>) {
    for (p, mut t, mut light) in &mut projectors {
        t.rotation = p.lean * Quat::from_rotation_y(time.elapsed_secs() * TURN);
        let near = t.translation.distance(camera.translation) < SHADOWS;
        if light.shadow_maps_enabled != near {
            light.shadow_maps_enabled = near;
        }
    }
}

/// The stars, as the light's cube (Bevy's vertical sequence: +x, -x, +y,
/// -y, -z, +z, each face's corners as the shader takes them): stars at
/// random over the sphere, most faint, a few bright, crowding towards a
/// band, which glows faintly of its own; and over all, the little light a
/// projector lets out, which shows the room dimly (lit from anywhere else,
/// a light in the middle lit only the pedestal by it, lamps by the walls
/// only pools round them).
fn stars() -> Image {
    let n = FACE as usize;
    let mut v = vec![0.0f32; n * n * 6];
    let band = Vec3::new(0.3, 0.55, -0.78).normalize();
    for face in 0..6 {
        for y in 0..n {
            for x in 0..n {
                let d = direction(face, (x as f32 + 0.5) / FACE as f32, (y as f32 + 0.5) / FACE as f32);
                v[face * n * n + y * n + x] = LEAK + 0.012 * (-(d.dot(band) / 0.2).powi(2)).exp();
            }
        }
    }
    let mut placed = 0;
    let mut k = 0;
    while placed < STARS {
        k += 1;
        let r = |j: i32| hash01(k, j, 0, 0x57a2);
        let (z, a) = (r(1) * 2.0 - 1.0, r(2) * std::f32::consts::TAU);
        let d = Vec3::new((1.0 - z * z).sqrt() * a.cos(), z, (1.0 - z * z).sqrt() * a.sin());
        // (Thinned away from the band.)
        if r(3) > 0.3 + 0.7 * (-(d.dot(band) / 0.25).powi(2)).exp() {
            continue;
        }
        placed += 1;
        let peak = 0.2 + 0.8 * r(4).powf(6.0);
        let sigma = 0.7 + 0.9 * peak;
        let (face, u, w) = face_of(d);
        let (cx, cy) = (u * FACE as f32, w * FACE as f32);
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                let (px, py) = (cx as i32 + dx, cy as i32 + dy);
                if px < 0 || py < 0 || px >= FACE as i32 || py >= FACE as i32 {
                    continue;
                }
                let (ox, oy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                v[face * n * n + py as usize * n + px as usize] += peak * (-(ox * ox + oy * oy) / (2.0 * sigma * sigma)).exp();
            }
        }
    }
    // (Dithered: the band's glow is a few of the texture's 255 steps, and
    // it lay on the walls in stripes, one a step.)
    let data = v.iter().enumerate().map(|(i, x)| (x.clamp(0.0, 1.0) * 255.0 + hash01(i as i32, 0, 0, 0xd17)).min(255.0) as u8).collect();
    Image::new(Extent3d { width: FACE, height: FACE * 6, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::R8Unorm, RenderAssetUsages::RENDER_WORLD)
}

/// Which face a direction falls on, and where on it (0 to 1 across, down),
/// as Bevy's shader takes it (`cubemap_uv`).
fn face_of(d: Vec3) -> (usize, f32, f32) {
    let a = d.abs();
    let (face, (u, w), div) = if a.x >= a.y && a.x >= a.z {
        if d.x > 0.0 { (0, (d.z, -d.y), d.x) } else { (1, (-d.z, -d.y), -d.x) }
    } else if a.y >= a.z {
        if d.y > 0.0 { (2, (d.x, -d.z), d.y) } else { (3, (d.x, d.z), -d.y) }
    } else if d.z < 0.0 {
        (4, (d.x, -d.y), -d.z)
    } else {
        (5, (d.x, d.y), d.z)
    };
    (face, u / div * 0.5 + 0.5, w / div * 0.5 + 0.5)
}

/// The direction at a point on a face (`face_of` turned round).
fn direction(face: usize, u: f32, w: f32) -> Vec3 {
    let (s, t) = (u * 2.0 - 1.0, w * 2.0 - 1.0);
    match face {
        0 => Vec3::new(1.0, -t, s),
        1 => Vec3::new(-1.0, -t, -s),
        2 => Vec3::new(s, 1.0, -t),
        3 => Vec3::new(s, -1.0, t),
        4 => Vec3::new(s, -t, -1.0),
        _ => Vec3::new(s, t, 1.0),
    }
    .normalize()
}
