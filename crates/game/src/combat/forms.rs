//! Swarmer forms, each with a form at rest and a form when looked at: a
//! swarmer you look towards (within about 35 degrees) turns into its second
//! form over a second and a half, and relaxes slowly when you look away. `--opt bud`, `face` or
//! `chandelier` gives the swarm that form (otherwise the old knot of spikes).
//!
//! - The bud: black petals closed round a light leaking through the seam;
//!   looked at, they open like an eye, pale inner petals and the light inside.
//! - The face: a scatter of pale facets, turned away; looked at, it turns to
//!   you and the facets fall into a face, eyes lit deep under the brows.
//! - The chandelier: needles hanging round a light from a thread up into the
//!   dark; looked at, they bristle out towards you.
//!
//! The lab (`--opt swarmlab`, with `--opt peace`): the three in an arc in
//! front of you. `--set gaze=0..1` holds every form at one point between the
//! two (for captures), `--set pick=N` shows only form N, close, and `--set
//! gloom=0..1` holds the swarm's darkness.

use bevy::prelude::*;
use worldgen::noise::hash01;

use crate::{Args, camera::FlyCam, player::tether::shard_mesh, terrain::Streamer};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Form {
    Bud,
    Face,
    Chandelier,
}

impl Form {
    const ALL: [Form; 3] = [Form::Bud, Form::Face, Form::Chandelier];

    /// The form the swarm takes, if any.
    pub(super) fn chosen(args: &Args) -> Option<Form> {
        Form::ALL.into_iter().find(|f| args.opt(f.name()))
    }

    fn name(self) -> &'static str {
        match self {
            Form::Bud => "bud",
            Form::Face => "face",
            Form::Chandelier => "chandelier",
        }
    }

    /// Facing you upright (turning only about the vertical), or straight at
    /// you.
    pub(super) fn upright(self) -> bool {
        self == Form::Chandelier
    }
}

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

/// What the forms are made of.
#[derive(Resource)]
pub(super) struct Kit {
    /// A blade: a flat four-sided point along +Y, unit length and half-width
    /// (scaled to size), with a short point back the other way to close it.
    blade: Handle<Mesh>,
    /// A facet: a flat diamond in the XY plane, unit half-sizes.
    facet: Handle<Mesh>,
    core: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
    pale: Handle<StandardMaterial>,
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(Kit {
        blade: meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 1.0), (Vec3::NEG_Y, 0.08, 1.0)])),
        facet: meshes.add(shard_mesh(&[(Vec3::Y, 1.0, 1.0), (Vec3::NEG_Y, 1.0, 1.0)])),
        core: meshes.add(Sphere::new(0.13).mesh().ico(1).unwrap()),
        dark: materials.add(StandardMaterial { base_color: Color::srgb(0.03, 0.03, 0.03), perceptual_roughness: 0.35, reflectance: 0.6, ..default() }),
        pale: materials.add(StandardMaterial { base_color: Color::srgb(0.72, 0.72, 0.72), perceptual_roughness: 0.7, ..default() }),
    });
}

/// Builds `form` under `root` (which faces you along -Z), its light with a
/// material of its own.
pub(super) fn build(commands: &mut Commands, kit: &Kit, materials: &mut Assets<StandardMaterial>, root: Entity, form: Form) {
    let glow = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::BLACK, fog_enabled: false, ..default() });
    match form {
        Form::Bud => bud(commands, kit, root, glow),
        Form::Face => face(commands, kit, root, glow),
        Form::Chandelier => chandelier(commands, kit, root, glow),
    }
}

/// A rotation taking +Y to `dir`, with +X along `across`.
fn aim(dir: Vec3, across: Vec3) -> Quat {
    let y = dir.normalize();
    let x = (across - y * across.dot(y)).normalize_or(y.any_orthonormal_vector());
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

/// The lab: the forms in an arc in front of you, a moment after loading.
#[allow(clippy::too_many_arguments)]
pub(super) fn lab(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time>,
    streamer: Res<Streamer>,
    kit: Res<Kit>,
    camera: Single<&Transform, With<FlyCam>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gloom: ResMut<crate::look::Gloom>,
    mut settled_for: Local<f32>,
    mut spawned: Local<bool>,
) {
    if !args.opt("swarmlab") {
        return;
    }
    let held = args.num("gloom", -1.0);
    if held >= 0.0 {
        gloom.0 = held;
    }
    if *spawned {
        return;
    }
    *settled_for = if streamer.near { *settled_for + time.delta_secs() } else { 0.0 };
    if *settled_for < 1.0 {
        return;
    }
    *spawned = true;
    let eye = camera.translation;
    let ahead = Vec3::new(camera.forward().x, 0.0, camera.forward().z).normalize_or(Vec3::NEG_Z);
    let pick = args.num("pick", 0.0) as usize;
    for (k, form) in Form::ALL.into_iter().enumerate() {
        if pick != 0 && pick != k + 1 {
            continue;
        }
        let (angle, distance) = if pick != 0 { (0.0, 1.6) } else { ((k as f32 - 1.0) * 0.4, 4.0) };
        let at = eye + Quat::from_rotation_y(-angle) * ahead * distance - Vec3::Y * 0.1;
        let root = commands.spawn((Gazed::default(), Transform::from_translation(at).looking_at(eye, Vec3::Y), Visibility::default())).id();
        build(&mut commands, &kit, &mut materials, root, form);
    }
}

/// How far each form has turned, following your gaze (quick to turn, slow to
/// relax; or held with `--set gaze`), and its pieces and light placed to
/// match.
pub(super) fn gaze(
    args: Res<Args>,
    time: Res<Time>,
    camera: Single<&Transform, With<FlyCam>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gazed: Query<(&GlobalTransform, &mut Gazed)>,
    mut morphs: Query<(&Morph, &mut Transform), Without<FlyCam>>,
    glows: Query<&Glow>,
) {
    let dt = time.delta_secs().min(0.05);
    let fixed = args.num("gaze", -1.0);
    for (transform, mut g) in &mut gazed {
        if fixed >= 0.0 {
            g.gaze = fixed;
            continue;
        }
        let to = transform.translation() - camera.translation;
        let looked = to.length() < LOOK_WITHIN && to.normalize_or(Vec3::Y).dot(*camera.forward()) > LOOK_COS;
        g.gaze = if looked { (g.gaze + dt / 1.5).min(1.0) } else { (g.gaze - dt / 2.5).max(0.0) };
    }
    for (m, mut transform) in &mut morphs {
        let Ok((_, g)) = gazed.get(m.of) else { continue };
        let k = g.gaze * g.gaze * (3.0 - 2.0 * g.gaze);
        transform.translation = m.rest.translation.lerp(m.gazed.translation, k);
        transform.rotation = m.rest.rotation.slerp(m.gazed.rotation, k);
        transform.scale = m.rest.scale.lerp(m.gazed.scale, k);
    }
    for glow in &glows {
        let Ok((_, g)) = gazed.get(glow.of) else { continue };
        if let Some(mut material) = materials.get_mut(&glow.material) {
            let e = glow.rest + (glow.gazed - glow.rest) * g.gaze;
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

/// Turned away, it is a scatter of pale shards on a dark knot; turned to you,
/// they fall into a face.
fn face(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    let turn = joint(commands, root, root, Transform::from_rotation(Quat::from_rotation_y(2.4)), Transform::IDENTITY);
    // The knot behind the face: spines thrown back like hair.
    for i in 0..7 {
        let r = |j: i32| hash01(i, j, 2, 0x5d1) - 0.5;
        let d = Vec3::new(r(0) * 1.4, r(1) * 1.2 + 0.2, 1.0);
        let t = Transform::from_translation(Vec3::Z * 0.02).with_rotation(aim(d, Vec3::X)).with_scale(Vec3::new(0.03, 0.3 + r(2) * 0.2, 0.02));
        piece(commands, turn, root, &kit.blade, &kit.dark, t, t);
    }
    // (centre, half-width, half-height, roll); mirrored ones once each side.
    let facets: [(Vec3, f32, f32, f32); 11] = [
        (Vec3::new(0.0, 0.15, -0.06), 0.11, 0.05, 0.0),
        (Vec3::new(0.065, 0.075, -0.1), 0.055, 0.018, -0.3),
        (Vec3::new(-0.065, 0.075, -0.1), 0.055, 0.018, 0.3),
        (Vec3::new(0.075, -0.03, -0.09), 0.04, 0.06, 0.35),
        (Vec3::new(-0.075, -0.03, -0.09), 0.04, 0.06, -0.35),
        (Vec3::new(0.0, 0.01, -0.15), 0.016, 0.06, 0.0),
        (Vec3::new(0.0, -0.12, -0.07), 0.06, 0.035, 0.0),
        (Vec3::new(0.06, -0.1, -0.04), 0.03, 0.05, 0.6),
        (Vec3::new(-0.06, -0.1, -0.04), 0.03, 0.05, -0.6),
        (Vec3::new(0.105, 0.06, -0.03), 0.02, 0.06, 0.15),
        (Vec3::new(-0.105, 0.06, -0.03), 0.02, 0.06, -0.15),
    ];
    for (i, &(at, w, h, roll)) in facets.iter().enumerate() {
        let r = |j: i32| hash01(i as i32, j, 3, 0x5d2) - 0.5;
        let scale = Vec3::new(w, h, 0.008);
        let gazed = Transform::from_translation(at).with_rotation(Quat::from_rotation_z(roll)).with_scale(scale);
        let rest = Transform::from_translation(at + Vec3::new(r(0) * 0.24, r(1) * 0.24, r(2) * 0.3))
            .with_rotation(Quat::from_euler(EulerRot::XYZ, r(3) * 1.6, r(4) * 1.6, roll + r(5) * 1.6))
            .with_scale(scale);
        piece(commands, turn, root, &kit.facet, &kit.pale, rest, gazed);
    }
    // Eyes, deep under the brows.
    for side in [-1.0, 1.0] {
        let t = Transform::from_xyz(side * 0.045, 0.045, -0.05).with_scale(Vec3::splat(0.12));
        light(commands, kit, turn, root, glow.clone(), 10.0, 2500.0, t, t);
    }
}

/// Needles hanging round a light from a thread up into the dark; looked at,
/// each turns its point to you.
fn chandelier(commands: &mut Commands, kit: &Kit, root: Entity, glow: Handle<StandardMaterial>) {
    let core = Transform::from_scale(Vec3::splat(0.8));
    light(commands, kit, root, root, glow, 1500.0, 5000.0, core, core.with_scale(Vec3::splat(0.9)));
    let thread = Transform::from_xyz(0.0, 0.08, 0.0).with_scale(Vec3::new(0.004, 2.0, 0.004));
    piece(commands, root, root, &kit.blade, &kit.dark, thread, thread);
    let mut n = 0;
    for (ring, (radius, count)) in [(0.05, 6), (0.13, 9), (0.21, 11)].into_iter().enumerate() {
        for i in 0..count {
            n += 1;
            let r = |j: i32| hash01(n, j, 5, 0x5d4) - 0.5;
            let a = (i as f32 + r(0) * 0.6) / count as f32 * std::f32::consts::TAU;
            let at = Vec3::new(a.cos() * radius, -0.02 - ring as f32 * 0.05 + r(1) * 0.04, a.sin() * radius);
            let len = 0.22 + ring as f32 * 0.07 + r(2) * 0.1;
            let scale = Vec3::new(0.012, len, 0.012);
            let hang = Vec3::new(r(3) * 0.15, -1.0, r(4) * 0.15);
            // Bristling: out from the middle, and towards you.
            let point = Vec3::new(at.x, 0.0, at.z).normalize_or(Vec3::X) + Vec3::new(0.0, -0.15, -0.7);
            let rest = Transform::from_translation(at).with_rotation(aim(hang, Vec3::X)).with_scale(scale);
            let gazed = Transform::from_translation(at).with_rotation(aim(point, Vec3::X)).with_scale(scale);
            piece(commands, root, root, &kit.blade, if n % 3 == 0 { &kit.pale } else { &kit.dark }, rest, gazed);
        }
    }
}
