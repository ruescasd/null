//! The ground lab in the game (`--ground NAME`): a patch of one of
//! worldgen's kinds of ground (see its `grounds.rs`), standing on the lab's
//! flat ground; solid, to walk and tether on. Its capture tour:
//! `tools/grounds.ps1`.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use worldgen::grounds::{self, Patch};

use crate::Args;

/// Where the patch stands (its middle), whatever the camera.
pub const CENTRE: (f64, f64) = (8192.0, 8192.0);

/// The patch, for what needs its height (where you start, the camera).
#[derive(Resource)]
pub struct LabGround(pub Arc<Patch>);

impl LabGround {
    /// Its height at (`x`, `z`), as built (0 off it).
    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.0.height(x as f64, z as f64) as f32
    }
}

pub struct GroundLabPlugin;

impl Plugin for GroundLabPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<Args>();
        let Some(name) = args.ground.clone() else { return };
        let patch = grounds::build(&name, CENTRE, args.seed).unwrap_or_else(|e| panic!("--ground: {e}"));
        app.insert_resource(LabGround(Arc::new(patch))).add_systems(Startup, build);
    }
}

/// The patch's meshes, each with its collider.
fn build(mut commands: Commands, args: Res<Args>, ground: Res<LabGround>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // (White, its pieces' tones in their vertices' colours: each its own,
    // as the plates.)
    let material = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, reflectance: 0.3, ..default() });
    let mut triangles = 0;
    for piece in &ground.0.pieces {
        triangles += piece.indices.len() / 3;
        let vertices: Vec<Vec3> = piece.positions.iter().map(|&p| Vec3::from(p)).collect();
        let tris: Vec<[u32; 3]> = piece.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        let collider = Collider::try_trimesh_with_config(vertices, tris, TrimeshFlags::MERGE_DUPLICATE_VERTICES | TrimeshFlags::DELETE_DEGENERATE_TRIANGLES);
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, piece.positions.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, piece.normals.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, piece.tones.iter().map(|&t| [t, t, t, 1.0]).collect::<Vec<[f32; 4]>>())
            .with_inserted_indices(Indices::U32(piece.indices.clone()));
        let mut e = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), Transform::IDENTITY));
        if let Ok(c) = collider {
            e.insert((RigidBody::Static, c));
        }
    }
    info!("ground lab: {} round ({:.0}, {:.0}), {} meshes, {triangles} triangles", args.ground.as_deref().unwrap_or(""), CENTRE.0, CENTRE.1, ground.0.pieces.len());
}
