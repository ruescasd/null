//! A star projector (the user's: a starfield on a rotunda's walls): in the
//! middle of a dark rotunda, on a pedestal, two dark spheres on an axis
//! tilted as a sky's pole is, each set with faintly lit lenses, and between
//! them a light whose throw is a starfield (a light texture: a cube of
//! stars, in the light's own frame, Bevy's `PointLightTexture`). It turns
//! slowly about its pole, and the stars wheel over the walls, the dome, the
//! floor; what stands in their way (the room's columns, whoever walks in)
//! casts its shadow across them.

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::CubemapLayout,
    light::{NotShadowCaster, PointLightTexture},
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
/// The spheres: their radius, how far each is from the middle.
const BALL: f32 = 0.4;
const ARM: f32 = 1.0;

/// A projector: how it leans (from upright to its pole).
#[derive(Component)]
pub(crate) struct Projector {
    lean: Quat,
}

/// Builds a projector with its middle at `at`, its light reaching `reach`,
/// its post down to `foot`.
pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, at: Vec3, reach: f32, foot: f32) {
    let metal = materials.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.055), metallic: 0.8, perceptual_roughness: 0.35, ..default() });
    let lens = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(0.8), ..default() });
    let lean = Quat::from_rotation_z(LEAN);
    let projector = commands.spawn((Projector { lean }, Transform::from_translation(at).with_rotation(lean), Visibility::default())).id();
    let stars = images.add(stars());
    let mut parts = vec![commands
        .spawn((
            PointLight { intensity: LIGHT * (reach / REACH).powi(2).max(1.0), range: reach, shadow_maps_enabled: true, ..default() },
            PointLightTexture { image: stars, cubemap_layout: CubemapLayout::SequenceVertical },
            Transform::IDENTITY,
        ))
        .id()];
    let ball = meshes.add(Sphere::new(BALL).mesh().uv(32, 18));
    let dot = meshes.add(Cuboid::new(0.045, 0.045, 0.02));
    for side in [-1.0f32, 1.0] {
        let c = Vec3::Y * ARM * side;
        parts.push(commands.spawn((Mesh3d(ball.clone()), MeshMaterial3d(metal.clone()), Transform::from_translation(c), NotShadowCaster)).id());
        // (Its lenses, over the sphere but its foot: they show it turning.)
        for k in 0..20 {
            let r = |j: i32| hash01(k, j, side as i32, 0x1e75);
            let d = Vec3::new(r(1) - 0.5, (r(2) - 0.5) * 0.6 + side * 0.4, r(3) - 0.5).normalize();
            if d.dot(Vec3::Y * side) < -0.3 {
                continue;
            }
            parts.push(commands.spawn((Mesh3d(dot.clone()), MeshMaterial3d(lens.clone()), Transform::from_translation(c + d * BALL).with_rotation(Quat::from_rotation_arc(Vec3::Z, d)), NotShadowCaster)).id());
        }
    }
    // (The axis between them, a collar at the middle round the light.)
    parts.push(commands.spawn((Mesh3d(meshes.add(Cylinder::new(0.12, ARM * 2.0))), MeshMaterial3d(metal.clone()), Transform::IDENTITY, NotShadowCaster)).id());
    parts.push(commands.spawn((Mesh3d(meshes.add(Cylinder::new(0.3, 0.3))), MeshMaterial3d(metal.clone()), Transform::IDENTITY, NotShadowCaster)).id());
    commands.entity(projector).add_children(&parts);
    // The post it turns on, from its pedestal up to its collar (standing
    // still).
    let post = at.y - 0.15 - foot;
    commands.spawn((Mesh3d(meshes.add(Cylinder::new(0.12, post))), MeshMaterial3d(metal), Transform::from_translation(Vec3::new(at.x, foot + post * 0.5, at.z)), NotShadowCaster));
}

/// It turns about its pole.
pub(crate) fn animate(time: Res<Time>, mut projectors: Query<(&Projector, &mut Transform)>) {
    for (p, mut t) in &mut projectors {
        t.rotation = p.lean * Quat::from_rotation_y(time.elapsed_secs() * TURN);
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
