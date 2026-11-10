//! An authored creature's shade, its teleports' look on district ground
//! (`--opt blink`; the user, 2026-11-04: derived from the echo, its dark
//! silhouette splits from it, goes to where it lands as a dash goes, in a
//! straight line, and it reforms there; being shadow, it goes through
//! whatever is in the way). Its model again, all black and unlit. It leaves
//! the body where it stands (no tell, unless `blink_tell` is given: then it
//! comes out of it toward where it will land as it tells), and goes along
//! the teleport's line facing its way, still, in the body's pose as it left
//! (it need not walk, the user); the body stays behind, held still in its
//! stride, till the shade is there (the user: the body
//! should remain until the shadow reaches its destination). Then the body
//! is there, and the shade lingers on it a moment (`REFORM`). Otherwise it
//! is hidden. One mind, one body the shots strike: the one left behind.

use bevy::{light::NotShadowCaster, prelude::*};

use super::{Body, Kits};
use crate::navmesh::district::Blink;

/// How far its shade comes out of it as it tells (m), and how long it
/// lingers on it, landed (s).
const SPLIT: f32 = 0.6;
const REFORM: f32 = 0.25;

#[derive(Component)]
pub(super) struct Shade {
    /// The body it is the shade of, and its hunter.
    source: Entity,
    hunter: Entity,
    /// Each of the body's named parts and its own, once both are in.
    pairs: Vec<(Entity, Entity)>,
    /// Whether it glided last frame, and how long yet it lingers, landed.
    glided: bool,
    reform: f32,
}

impl Shade {
    pub(super) fn new(source: Entity, hunter: Entity) -> Self {
        Self { source, hunter, pairs: Vec::new(), glided: false, reform: 0.0 }
    }
}

/// Its look: black, unlit.
#[derive(Resource)]
pub(super) struct Black(Handle<StandardMaterial>);

pub(super) fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(Black(materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, ..default() })));
}

/// Each shade posed as its body is, placed and shown (or not) by what its
/// hunter's teleport is doing; gone with it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn follow(
    mut commands: Commands,
    time: Res<Time>,
    black: Res<Black>,
    kits: Res<Kits>,
    blinks: Query<&Blink>,
    bodies: Query<(&Transform, &Body), Without<Shade>>,
    mut shades: Query<(Entity, &mut Shade, &mut Transform, &mut Visibility), Without<Body>>,
    mut parts: Query<&mut Transform, (Without<Shade>, Without<Body>)>,
    children: Query<&Children>,
    names: Query<&Name>,
    painted: Query<(Entity, &MeshMaterial3d<StandardMaterial>)>,
) {
    let dt = time.delta_secs();
    for (entity, mut shade, mut placed, mut shown) in &mut shades {
        let Ok((at, body)) = bodies.get(shade.source) else {
            commands.entity(entity).despawn();
            continue;
        };
        let spec = &kits.0[body.kit].spec;
        // (Its model's pieces, as they come in: black, casting no shadow.)
        for e in children.iter_descendants(entity) {
            if let Ok((e, m)) = painted.get(e)
                && m.0 != black.0
            {
                commands.entity(e).insert((MeshMaterial3d(black.0.clone()), NotShadowCaster));
            }
        }
        // (Both the same model: their named parts in the same order.)
        if shade.pairs.is_empty() {
            let named = |root: Entity| children.iter_descendants(root).filter(|&e| names.contains(e)).collect::<Vec<_>>();
            let (from, to) = (named(shade.source), named(entity));
            if from.is_empty() || from.len() != to.len() {
                continue;
            }
            shade.pairs = from.into_iter().zip(to).collect();
        }
        let blink = blinks.get(shade.hunter).ok();
        let glide = blink.and_then(|b| b.glide());
        let splitting = blink.and_then(|b| b.splitting());
        if shade.glided && glide.is_none() {
            shade.reform = REFORM;
        }
        shade.glided = glide.is_some();
        shade.reform -= dt;
        let want = if glide.is_some() || splitting.is_some() || shade.reform > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *shown != want {
            *shown = want;
        }
        // Where it is: along its line, facing its way, gliding; out of the
        // body toward where it lands, telling; on it, lingering.
        match (glide, splitting) {
            (Some((spot, way)), _) => *placed = super::placed(spec, spot, way.x.atan2(way.z)),
            (None, Some((land, f))) => {
                placed.translation = at.translation + (land - at.translation).with_y(0.0).normalize_or_zero() * SPLIT * f * f * (3.0 - 2.0 * f);
                placed.rotation = at.rotation;
                placed.scale = at.scale;
            }
            (None, None) if shade.reform > 0.0 => *placed = *at,
            (None, None) => continue,
        }
        for &(a, b) in &shade.pairs {
            let Ok(pose) = parts.get(a).copied() else { continue };
            if let Ok(mut t) = parts.get_mut(b) {
                *t = pose;
            }
        }
    }
}
