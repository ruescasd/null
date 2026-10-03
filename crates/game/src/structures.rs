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

use avian3d::prelude::{Collider, Position, Rotation};
use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
use worldgen::{
    ColumnMesh,
    sites,
    structure::{self, Library, Solid},
};

use crate::{
    Args,
    camera::FlyCam,
    landmarks::{FractalTask, Landmark},
    terrain::WorldGen,
};

pub struct StructuresPlugin;

impl Plugin for StructuresPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Watch>()
            .add_systems(Startup, setup_notice)
            .add_systems(Update, (watch, stream_sites.after(watch), fade_notice));
    }
}

/// Blocks per structure at most, to keep a typo from freezing the game.
const MAX_LEAVES: usize = 40_000;

/// Sites are built within this distance of the camera...
const SITE_RADIUS: f32 = 2500.0;
/// ...and dropped beyond this one.
const SITE_DROP: f32 = 2900.0;

/// Anything built from the library, rebuilt when it changes.
#[derive(Component)]
struct Structure;

/// A site, by its grid cell.
#[derive(Component)]
struct Site((i32, i32));

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
    let library = match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Library::parse(&t)) {
        Ok(library) => Arc::new(library),
        Err(error) => {
            warn!("{}: {error}", path.display());
            text.0 = format!("structures.ron: {error}");
            // Errors stay up until the file is fixed.
            note.shown_at = f32::INFINITY;
            return;
        }
    };
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let pool = AsyncComputeTaskPool::get();
    for placement in library.structures.iter().cloned() {
        let (x, z) = placement.at;
        let origin = Vec3::new(x, world.ground_height(x, z), z);
        let library = library.clone();
        let task = pool.spawn(async move { finish(structure::build(&library, &placement, MAX_LEAVES)) });
        commands.spawn((Structure, FractalTask(task), Landmark { origin }, Transform::from_translation(origin)));
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

/// Mesh and colliders for built solids.
fn finish(solids: Vec<Solid>) -> (ColumnMesh, Option<Collider>) {
    let mesh = structure::mesh(&solids);
    // A box or wedge collider per solid: robust for the player.
    let shapes: Vec<(Position, Rotation, Collider)> = solids
        .iter()
        .filter_map(|s| {
            let shape = if s.wedge {
                Collider::convex_hull(structure::wedge_points(s.half))?
            } else {
                Collider::cuboid(s.half.x * 2.0, s.half.y * 2.0, s.half.z * 2.0)
            };
            Some((Position(s.center), Rotation(s.rotation), shape))
        })
        .collect();
    let collider = (!shapes.is_empty()).then(|| Collider::compound(shapes));
    (mesh, collider)
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
    if args.opt("nosites") {
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
        if distance(landmark.origin.x, landmark.origin.z) > SITE_DROP {
            commands.entity(entity).despawn();
        } else {
            have.insert(site.0);
        }
    }
    let pool = AsyncComputeTaskPool::get();
    for site in sites::near(&library, plates, cam.x, cam.z, SITE_RADIUS) {
        let (x, z) = site.placement.at;
        if distance(x, z) > SITE_RADIUS || !have.insert(site.cell) {
            continue;
        }
        // The height it stands at is only known once built, so the entity
        // sits at the ground at its centre and the solids are lifted.
        let ground = plates.height_at(x, z);
        let cell = site.cell;
        let (library, plates) = (library.clone(), plates.clone());
        let task = pool.spawn(async move {
            let built = sites::build(&library, &plates, &site, MAX_LEAVES);
            let lift = Vec3::Y * (built.base - ground);
            finish(built.solids.into_iter().map(|s| Solid { center: s.center + lift, ..s }).collect())
        });
        let origin = Vec3::new(x, ground, z);
        commands.spawn((
            Structure,
            Site(cell),
            FractalTask(task),
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
