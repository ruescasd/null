//! Spawns the world's megastructures and keeps each one at whichever of its
//! wrapped copies is nearest the camera.

use avian3d::prelude::RigidBody;
use bevy::prelude::*;

use crate::{
    camera::FlyCam,
    terrain::{StreamSet, TerrainMaterialHandle, WorldGen, column_collider, to_bevy_mesh},
};

pub struct LandmarksPlugin;

impl Plugin for LandmarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn)
            .add_systems(Update, follow_wrap.after(StreamSet));
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
    if args.opt("nolandmarks") {
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
