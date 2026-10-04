//! Structures from the hand-edited library in `data/structures.ron` (see
//! `worldgen::structure`): the test placements it lists, and the sites that
//! grow out of the world by its rules (`worldgen::sites`), streamed in around
//! the camera. The file is watched: saving it rebuilds every structure within
//! a second, and mistakes are shown on screen while the last good version
//! stays up.

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};

use avian3d::prelude::{Collider, Position, RigidBody, Rotation};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};
use worldgen::{
    ColumnMesh,
    plates::PlateWorld,
    dressing::{self, Tube},
    forms::{self, Prism},
    lab,
    sites::{self, Layer},
    structure::{self, Library, Solid},
};

use crate::{
    Args,
    camera::FlyCam,
    landmarks::Landmark,
    terrain::{StreamSet, Streamer, StructureMaterialHandle, WorldGen, bounds, to_bevy_mesh},
};
use bevy::camera::visibility::NoAutoAabb;

pub struct StructuresPlugin;

impl Plugin for StructuresPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Watch>()
            .add_systems(Startup, setup_notice)
            .add_systems(
                Update,
                (watch, stream_sites.after(watch), receive, choose_levels.after(StreamSet), fade_notice),
            );
    }
}

/// Blocks per structure at most, to keep a typo from freezing the game.
const MAX_LEAVES: usize = 40_000;

/// Each layer of sites is built within the first distance of the camera
/// and dropped beyond the second: colossi are seen from much further.
const LAYERS: [(Layer, f32, f32); 2] = [(Layer::Sites, 2500.0, 2900.0), (Layer::Colossi, 4500.0, 5000.0)];

/// Anything built from the library, rebuilt when it changes.
#[derive(Component)]
struct Structure;

/// A site, by its layer and grid cell.
#[derive(Component)]
struct Site(Layer, (i32, i32));

/// Plate pieces narrower than these are left out of the coarser versions
/// of a structure's plate buildings (box structures are always drawn whole:
/// fewer levels of their rules change their outline, which shows)...
const LEVEL_MIN_WIDTH: [f32; 2] = [3.0, 8.0];
/// ...shown beyond these distances from the camera (to the structure's edge).
const LEVEL_DISTANCE: [f32; 2] = [350.0, 900.0];

/// A structure built in the background: the mesh of its box solids (always
/// shown), its plate pieces' meshes from fine to coarse, the collider and
/// the structure's radius.
type Levels = (ColumnMesh, Vec<ColumnMesh>, Option<Collider>, f32, f32);

/// A structure still being built in the background.
#[derive(Component)]
pub struct Building(Task<Levels>);

/// A structure's versions are its children; `radius` is how far it reaches
/// from its origin.
#[derive(Component)]
pub struct Detail {
    pub radius: f32,
    /// How high it reaches above its origin.
    pub height: f32,
}

/// A candidate in the lab (`--opt lab`), by name.
#[derive(Component)]
pub struct LabItem(pub String);

/// Which version a child mesh is.
#[derive(Component)]
struct Level(usize);

/// A structure's collider, kept aside while the structure is far away:
/// keeping thousands of pieces in the physics world costs every frame.
#[derive(Component)]
struct Solidity {
    collider: Collider,
    active: bool,
}

/// Colliders are active within this distance of the camera (to the
/// structure's edge), and put aside beyond the second.
const COLLIDE_DISTANCE: [f32; 2] = [300.0, 400.0];

#[derive(Component)]
struct Notice {
    shown_at: f32,
}

#[derive(Resource, Default)]
struct Watch {
    modified: Option<SystemTime>,
    last_check: f32,
    /// The last library that loaded.
    library: Option<Arc<Library>>,
    last_stream: f32,
}

/// The data file: `data/structures.ron` from the working directory if it is
/// there, else next to this crate's workspace.
fn data_path() -> PathBuf {
    let local = PathBuf::from("data/structures.ron");
    if local.exists() {
        return local;
    }
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron"))
}

/// Reads and checks the data file.
pub fn load_library() -> Result<Library, String> {
    let path = data_path();
    std::fs::read_to_string(&path)
        .map_err(|e| format!("{}: {e}", path.display()))
        .and_then(|t| Library::parse(&t))
}

fn setup_notice(mut commands: Commands) {
    commands.spawn((
        Notice { shown_at: f32::MIN },
        Text::new(""),
        TextFont { font_size: FontSize::Px(15.0), ..default() },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: px(8),
            right: px(12),
            max_width: px(700),
            ..default()
        },
    ));
}

/// Checks the file twice a second; on change, rebuilds the structures or
/// reports what is wrong.
#[allow(clippy::too_many_arguments)]
fn watch(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    world: Res<WorldGen>,
    mut state: ResMut<Watch>,
    mut streamer: ResMut<Streamer>,
    existing: Query<Entity, With<Structure>>,
    mut notice: Single<(&mut Text, &mut Notice)>,
) {
    if args.opt("nofractals") {
        return;
    }
    let now = time.elapsed_secs();
    if now - state.last_check < 0.5 {
        return;
    }
    state.last_check = now;
    let path = data_path();
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    if modified == state.modified {
        return;
    }
    state.modified = modified;

    let (text, note) = &mut *notice;
    note.shown_at = now;
    let library = match load_library() {
        Ok(library) => Arc::new(library),
        Err(error) => {
            warn!("{}: {error}", path.display());
            text.0 = format!("structures.ron: {error}");
            // Errors stay up until the file is fixed.
            note.shown_at = f32::INFINITY;
            return;
        }
    };
    // Sites shape the ground: if they changed, the terrain is regenerated
    // (what is shown stays up until its replacement is ready).
    if let WorldGen::Plates(old) = &*world
        && !args.opt("nosites")
        && !args.opt("lab")
    {
        let new = PlateWorld::new(old.size(), args.seed).with_sites(&library);
        if !old.same_ground(&new) {
            info!("sites changed: regenerating the terrain");
            commands.insert_resource(WorldGen::Plates(Arc::new(new)));
            streamer.reset();
        }
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let pool = AsyncComputeTaskPool::get();
    // The lab: its candidates in a row on flat ground, nothing else.
    if args.opt("lab") {
        let entries = lab::layout(&library);
        for (entry, (x, z)) in entries.iter().cloned() {
            let origin = Vec3::new(x, world.ground_height(x, z), z);
            let library = library.clone();
            let name = entry.label();
            let task = pool.spawn(async move {
                let parts = lab::build(&library, &entry, MAX_LEAVES);
                finish(parts.solids, parts.prisms, parts.tubes)
            });
            commands.spawn((
                Structure,
                LabItem(name),
                Building(task),
                Landmark { origin },
                Transform::from_translation(origin),
            ));
        }
        state.library = Some(library.clone());
        text.0 = format!("structures.ron loaded: {} lab candidates", entries.len());
        info!("{}: {} lab candidates", path.display(), entries.len());
        return;
    }
    for placement in library.structures.iter().cloned() {
        let (x, z) = placement.at;
        let origin = Vec3::new(x, world.ground_height(x, z), z);
        let library = library.clone();
        let task = pool.spawn(async move { finish(structure::build(&library, &placement, MAX_LEAVES), Vec::new(), Vec::new()) });
        commands.spawn((Structure, Building(task), Landmark { origin }, Transform::from_translation(origin)));
    }
    // Sites stream back in on their own.
    state.library = Some(library.clone());
    state.last_stream = f32::MIN;
    text.0 = format!(
        "structures.ron loaded: {} structures, {} site rules",
        library.structures.len(),
        library.sites.len()
    );
    info!("{}: {} structures, {} site rules", path.display(), library.structures.len(), library.sites.len());
}

/// Meshes (box solids; plate pieces fine to coarse) and a collider: a box,
/// wedge or hull per piece, robust for the player.
fn finish(solids: Vec<Solid>, prisms: Vec<Prism>, tubes: Vec<Tube>) -> Levels {
    let mut fixed = structure::mesh(&solids);
    dressing::mesh_tubes(&mut fixed, &tubes);
    let mut levels = vec![ColumnMesh::default()];
    forms::mesh_into(&mut levels[0], &prisms);
    for min_width in LEVEL_MIN_WIDTH {
        let mut mesh = ColumnMesh::default();
        forms::mesh_into(&mut mesh, &sites::wide_prisms(&prisms, min_width));
        levels.push(mesh);
    }
    let radius = fixed
        .positions
        .iter()
        .chain(&levels[0].positions)
        .map(|p| Vec2::new(p[0], p[2]).length())
        .fold(0.0, f32::max);
    let height = fixed.positions.iter().chain(&levels[0].positions).map(|p| p[1]).fold(0.0, f32::max);
    let hulls = prisms
        .iter()
        .map(|p| p.hull_points())
        .chain(tubes.iter().map(|t| t.hull_points()))
        .filter_map(|points| Some((Position(Vec3::ZERO), Rotation::default(), Collider::convex_hull(points)?)));
    let shapes: Vec<(Position, Rotation, Collider)> = solids
        .iter()
        .filter_map(|s| {
            let shape = if s.round {
                Collider::convex_hull(structure::round_points(s.half))?
            } else if s.wedge {
                Collider::convex_hull(structure::wedge_points(s.half))?
            } else {
                Collider::cuboid(s.half.x * 2.0, s.half.y * 2.0, s.half.z * 2.0)
            };
            Some((Position(s.center), Rotation(s.rotation), shape))
        })
        .chain(hulls)
        .collect();
    let collider = (!shapes.is_empty()).then(|| Collider::compound(shapes));
    (fixed, levels, collider, radius, height)
}

/// Puts finished structures in the world: one child mesh per version.
fn receive(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut Building)>,
    material: Res<StructureMaterialHandle>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, mut task) in &mut tasks {
        let Some((fixed, levels, collider, radius, height)) = check_ready(&mut task.0) else { continue };
        let mut e = commands.entity(entity);
        e.remove::<Building>().insert((Detail { radius, height }, Visibility::default()));
        if let Some(collider) = collider {
            e.insert(Solidity { collider, active: false });
        }
        if !fixed.is_empty() {
            e.with_child((
                bounds(&fixed),
                NoAutoAabb,
                Mesh3d(meshes.add(to_bevy_mesh(fixed))),
                MeshMaterial3d(material.0.clone()),
                Transform::default(),
            ));
        }
        for (i, mesh) in levels.into_iter().enumerate() {
            if mesh.is_empty() {
                continue;
            }
            e.with_child((
                Level(i),
                bounds(&mesh),
                NoAutoAabb,
                Mesh3d(meshes.add(to_bevy_mesh(mesh))),
                MeshMaterial3d(material.0.clone()),
                Transform::default(),
                if i == 0 { Visibility::Inherited } else { Visibility::Hidden },
            ));
        }
    }
}

/// Shows each structure's version for its distance from the camera, and
/// makes it solid only when near.
fn choose_levels(
    mut commands: Commands,
    camera: Single<&Transform, With<FlyCam>>,
    mut structures: Query<(Entity, &Transform, &Detail, &Children, Option<&mut Solidity>), Without<FlyCam>>,
    mut levels: Query<(&Level, &mut Visibility)>,
) {
    let cam = camera.translation;
    for (entity, transform, detail, children, solidity) in &mut structures {
        let to = transform.translation - cam;
        let distance = (Vec2::new(to.x, to.z).length() - detail.radius).max(0.0);
        if let Some(mut solid) = solidity {
            if !solid.active && distance < COLLIDE_DISTANCE[0] {
                solid.active = true;
                commands.entity(entity).insert((RigidBody::Static, solid.collider.clone()));
            } else if solid.active && distance > COLLIDE_DISTANCE[1] {
                solid.active = false;
                commands.entity(entity).remove::<(RigidBody, Collider)>();
            }
        }
        let level = LEVEL_DISTANCE.iter().filter(|&&d| distance > d).count();
        // The coarsest version there is, if this one was empty.
        let present: Vec<usize> = children.iter().filter_map(|c| levels.get(c).ok().map(|(l, _)| l.0)).collect();
        let shown = present.iter().copied().filter(|&l| l <= level).max().unwrap_or(0);
        for child in children.iter() {
            if let Ok((l, mut visibility)) = levels.get_mut(child) {
                let want = if l.0 == shown { Visibility::Inherited } else { Visibility::Hidden };
                visibility.set_if_neq(want);
            }
        }
    }
}

/// Twice a second: builds the sites that came into range in the background
/// and drops those that left it.
fn stream_sites(
    mut commands: Commands,
    time: Res<Time>,
    args: Res<Args>,
    world: Res<WorldGen>,
    mut state: ResMut<Watch>,
    camera: Single<&Transform, With<FlyCam>>,
    existing: Query<(Entity, &Site, &Landmark)>,
) {
    let WorldGen::Plates(plates) = &*world else { return };
    let Some(library) = state.library.clone() else { return };
    if args.opt("nosites") || args.opt("lab") {
        return;
    }
    let now = time.elapsed_secs();
    if now - state.last_stream < 0.5 {
        return;
    }
    state.last_stream = now;
    let size = world.size();
    let cam = camera.translation;
    let distance = |x: f32, z: f32| {
        let wrap = |d: f32| d - (d / size).round() * size;
        wrap(x - cam.x).hypot(wrap(z - cam.z))
    };
    let mut have = HashSet::new();
    for (entity, site, landmark) in &existing {
        let drop = LAYERS.iter().find(|l| l.0 == site.0).map_or(0.0, |l| l.2);
        if distance(landmark.origin.x, landmark.origin.z) > drop {
            commands.entity(entity).despawn();
        } else {
            have.insert((site.0, site.1));
        }
    }
    let pool = AsyncComputeTaskPool::get();
    let wanted = LAYERS
        .iter()
        .flat_map(|&(layer, radius, _)| sites::near_in(layer, &library, plates, cam.x, cam.z, radius).into_iter().map(move |s| (s, radius)));
    for (site, radius) in wanted {
        let (x, z) = site.at;
        if distance(x, z) > radius || !have.insert((site.layer, site.cell)) {
            continue;
        }
        // The height it stands at is only known once built, so the entity
        // sits at the ground at its centre and the solids are lifted.
        let ground = plates.height_at(x, z);
        let key = Site(site.layer, site.cell);
        let (library, plates) = (library.clone(), plates.clone());
        let task = pool.spawn(async move {
            let built = sites::build(&library, &plates, &site, MAX_LEAVES);
            let lift = built.base - ground;
            let solids = built.solids.into_iter().map(|s| Solid { center: s.center + Vec3::Y * lift, ..s }).collect();
            let prisms = built.prisms.into_iter().map(|p| Prism { y0: p.y0 + lift, y1: p.y1 + lift, ..p }).collect();
            finish(solids, prisms, Vec::new())
        });
        let origin = Vec3::new(x, ground, z);
        commands.spawn((
            Structure,
            key,
            Building(task),
            Landmark { origin },
            Transform::from_translation(origin),
        ));
    }
}

fn fade_notice(time: Res<Time>, mut notice: Single<(&mut TextColor, &Notice)>) {
    let (color, note) = &mut *notice;
    let age = time.elapsed_secs() - note.shown_at;
    let alpha = (1.0 - (age - 6.0) / 2.0).clamp(0.0, 1.0);
    color.0 = Color::srgba(0.9, 0.9, 0.9, alpha);
}
