//! The ground lab in the game (`--ground NAME`): a patch of one of
//! worldgen's kinds of ground (see its `grounds.rs`), standing on the lab's
//! flat ground; solid, to walk and tether on. Its capture tour:
//! `tools/grounds.ps1`.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
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
        embedded_asset!(app, "core_glow.wgsl");
        app.add_plugins(MaterialPlugin::<CoreMaterial>::default()).insert_resource(LabGround(Arc::new(patch))).add_systems(Startup, build);
    }
}

/// What glows from within (the void's cores; see `core_glow.wgsl`).
type CoreMaterial = ExtendedMaterial<StandardMaterial, CoreGlow>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
struct CoreGlow {
    /// x: the light between pulses, y: a pulse's peak, z: their speed up
    /// (m/s), w: how far apart (m).
    #[uniform(100)]
    light: Vec4,
    /// x: the core's own tone; y: how the steady light gathers to a groove's
    /// middle height (its spread, of the groove's height); z: a pulse's
    /// tail (m), w: its head (m).
    #[uniform(101)]
    surface: Vec4,
    /// The grooves: x the lowest's floor (m), y how far apart, z how high;
    /// w a lit lamp's light.
    #[uniform(102)]
    grooves: Vec4,
}

impl MaterialExtension for CoreGlow {
    fn fragment_shader() -> ShaderRef {
        "embedded://game/core_glow.wgsl".into()
    }
}

/// The void's lamps: their glowing parts' light (as the cores'), the light
/// they shine (lumens), how far (m) (`--set lamp_glow`, `lamp_light`,
/// `lamp_range`).
const LAMP: (f32, f32, f32) = (1500.0, 2.0e6, 30.0);
/// The void's metal: how metallic, how rough, its tones brightened so much.
const VOID_METAL: (f32, f32, f32) = (1.0, 0.5, 4.0);

/// The patch's meshes, each with its collider.
#[allow(clippy::too_many_arguments)]
fn build(
    mut commands: Commands,
    args: Res<Args>,
    ground: Res<LabGround>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cores: ResMut<Assets<CoreMaterial>>,
) {
    // (White, its pieces' tones in their vertices' colours: each its own,
    // as the plates.)
    // (The void's metal, the user: its cylinders are a machine's, not
    // stone; its colour the tones brightened so much, a metal's being its
    // reflection's. Rougher than 0.5, dull; smoother, glare. Others matte;
    // any overridden by `--set metal=… rough=… metal_gain=…`. The walkways'
    // tops once rougher and darker, a walked face: too subtle to look meant.)
    let void = args.ground.as_deref().is_some_and(|g| g.starts_with("void"));
    let (metal, rough, gain) = if void { VOID_METAL } else { (0.0, 0.9, 1.0) };
    let (metal, rough, gain) = (args.num("metal", metal), args.num("rough", rough), args.num("metal_gain", gain));
    let material = materials.add(StandardMaterial { base_color: Color::linear_rgb(gain, gain, gain), metallic: metal, perceptual_roughness: rough, reflectance: 0.3, ..default() });
    // (Dim: the machine glimpsed, never lit up. Light as the structures'
    // seams is, against the camera's exposure: theirs 3000. Matte and dark:
    // glossy, a low sun shone off it and the wall seemed lit, not what is in
    // it.)
    let core = cores.add(ExtendedMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.1, ..default() },
        extension: CoreGlow {
            light: Vec4::new(args.num("core_glow", 40.0), args.num("core_pulse", 600.0), args.num("core_speed", 12.0), grounds::VOID_PULSE as f32),
            surface: Vec4::new(args.num("core_tone", 0.03), args.num("core_spread", 0.18), args.num("core_tail", 15.0), args.num("core_head", 1.5)),
            grooves: Vec4::new(grounds::VOID_LEVELS.0 as f32, grounds::VOID_LEVELS.1 as f32, grounds::VOID_GROOVE as f32, args.num("lamp_glow", LAMP.0)),
        },
    });
    let mut triangles = 0;
    for (piece, glows) in ground.0.pieces.iter().map(|p| (p, false)).chain(ground.0.glows.iter().map(|p| (p, true))) {
        triangles += piece.indices.len() / 3;
        let vertices: Vec<Vec3> = piece.positions.iter().map(|&p| Vec3::from(p)).collect();
        let tris: Vec<[u32; 3]> = piece.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        let collider = Collider::try_trimesh_with_config(vertices, tris, TrimeshFlags::MERGE_DUPLICATE_VERTICES | TrimeshFlags::DELETE_DEGENERATE_TRIANGLES);
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, piece.positions.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, piece.normals.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, piece.tones.iter().map(|&t| [t, t, t, 1.0]).collect::<Vec<[f32; 4]>>())
            .with_inserted_indices(Indices::U32(piece.indices.clone()));
        let mut e = commands.spawn((Mesh3d(meshes.add(mesh)), Transform::IDENTITY));
        if glows {
            e.insert(MeshMaterial3d(core.clone()));
        } else {
            e.insert(MeshMaterial3d(material.clone()));
        }
        if let Ok(c) = collider {
            e.insert((RigidBody::Static, c));
        }
    }
    // The lamps' light (no shadows, yet).
    for &p in &ground.0.lights {
        commands.spawn((PointLight { intensity: args.num("lamp_light", LAMP.1), range: args.num("lamp_range", LAMP.2), shadow_maps_enabled: false, ..default() }, Transform::from_translation(Vec3::from(p))));
    }
    info!("ground lab: {} lights", ground.0.lights.len());
    info!("ground lab: {} round ({:.0}, {:.0}), {} meshes, {triangles} triangles", args.ground.as_deref().unwrap_or(""), CENTRE.0, CENTRE.1, ground.0.pieces.len());
}
