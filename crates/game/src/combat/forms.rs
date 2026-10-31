//! The swarmer's form, the bud: black petals closed round a light leaking
//! through the seam; looked at (towards it, within about 35 degrees), it
//! opens like an eye over a second and a half, pale inner petals and the
//! light inside, and closes again slowly when you look away. A bud's `gaze`
//! option (`spawn bud gaze=0.5`) holds it at one point between closed and
//! open, to judge it. (The swarm lab, `--opt swarmlab`, one placed before
//! you by itself, went with the spawning switches: `spawn bud` does it.)

use bevy::prelude::*;

use crate::{camera::FlyCam, player::tether::shard_mesh};

/// The bud's size, closed and open.
const BUD_CLOSED: f32 = 1.6;
const BUD_OPEN: f32 = 1.35;
/// Looking towards a form within this angle (its cosine), and this near,
/// turns it.
const LOOK_COS: f32 = 0.82;
const LOOK_WITHIN: f32 = 25.0;

/// How far something has turned into its second form.
#[derive(Component, Default)]
pub(super) struct Gazed {
    gaze: f32,
}

impl Gazed {
    /// Thrown wide open, at once (it closes again slowly).
    pub(super) fn startle(&mut self) {
        self.gaze = 1.0;
    }
}

/// Asleep: shut, whoever looks (see `grove.rs`).
#[derive(Component)]
pub(super) struct Asleep;

/// A piece moving between its rest and looked-at forms (in its parent's
/// space).
#[derive(Component)]
pub(super) struct Morph {
    of: Entity,
    rest: Transform,
    gazed: Transform,
}

/// A light brightening as its owner is looked at.
#[derive(Component)]
pub(super) struct Glow {
    of: Entity,
    material: Handle<StandardMaterial>,
    rest: f32,
    gazed: f32,
}

/// What buds are made of.
#[derive(Resource)]
pub(super) struct Kit {
    /// A blade: a flat four-sided point along +Y, unit length and half-width
    /// (scaled to size), with a short point back the other way to close it.
    blade: Handle<Mesh>,
    core: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
    pale: Handle<StandardMaterial>,
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(Kit {
        blade: meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 1.0), (Vec3::NEG_Y, 0.08, 1.0)])),
        core: meshes.add(Sphere::new(0.13).mesh().ico(1).unwrap()),
        dark: materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.35, reflectance: 0.6, ..default() }),
        pale: materials.add(StandardMaterial { base_color: Color::srgb(0.72, 0.72, 0.72), perceptual_roughness: 0.7, ..default() }),
    });
}

/// Builds a bud under `root` (which faces you along -Z), its light with a
/// material of its own.
pub(super) fn build(commands: &mut Commands, kit: &Kit, materials: &mut Assets<StandardMaterial>, root: Entity) {
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::BLACK, fog_enabled: false, ..default() });
    bud(commands, kit, root, glow);
}

/// A rotation taking +Y to `dir`, with +X along `across`.
fn aim(dir: Vec3, across: Vec3) -> Quat {
    let y = dir.normalize();
    let x = (across - y * across.dot(y)).normalize_or(y.any_orthonormal_vector());
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

/// The lab: a bud in front of you, a moment after loading.
/// How far each form has turned, following your gaze (quick to turn, slow to
/// relax; or held by its `gaze` option), and its pieces and light placed to
/// match.
pub(super) fn gaze(
    time: Res<Time>,
    stutter: Res<crate::look::Stutter>,
    ground: crate::ground::Ground,
    camera: Single<&Transform, With<FlyCam>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gazed: Query<(&GlobalTransform, &mut Gazed, Has<Asleep>, Option<&crate::traits::Traits>)>,
    mut morphs: Query<(&Morph, &mut Transform), Without<FlyCam>>,
    glows: Query<&Glow>,
) {
    let dt = time.delta_secs().min(0.05);
    for (transform, mut g, asleep, traits) in &mut gazed {
        if asleep {
            g.gaze = 0.0;
            continue;
        }
        let fixed = traits.map_or(-1.0, |t| t.value("gaze", -1.0));
        if fixed >= 0.0 {
            g.gaze = fixed;
            continue;
        }
        let to = transform.translation() - camera.translation;
        // (Seen: not through a wall or a floor.)
        let looked = to.length() < LOOK_WITHIN && to.normalize_or(Vec3::Y).dot(*camera.forward()) > LOOK_COS && ground.clear(camera.translation, transform.translation());
        g.gaze = if looked { (g.gaze + dt / 1.5).min(1.0) } else { (g.gaze - dt / 2.5).max(0.0) };
    }
    for (m, mut transform) in &mut morphs {
        let Ok((_, g, ..)) = gazed.get(m.of) else { continue };
        let k = g.gaze * g.gaze * (3.0 - 2.0 * g.gaze);
        transform.translation = m.rest.translation.lerp(m.gazed.translation, k);
        transform.rotation = m.rest.rotation.slerp(m.gazed.rotation, k);
        transform.scale = m.rest.scale.lerp(m.gazed.scale, k);
    }
    for glow in &glows {
        let Ok((_, g, ..)) = gazed.get(glow.of) else { continue };
        if let Some(mut material) = materials.get_mut(&glow.material) {
            let e = (glow.rest + (glow.gazed - glow.rest) * g.gaze) * stutter.0;
            material.emissive = LinearRgba::rgb(e, e, e);
        }
    }
}

/// A piece under `parent`, moving between two forms.
#[allow(clippy::too_many_arguments)]
fn piece(commands: &mut Commands, parent: Entity, of: Entity, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, rest: Transform, gazed: Transform) -> Entity {
    commands.spawn((Morph { of, rest, gazed }, Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), rest, ChildOf(parent))).id()
}

/// A node with nothing drawn, moving between two forms (a joint, a neck).
fn joint(commands: &mut Commands, parent: Entity, of: Entity, rest: Transform, gazed: Transform) -> Entity {
    commands.spawn((Morph { of, rest, gazed }, rest, Visibility::default(), ChildOf(parent))).id()
}

fn light(commands: &mut Commands, kit: &Kit, parent: Entity, of: Entity, material: Handle<StandardMaterial>, rest: f32, gazed: f32, at: Transform, grown: Transform) {
    commands.spawn((
        Morph { of, rest: at, gazed: grown },
        Glow { of, material: material.clone(), rest, gazed },
        Mesh3d(kit.core.clone()),
        MeshMaterial3d(material),
        at,
        ChildOf(parent),
        bevy::light::NotShadowCaster,
    ));
}

/// Its back is to you (+Z away); the petals' bases sit behind the core.
fn bud(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    // Large while closed, so the shut bud holds its own; a little smaller as
    // it opens.
    let whole = joint(commands, root, root, Transform::from_scale(Vec3::splat(BUD_CLOSED)), Transform::from_scale(Vec3::splat(BUD_OPEN)));
    light(commands, kit, whole, root, glow, 300.0, 3000.0, Transform::from_scale(Vec3::splat(0.7)), Transform::from_scale(Vec3::splat(0.85)));
    for (count, length, width, material, closed, open, base) in [
        (7, 0.36, 0.09, &kit.dark, 0.38, 0.35, 0.16),
        (5, 0.24, 0.06, &kit.pale, 0.5, -0.3, 0.09),
    ] {
        for i in 0..count {
            let a = (i as f32 + if material == &kit.pale { 0.5 } else { 0.0 }) / count as f32 * std::f32::consts::TAU;
            let (r, t) = (Vec3::new(a.cos(), a.sin(), 0.0), Vec3::new(-a.sin(), a.cos(), 0.0));
            let scale = Vec3::new(width, length, 0.012);
            // (The inner ones tucked in, shorter, while closed.)
            let tucked = if material == &kit.pale { scale * Vec3::new(1.0, 0.55, 1.0) } else { scale };
            let rest = Transform::from_translation(r * base + Vec3::Z * 0.08).with_rotation(aim(Vec3::NEG_Z - r * closed, t)).with_scale(tucked);
            let gazed = Transform::from_translation(r * base * 0.8 + Vec3::Z * 0.06).with_rotation(aim(r + Vec3::Z * open, t)).with_scale(scale);
            piece(commands, whole, root, &kit.blade, material, rest, gazed);
        }
    }
    // A short stalk of spines behind.
    for i in 0..3 {
        let a = i as f32 / 3.0 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos() * 0.4, a.sin() * 0.4, 1.0);
        let t = Transform::from_translation(Vec3::Z * 0.1).with_rotation(aim(d, Vec3::X)).with_scale(Vec3::new(0.02, 0.4, 0.02));
        piece(commands, whole, root, &kit.blade, &kit.dark, t, t);
    }
}
