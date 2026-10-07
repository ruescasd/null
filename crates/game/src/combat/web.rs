//! The web (`--opt web`): when the swarm gathers into a hunter, each member
//! shoots a cable to its two nearest fellows, and the cables reel in, taut
//! and trembling, as the members are drawn together. A member destroyed
//! mid-gather snaps its cables, which whip back and hang from the others
//! before they drop away; if the gathering fails, all of its cables snap.
//! When the hunter forms they go into it with the members.

use bevy::{asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology, prelude::*};
use worldgen::noise::hash01;

use super::{
    Mode, Swarmer,
    hunter::{
        Assembly,
        cables::{rope, tubes},
    },
};
use crate::{Args, terrain::WorldGen};

/// Seconds a cable takes to shoot across, how much longer than the gap it
/// stays, how hard a taut one trembles (metres), and how long a snapped one
/// hangs before it goes.
const SHOOT: f32 = 0.25;
const SLACK: f32 = 1.04;
const TREMBLE: f32 = 0.025;
const SNAPPED: f32 = 1.5;
const WIDTH: f32 = 0.035;
const SHADE: f32 = 0.5;

/// A cable between two members of a gathering (or, snapped, hanging from
/// one).
#[derive(Component)]
pub(super) struct Thread {
    a: Entity,
    b: Option<Entity>,
    age: f32,
    /// Seconds since it snapped.
    snapped: Option<f32>,
    length: f32,
    points: Vec<Vec3>,
    previous: Vec<Vec3>,
    mesh: Handle<Mesh>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn web(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    world: Res<WorldGen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material: Local<Option<Handle<StandardMaterial>>>,
    started: Query<Entity, Added<Assembly>>,
    swarm: Query<(Entity, &Transform, &Swarmer)>,
    mut threads: Query<(Entity, &mut Thread)>,
) {
    if !args.opt("web") {
        return;
    }
    let dt = time.delta_secs().clamp(0.001, 1.0 / 30.0);
    let material = material
        .get_or_insert_with(|| materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.3, reflectance: 0.6, ..default() }))
        .clone();

    // A gathering begins: each member shoots a cable to its two nearest.
    for assembly in &started {
        let members: Vec<(Entity, Vec3)> =
            swarm.iter().filter(|(_, _, s)| s.mode == Mode::Gather(assembly)).map(|(e, t, _)| (e, t.translation)).collect();
        let mut pairs: Vec<(Entity, Entity)> = Vec::new();
        for &(e, p) in &members {
            let mut others: Vec<&(Entity, Vec3)> = members.iter().filter(|(o, _)| *o != e).collect();
            others.sort_by(|x, y| x.1.distance(p).total_cmp(&y.1.distance(p)));
            for &&(o, _) in others.iter().take(2) {
                if !pairs.iter().any(|&(x, y)| (x, y) == (e, o) || (x, y) == (o, e)) {
                    pairs.push((e, o));
                }
            }
        }
        for (a, b) in pairs {
            let p = members.iter().find(|(x, _)| *x == a).map_or(Vec3::ZERO, |m| m.1);
            let (mut points, mut previous) = (Vec::new(), Vec::new());
            rope(&mut points, &mut previous, p, Some(p + Vec3::Y * 0.01), 0.1, &world, dt);
            // (Built whole from the start: the renderer does not take an
            // empty mesh growing.)
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            tubes(&mut mesh, [(&points[..], (WIDTH, WIDTH), SHADE)]);
            let mesh = meshes.add(mesh);
            commands.spawn((
                Thread { a, b: Some(b), age: 0.0, snapped: None, length: 0.1, points, previous, mesh: mesh.clone() },
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
                Visibility::default(),
                NoFrustumCulling,
            ));
        }
    }

    let t = time.elapsed_secs();
    for (entity, mut thread) in &mut threads {
        thread.age += dt;
        let alive = |e: Entity| swarm.get(e).ok().filter(|(_, _, s)| matches!(s.mode, Mode::Gather(_)));
        // A member gone, or the gathering over: it snaps, hanging from
        // whichever end is left; both gone (taken into the hunter), it goes.
        if thread.snapped.is_none() {
            let (a, b) = (alive(thread.a).is_some(), thread.b.is_some_and(|b| alive(b).is_some()));
            if !a || !b {
                let gone_together = swarm.get(thread.a).is_err() && thread.b.is_none_or(|b| swarm.get(b).is_err());
                if gone_together {
                    commands.entity(entity).despawn();
                    continue;
                }
                if swarm.get(thread.a).is_err() {
                    thread.a = thread.b.unwrap_or(thread.a);
                }
                thread.b = None;
                thread.snapped = Some(0.0);
            }
        }
        if let Some(s) = thread.snapped.as_mut() {
            *s += dt;
            if *s > SNAPPED || swarm.get(thread.a).is_err() {
                commands.entity(entity).despawn();
                continue;
            }
        }
        let Ok((_, ta, _)) = swarm.get(thread.a) else { continue };
        let start = ta.translation;
        let end = thread.b.and_then(|b| swarm.get(b).ok()).map(|(_, tb, _)| {
            // Shooting across, then held.
            start.lerp(tb.translation, (thread.age / SHOOT).min(1.0))
        });
        if let Some(end) = end {
            thread.length = (start.distance(end) * SLACK).max(0.05);
        }
        let length = thread.length;
        let seed = entity.index_u32() as i32;
        let Thread { points, previous, mesh, snapped, .. } = &mut *thread;
        rope(points, previous, start, end, length, &world, dt);
        // Taut, it trembles.
        if snapped.is_none() {
            let n = points.len();
            for (i, p) in points.iter_mut().enumerate().take(n - 1).skip(1) {
                let k = (std::f32::consts::PI * i as f32 / (n - 1) as f32).sin();
                let r = |j: i32| hash01(seed, i as i32, j, (t * 40.0) as u32 ^ 0x6e1) - 0.5;
                *p += Vec3::new(r(0), r(1), r(2)) * TREMBLE * k;
            }
        }
        if let Some(mut m) = meshes.get_mut(&*mesh) {
            tubes(&mut m, [(&points[..], (WIDTH, WIDTH), SHADE)]);
        }
    }
}
