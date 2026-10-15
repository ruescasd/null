//! The porter's leash (`--opt chained`; the user's idea: porters are
//! enslaved by stalkers, each kept by one on a chain to its neck; both
//! struggle as they walk, slave and guard, and the two struggles could be
//! one). The porter goes its own way ahead, the chain fast at the back of
//! its neck; its stalker follows behind on its track, the chain in its
//! hand, watching it, controlling it (not pulling it: the user). The chain
//! only seen, holding nothing (holding the porter back as it drew taut, it
//! upset its walk: the user). The chain itself a rope (see
//! `rope.rs`), drawn as heavy dark links, each turned square to the last:
//! worked metal, worn by use.

use bevy::prelude::*;

use super::hunter::Hunter;
use crate::{rig::Rig, rope::rope};

/// How long the chain is (m); how far behind its porter a stalker keeps
/// (m, on its track: 1.6 first, too close, the user); the size of a link (m along it, round, thick).
pub(super) const LENGTH: f32 = 5.0;
pub(super) const BEHIND: f32 = 4.0;
const LINK: (f32, f32, f32) = (0.085, 0.032, 0.009);

/// On a stalker: the porter it keeps.
#[derive(Component)]
pub(super) struct Keeps(pub(super) Entity);

/// Where a stalker holds the chain: in its first hand (else at its chest).
fn hand(rig: &Rig) -> Vec3 {
    let arms = 5 + 2 * rig.plan.legs.len();
    if rig.plan.arms.is_empty() { rig.chest() } else { rig.bones[arms + 1].b }
}

/// Where the chain is fast to a porter: at the back of its neck, near its
/// base (the side away from where it looks).
fn nape(rig: &Rig) -> Vec3 {
    let neck = rig.bones[3];
    let along = (neck.b - neck.a).normalize_or(Vec3::Y);
    let back = (Vec3::Y - along * along.y).normalize_or(Vec3::Y);
    neck.a.lerp(neck.b, 0.35) + back * neck.radius
}

/// A chain between a stalker and its porter: its points (the rope), and
/// its links.
#[derive(Component)]
pub(super) struct Chain {
    leader: Entity,
    follower: Entity,
    points: Vec<Vec3>,
    previous: Vec<Vec3>,
    links: Vec<Entity>,
}

/// Each stalker that keeps a porter has its chain (made when first seen);
/// each chain swings between the hand and the neck, its links laid along
/// it; either gone, the chain is gone.
#[allow(clippy::too_many_arguments)]
pub(super) fn chains(
    mut commands: Commands,
    time: Res<Time>,
    ground: crate::ground::Ground,
    hunters: Query<&Hunter>,
    leaders: Query<(Entity, &Keeps)>,
    mut chains: Query<(Entity, &mut Chain)>,
    mut links: Query<&mut Transform, Without<Hunter>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut kit: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut made: Local<Vec<Entity>>,
) {
    let dt = time.delta_secs().min(1.0 / 30.0);
    // (A link: a ring, drawn out along the chain. Dark, oiled, worked.)
    let (mesh, material) = kit
        .get_or_insert_with(|| {
            let ring = Torus { minor_radius: LINK.2, major_radius: LINK.1 }.mesh().minor_resolution(6).major_resolution(12).build();
            (meshes.add(ring), materials.add(StandardMaterial { base_color: Color::srgb(0.06, 0.055, 0.05), perceptual_roughness: 0.5, metallic: 0.7, ..default() }))
        })
        .clone();
    for (leader, keeps) in &leaders {
        if made.contains(&leader) {
            continue;
        }
        made.push(leader);
        let count = (LENGTH / LINK.0) as usize;
        let links = (0..count).map(|_| commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id()).collect();
        commands.spawn(Chain { leader, follower: keeps.0, points: Vec::new(), previous: Vec::new(), links });
    }
    for (entity, mut chain) in &mut chains {
        let (Ok(a), Ok(b)) = (hunters.get(chain.leader), hunters.get(chain.follower)) else {
            for &l in &chain.links {
                commands.entity(l).despawn();
            }
            commands.entity(entity).despawn();
            continue;
        };
        let chain = &mut *chain;
        let (start, end) = (hand(a.rig()), nape(b.rig()));
        rope(&mut chain.points, &mut chain.previous, start, Some(end), LENGTH, &|p| ground.at(p), dt);
        // Its links along it, as far apart as they are long (or spread
        // out, drawn tighter than it is long), each square to the last.
        let lengths: Vec<f32> = chain.points.windows(2).map(|w| w[0].distance(w[1])).collect();
        let total: f32 = lengths.iter().sum();
        let n = chain.links.len();
        for (k, &link) in chain.links.iter().enumerate() {
            let Ok(mut transform) = links.get_mut(link) else { continue };
            let mut s = (k as f32 + 0.5) / n as f32 * total;
            let mut j = 0;
            while j + 1 < lengths.len() && s > lengths[j] {
                s -= lengths[j];
                j += 1;
            }
            let (p, q) = (chain.points[j], chain.points[j + 1]);
            let along = (q - p).normalize_or(Vec3::Y);
            let at = p.lerp(q, (s / lengths[j].max(1e-5)).clamp(0.0, 1.0));
            let roll = if k % 2 == 0 { 0.0 } else { std::f32::consts::FRAC_PI_2 };
            *transform = Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::Z, along) * Quat::from_rotation_z(roll)).with_scale(Vec3::new(1.0, 1.0, LINK.0 / (2.0 * LINK.1)));
        }
    }
}
