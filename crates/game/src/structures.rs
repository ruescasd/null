//! Structures from the hand-edited library in `data/structures.ron` (see
//! `worldgen::structure`). The file is watched: saving it rebuilds every
//! structure within a second, and mistakes are shown on screen while the
//! last good version stays up.

use std::{
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};

use avian3d::prelude::{Collider, Position, Rotation};
use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
use worldgen::structure::{self, Library};

use crate::{
    Args,
    landmarks::{FractalTask, Landmark},
    terrain::WorldGen,
};

pub struct StructuresPlugin;

impl Plugin for StructuresPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Watch>()
            .add_systems(Startup, setup_notice)
            .add_systems(Update, (watch, fade_notice));
    }
}

/// Blocks per structure at most, to keep a typo from freezing the game.
const MAX_LEAVES: usize = 40_000;

#[derive(Component)]
struct Structure;

#[derive(Component)]
struct Notice {
    shown_at: f32,
}

#[derive(Resource, Default)]
struct Watch {
    modified: Option<SystemTime>,
    last_check: f32,
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
        let task = pool.spawn(async move {
            let solids = structure::build(&library, &placement, MAX_LEAVES);
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
        });
        commands.spawn((Structure, FractalTask(task), Landmark { origin }, Transform::from_translation(origin)));
    }
    text.0 = format!("structures.ron loaded: {} structures", library.structures.len());
    info!("{}: {} structures", path.display(), library.structures.len());
}

fn fade_notice(time: Res<Time>, mut notice: Single<(&mut TextColor, &Notice)>) {
    let (color, note) = &mut *notice;
    let age = time.elapsed_secs() - note.shown_at;
    let alpha = (1.0 - (age - 6.0) / 2.0).clamp(0.0, 1.0);
    color.0 = Color::srgba(0.9, 0.9, 0.9, alpha);
}
