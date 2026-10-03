//! Spawns the world's megastructures and keeps each one at whichever of its
//! wrapped copies is nearest the camera. Also the fractal prototypes: built
//! in the background at startup, standing in the open near the spawn point.

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
        app.add_systems(PostStartup, (spawn, spawn_fractals))
            .add_systems(Update, (receive_fractals, follow_wrap.after(StreamSet)));
    }
}

#[derive(Component)]
struct Landmark {
    origin: Vec3,
}

fn spawn(
    mut commands: Commands,
    world: Res<WorldGen>,
    args: Res<crate::Args>,
    material: Res<TerrainMaterialHandle>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Off by default until they look right; `--opt landmarks` shows them.
    if !args.opt("landmarks") {
        return;
    }
    let ground = |x: f32, z: f32| world.ground_height(x, z);
    let district = |x: f32, z: f32| world.district(x, z);
    for landmark in worldgen::landmarks::place(world.size(), args.seed, ground, district) {
        info!("{} at {:.0}, {:.0}", landmark.kind, landmark.origin.x, landmark.origin.z);
        let collider = column_collider(&landmark.mesh);
        let mut entity = commands.spawn((
            Landmark { origin: landmark.origin },
            Mesh3d(meshes.add(to_bevy_mesh(landmark.mesh))),
            MeshMaterial3d(material.0.clone()),
            Transform::from_translation(landmark.origin),
        ));
        if let Some(collider) = collider {
            entity.insert((RigidBody::Static, collider));
        }
    }
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
struct FractalTask(Task<(ColumnMesh, Option<Collider>)>);

/// The fractal prototypes: a KIFS block and a KIFS spire, near the
/// default spawn point (`--opt nofractals` leaves them out).
fn spawn_fractals(mut commands: Commands, world: Res<WorldGen>, args: Res<crate::Args>) {
    if args.opt("nofractals") {
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
