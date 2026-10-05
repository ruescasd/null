//! Keeps things placed once in the world (structures, sites) at whichever of
//! their wrapped copies is nearest the camera.

use bevy::prelude::*;

use crate::{
    camera::FlyCam,
    terrain::{StreamSet, WorldGen},
};

pub struct LandmarksPlugin;

impl Plugin for LandmarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, follow_wrap.after(StreamSet));
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
