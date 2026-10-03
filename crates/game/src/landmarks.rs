//! Keeps things placed once in the world (structures, sites) at whichever of
//! their wrapped copies is nearest the camera, and receives the ones built
//! in the background. Also the distance-field fractal prototypes, for
//! comparison.

use avian3d::prelude::{Collider, RigidBody};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};
use worldgen::{
    ColumnMesh,
    fractal::{Kifs, build},
};

use crate::{
    camera::FlyCam,
    terrain::{StreamSet, TerrainMaterialHandle, WorldGen, column_collider, to_bevy_mesh},
};

pub struct LandmarksPlugin;

impl Plugin for LandmarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_fractals)
            .add_systems(Update, (receive_fractals, follow_wrap.after(StreamSet)));
    }
}

/// Something placed once in the world, kept at whichever of its wrapped
/// copies is nearest the camera.
#[derive(Component)]
pub struct Landmark {
    pub origin: Vec3,
}

fn follow_wrap(
    world: Res<WorldGen>,
    camera: Single<&Transform, (With<FlyCam>, Without<Landmark>)>,
    mut landmarks: Query<(&Landmark, &mut Transform)>,
) {
    let size = world.size();
    let cam = camera.translation;
    for (landmark, mut transform) in &mut landmarks {
        let nearest = |o: f32, c: f32| o + ((c - o) / size).round() * size;
        transform.translation.x = nearest(landmark.origin.x, cam.x);
        transform.translation.z = nearest(landmark.origin.z, cam.z);
    }
}

#[derive(Component)]
pub struct FractalTask(pub Task<(ColumnMesh, Option<Collider>)>);

/// The first fractal prototypes, meshed from distance fields (soft edges,
/// heavy, fidgety collision); only with `--opt sdf_fractals`, for comparison.
fn spawn_fractals(mut commands: Commands, world: Res<WorldGen>, args: Res<crate::Args>) {
    if !args.opt("sdf_fractals") {
        return;
    }
    let pool = AsyncComputeTaskPool::get();
    // A low, massive block with unfamiliar fold offsets...
    let block = Kifs {
        size: Vec3::new(200.0, 110.0, 160.0),
        taper: 0.12,
        iterations: 3,
        scale: 3.0,
        offset: Vec3::new(1.0, 0.85, 0.7),
        twist: Mat3::from_euler(EulerRot::YXZ, 0.12, 0.0, 0.18),
    };
    // ...and a tall twisted spire.
    let spire = Kifs {
        size: Vec3::new(80.0, 380.0, 80.0),
        taper: 0.55,
        iterations: 3,
        scale: 3.0,
        offset: Vec3::ONE,
        twist: Mat3::from_euler(EulerRot::YXZ, 0.25, 0.08, 0.04),
    };
    let (bx, bz) = (1420.0, 620.0);
    let (sx, sz) = (1150.0, 560.0);
    let block_at = Vec3::new(bx, world.ground_height(bx, bz) - 8.0, bz);
    let spire_at = Vec3::new(sx, world.ground_height(sx, sz) - 5.0, sz);
    let task = pool.spawn(async move {
        let mesh = build(&block, 1.0);
        let collider = column_collider(&mesh);
        (mesh, collider)
    });
    commands.spawn((FractalTask(task), Landmark { origin: block_at }, Transform::from_translation(block_at)));
    let task = pool.spawn(async move {
        let mesh = build(&spire, 1.0);
        let collider = column_collider(&mesh);
        (mesh, collider)
    });
    commands.spawn((FractalTask(task), Landmark { origin: spire_at }, Transform::from_translation(spire_at)));
}

fn receive_fractals(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut FractalTask)>,
    material: Res<TerrainMaterialHandle>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, mut task) in &mut tasks {
        let Some((mesh, collider)) = check_ready(&mut task.0) else { continue };
        info!("fractal ready: {} triangles", mesh.indices.len() / 3);
        let mut e = commands.entity(entity);
        e.remove::<FractalTask>().insert((
            Mesh3d(meshes.add(to_bevy_mesh(mesh))),
            MeshMaterial3d(material.0.clone()),
        ));
        if let Some(collider) = collider {
            e.insert((RigidBody::Static, collider));
        }
    }
}
