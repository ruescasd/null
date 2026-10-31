//! Streetlights along the canals (the user, 2026-10-27: the void's echo
//! streetlight worked in the void and in contrast; along a canal its
//! rhythm suits). The world's streetlights are its near scale: a known
//! size by what you see (see the ground lab's `void_lamp_echo`). Here, on
//! both banks of every canal, staggered, a little back from the rim, their
//! arms out towards the pipe; lit only while their canal is on (see
//! `beams::CanalsOn`), dark and unlit off it.
//!
//! Placed once, each at whichever of its wrapped copies is nearest the
//! camera, and lowered as the terrain bends with the planet (the terrain
//! does it in its shader; these are plain meshes). Their lights are off
//! beyond `street_near` m.

use bevy::{
    asset::RenderAssetUsages,
    math::DVec2,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use worldgen::{canal::PIPE_RADIUS, tilted::SheetMesh};

use crate::{
    Args,
    beams::CanalsOn,
    camera::FlyCam,
    terrain::{PLANET_RADIUS, StreamSet, WorldGen},
};

/// How far apart along a canal (m: each bank every other), how far back
/// from the rim (m), how far their lights shine from the camera (m). (At
/// 48 m, a road's rhythm, too familiar: the user's 144.)
const STREET: (f32, f32, f32) = (144.0, 1.5, 300.0);
/// A lit one's glow (as the cores', against the camera's exposure), its
/// lights' (lumens), how far they reach (m): the ground lab's.
const GLOW: (f32, f32, f32) = (1500.0, 2.0e6, 30.0);

pub struct StreetlightsPlugin;

impl Plugin for StreetlightsPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<Args>();
        if args.opt("chasm") || args.opt("lab") || args.opt("nostreetlights") {
            return;
        }
        app.add_systems(Startup, setup).add_systems(Update, (place.after(StreamSet), show));
    }
}

/// A streetlight: where it stands (in the world's first copy).
#[derive(Component)]
struct Streetlight {
    origin: Vec3,
}

/// A streetlight's glowing part, and its light, by its canal.
#[derive(Component)]
struct Glow(usize);
#[derive(Component)]
struct Shine(usize);

#[derive(Resource)]
struct Looks {
    lit: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    near: f32,
    curvature: f32,
}

fn mesh_of(pieces: &[SheetMesh]) -> Mesh {
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for piece in pieces {
        let k = positions.len() as u32;
        positions.extend_from_slice(&piece.positions);
        normals.extend_from_slice(&piece.normals);
        indices.extend(piece.indices.iter().map(|i| i + k));
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}

fn setup(mut commands: Commands, args: Res<Args>, world: Res<WorldGen>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let one = worldgen::grounds::streetlight();
    let body = meshes.add(mesh_of(&one.gloss));
    let glow = meshes.add(mesh_of(&one.glows));
    // (Glossy black, the ichor's; its glow a plain light.)
    let black = materials.add(StandardMaterial { base_color: Color::srgb(0.01, 0.01, 0.01), perceptual_roughness: 0.12, reflectance: 0.7, ..default() });
    let lit = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(args.num("street_glow", GLOW.0)), ..default() });
    commands.spawn(crate::look::LampGlow(lit.clone(), args.num("street_glow", GLOW.0)));
    let curvature = if args.opt("flat") { 0.0 } else { 0.5 / PLANET_RADIUS };
    commands.insert_resource(Looks { lit, dark: black.clone(), near: args.num("street_near", STREET.2), curvature });
    let (every, back) = (args.num("street_every", STREET.0) as f64, args.num("street_back", STREET.1) as f64);
    let (light, range) = (args.num("street_light", GLOW.1), args.num("street_range", GLOW.2));
    let canals = &world.0.canals;
    let field = |c: DVec2| world.0.canal_floor(c);
    let mut count = 0;
    let mut first = None;
    for i in 0..canals.lines.len() {
        let across = canals.across(i);
        let track = canals.track(i, every, &field);
        let middle = track.len() / 2;
        for (n, (at, _, floor)) in track.into_iter().enumerate() {
            // (Each bank every other: staggered.)
            let side = if n % 2 == 0 { 1.0 } else { -1.0 };
            let p = at + across * side * (PIPE_RADIUS + back);
            let y = world.ground_height(p.x as f32, p.y as f32);
            let origin = Vec3::new(p.x as f32, y, p.y as f32);
            // (Its arms, out over its +z, towards the pipe; along it, its +x.)
            let toward = Vec3::new(-across.x as f32, 0.0, -across.y as f32) * side as f32;
            let along = Vec3::new(toward.z, 0.0, -toward.x);
            let turn = Quat::from_mat3(&Mat3::from_cols(along, Vec3::Y, toward));
            let on = false;
            commands
                .spawn((Streetlight { origin }, Transform::from_translation(origin).with_rotation(turn), Visibility::default()))
                .with_children(|lamp| {
                    lamp.spawn((Mesh3d(body.clone()), MeshMaterial3d(black.clone())));
                    lamp.spawn((Glow(i), Mesh3d(glow.clone()), MeshMaterial3d(black.clone())));
                    for &l in &one.lights {
                        lamp.spawn((
                            Shine(i),
                            PointLight { intensity: light, range, shadow_maps_enabled: false, ..default() },
                            Transform::from_translation(Vec3::from(l)),
                            if on { Visibility::Inherited } else { Visibility::Hidden },
                        ));
                    }
                });
            if n == middle {
                first.get_or_insert((origin, toward, floor));
            }
            count += 1;
        }
    }
    info!("streetlights: {count} along the canals, the middle of the first at {:?}, its arms towards {:?}, its canal's floor {:?}", first.map(|f| f.0), first.map(|f| f.1), first.map(|f| f.2));
}

/// Each at its copy nearest the camera, lowered with the planet's bend.
fn place(looks: Res<Looks>, world: Res<WorldGen>, camera: Single<&Transform, (With<FlyCam>, Without<Streetlight>)>, mut lamps: Query<(&Streetlight, &mut Transform)>) {
    let size = world.size();
    let cam = camera.translation;
    for (lamp, mut transform) in &mut lamps {
        let nearest = |o: f32, c: f32| o + ((c - o) / size).round() * size;
        let (x, z) = (nearest(lamp.origin.x, cam.x), nearest(lamp.origin.z, cam.z));
        let d2 = (x - cam.x).powi(2) + (z - cam.z).powi(2);
        transform.translation = Vec3::new(x, lamp.origin.y - looks.curvature * d2, z);
    }
}

/// Lit while their canal is on: their glow, and their lights if near.
fn show(
    on: Res<CanalsOn>,
    looks: Res<Looks>,
    camera: Single<&Transform, With<FlyCam>>,
    mut glows: Query<(&Glow, &mut MeshMaterial3d<StandardMaterial>)>,
    mut shines: Query<(&Shine, &GlobalTransform, &mut Visibility)>,
) {
    if on.is_changed() {
        for (glow, mut material) in &mut glows {
            let lit = on.0.get(glow.0).copied().unwrap_or(false);
            material.0 = if lit { looks.lit.clone() } else { looks.dark.clone() };
        }
    }
    let cam = camera.translation;
    for (shine, at, mut visibility) in &mut shines {
        let lit = on.0.get(shine.0).copied().unwrap_or(false) && at.translation().distance_squared(cam) < looks.near * looks.near;
        let want = if lit { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != want {
            *visibility = want;
        }
    }
}
