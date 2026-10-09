//! Streams world columns around the camera at several levels of detail.
//!
//! Columns are identified two ways: an *unwrapped* key (where an instance sits
//! in the scene, relative to the camera) and a *wrapped* key (which piece of the
//! torus it shows). Meshes are generated and cached by wrapped key; instances
//! are spawned by unwrapped key, so the same mesh appears again after you walk
//! once around the world.
//!
//! Each level covers a disc around the camera. A coarse column is drawn until
//! every finer column covering it is ready, so there are never holes; where a
//! coarse column still overlaps finer ones it sits slightly lower so the finer
//! surface wins.

use std::sync::Arc;

use avian3d::prelude::{Collider, RigidBody};
use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};
use worldgen::{
    ColumnMesh, LOD_FACTOR, LOD_LEVELS, WORLD_SIZE, column_size,
    plates::{CanalFlow, PlateWorld},
};

use bevy::camera::{primitives::Aabb, visibility::NoAutoAabb};

use crate::Args;

/// Radius of each detail level's disc, in metres.
pub const LOD_RADIUS: [f32; LOD_LEVELS as usize] = [450.0, 1700.0, 4800.0];
/// How far round the camera the finest terrain must be in place to play
/// (`Streamer::near`); the rest streams in while you do.
const NEAR: f32 = 80.0;

/// Radius of the fake planet used to bend the world below the horizon.
pub const PLANET_RADIUS: f32 = 40_000.0;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        let args = app.world().resource::<Args>();
        let world = if args.opt("lab") || args.opt("chasm") {
            // The lab: flat plates, nothing else (see `structures.rs`); the
            // chasm stands on them (see `chasm.rs`).
            WorldGen(Arc::new(PlateWorld::lab(WORLD_SIZE, args.seed)))
        } else {
            // The sites in the structure library reshape the ground.
            let world = PlateWorld::new(WORLD_SIZE, args.seed);
            match crate::structures::load_library() {
                Ok(library) if !args.opt("nosites") => WorldGen(Arc::new(world.with_sites(&library))),
                _ => WorldGen(Arc::new(world)),
            }
        };
        embedded_asset!(app, "terrain.wgsl");
        embedded_asset!(app, "terrain_vertex.wgsl");
        embedded_asset!(app, "terrain_prepass.wgsl");
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default())
            .insert_resource(world)
            .init_resource::<Streamer>()
            .add_systems(Startup, setup_material)
            .add_systems(
                Update,
                (receive_meshes, stream_columns, update_material).chain().in_set(StreamSet),
            );
    }
}

/// Systems that move the stream anchor must run before this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamSet;

/// The world being shown: the plate world.
#[derive(Resource, Clone)]
pub struct WorldGen(pub Arc<PlateWorld>);

impl WorldGen {
    pub fn size(&self) -> f32 {
        self.0.size()
    }

    pub fn ground_height(&self, x: f32, z: f32) -> f32 {
        self.0.height_at(x, z)
    }

    /// The canal flow at a point, if it is inside a canal's pipe.
    pub fn canal_at(&self, x: f32, z: f32) -> Option<CanalFlow> {
        self.0.canal_at(x, z)
    }

    /// The nearest canal centreline point, its flow direction and floor.
    pub fn nearest_canal(&self, x: f32, z: f32) -> Option<(Vec2, Vec2, f32)> {
        self.0.nearest_canal(x, z)
    }

    fn mesh(&self, lod: u32, cx: i32, cz: i32) -> ColumnMesh {
        self.0.mesh_column(lod, cx, cz)
    }

    /// How far a coarse column is lowered so finer overlapping terrain wins:
    /// finer plates step at most a few metres from their parent.
    fn sink(&self, lod: u32) -> f32 {
        3.0 * lod as f32
    }

    pub fn columns_per_side(&self, lod: u32) -> i32 {
        (self.size() / column_size(lod)) as i32
    }
}

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExtension>;

/// Applies baked sky visibility (vertex colour alpha) and bends the world
/// around the camera to fake a planet's curvature.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TerrainExtension {
    /// x: baked occlusion strength, y: curvature (1 / 2R), zw: camera x and z.
    #[uniform(100)]
    pub params: Vec4,
    /// x: albedo grain strength, y: world wrap period (m), z: relief strength.
    #[uniform(101)]
    pub grain: Vec4,
    /// x: brightness of lit geometry (glowing etchings and seams).
    #[uniform(102)]
    pub glow: Vec4,
    /// Paving on some of the ground (`--set paving=N`, off by default): x the
    /// pattern (0 none, 1 checkerboard, 2 fractal), y the tile size (m), z
    /// the contrast, w the share of the ground paved.
    #[uniform(103)]
    pub paving: Vec4,
}

impl MaterialExtension for TerrainExtension {
    fn vertex_shader() -> ShaderRef {
        "embedded://game/terrain_vertex.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://game/terrain.wgsl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://game/terrain_prepass.wgsl".into()
    }
}

#[derive(Resource)]
pub struct TerrainMaterialHandle(pub Handle<TerrainMaterial>);

/// The same surface for structures, with its own panelling.
#[derive(Resource)]
pub struct StructureMaterialHandle(pub Handle<TerrainMaterial>);

#[derive(Component)]
pub struct TerrainColumn;

/// Marks the entity whose position drives streaming.
#[derive(Component, Default)]
pub struct StreamAnchor;

type Key = (u32, IVec2);

/// A generated column, ready to instance.
struct Built {
    mesh: Handle<Mesh>,
    /// Its bounds, computed once (see `bounds`).
    aabb: Aabb,
    /// Collision shape; only the finest level gets one, since that is the
    /// only level the player can reach.
    collider: Option<Collider>,
}

#[derive(Resource, Default)]
pub struct Streamer {
    /// (lod, wrapped key) -> mesh (None when the column is empty).
    cache: HashMap<Key, Option<Built>>,
    pending: HashMap<Key, Task<(ColumnMesh, Option<Collider>)>>,
    /// (lod, unwrapped key) -> instance.
    spawned: HashMap<Key, Entity>,
    /// Instances of a world that has since changed, kept until their
    /// replacements are drawn so the ground never disappears.
    stale: HashMap<Key, Entity>,
    /// True once every column in view has been generated and spawned.
    pub settled: bool,
    /// True once the finest ring round the camera is (enough to stand, walk
    /// and fight on while the distance is still coming in).
    pub near: bool,
}

impl Streamer {
    /// Regenerates everything, for when the world has changed.
    pub fn reset(&mut self) {
        let spawned: Vec<_> = self.spawned.drain().collect();
        self.stale.extend(spawned);
        self.cache.clear();
        self.pending.clear();
        self.settled = false;
        self.near = false;
    }
}

fn setup_material(
    mut commands: Commands,
    args: Res<Args>,
    world: Res<WorldGen>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    // Albedo comes from vertex colours; the material only sets the surface response.
    let make = |glow: f32, grain: f32, relief: f32, paving: f32| ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.2,
            ..default()
        },
        extension: TerrainExtension {
            params: Vec4::new(if args.opt("noao") { 0.0 } else { 1.0 }, 0.0, 0.0, 0.0),
            grain: if args.opt("nograin") {
                Vec4::new(0.0, world.size(), 0.0, 0.0)
            } else {
                Vec4::new(grain, world.size(), relief, 0.0)
            },
            glow: Vec4::new(glow, 0.0, 0.0, 0.0),
            paving: Vec4::new(paving, args.num("tile", 2.0), args.num("paving_contrast", 0.18), args.num("paved", 0.45)),
        },
    };
    // The ground has grain; structures are plain, so their geometry reads.
    // `glow` is the brightness of lit geometry.
    let ground = make(0.0, args.num("grain", 0.2), args.num("relief", 2.5), args.num("paving", 0.0));
    let structures = make(args.num("glow", 3000.0), 0.0, 0.0, 0.0);
    commands.insert_resource(TerrainMaterialHandle(materials.add(ground)));
    commands.insert_resource(StructureMaterialHandle(materials.add(structures)));
}

fn update_material(
    args: Res<Args>,
    terrain: Res<TerrainMaterialHandle>,
    structures: Res<StructureMaterialHandle>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
    anchor: Single<&Transform, With<StreamAnchor>>,
) {
    let curvature = if args.opt("flat") { 0.0 } else { 0.5 / PLANET_RADIUS };
    let p = anchor.translation;
    for handle in [&terrain.0, &structures.0] {
        let Some(mut material) = materials.get_mut(handle) else { continue };
        material.extension.params = Vec4::new(material.extension.params.x, curvature, p.x, p.z);
    }
}

fn wrap_key(key: IVec2, n: i32) -> IVec2 {
    IVec2::new(key.x.rem_euclid(n), key.y.rem_euclid(n))
}

/// Distance from `pos` to the centre of a column.
fn column_distance(lod: u32, key: IVec2, pos: Vec2) -> f32 {
    let size = column_size(lod);
    pos.distance((key.as_vec2() + 0.5) * size)
}

fn children(key: IVec2) -> impl Iterator<Item = IVec2> {
    let base = key * LOD_FACTOR;
    (0..LOD_FACTOR * LOD_FACTOR).map(move |i| base + IVec2::new(i % LOD_FACTOR, i / LOD_FACTOR))
}

struct Coverage<'a> {
    wanted: &'a [HashSet<IVec2>],
    cache: &'a HashMap<Key, Option<Built>>,
    world: &'a WorldGen,
}

impl Coverage<'_> {
    /// Whether a wanted column's area can be drawn: it is generated, or its
    /// children together cover it.
    fn ready(&self, lod: u32, key: IVec2) -> bool {
        self.wanted[lod as usize].contains(&key)
            && (self.cache.contains_key(&(lod, wrap_key(key, self.world.columns_per_side(lod))))
                || self.covered(lod, key))
    }

    /// Whether finer terrain is ready everywhere over this column.
    fn covered(&self, lod: u32, key: IVec2) -> bool {
        lod > 0 && children(key).all(|c| self.ready(lod - 1, c))
    }
}

fn stream_columns(
    mut commands: Commands,
    mut streamer: ResMut<Streamer>,
    world: Res<WorldGen>,
    material: Res<TerrainMaterialHandle>,
    anchor: Single<&Transform, With<StreamAnchor>>,
) {
    // Borrow fields separately.
    let streamer = &mut *streamer;
    let pos = Vec2::new(anchor.translation.x, anchor.translation.z);

    // Columns wanted at each level: those touching the level's disc.
    let mut wanted: Vec<HashSet<IVec2>> = Vec::new();
    for lod in 0..LOD_LEVELS {
        let size = column_size(lod);
        let radius = LOD_RADIUS[lod as usize];
        let center = (pos / size).floor().as_ivec2();
        let r = (radius / size).ceil() as i32 + 1;
        let mut set = HashSet::new();
        for dz in -r..=r {
            for dx in -r..=r {
                let key = center + IVec2::new(dx, dz);
                if column_distance(lod, key, pos) <= radius + size * 0.5 {
                    set.insert(key);
                }
            }
        }
        wanted.push(set);
    }

    // A coarse column is needed unless all its children are well inside the
    // finer disc (the margin makes it ready before they unload), and drawn
    // unless finer terrain covering it is ready.
    let ctx = Coverage { wanted: &wanted, cache: &streamer.cache, world: &world };
    let mut draw: HashSet<Key> = HashSet::new();
    let mut requests: Vec<(f32, Key)> = Vec::new();
    let mut settled = true;
    let mut near = true;
    for lod in 0..LOD_LEVELS {
        let n = world.columns_per_side(lod);
        for &key in &wanted[lod as usize] {
            if lod > 0 {
                let fine = lod - 1;
                let margin = column_size(lod);
                let needed = children(key)
                    .any(|c| column_distance(fine, c, pos) > LOD_RADIUS[fine as usize] - margin);
                if !needed || ctx.covered(lod, key) {
                    continue;
                }
            }
            let wrapped = wrap_key(key, n);
            if streamer.cache.contains_key(&(lod, wrapped)) {
                draw.insert((lod, key));
            } else {
                settled = false;
                near &= lod > 0 || column_distance(lod, key, pos) > NEAR;
                let urgency = column_distance(lod, key, pos) / column_size(lod);
                requests.push((urgency, (lod, wrapped)));
            }
        }
    }
    streamer.settled = settled;
    streamer.near = near;

    // Start the most urgent generation work.
    requests.sort_by(|a, b| a.0.total_cmp(&b.0));
    let pool = AsyncComputeTaskPool::get();
    let max_in_flight = pool.thread_num().max(1) * 2;
    for (_, key) in requests {
        if streamer.pending.len() >= max_in_flight {
            break;
        }
        if streamer.pending.contains_key(&key) {
            continue;
        }
        let world = world.clone();
        let task = pool.spawn(async move {
            let column = world.mesh(key.0, key.1.x, key.1.y);
            let collider = (key.0 == 0).then(|| column_collider(&column)).flatten();
            (column, collider)
        });
        streamer.pending.insert(key, task);
    }

    // Despawn instances no longer drawn (including all of them right after the
    // camera wraps around the world; they respawn from the cache this frame).
    streamer.spawned.retain(|key, entity| {
        let keep = draw.contains(key);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
    for &(lod, key) in &draw {
        if streamer.spawned.contains_key(&(lod, key)) {
            continue;
        }
        let n = world.columns_per_side(lod);
        let Some(Some(built)) = streamer.cache.get(&(lod, wrap_key(key, n))) else { continue };
        if let Some(old) = streamer.stale.remove(&(lod, key)) {
            commands.entity(old).despawn();
        }
        let size = column_size(lod);
        let sink = world.sink(lod);
        let mut entity = commands.spawn((
            TerrainColumn,
            Mesh3d(built.mesh.clone()),
            built.aabb,
            NoAutoAabb,
            MeshMaterial3d(material.0.clone()),
            Transform::from_xyz(key.x as f32 * size, -sink, key.y as f32 * size),
        ));
        if let Some(collider) = &built.collider {
            // Character movement only collides with colliders on bodies.
            entity.insert((RigidBody::Static, collider.clone()));
        }
        let entity = entity.id();
        streamer.spawned.insert((lod, key), entity);
    }

    if settled {
        for (_, old) in streamer.stale.drain() {
            commands.entity(old).despawn();
        }
    }

    // Forget meshes and cancel work for columns well outside their disc.
    let far = |(lod, key): &Key| {
        let n = world.columns_per_side(*lod);
        let size = column_size(*lod);
        let center = wrap_key((pos / size).floor().as_ivec2(), n);
        let d = (*key - center).abs();
        let d = IVec2::new(d.x.min(n - d.x), d.y.min(n - d.y));
        d.max_element() as f32 * size > LOD_RADIUS[*lod as usize] + size * 3.0
    };
    streamer.cache.retain(|key, _| !far(key));
    streamer.pending.retain(|key, _| !far(key));
}

fn receive_meshes(mut streamer: ResMut<Streamer>, mut meshes: ResMut<Assets<Mesh>>) {
    let mut done = Vec::new();
    for (key, task) in streamer.pending.iter_mut() {
        if let Some((column, collider)) = check_ready(task) {
            let built = (!column.is_empty())
                .then(|| Built { aabb: bounds(&column), mesh: meshes.add(to_bevy_mesh(column)), collider });
            done.push((*key, built));
        }
    }
    for (key, built) in done {
        streamer.pending.remove(&key);
        streamer.cache.insert(key, built);
    }
}

/// A mesh's bounds for culling. Bevy would compute them itself, but keeps
/// recomputing them for big meshes every frame (milliseconds with the
/// structures); these meshes never change, so they get them once, with
/// `NoAutoAabb`.
pub fn bounds(mesh: &ColumnMesh) -> Aabb {
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for &p in &mesh.positions {
        lo = lo.min(Vec3::from(p));
        hi = hi.max(Vec3::from(p));
    }
    if lo.x > hi.x {
        return Aabb::from_min_max(Vec3::ZERO, Vec3::ZERO);
    }
    Aabb::from_min_max(lo, hi)
}

/// A static triangle-mesh collider for a column's geometry, if it has any.
pub fn column_collider(column: &ColumnMesh) -> Option<Collider> {
    if column.is_empty() {
        return None;
    }
    let vertices = column.positions.iter().map(|&p| Vec3::from(p)).collect();
    let indices = column.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
    Collider::try_trimesh(vertices, indices)
        .inspect_err(|e| warn!("column collider failed: {e:?}"))
        .ok()
}

pub fn to_bevy_mesh(mut column: ColumnMesh) -> Mesh {
    // Albedo in red, the face's size in green, glow in blue, sky visibility
    // in alpha (read by terrain.wgsl).
    column.face.resize(column.positions.len(), worldgen::mesh::OPEN_GROUND);
    column.glow.resize(column.positions.len(), 0.0);
    let colors: Vec<[f32; 4]> = column
        .albedo
        .iter()
        .zip(&column.ao)
        .zip(column.face.iter().zip(&column.glow))
        .map(|((&a, &v), (&f, &g))| [a, f, g, v])
        .collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, column.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, column.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(column.indices))
}
