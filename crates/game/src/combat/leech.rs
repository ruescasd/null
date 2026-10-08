//! Leeches (`--opt leech`): a bud waiting near you (seen or not) may shoot a
//! cable into you and drink: a little health a second,
//! for as long as it holds. It snaps when the bud is destroyed or you get far
//! enough away, whipping back and hanging before it goes. The cable is
//! black, and glowing gulps run along it from you to the bud as it drinks
//! (so it shows in the dark); it shows you where the bud is.

use bevy::{asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology, prelude::*};
use worldgen::noise::hash01;

use super::{
    Mode, Swarmer,
    hunter::cables::{rope, tubes},
};
use crate::{Args, camera::FlyCam, player::Player, terrain::WorldGen};

/// How near a bud must be to latch on, how often a waiting one does (about
/// once in this many seconds), how many at once, and how far away its cable
/// snaps.
const REACH: f32 = 10.0;
const EVERY: f32 = 1.5;
const AT_ONCE: usize = 3;
const BREAKS: f32 = 12.0;
/// Health a second each one drinks.
const DRAIN: f32 = 1.0;
/// Seconds a cable takes to shoot across, and to hang once snapped.
const SHOOT: f32 = 0.2;
const SNAPPED: f32 = 1.2;
const WIDTH: f32 = 0.025;
const SHADE: f32 = 0.035;
/// The gulps: how bright, how many a second, how long each takes to run
/// its length, and how much of the cable each covers.
const GULP_GLOW: f32 = 140.0;
const GULPS: f32 = 2.0;
const GULP_RUN: f32 = 0.45;
const GULP_SPAN: f32 = 0.2;

#[derive(Component)]
pub(super) struct Leech {
    bud: Entity,
    age: f32,
    snapped: Option<f32>,
    length: f32,
    points: Vec<Vec3>,
    previous: Vec<Vec3>,
    mesh: Handle<Mesh>,
    /// The gulp running along it (a short glowing piece of it).
    gulp: Entity,
    gulp_mesh: Handle<Mesh>,
}

/// The piece of a cable (points from the bud to you) a gulp covers at `age`,
/// or the whole cable's first two points when between gulps (hidden).
fn gulp(points: &[Vec3], age: f32) -> Option<&[Vec3]> {
    let n = points.len();
    let t = (age * GULPS).fract() / GULPS / GULP_RUN;
    if t >= 1.0 || n < 3 {
        return None;
    }
    // From your end (the last point) towards the bud.
    let head = 1.0 - t;
    let a = ((head - GULP_SPAN).max(0.0) * (n - 1) as f32) as usize;
    let b = ((head * (n - 1) as f32).ceil() as usize).clamp(a + 1, n - 1);
    Some(&points[a..=b])
}

/// Where a cable goes into you: your chest, a little below the eye.
fn chest(eye: &Transform) -> Vec3 {
    eye.translation - Vec3::Y * 0.55 + eye.forward() * 0.15
}

#[allow(clippy::too_many_arguments)]
pub(super) fn leech(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    world: Res<WorldGen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut materials_shared: Local<Option<(Handle<StandardMaterial>, Handle<StandardMaterial>)>>,
    mut visibility: Query<&mut Visibility>,
    player: Single<(&Transform, &mut Player), With<FlyCam>>,
    swarm: Query<(Entity, &Transform, &Swarmer), Without<FlyCam>>,
    mut leeches: Query<(Entity, &mut Leech)>,
) {
    if !args.opt("leech") {
        return;
    }
    let dt = time.delta_secs().clamp(0.001, 1.0 / 30.0);
    // Black, matte; the gulps glow, not hazed.
    let (black, glow) = materials_shared
        .get_or_insert_with(|| {
            (
                materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.8, reflectance: 0.15, ..default() }),
                materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::gray(GULP_GLOW), fog_enabled: false, ..default() }),
            )
        })
        .clone();
    let (eye, mut p) = player.into_inner();
    let into = chest(eye);

    // A waiting bud near you may latch on.
    let held: Vec<Entity> = leeches.iter().filter(|(_, l)| l.snapped.is_none()).map(|(_, l)| l.bud).collect();
    if held.len() < AT_ONCE {
        let t = (time.elapsed_secs() * 1000.0) as i32;
        for (bud, transform, s) in &swarm {
            let to = transform.translation - eye.translation;
            if s.mode != Mode::Free || s.dark < 0.5 || to.length() > REACH || held.contains(&bud) {
                continue;
            }
            if hash01(bud.index_u32() as i32, t, 0, 0x711) >= dt / EVERY {
                continue;
            }
            let from = transform.translation;
            let (mut points, mut previous) = (Vec::new(), Vec::new());
            rope(&mut points, &mut previous, from, Some(from + Vec3::Y * 0.01), 0.1, &world, dt);
            // (Built whole from the start: the renderer does not take an
            // empty mesh growing.)
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            tubes(&mut mesh, [(&points[..], (WIDTH, WIDTH * 0.6), SHADE)]);
            let mesh = meshes.add(mesh);
            let mut gulp_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            tubes(&mut gulp_mesh, [(&points[..2], (WIDTH * 1.4, WIDTH * 1.4), 1.0)]);
            let gulp_mesh = meshes.add(gulp_mesh);
            let gulp = commands
                .spawn((Mesh3d(gulp_mesh.clone()), MeshMaterial3d(glow.clone()), Transform::IDENTITY, Visibility::Hidden, NoFrustumCulling, bevy::light::NotShadowCaster))
                .id();
            commands.spawn((
                Leech { bud, age: 0.0, snapped: None, length: 0.1, points, previous, mesh: mesh.clone(), gulp, gulp_mesh },
                Mesh3d(mesh),
                MeshMaterial3d(black.clone()),
                Transform::IDENTITY,
                Visibility::default(),
                NoFrustumCulling,
            ));
            break;
        }
    }

    for (entity, mut l) in &mut leeches {
        l.age += dt;
        let bud = swarm.get(l.bud).ok().map(|(_, t, _)| t.translation);
        // Snapped: the bud is gone (it hangs from you), or you got away (it
        // hangs from the bud).
        if l.snapped.is_none() && bud.is_none_or(|b| b.distance(into) > BREAKS) {
            l.snapped = Some(0.0);
            if bud.is_none() {
                l.points.reverse();
                l.previous.reverse();
            }
        }
        if let Some(s) = l.snapped.as_mut() {
            *s += dt;
            if *s > SNAPPED {
                commands.entity(l.gulp).despawn();
                commands.entity(entity).despawn();
                continue;
            }
        }
        let (start, end) = match (l.snapped, bud) {
            (None, Some(b)) => (b, Some(b.lerp(into, (l.age / SHOOT).min(1.0)))),
            (Some(_), Some(b)) => (b, None),
            (_, None) => (into, None),
        };
        if let Some(end) = end {
            l.length = start.distance(end) * 1.04;
            // Holding on: it drinks.
            if l.age >= SHOOT {
                p.health -= DRAIN * dt;
            }
        }
        let length = l.length.max(0.1);
        let drinking = l.snapped.is_none() && l.age >= SHOOT;
        let age = l.age;
        let Leech { points, previous, mesh, gulp: gulp_entity, gulp_mesh, .. } = &mut *l;
        rope(points, previous, start, end, length, &world, dt);
        if let Some(mut m) = meshes.get_mut(&*mesh) {
            tubes(&mut m, [(&points[..], (WIDTH, WIDTH * 0.6), SHADE)]);
        }
        // A gulp running up it while it drinks.
        let piece = if drinking { gulp(points, age) } else { None };
        if let Ok(mut v) = visibility.get_mut(*gulp_entity) {
            *v = if piece.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        }
        if let (Some(piece), Some(mut m)) = (piece, meshes.get_mut(&*gulp_mesh)) {
            tubes(&mut m, [(piece, (WIDTH * 1.4, WIDTH * 1.4), 1.0)]);
        }
    }
}
