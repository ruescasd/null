//! `show FILE` at the console: a model from `creatures/` stood on the ground
//! before you, in the game's own light, to see how a generated mesh looks
//! before anything is made of it (the user, 2026-11-06: two new meshes, "can
//! we see how they look?"). Static: no rig, no clips. Measured once loaded,
//! scaled to the height asked, its lowest point on the ground, facing you.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

/// A model asked for: its file in `creatures/`, how tall (m), where it
/// stands, which way it faces (radians about up, its +Z towards it), and
/// whether it turns slowly round.
#[derive(Clone, Debug)]
pub struct Asked {
    pub file: String,
    pub height: f32,
    pub at: Vec3,
    pub facing: f32,
    pub spin: bool,
}

/// A model shown: as asked, and whether it is measured and set yet.
#[derive(Component)]
pub struct Shown {
    asked: Asked,
    set: bool,
}

/// How fast a spinning model turns (radians/s): once round in 20 s.
const SPIN: f32 = std::f32::consts::TAU / 20.0;

/// The `.glb` files in `creatures/`, to choose from.
pub fn files() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(crate::combat::creatures_folder()).into_iter().flatten().flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.to_ascii_lowercase().ends_with(".glb")).collect();
    out.sort();
    out
}

/// Each asked for at the console, loaded hidden at its own size, to be
/// measured (`set`); `show off`, those shown gone.
fn take(mut commands: Commands, server: Res<AssetServer>, mut spawns: ResMut<crate::console::Spawns>, shown: Query<Entity, With<Shown>>) {
    if std::mem::take(&mut spawns.unshow) {
        for e in &shown {
            commands.entity(e).despawn();
        }
    }
    for asked in spawns.shows.drain(..) {
        let at = format!("creatures://{}", asked.file);
        commands.spawn((Shown { asked, set: false }, WorldAssetRoot(server.load(GltfAssetLabel::Scene(0).from_asset(at))), Transform::IDENTITY, Visibility::Hidden));
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (take, set).chain());
}

/// Once its meshes are in: its bounds measured (in its own frame), scaled
/// to its height, its lowest point stood on the ground where asked, turned
/// to face as asked, and shown; said in the log. Spinning ones turned.
pub fn set(time: Res<Time>, mut shown: Query<(Entity, &mut Shown, &mut Transform, &mut Visibility)>, children: Query<&Children>, meshes: Query<(&Aabb, &GlobalTransform)>, globals: Query<&GlobalTransform>, world: Option<Res<crate::terrain::WorldGen>>) {
    for (entity, mut s, mut t, mut visible) in &mut shown {
        if s.set {
            if s.asked.spin {
                t.rotate_y(SPIN * time.delta_secs());
            }
            continue;
        }
        let Ok(root) = globals.get(entity) else { continue };
        let into = root.affine().inverse();
        let (mut low, mut high, mut any) = (Vec3::MAX, Vec3::MIN, false);
        for e in children.iter_descendants(entity) {
            let Ok((aabb, g)) = meshes.get(e) else { continue };
            let (c, h) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            for k in 0..8 {
                let corner = c + h * Vec3::new(if k & 1 == 0 { -1.0 } else { 1.0 }, if k & 2 == 0 { -1.0 } else { 1.0 }, if k & 4 == 0 { -1.0 } else { 1.0 });
                let p = into.transform_point3(g.transform_point(corner));
                low = low.min(p);
                high = high.max(p);
            }
            any = true;
        }
        if !any {
            continue;
        }
        let size = high - low;
        let scale = s.asked.height / size.y.max(1e-4);
        let at = s.asked.at;
        let ground = world.as_ref().map_or(at.y, |w| w.ground_height(at.x, at.z));
        let rotation = Quat::from_rotation_y(s.asked.facing);
        let middle = Vec3::new((low.x + high.x) * 0.5, low.y, (low.z + high.z) * 0.5);
        *t = Transform { translation: Vec3::new(at.x, ground, at.z) - rotation * middle * scale, rotation, scale: Vec3::splat(scale) };
        *visible = Visibility::Inherited;
        s.set = true;
        info!("show {}: {:.2} x {:.2} x {:.2} in its units (x y z), shown {:.2} m tall, {:.2} m wide, {:.2} m deep", s.asked.file, size.x, size.y, size.z, s.asked.height, size.x * scale, size.z * scale);
    }
}
