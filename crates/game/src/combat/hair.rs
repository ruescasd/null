//! Hair (`--opt hair`): thin strands hanging from round the back of every
//! swarmer with a form (and the lab's), swinging as it moves, streaming out
//! behind it when it flies, settling when it stops: free-hanging ropes, the
//! case simple rope physics does well.

use bevy::{asset::RenderAssetUsages, camera::visibility::NoFrustumCulling, mesh::PrimitiveTopology, prelude::*};
use worldgen::noise::hash01;

use super::{
    forms::Gazed,
    hunter::cables::{rope, tubes},
};
use crate::{Args, terrain::WorldGen};

/// Strands on each, how far out round its back they hang from (metres), and
/// their length.
const STRANDS: usize = 10;
const RIM: f32 = 0.25;
const LENGTH: (f32, f32) = (0.6, 1.5);

/// Something's hair: each strand where it hangs from (in its owner's space),
/// its length, its points, and its shade.
#[derive(Component)]
pub(super) struct Hair {
    owner: Entity,
    strands: Vec<(Vec3, f32, Vec<Vec3>, Vec<Vec3>, f32)>,
    mesh: Handle<Mesh>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn hair(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    world: Res<WorldGen>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material: Local<Option<Handle<StandardMaterial>>>,
    added: Query<(Entity, &Transform), Added<Gazed>>,
    owners: Query<&Transform, With<Gazed>>,
    mut hair: Query<(Entity, &mut Hair)>,
) {
    if !args.opt("hair") {
        return;
    }
    let dt = time.delta_secs().clamp(0.001, 1.0 / 30.0);
    let material = material
        .get_or_insert_with(|| materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.3, reflectance: 0.6, ..default() }))
        .clone();
    let draw = |strands: &[(Vec3, f32, Vec<Vec3>, Vec<Vec3>, f32)], mesh: &mut Mesh| {
        tubes(mesh, strands.iter().map(|(_, _, points, _, shade)| (&points[..], (0.012, 0.003), *shade)));
    };

    for (owner, transform) in &added {
        let seed = owner.index_u32() as i32;
        let mut strands = Vec::new();
        for k in 0..STRANDS {
            let r = |j: i32| hash01(seed, k as i32, j, 0x6d1);
            let a = (k as f32 + r(0) * 0.6) / STRANDS as f32 * std::f32::consts::TAU;
            // Round the back of its rim, a little behind it.
            let from = Vec3::new(a.cos() * RIM, a.sin() * RIM, 0.12 + r(1) * 0.08);
            let length = LENGTH.0 + (LENGTH.1 - LENGTH.0) * r(2);
            let shade = if k % 4 == 1 { 0.5 } else { 0.03 + r(3) * 0.04 };
            let (mut points, mut previous) = (Vec::new(), Vec::new());
            rope(&mut points, &mut previous, transform.transform_point(from), None, length, &world, dt);
            strands.push((from, length, points, previous, shade));
        }
        // (Built whole from the start: the renderer does not take an empty
        // mesh growing.)
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        draw(&strands, &mut mesh);
        let mesh = meshes.add(mesh);
        commands.spawn((
            Hair { owner, strands, mesh: mesh.clone() },
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            Visibility::default(),
            NoFrustumCulling,
        ));
    }

    for (entity, mut h) in &mut hair {
        let Ok(transform) = owners.get(h.owner) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Hair { strands, mesh, .. } = &mut *h;
        for (from, length, points, previous, _) in strands.iter_mut() {
            rope(points, previous, transform.transform_point(*from), None, *length, &world, dt);
        }
        if let Some(mut m) = meshes.get_mut(&*mesh) {
            draw(strands, &mut m);
        }
    }
}
