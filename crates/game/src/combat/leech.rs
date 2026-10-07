//! Leeches (`--opt leech`): a bud waiting near you, where you are not
//! looking, may shoot a cable into you and drink: a little health a second,
//! for as long as it holds. It snaps when the bud is destroyed or you get far
//! enough away, whipping back and hanging before it goes. The cable glows
//! faintly, pulsing as it drinks (in the dark, only what is lit shows); turn
//! round and it shows you where the bud is.

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
const REACH: f32 = 6.0;
const EVERY: f32 = 4.0;
const AT_ONCE: usize = 3;
const BREAKS: f32 = 12.0;
/// Health a second each one drinks.
const DRAIN: f32 = 1.0;
/// Seconds a cable takes to shoot across, and to hang once snapped.
const SHOOT: f32 = 0.2;
const SNAPPED: f32 = 1.2;
const WIDTH: f32 = 0.025;
const SHADE: f32 = 0.035;
/// Its glow, between and at the peak of each pulse, and pulses a second.
const GLOW: (f32, f32) = (15.0, 90.0);
const PULSE: f32 = 1.6;

#[derive(Component)]
pub(super) struct Leech {
    bud: Entity,
    age: f32,
    snapped: Option<f32>,
    length: f32,
    points: Vec<Vec3>,
    previous: Vec<Vec3>,
    mesh: Handle<Mesh>,
    /// Its own, to pulse.
    material: Handle<StandardMaterial>,
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
    player: Single<(&Transform, &mut Player), With<FlyCam>>,
    swarm: Query<(Entity, &Transform, &Swarmer), Without<FlyCam>>,
    mut leeches: Query<(Entity, &mut Leech)>,
) {
    if !args.opt("leech") {
        return;
    }
    let dt = time.delta_secs().clamp(0.001, 1.0 / 30.0);
    let (eye, mut p) = player.into_inner();
    let into = chest(eye);

    // A waiting bud near you, unseen, may latch on.
    let held: Vec<Entity> = leeches.iter().filter(|(_, l)| l.snapped.is_none()).map(|(_, l)| l.bud).collect();
    if held.len() < AT_ONCE {
        let t = (time.elapsed_secs() * 1000.0) as i32;
        for (bud, transform, s) in &swarm {
            let to = transform.translation - eye.translation;
            let seen = to.normalize_or(Vec3::Y).dot(*eye.forward()) > 0.55;
            if s.mode != Mode::Free || s.dark < 0.5 || seen || to.length() > REACH || held.contains(&bud) {
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
            // (Not hazed: it shows in the dark.)
            let material = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.8,
                reflectance: 0.15,
                emissive: LinearRgba::gray(GLOW.0),
                fog_enabled: false,
                ..default()
            });
            commands.spawn((
                Leech { bud, age: 0.0, snapped: None, length: 0.1, points, previous, mesh: mesh.clone(), material: material.clone() },
                Mesh3d(mesh),
                MeshMaterial3d(material),
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
        // Pulsing while it drinks; dying away once snapped.
        let pulse = (0.5 + 0.5 * (l.age * PULSE * std::f32::consts::TAU).sin()).powi(3);
        let glow = (GLOW.0 + (GLOW.1 - GLOW.0) * pulse) * l.snapped.map_or(1.0, |s| (1.0 - s / SNAPPED).max(0.0));
        if let Some(mut m) = materials.get_mut(&l.material) {
            m.emissive = LinearRgba::gray(glow);
        }
        let length = l.length.max(0.1);
        let Leech { points, previous, mesh, .. } = &mut *l;
        rope(points, previous, start, end, length, &world, dt);
        if let Some(mut m) = meshes.get_mut(&*mesh) {
            tubes(&mut m, [(&points[..], (WIDTH, WIDTH * 0.6), SHADE)]);
        }
    }
}
