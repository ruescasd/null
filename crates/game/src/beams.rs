//! The canals' beams: over each canal that is on, a ray of light from one
//! station to the next, a few hundred metres up. Seen from afar, they give
//! the land a direction and a scale, and lead to stations (the user: beams
//! are a reference, of distance and of direction, and they look well lined
//! up with distant architecture). Pure light, unexplained.
//!
//! A canal is off until the player turns it on (how is open), and an off
//! canal has no beam at all: off the lit network you should feel lost, even
//! afraid, the user. `--opt canals_on` turns every canal on.
//!
//! (Born of a planetary grid of filaments at 8 km, crossing square: from
//! below, with no parallax as you walk, it read as a skybox, and not an
//! attractive one, the user; set low by hand, it became these. It is kept,
//! for its looks, in samples/shaders/grid.)

use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

use crate::{
    Args,
    landmarks::Landmark,
    look::Gloom,
    terrain::{PLANET_RADIUS, WorldGen},
};

/// How far apart a beam's points are (m): it bends with the planet between
/// them.
const STEP: f64 = 50.0;

pub struct BeamsPlugin;

impl Plugin for BeamsPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<Args>();
        if args.opt("chasm") || args.opt("lab") {
            return;
        }
        embedded_asset!(app, "beams.wgsl");
        app.add_plugins(MaterialPlugin::<Beams>::default()).add_systems(Startup, setup).add_systems(Update, (show, darken));
    }
}

/// Which canals are on (by their index in the world's canals).
#[derive(Resource)]
pub struct CanalsOn(pub Vec<bool>);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct Beams {
    /// x: thickness (m), y: least width drawn (px), z: curvature 1 / 2R.
    #[uniform(0)]
    shape: Vec4,
    /// x: brightness, y: how far the haze lets them be seen (m), z: how
    /// much is left of the sky.
    #[uniform(1)]
    light: Vec4,
}

impl Material for Beams {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/beams.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/beams.wgsl".into()
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

/// The beam over canal `.0`.
#[derive(Component)]
struct Beam(usize);

#[derive(Resource)]
struct BeamMaterial(Handle<Beams>);

fn setup(mut commands: Commands, args: Res<Args>, world: Res<WorldGen>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<Beams>>) {
    let canals = &world.0.canals;
    commands.insert_resource(CanalsOn(vec![args.opt("canals_on"); canals.lines.len()]));
    let material = materials.add(Beams {
        shape: Vec4::new(args.num("beam_width", 6.0), args.num("beam_px", 2.0), 0.5 / PLANET_RADIUS, 0.0),
        // (Light sinks into the haze more slowly than what it lights: seen
        // further, by `beam_haze`.)
        light: Vec4::new(args.num("beam_glow", 0.6), args.num("fog", 4500.0) * args.num("beam_haze", 1.6), 1.0, 0.0),
    });
    commands.insert_resource(BeamMaterial(material.clone()));
    let height = args.num("beam_height", 350.0) as f64;
    let field = |c: bevy::math::DVec2| world.0.canal_floor(c);
    for (i, canal) in canals.lines.iter().enumerate() {
        // From above one station to above the other, straight.
        let (a, b) = canals.ends(i);
        let (ya, yb) = (canals.station_floor(canal.a, &field) + height, canals.station_floor(canal.b, &field) + height);
        let (from, to) = (Vec3::new(a.x as f32, ya as f32, a.y as f32), Vec3::new(b.x as f32, yb as f32, b.y as f32));
        let middle = (from + to) * 0.5;
        let along = (to - from).normalize();
        let n = ((b - a).length() / STEP).ceil().max(1.0) as u32;
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        for k in 0..=n {
            let p = from.lerp(to, k as f32 / n as f32) - middle;
            for side in [-1.0, 1.0] {
                positions.push(p.to_array());
                uvs.push([side, 0.0]);
            }
            if k < n {
                let v = 2 * k;
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
            Beam(i),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(middle),
            Landmark { origin: middle },
            Visibility::Hidden,
            // (Bent down in the shader: its bounds would not hold.)
            NoFrustumCulling,
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// A beam shows while its canal is on.
fn show(on: Res<CanalsOn>, mut beams: Query<(&Beam, &mut Visibility)>) {
    if !on.is_changed() {
        return;
    }
    for (beam, mut visibility) in &mut beams {
        *visibility = if on.0.get(beam.0).copied().unwrap_or(false) { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// The swarm's darkness puts them out with the sky.
fn darken(gloom: Res<Gloom>, material: Res<BeamMaterial>, mut materials: ResMut<Assets<Beams>>) {
    let left = (1.0 - gloom.0) * (1.0 - gloom.0);
    // (Changed only when it changes: each change uploads it again.)
    if materials.get(&material.0).is_some_and(|m| m.light.z != left)
        && let Some(mut m) = materials.get_mut(&material.0)
    {
        m.light.z = left;
    }
}
