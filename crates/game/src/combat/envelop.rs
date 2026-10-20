//! Where a stalker's shot strikes: electrified ground, bolts crawling over
//! it, and a pillar of light over it (the user's twist; a dome there, its
//! lightning growing over it, read as a science experiment, the user: see
//! docs/removed.md). Standing on it you are as in the old grid's comet (the
//! user found it flying into a canal's beam and staying in it: everything
//! silhouetted): the sky one flat pale sheet behind everything, everything
//! darkened against it. A place: walk out of it and it is gone (a moment's
//! flash, it was over before you could do anything). See `envelop.wgsl`.

use bevy::{
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::ShaderRef,
};

use crate::Args;
use worldgen::noise::hash01;

/// How pale the sky (`--set envelop_pale`; at 0.9 the stars were lost in
/// it: in the beam they still showed, the user), how dark the world (of
/// black), deep in a zone; and over how far its edge it eases out (m).
const PALE: f32 = 0.35;
const DARK: f32 = 0.9;
const EDGE: f32 = 0.8;
/// How much of your health a zone takes a second, deep in it (stood in
/// through all of one, about half: two shots unheeded and you are gone).
const HARM: f32 = 17.0;
/// Electrified ground: bolts crawling over the ground where a shot struck,
/// all at once, each striking anew along another way now and then (s, at
/// least and at most), flickering; how far they reach (m), how long it
/// lasts (s, then gone at once), how many bolts, how bright.
const ZONE: (f32, f32) = (4.0, 3.0);
const ZONE_BOLTS: usize = 10;
const RESTRIKE: (f32, f32) = (0.25, 0.7);
const ZONE_GLOW: f32 = 70.0;
/// A pillar of light over it (the user's twist; and stone hanging in it,
/// `strike::levitate`): how wide (m, its radius), how
/// tall, how long it stands (s, then gone at once), how bright (flat, as the
/// comet's band, fading upwards). Seen from outside only: inside, the
/// comet.
const PILLAR: (f32, f32, f32) = (2.5, 60.0, 3.0);
const PILLAR_GLOW: f32 = 0.35;
/// Its bolts: how wide (m), how jagged (see `flat_fractal`), how many
/// branches each.
const BOLT_WIDTH: f32 = 0.02;
const ROUGH: f32 = 0.22;
const BRANCHES: usize = 4;



/// The layers over the screen: the sky's (pale, added, only where the sky
/// shows) and the world's (dark, multiplied, only where something is).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct Sky {
    #[uniform(0)]
    tone: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct World {
    #[uniform(0)]
    tone: Vec4,
}

/// The layers' depth test: passing only before the sky (nothing nearer),
/// or only where something is.
fn tested(descriptor: &mut RenderPipelineDescriptor, compare: CompareFunction) {
    if let Some(depth) = &mut descriptor.depth_stencil {
        depth.depth_compare = Some(compare);
        depth.depth_write_enabled = Some(false);
    }
}

impl Material for Sky {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/combat/envelop.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/combat/envelop.wgsl".into()
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

    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        tested(descriptor, CompareFunction::Greater);
        Ok(())
    }
}

impl Material for World {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/combat/envelop.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/combat/envelop.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Multiply
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        tested(descriptor, CompareFunction::Less);
        Ok(())
    }
}

/// The two layers' entities and finishes, and a pillar's shape and light.
#[derive(Resource)]
pub(super) struct Layers {
    entities: [Entity; 2],
    sky: Handle<Sky>,
    world: Handle<World>,
    pillar: (Handle<Mesh>, Handle<StandardMaterial>),
}

pub(super) fn setup(mut commands: Commands, args: Res<Args>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut skies: ResMut<Assets<Sky>>, mut worlds: ResMut<Assets<World>>) {
    let screen = meshes.add(Rectangle::new(2.0, 2.0));
    let sky = skies.add(Sky { tone: Vec4::new(0.0, 1.0, args.num("envelop_pale", PALE), DARK) });
    let world = worlds.add(World { tone: Vec4::new(0.0, 0.0, PALE, DARK) });
    let layer = |commands: &mut Commands| commands.spawn((Mesh3d(screen.clone()), Transform::default(), Visibility::Hidden, NoFrustumCulling, NotShadowCaster, NotShadowReceiver)).id();
    let (a, b) = (layer(&mut commands), layer(&mut commands));
    commands.entity(a).insert(MeshMaterial3d(sky.clone()));
    commands.entity(b).insert(MeshMaterial3d(world.clone()));
    let pillar = materials.add(StandardMaterial { base_color: Color::LinearRgba(LinearRgba::gray(PILLAR_GLOW)), unlit: true, alpha_mode: AlphaMode::Add, fog_enabled: false, ..default() });
    commands.insert_resource(Layers { entities: [a, b], sky, world, pillar: (meshes.add(pillar_mesh()), pillar) });
}



/// A bolt: its main way, and its branches (where on the main way each
/// splits off, and its own way).
struct Bolt {
    way: Vec<Vec3>,
    branches: Vec<(usize, Vec<Vec3>)>,
}


/// A way's first `grown` (of its points), its last between two of them.
fn reached(way: &[Vec3], grown: f32) -> Vec<Vec3> {
    let reach = grown.clamp(0.0, 1.0) * (way.len() - 1) as f32;
    let whole = reach.floor() as usize;
    let mut points = way[..=whole].to_vec();
    if whole + 1 < way.len() && reach > whole as f32 {
        points.push(way[whole].lerp(way[whole + 1], reach - whole as f32));
    }
    points
}

/// A bolt drawn as far as it has grown (`grown`, of its main way; its
/// branches as the main way passes them, growing as fast), scaled by `r`:
/// thin ribbons turned to the eye, tapering to their tips, no
/// wider close up than a thread.
fn bolt_mesh(bolt: &Bolt, r: f32, grown: f32, eye: Vec3) -> Mesh {
    let (mut positions, mut normals) = (Vec::new(), Vec::new());
    let mut ribbon = |line: &[Vec3], width: f32| {
        let n = line.len();
        for (i, w) in line.windows(2).enumerate() {
            let (a, b) = (w[0] * r, w[1] * r);
            let along = (b - a).normalize_or(Vec3::Y);
            let side = along.cross((eye - a).normalize_or(Vec3::Z)).normalize_or(Vec3::X);
            let near = |p: Vec3| (p.distance(eye) * 0.004 / width).min(1.0);
            let t0 = width * (1.0 - 0.7 * i as f32 / n as f32) * near(a);
            let t1 = width * (1.0 - 0.7 * (i + 1) as f32 / n as f32) * near(b);
            let normal = (eye - a).normalize_or(Vec3::Z).to_array();
            for p in [a - side * t0, a + side * t0, b + side * t1, a - side * t0, b + side * t1, b - side * t1] {
                positions.push(p.to_array());
                normals.push(normal);
            }
        }
    };
    let main = reached(&bolt.way, grown);
    if main.len() > 1 {
        ribbon(&main, BOLT_WIDTH * 0.5);
    }
    let steps = (bolt.way.len() - 1) as f32;
    for (at, way) in &bolt.branches {
        let past = grown * steps - *at as f32;
        if past > 0.0 {
            let line = reached(way, past / (way.len() - 1) as f32);
            if line.len() > 1 {
                ribbon(&line, BOLT_WIDTH * 0.3);
            }
        }
    }
    // (Never empty: the renderer does not take an empty mesh growing.)
    if positions.is_empty() {
        let p = (bolt.way[0] * r).to_array();
        positions.extend([p; 3]);
        normals.extend([[0.0, 1.0, 0.0]; 3]);
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

/// Electrified ground where a shot struck: how long it has been, its seed,
/// its bolts (each its mesh, its way, how long yet before it strikes anew,
/// how many times it has), and their finish (its own, to flicker).
#[derive(Component)]
pub(super) struct Zone {
    age: f32,
    seed: i32,
    bolts: Vec<(Handle<Mesh>, Bolt, f32, i32)>,
    glow: Handle<StandardMaterial>,
}

/// A pillar of light where a shot struck: how long it has stood.
#[derive(Component)]
pub(super) struct Pillar {
    age: f32,
}

/// Where a shot strikes, a pillar of light (see `PILLAR`).
pub(super) fn raise(commands: &mut Commands, layers: &Layers, at: Vec3) {
    commands.spawn((
        Pillar { age: 0.0 },
        Mesh3d(layers.pillar.0.clone()),
        MeshMaterial3d(layers.pillar.1.clone()),
        Transform::from_translation(at).with_scale(Vec3::new(PILLAR.0, PILLAR.1, PILLAR.0)),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// A pillar's shape: an open tube, a unit across and tall, its faces
/// outward (from inside, unseen), its light fading upwards (its vertex
/// colours).
fn pillar_mesh() -> Mesh {
    let (around, up) = (48, 16);
    let (mut positions, mut normals, mut colors, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for i in 0..=up {
        let t = i as f32 / up as f32;
        let c = (1.0 - t).powf(1.5);
        for j in 0..=around {
            let a = j as f32 / around as f32 * std::f32::consts::TAU;
            positions.push([a.cos(), t, a.sin()]);
            normals.push([a.cos(), 0.0, a.sin()]);
            colors.push([c, c, c, 1.0]);
        }
    }
    let row = (around + 1) as u32;
    for i in 0..up as u32 {
        for j in 0..around as u32 {
            let (v00, v10, v01, v11) = (i * row + j, i * row + j + 1, (i + 1) * row + j, (i + 1) * row + j + 1);
            indices.extend_from_slice(&[v00, v01, v10, v10, v01, v11]);
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}

/// Where a shot strikes, electrified ground (see `ZONE`); its bolts made as
/// it first stands (see `envelop`).
pub(super) fn electrify(commands: &mut Commands, at: Vec3) {
    let seed = ((at.x * 12.9898 + at.z * 78.233).sin().abs() * 1.0e6) as i32;
    commands.spawn((Zone { age: 0.0, seed, bolts: Vec::new(), glow: Handle::default() }, Transform::from_translation(at), Visibility::default()));
}

/// A way over flat ground (from its middle, level), split in half again
/// and again, each half's middle pushed aside by as much as `ROUGH` of its
/// length (lightning: jagged at every scale, its large wanders down to its
/// finest; one scale of kinks, it read as lines painted, the user).
fn flat_fractal(from: Vec3, to: Vec3, levels: u32, seed: i32) -> Vec<Vec3> {
    let mut way = vec![from, to];
    for level in 0..levels as i32 {
        let mut finer = Vec::with_capacity(way.len() * 2);
        for (k, w) in way.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let aside = Vec3::new(a.z - b.z, 0.0, b.x - a.x).normalize_or(Vec3::X);
            let push = (hash01(seed, level, k as i32, 0x7e5) - 0.5) * 2.0 * ROUGH * a.distance(b);
            finer.extend_from_slice(&[a, (a + b) * 0.5 + aside * push]);
        }
        finer.push(*way.last().unwrap());
        way = finer;
    }
    way
}

/// A bolt over the ground round `middle`: out from near it, its branches
/// off its middle stretch, each point laid on the ground under it.
fn ground_bolt(seed: i32, middle: Vec3, ground: &crate::ground::Ground) -> Bolt {
    let r = |j: i32| hash01(seed, j, 0, 0x7e6);
    let a = r(0) * std::f32::consts::TAU;
    let b = a + (r(1) - 0.5) * 1.2;
    let from = Vec3::new(a.cos(), 0.0, a.sin()) * (0.2 + 0.6 * r(2));
    let to = Vec3::new(b.cos(), 0.0, b.sin()) * ZONE.0 * (0.6 + 0.4 * r(3));
    let way = flat_fractal(from, to, 6, seed);
    let branches = (0..BRANCHES as i32)
        .map(|k| {
            let at = ((0.2 + 0.6 * r(10 + k)) * (way.len() - 1) as f32) as usize;
            let along = (way[(at + 1).min(way.len() - 1)] - way[at.saturating_sub(1)]).with_y(0.0).normalize_or(Vec3::X);
            let turn = if r(20 + k) < 0.5 { -0.7 } else { 0.7 };
            let out = Quat::from_rotation_y(turn) * along;
            (at, flat_fractal(way[at], way[at] + out * from.distance(to) * (0.2 + 0.25 * r(30 + k)), 4, seed * 31 + k))
        })
        .collect::<Vec<_>>();
    let lay = |p: Vec3| Vec3::new(p.x, ground.at(middle + p + Vec3::Y * 0.5) - middle.y + 0.04, p.z);
    Bolt { way: way.into_iter().map(lay).collect(), branches: branches.into_iter().map(|(at, w)| (at, w.into_iter().map(lay).collect())).collect() }
}




/// Each zone crackles and goes, its bolts striking anew; each pillar stands
/// and goes; the layers as strong as you are deep in a zone (your eyes;
/// easing out over its edge), hidden out of them all.
#[allow(clippy::too_many_arguments)]
pub(super) fn envelop(
    mut commands: Commands,
    time: Res<Time>,
    layers: Res<Layers>,
    camera: Single<&Transform, With<crate::camera::FlyCam>>,
    player: Option<Single<&mut crate::player::Player>>,
    ground: crate::ground::Ground,
    mut zones: Query<(Entity, &mut Zone, &Transform), Without<crate::camera::FlyCam>>,
    mut pillars: Query<(Entity, &mut Pillar)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut skies: ResMut<Assets<Sky>>,
    mut worlds: ResMut<Assets<World>>,
    mut visibility: Query<&mut Visibility>,
) {
    let dt = time.delta_secs();
    let mut strength: f32 = 0.0;
    for (entity, mut zone, transform) in &mut zones {
        let zone = &mut *zone;
        zone.age += dt;
        if zone.age > ZONE.1 {
            materials.remove(&zone.glow);
            for (mesh, ..) in &zone.bolts {
                meshes.remove(mesh);
            }
            commands.entity(entity).despawn();
            continue;
        }
        let middle = transform.translation;
        let eye = camera.translation - middle;
        let seed = zone.seed;
        // All at once (sudden, as a blow), made as it first stands.
        if zone.bolts.is_empty() {
            zone.glow = materials.add(StandardMaterial { base_color: Color::BLACK, unlit: true, alpha_mode: AlphaMode::Add, double_sided: true, cull_mode: None, fog_enabled: false, ..default() });
            for k in 0..ZONE_BOLTS as i32 {
                let bolt = ground_bolt(seed * 7919 + k * 131, middle, &ground);
                let mesh = meshes.add(bolt_mesh(&bolt, 1.0, 1.0, eye));
                commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(zone.glow.clone()), Transform::IDENTITY, NoFrustumCulling, NotShadowCaster, NotShadowReceiver, ChildOf(entity)));
                let life = RESTRIKE.0 + (RESTRIKE.1 - RESTRIKE.0) * hash01(seed, k, 0, 0x7e7);
                zone.bolts.push((mesh, bolt, life, 0));
            }
        }
        for (k, (mesh, bolt, life, strikes)) in zone.bolts.iter_mut().enumerate() {
            let k = k as i32;
            *life -= dt;
            if *life <= 0.0 {
                // Striking anew, along another way.
                *strikes += 1;
                *bolt = ground_bolt(seed * 7919 + k * 131 + *strikes * 17, middle, &ground);
                *life = RESTRIKE.0 + (RESTRIKE.1 - RESTRIKE.0) * hash01(seed, k, *strikes, 0x7e7);
            }
            if let Some(mut built) = meshes.get_mut(mesh) {
                *built = bolt_mesh(bolt, 1.0, 1.0, eye);
            }
        }
        let flicker = 0.5 + 0.9 * hash01(seed, (zone.age / 0.04) as i32, 0, 0x7e8);
        if let Some(mut m) = materials.get_mut(&zone.glow) {
            m.base_color = Color::LinearRgba(LinearRgba::gray(ZONE_GLOW * flicker));
        }
        // On it (over it, not far above).
        let off = Vec2::new(eye.x, eye.z).length();
        if eye.y - crate::player::EYE < 2.0 {
            strength = strength.max(((ZONE.0 - off) / EDGE).clamp(0.0, 1.0));
        }
    }
    for (entity, mut pillar) in &mut pillars {
        pillar.age += dt;
        if pillar.age > PILLAR.2 {
            commands.entity(entity).despawn();
        }
    }
    // Deep in one, it harms you.
    if let Some(mut player) = player {
        player.health -= HARM * strength * dt;
    }
    for entity in layers.entities {
        if let Ok(mut v) = visibility.get_mut(entity) {
            let want = if strength > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
            if *v != want {
                *v = want;
            }
        }
    }
    if strength > 0.0 {
        if let Some(mut m) = skies.get_mut(&layers.sky) {
            m.tone.x = strength;
        }
        if let Some(mut m) = worlds.get_mut(&layers.world) {
            m.tone.x = strength;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A way grown part of its way: whole, and part, its tip between two
    /// of its points.
    #[test]
    fn grows_ways() {
        let way: Vec<Vec3> = (0..5).map(|k| Vec3::X * k as f32).collect();
        assert_eq!(reached(&way, 1.0), way);
        let part = reached(&way, 0.6);
        assert!((part.last().unwrap().x - 2.4).abs() < 1e-5 && part.len() == 4, "{part:?}");
    }
}
