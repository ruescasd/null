//! The tether's state and look. Physically it is a grappling hook (the pull is
//! in `walk`); visually it is a bundle of dark twisting strands ending in a
//! black crystalline burr: a single needle in flight that splays into a
//! cluster of shards where it roots into a surface.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use super::Player;
use crate::camera::FlyCam;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tether {
    #[default]
    Idle,
    /// The tip is on its way out.
    Flying { tip: Vec3, dir: Vec3, travelled: f32 },
    /// Rooted at `point` on a surface facing `normal`, pulling.
    Anchored { point: Vec3, normal: Vec3 },
    /// Missed or let go by itself; waits for the button to be released.
    Missed,
}

impl Tether {
    /// Moves world-space positions when the camera wraps around the world.
    pub fn shift(&mut self, by: Vec3) {
        match self {
            Tether::Flying { tip, .. } => *tip += by,
            Tether::Anchored { point, .. } => *point += by,
            _ => {}
        }
    }
}

#[derive(Component)]
pub struct Needle;

#[derive(Component)]
pub struct Burr;

/// A small light carried by the spike, so it lightly picks out the surface
/// around where it roots.
#[derive(Component)]
pub struct SpikeLight;

/// Light output (lumens) of the spike when rooted; dimmer in flight.
const LIGHT_ROOTED: f32 = 6.0e5;
const LIGHT_FLYING: f32 = 5.0e4;

/// Flat-shaded pyramids ("shards") from a common base point, each given as
/// (direction, length, half-width at the base).
fn shard_mesh(shards: &[(Vec3, f32, f32)]) -> Mesh {
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for &(dir, len, width) in shards {
        let dir = dir.normalize();
        let (u, v) = dir.any_orthonormal_pair();
        let tip = dir * len;
        let base = [u * width, v * width, -u * width, -v * width];
        for i in 0..4 {
            let (a, b) = (base[i], base[(i + 1) % 4]);
            let mut n = (b - a).cross(tip - a).normalize();
            let mut tri = [a, b, tip];
            if n.dot(a + b) < 0.0 {
                n = -n;
                tri.swap(0, 1);
            }
            let start = positions.len() as u32;
            for p in tri {
                positions.push(p.to_array());
                normals.push(n.to_array());
            }
            indices.extend_from_slice(&[start, start + 1, start + 2]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Black and glossy, so it is a silhouette that glints where lit.
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.015, 0.015, 0.015),
        perceptual_roughness: 0.2,
        reflectance: 0.9,
        ..default()
    });

    // In flight: a long needle with a short tail, pointing along +Y.
    let needle = shard_mesh(&[(Vec3::Y, 1.1, 0.045), (Vec3::NEG_Y, 0.35, 0.045)]);
    // Rooted: a short core and seven shards splayed around +Y (the surface
    // normal), of uneven length and lean.
    let mut shards = vec![(Vec3::Y, 0.45, 0.06)];
    for i in 0..7 {
        let a = i as f32 / 7.0 * std::f32::consts::TAU + 0.4 * (i % 2) as f32;
        let lean = 0.75 + 0.35 * ((i * 5 % 7) as f32 / 7.0);
        let dir = Vec3::new(a.cos() * lean.sin(), lean.cos(), a.sin() * lean.sin());
        shards.push((dir, 0.55 + 0.75 * ((i * 3 % 7) as f32 / 7.0), 0.035));
    }
    let burr = shard_mesh(&shards);

    commands.spawn((
        Needle,
        Mesh3d(meshes.add(needle)),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.spawn((
        Burr,
        Mesh3d(meshes.add(burr)),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.spawn((
        SpikeLight,
        PointLight {
            intensity: 0.0,
            // A cool, faintly blue-white; the grade makes it grey anyway.
            color: Color::srgb(0.85, 0.9, 1.0),
            range: 25.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default(),
    ));
}

/// Places the needle or burr and draws the strands from the left hand to it.
#[allow(clippy::type_complexity)]
pub fn draw(
    time: Res<Time>,
    camera: Single<(&Transform, &FlyCam, &Player), (Without<Needle>, Without<Burr>, Without<SpikeLight>)>,
    mut needle: Single<(&mut Transform, &mut Visibility), (With<Needle>, Without<Burr>, Without<SpikeLight>)>,
    mut burr: Single<(&mut Transform, &mut Visibility), (With<Burr>, Without<Needle>, Without<SpikeLight>)>,
    mut light: Single<(&mut Transform, &mut PointLight), (With<SpikeLight>, Without<Needle>, Without<Burr>)>,
    mut gizmos: Gizmos,
) {
    let (transform, fly, player) = *camera;
    *needle.1 = Visibility::Hidden;
    *burr.1 = Visibility::Hidden;
    light.1.intensity = 0.0;
    // A slow, slight pulse so the light feels alive rather than a lamp.
    let pulse = 0.85 + 0.15 * (time.elapsed_secs() * 2.3).sin();
    let (end, taut) = match player.tether {
        Tether::Flying { tip, dir, .. } => {
            *needle.0 = Transform::from_translation(tip)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir.normalize()));
            *needle.1 = Visibility::Visible;
            light.0.translation = tip;
            light.1.intensity = LIGHT_FLYING * pulse;
            (tip, false)
        }
        Tether::Anchored { point, normal } => {
            *burr.0 = Transform::from_translation(point)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, normal.normalize()));
            *burr.1 = Visibility::Visible;
            // Slightly off the surface, so it washes the area around the burr.
            light.0.translation = point + normal.normalize() * 4.0;
            light.1.intensity = LIGHT_ROOTED * pulse;
            (point, true)
        }
        _ => return,
    };

    let rotation = fly.rotation();
    let (aim, right, up) = (rotation * Vec3::NEG_Z, rotation * Vec3::X, rotation * Vec3::Y);
    let start = transform.translation + aim * 0.6 - right * 0.28 - up * 0.25;
    let line = end - start;
    let length = line.length();
    if length < 0.1 {
        return;
    }
    let (u, v) = (line / length).any_orthonormal_pair();

    // Six strands twisting around the line, near-black to mid grey; slack
    // and slow in flight, drawn tight and spinning faster while pulling.
    const STRANDS: usize = 6;
    const SEGMENTS: usize = 32;
    let t_now = time.elapsed_secs();
    let (radius, spin) = if taut { (0.05, 6.0) } else { (0.11, 2.0) };
    for s in 0..STRANDS {
        let grey = [0.004, 0.02, 0.05, 0.09, 0.14, 0.22][s];
        let mut prev = start;
        for i in 1..=SEGMENTS {
            let t = i as f32 / SEGMENTS as f32;
            let phase = s as f32 / STRANDS as f32 * std::f32::consts::TAU + t * length * 0.5
                - t_now * spin;
            let swell = (std::f32::consts::PI * t).sin().powf(0.6)
                * (1.0 + 0.3 * (t * 17.0 + s as f32 * 1.7 + t_now * 3.0).sin());
            let offset = (u * phase.cos() + v * phase.sin()) * radius * swell;
            let point = start + line * t + offset;
            gizmos.line(prev, point, LinearRgba::rgb(grey, grey, grey));
            prev = point;
        }
    }
}
