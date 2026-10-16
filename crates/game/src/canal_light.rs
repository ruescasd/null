//! The light in a canal's grooves (see worldgen's `canal.rs`): while the
//! canal is on, a steady glow along them, and pulses running the way it
//! flows; off, the grooves are dark lines in the stone. Technology at work,
//! with restraint: the light lives in grooves the geometry cuts (the user:
//! the canals too bare; texture work not out of place, to show some kind of
//! technology or energy at work).
//!
//! A strip per groove per canal, drawn by `canal_light.wgsl`; it is shown
//! while its canal is on (see `beams::CanalsOn`).

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    math::DVec2,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use worldgen::canal::{GROOVE_DEPTH, GROOVE_HALF, GROOVES, PIPE_RADIUS};

use crate::{
    Args,
    beams::CanalsOn,
    landmarks::Landmark,
    look::Gloom,
    terrain::{PLANET_RADIUS, WorldGen},
};

/// How far apart a strip's points are (m).
const STEP: f64 = 8.0;
/// How far above its groove's floor a strip lies (m).
const LIFT: f64 = 0.02;

pub struct CanalLightPlugin;

impl Plugin for CanalLightPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<Args>();
        if args.opt("chasm") || args.opt("lab") {
            return;
        }
        embedded_asset!(app, "canal_light.wgsl");
        app.add_plugins(MaterialPlugin::<CanalLight>::default())
            .add_systems(Startup, setup)
            .add_systems(Update, (show, darken));
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct CanalLight {
    /// x: curvature 1 / 2R, y: how far it is seen (m).
    #[uniform(0)]
    shape: Vec4,
    /// x: steady glow, y: a pulse's peak, z: pulses' speed (m/s), w: how
    /// far apart they are (m).
    #[uniform(1)]
    light: Vec4,
    /// x: a pulse's length (m), y: how much is left of the sky.
    #[uniform(2)]
    extra: Vec4,
}

impl Material for CanalLight {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/canal_light.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/canal_light.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// The light in canal `.0`'s grooves.
#[derive(Component)]
struct Strips(usize);

#[derive(Resource)]
struct LightMaterial(Handle<CanalLight>);

fn setup(mut commands: Commands, args: Res<Args>, world: Res<WorldGen>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<CanalLight>>) {
    let canals = &world.0.canals;
    // (A faint steady glow, the pulses carrying it: at 0.2 or more it was
    // as white as a pulse once tonemapped, and the pulses were lost.)
    let material = materials.add(CanalLight {
        shape: Vec4::new(0.5 / PLANET_RADIUS, args.num("groove_reach", 600.0), 0.0, 0.0),
        light: Vec4::new(args.num("groove_glow", 0.04), args.num("pulse_glow", 3.0), args.num("pulse_speed", 45.0), args.num("pulse_gap", 160.0)),
        extra: Vec4::new(args.num("pulse_tail", 12.0), 1.0, 0.0, 0.0),
    });
    commands.insert_resource(LightMaterial(material.clone()));
    let field = |c: DVec2| world.0.canal_floor(c);
    for (i, canal) in canals.lines.iter().enumerate() {
        let track = canals.track(i, STEP, &field);
        let across = canals.across(i);
        let middle = track[track.len() / 2];
        let origin = Vec3::new(middle.0.x as f32, middle.2 as f32, middle.0.y as f32);
        let along = canals.ends(i).1 - canals.ends(i).0;
        let along = Vec3::new(along.x as f32, 0.0, along.y as f32).normalize();
        let (mut positions, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new());
        // In each groove, a strip across its floor (a little in from its
        // sides), at each point along the pipe.
        let half = GROOVE_HALF / PIPE_RADIUS * 0.85;
        let radius = PIPE_RADIUS + GROOVE_DEPTH - LIFT;
        for g in GROOVES {
            let first = positions.len() as u32;
            for &(at, s, floor) in &track {
                for (side, phi) in [(-1.0, g - half), (1.0, g + half)] {
                    let p = at + across * (radius * phi.sin());
                    let y = floor + PIPE_RADIUS - radius * phi.cos();
                    positions.push((Vec3::new(p.x as f32, y as f32, p.y as f32) - origin).to_array());
                    uvs.push([side, (s * canal.flow) as f32]);
                }
            }
            for k in 0..track.len() as u32 - 1 {
                let v = first + 2 * k;
                indices.extend_from_slice(&[v, v + 1, v + 3, v, v + 3, v + 2]);
            }
        }
        let normals = vec![along.to_array(); positions.len()];
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_indices(Indices::U32(indices));
        commands.spawn((
            Strips(i),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(origin),
            Landmark { origin },
            Visibility::Hidden,
            // (Bent down in the shader: its bounds would not hold.)
            NoFrustumCulling,
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// The light shows while its canal is on.
fn show(on: Res<CanalsOn>, mut strips: Query<(&Strips, &mut Visibility)>) {
    if !on.is_changed() {
        return;
    }
    for (strip, mut visibility) in &mut strips {
        *visibility = if on.0.get(strip.0).copied().unwrap_or(false) { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// The swarm's darkness puts it out.
fn darken(gloom: Res<Gloom>, material: Res<LightMaterial>, mut materials: ResMut<Assets<CanalLight>>) {
    let left = (1.0 - gloom.0) * (1.0 - gloom.0);
    // (Changed only when it changes: each change uploads it again.)
    if materials.get(&material.0).is_some_and(|m| m.extra.y != left)
        && let Some(mut m) = materials.get_mut(&material.0)
    {
        m.extra.y = left;
    }
}
