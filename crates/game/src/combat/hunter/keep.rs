//! `keep N KIND[,KIND...]` at the console (the user, 2026-11-15: to watch
//! the walkers find their way about the districts, moving among them): N
//! creatures kept round you, one dropped out of the sky every `every`
//! seconds while there are fewer, somewhere between `near` and `far` m
//! from you, `height` m up, falling as the rig leaps (its pose, its
//! landing); on district ground (`--opt blink`), onto the nodes' walking
//! space, not a pillar's top. Those left more than twice `far` behind are
//! let go (killed where they are, out of sight, their bodies' ways of
//! going), so the number follows you about. `keep off`: no more made, those
//! kept left. A way of arriving to try, too: capsules falling, or
//! teleporting down from ships, the user's thoughts for play.

use bevy::prelude::*;

use super::{Assets3, Heads, Hunter, spawn_hunter};
use crate::navmesh::district::Blink;
use crate::console::Keeping;
use worldgen::noise::hash01;

/// What is being kept, and the creatures made for it.
#[derive(Default)]
pub(in crate::combat) struct Kept {
    asked: Option<Keeping>,
    mine: Vec<Entity>,
    next: f32,
    made: u32,
}

/// A creature just made, to be dropped this far (m) out of the sky.
#[derive(Component)]
pub(in crate::combat) struct Dropped(f32);

#[allow(clippy::too_many_arguments)]
pub(in crate::combat) fn keep(
    mut commands: Commands,
    time: Res<Time>,
    ground: crate::ground::Ground,
    districts: Option<Res<crate::navmesh::district::Districts>>,
    assets: Res<Assets3>,
    heads: Res<Heads>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawns: ResMut<crate::console::Spawns>,
    player: Single<&Transform, With<crate::player::Player>>,
    mut hunters: Query<(&mut Hunter, Option<&Blink>)>,
    mut kept: Local<Kept>,
) {
    if let Some(asked) = spawns.keep.take() {
        kept.asked = asked;
        kept.next = 0.0;
    }
    let you = player.translation;
    let Some(asked) = kept.asked.clone() else {
        kept.mine.clear();
        return;
    };
    // (Those killed gone from the count; those left far behind let go.)
    // (And those that left, hurt with no way to you, gone where they went:
    // let go there, out of sight; see `district::Stranded`.)
    kept.mine.retain(|&e| {
        let Ok((mut h, blink)) = hunters.get_mut(e) else { return false };
        if h.health <= 0.0 {
            return false;
        }
        if (h.rig.root - you).with_y(0.0).length() > asked.far * 2.0 || blink.is_some_and(|b| b.gone()) {
            h.health = 0.0;
            return false;
        }
        true
    });
    let now = time.elapsed_secs();
    if kept.mine.len() as u32 >= asked.count || now < kept.next {
        return;
    }
    kept.next = now + asked.every;
    let nodes = districts.as_deref().and_then(|d| d.nodes.as_deref());
    let n = kept.made;
    kept.made += 1;
    // A spot in the ring round you: on the walking space, where there is
    // district ground built (tried a few times), else the ground under it.
    let r = |k: i32| hash01(n as i32, k, (now * 100.0) as i32, 0x6b3);
    let spot = (0..8).find_map(|k| {
        let a = r(k * 2) * std::f32::consts::TAU;
        let d = asked.near + (asked.far - asked.near) * r(k * 2 + 1);
        let q = you + Vec3::new(a.cos(), 0.0, a.sin()) * d;
        let q = Vec3::new(q.x, ground.at(Vec3::new(q.x, you.y + 200.0, q.z)), q.z);
        match nodes.filter(|nodes| nodes.covers(q)) {
            Some(nodes) => nodes.locate(q, 0.0, None).map(|(_, at)| Vec3::new(at.x, q.y, at.y)),
            None => Some(q),
        }
    });
    let Some(at) = spot else { return };
    let (kind, traits) = &asked.kinds[n as usize % asked.kinds.len()];
    let seed = (now * 1000.0) as u32 + n * 7919;
    let t = std::time::Instant::now();
    let e = spawn_hunter(&mut commands, &ground, &assets, &heads, &mut materials, seed, at, you, Some(kind), traits);
    // (What it cost, for the hitches: see `hitches` in main.rs.)
    let took = t.elapsed().as_secs_f32() * 1000.0;
    if took > 5.0 {
        warn!("keep: a {kind} made in {took:.0} ms");
    }
    commands.entity(e).insert(Dropped(asked.height));
    kept.mine.push(e);
}

/// Each creature just made to drop: lifted, and let fall (the rig's leap,
/// from rest).
pub(in crate::combat) fn drop_in(mut commands: Commands, mut dropped: Query<(Entity, &mut Hunter, &Dropped)>) {
    for (e, mut h, d) in &mut dropped {
        h.rig.root.y += d.0;
        h.rig.leap(Vec3::Z, 0.0, 0.0);
        commands.entity(e).remove::<Dropped>();
    }
}
