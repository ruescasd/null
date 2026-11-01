//! A second body echoing an authored creature's (`spawn NAME echo`): its
//! model again, beside it, a moment behind (the user, 2026-10-31: several
//! spawned in one place, almost merged, read as one creature with two
//! identities, or a creature and its shadow). Only its look: one mind, one
//! body the shots strike, one death. Every frame it takes the body's pose
//! (each joint as it was turned, its heading, its weapon's bloom) from
//! `echo_lag` s before, so it walks, strikes, flinches and falls as the body
//! did, that much later; it stands where the body stands now, `echo_off` m
//! off to one side and behind, drifting. (Offset and lag first, to see if
//! it is workable; its own will, and more, later: the user.) It casts no
//! shadow: a few centimetres off the body, its shadow added nothing, and
//! cost (the user).

use std::collections::VecDeque;

use bevy::{light::NotShadowCaster, prelude::*};

use super::{Body, Kits, grip};

/// How far behind it is (s), and how far off (m), unless given.
pub(super) const LAG: f32 = 0.07;
pub(super) const OFF: f32 = 0.085;

#[derive(Component)]
pub(super) struct Echo {
    /// The body it echoes.
    source: Entity,
    lag: f32,
    off: f32,
    /// Each of the body's named parts and its own, once both are in.
    pairs: Vec<(Entity, Entity)>,
    /// Its weapon, once in its hand.
    weapon: Option<Entity>,
    /// The body as it was, lately: when, its turn, each part's, its bloom.
    past: VecDeque<(f32, Quat, Vec<Transform>, f32)>,
}

impl Echo {
    pub(super) fn new(source: Entity, lag: f32, off: f32) -> Self {
        Self { source, lag: lag.max(0.0), off, pairs: Vec::new(), weapon: None, past: VecDeque::new() }
    }
}

/// Each echo posed as its body was `lag` s before, beside where it is; gone
/// with it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn follow(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<crate::Args>,
    kits: Res<Kits>,
    bodies: Query<(&Body, &Transform), Without<Echo>>,
    mut echoes: Query<(Entity, &mut Echo, &mut Transform), Without<Body>>,
    mut parts: Query<&mut Transform, (Without<Echo>, Without<Body>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    shadowed: Query<(), (With<Mesh3d>, Without<NotShadowCaster>)>,
    globals: Query<&GlobalTransform>,
    mut maces: Query<&mut super::super::mace::Mace>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs();
    for (entity, mut echo, mut placed) in &mut echoes {
        let Ok((body, at)) = bodies.get(echo.source) else {
            commands.entity(entity).despawn();
            continue;
        };
        // (Its model's pieces, and its weapon's, as they come in.)
        for e in children.iter_descendants(entity).filter(|&e| shadowed.contains(e)) {
            commands.entity(e).insert(NotShadowCaster);
        }
        // (Both the same model: their named parts in the same order.)
        if echo.pairs.is_empty() {
            let named = |root: Entity| children.iter_descendants(root).filter(|&e| names.contains(e)).collect::<Vec<_>>();
            let (from, to) = (named(echo.source), named(entity));
            if from.is_empty() || from.len() != to.len() {
                continue;
            }
            for (&a, &b) in from.iter().zip(&to) {
                assert_eq!(names.get(a).ok(), names.get(b).ok(), "an echo's model unlike its body's");
            }
            echo.pairs = from.into_iter().zip(to).collect();
        }
        let pose: Vec<Transform> = echo.pairs.iter().map(|&(a, _)| parts.get(a).map_or(Transform::IDENTITY, |t| *t)).collect();
        echo.past.push_back((now, at.rotation, pose, body.bloom));
        // As it was `lag` before: between the two nearest.
        let then = now - echo.lag;
        while echo.past.len() > 2 && echo.past[1].0 <= then {
            echo.past.pop_front();
        }
        let (t0, r0, b0) = (echo.past[0].0, echo.past[0].1, echo.past[0].3);
        let (t1, r1, b1) = echo.past.get(1).map_or((t0, r0, b0), |p| (p.0, p.1, p.3));
        let f = if t1 > t0 { ((then - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 0.0 };
        for (k, &(_, b)) in echo.pairs.iter().enumerate() {
            let (p0, p1) = (echo.past[0].2[k], echo.past.get(1).map_or(echo.past[0].2[k], |p| p.2[k]));
            if let Ok(mut t) = parts.get_mut(b) {
                t.translation = p0.translation.lerp(p1.translation, f);
                t.rotation = p0.rotation.slerp(p1.rotation, f);
                t.scale = p0.scale.lerp(p1.scale, f);
            }
        }
        // Where the body stands, off to one side and behind, drifting a
        // little.
        let turn = r0.slerp(r1, f);
        let a = -2.3 + 0.5 * (now * 0.23).sin();
        placed.translation = at.translation + turn * Vec3::new(a.cos(), 0.0, a.sin()) * echo.off;
        placed.rotation = turn;
        placed.scale = at.scale;
        // Its weapon, as the body's, bloomed as it was.
        let spec = &kits.0[body.kit].spec;
        if !spec.mace {
            continue;
        }
        let bloom = b0 + (b1 - b0) * f;
        match echo.weapon {
            Some(w) => {
                if let Ok(mut m) = maces.get_mut(w)
                    && m.bloom != bloom
                {
                    m.bloom = bloom;
                }
            }
            None => {
                let Some(j) = &body.joints else { continue };
                let Some(&(_, hand)) = echo.pairs.iter().find(|(a, _)| *a == j.arms[1][2]) else { continue };
                let Ok(g) = globals.get(hand) else { continue };
                let scale = g.to_scale_rotation_translation().0.x;
                if scale > 0.0 {
                    echo.weapon = Some(super::super::mace::spawn(&mut commands, &mut meshes, &mut materials, hand, grip(&args, scale)));
                }
            }
        }
    }
}
