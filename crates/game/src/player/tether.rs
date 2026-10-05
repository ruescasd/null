//! The tether's state and look. Physically it is a grappling hook (the pull is
//! in `walk`); visually it is a bundle of dark twisting strands ending in a
//! black crystalline burr: a single needle in flight that splays into a
//! cluster of shards where it roots into a surface.
//!
//! The strands are real geometry (lit, glossy tubes that cast shadows), and
//! they leave the hand already apart, from a small crown of shards (the
//! emitter), converging only into the needle or burr. In flight the bundle
//! sags under its weight; when it catches it snaps taut with a short twang,
//! then stays straight and twists while pulling.

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

/// The crown of shards at the hand the strands leave from.
#[derive(Component)]
pub struct Emitter;

/// The strands' mesh, rebuilt every frame.
#[derive(Component)]
pub struct Strands;

#[derive(Resource)]
pub struct StrandsMesh(Handle<Mesh>);

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
    // The emitter: six short shards in a crown round +Y (the line), leaning
    // out, and a stub behind, so the strands have something to leave from.
    let mut crown = vec![(Vec3::NEG_Y, 0.06, 0.03)];
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        crown.push((Vec3::new(a.cos() * 0.45, 1.0, a.sin() * 0.45), 0.08, 0.016));
    }
    commands.spawn((
        Emitter,
        Mesh3d(meshes.add(shard_mesh(&crown))),
        MeshMaterial3d(material.clone()),
        Transform::default(),
        Visibility::Hidden,
    ));
    let strands = meshes.add(
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3])
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3])
            .with_inserted_indices(Indices::U32(vec![0, 1, 2])),
    );
    commands.insert_resource(StrandsMesh(strands.clone()));
    // The strands' tubes: the same finish, drawn from both sides (their
    // winding is not worth fussing over at a centimetre across).
    let strand_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.015, 0.015, 0.015),
        perceptual_roughness: 0.2,
        reflectance: 0.9,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Strands,
        Mesh3d(strands),
        MeshMaterial3d(strand_material),
        Transform::default(),
        Visibility::Hidden,
        // Its bounds change every frame.
        bevy::camera::visibility::NoFrustumCulling,
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn draw(
    time: Res<Time>,
    camera: Single<(&Transform, &FlyCam, &Player), (Without<Needle>, Without<Burr>, Without<SpikeLight>, Without<Emitter>, Without<Strands>)>,
    mut needle: Single<(&mut Transform, &mut Visibility), (With<Needle>, Without<Burr>, Without<SpikeLight>, Without<Emitter>, Without<Strands>)>,
    mut burr: Single<(&mut Transform, &mut Visibility), (With<Burr>, Without<Needle>, Without<SpikeLight>, Without<Emitter>, Without<Strands>)>,
    mut light: Single<(&mut Transform, &mut PointLight), (With<SpikeLight>, Without<Needle>, Without<Burr>, Without<Emitter>, Without<Strands>)>,
    mut emitter: Single<(&mut Transform, &mut Visibility), (With<Emitter>, Without<Needle>, Without<Burr>, Without<SpikeLight>, Without<Strands>)>,
    mut strands: Single<&mut Visibility, (With<Strands>, Without<Needle>, Without<Burr>, Without<SpikeLight>, Without<Emitter>)>,
    handle: Res<StrandsMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
    // When it last caught (for the twang), and where.
    mut caught: Local<(Option<Vec3>, f32)>,
) {
    let (transform, fly, player) = *camera;
    *needle.1 = Visibility::Hidden;
    *burr.1 = Visibility::Hidden;
    *emitter.1 = Visibility::Hidden;
    **strands = Visibility::Hidden;
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
        _ => {
            caught.0 = None;
            return;
        }
    };
    let t_now = time.elapsed_secs();
    // The moment it catches: start the twang.
    if taut && caught.0 != Some(end) {
        *caught = (Some(end), t_now);
    }
    let since_caught = if taut { t_now - caught.1 } else { f32::INFINITY };

    let rotation = fly.rotation();
    let (aim, right, up) = (rotation * Vec3::NEG_Z, rotation * Vec3::X, rotation * Vec3::Y);
    let start = transform.translation + aim * 0.6 - right * 0.28 - up * 0.25;
    let line = end - start;
    let length = line.length();
    if length < 0.1 {
        return;
    }
    let dir = line / length;
    let (u, v) = dir.any_orthonormal_pair();

    // The emitter at the hand, its crown along the line.
    *emitter.0 = Transform::from_translation(start).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir));
    *emitter.1 = Visibility::Visible;

    // Six strands twisting round the line. They leave the emitter's rim
    // already apart and converge only into the needle or burr; slack and
    // sagging in flight, tight and spinning faster while pulling.
    const STRANDS: usize = 6;
    const SEGMENTS: usize = 48;
    const SIDES: usize = 6;
    let (spread, spin) = if taut { (0.06, 6.0) } else { (0.12, 2.0) };
    let smooth = |a: f32, b: f32, x: f32| {
        let k = ((x - a) / (b - a)).clamp(0.0, 1.0);
        k * k * (3.0 - 2.0 * k)
    };
    // Sag under its weight in flight; a twang when it catches.
    let sag = if taut { 0.0 } else { (length * 0.035).min(2.5) };
    let twang = if since_caught.is_finite() { 0.35 * (-7.0 * since_caught).exp() * (38.0 * since_caught).sin() } else { 0.0 };
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::<u32>::new());
    for s in 0..STRANDS {
        let thick = 0.011 + 0.004 * ((s * 5 % 6) as f32 / 5.0);
        let points: Vec<Vec3> = (0..=SEGMENTS)
            .map(|i| {
                let t = i as f32 / SEGMENTS as f32;
                let phase = s as f32 / STRANDS as f32 * std::f32::consts::TAU + t * length * 0.5 - t_now * spin;
                // The bundle's radius: the emitter's rim at the hand, its
                // spread along the way, a point at the far end.
                let along = 0.04 + (spread - 0.04) * smooth(0.0, 0.15, t);
                let radius = along * (1.0 - smooth(0.85, 1.0, t)) + 0.012 * smooth(0.85, 1.0, t);
                let wobble = 1.0 + 0.25 * (t * 17.0 + s as f32 * 1.7 + t_now * 3.0).sin() * (std::f32::consts::PI * t).sin();
                let offset = (u * phase.cos() + v * phase.sin()) * radius * wobble;
                let hang = Vec3::NEG_Y * sag * (std::f32::consts::PI * t).sin();
                let shake = u * twang * (std::f32::consts::PI * t).sin();
                start + line * t + offset + hang + shake
            })
            .collect();
        // A tube round the polyline.
        let base = positions.len() as u32;
        for i in 0..=SEGMENTS {
            let tangent = if i == 0 {
                points[1] - points[0]
            } else if i == SEGMENTS {
                points[i] - points[i - 1]
            } else {
                points[i + 1] - points[i - 1]
            }
            .normalize_or(dir);
            let (a, b) = tangent.any_orthonormal_pair();
            // Thinner where it enters the needle or burr.
            let r = thick * (1.0 - 0.6 * smooth(0.9, 1.0, i as f32 / SEGMENTS as f32));
            for k in 0..SIDES {
                let angle = k as f32 / SIDES as f32 * std::f32::consts::TAU;
                let n = a * angle.cos() + b * angle.sin();
                positions.push((points[i] + n * r).to_array());
                normals.push(n.to_array());
            }
        }
        for i in 0..SEGMENTS as u32 {
            for k in 0..SIDES as u32 {
                let k2 = (k + 1) % SIDES as u32;
                let (p0, p1) = (base + i * SIDES as u32 + k, base + i * SIDES as u32 + k2);
                let (q0, q1) = (p0 + SIDES as u32, p1 + SIDES as u32);
                indices.extend_from_slice(&[p0, q0, p1, p1, q0, q1]);
            }
        }
    }
    if let Some(mut mesh) = meshes.get_mut(&handle.0) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_indices(Indices::U32(indices));
        **strands = Visibility::Visible;
    }
}
