//! Screenshots: F12 at any time, or `--shot path.png` to render a single frame
//! once the terrain in view has loaded, save it and exit. `--opt bench`
//! measures frame times once everything in view has loaded, and exits.

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};

use crate::{Args, terrain::Streamer};

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutoShot>()
            .init_resource::<Bench>()
            .init_resource::<Tour>()
            .add_systems(Update, (manual_shot, auto_shot, bench, lab_tour, lab_focus));
    }
}

#[derive(Resource, Default)]
struct AutoShot {
    settled_frames: u32,
    requested: bool,
    saved: bool,
    /// With `--opt shotpair`: frames since the first shot, and whether the
    /// second (a few frames later, `<path>_b.png`) is saved, to see what
    /// flickers between frames.
    after: u32,
    second: bool,
}

#[derive(Resource, Default)]
struct Bench {
    /// When everything had loaded.
    since: Option<f32>,
    frames: Vec<f32>,
}

/// Seconds to let things settle, then to measure.
const BENCH_WARMUP: f32 = 2.0;
const BENCH_TIME: f32 = 5.0;

/// With `--opt bench`: once the terrain and structures in view are built,
/// waits, measures frame times and logs the average and worst, then exits.
fn bench(
    args: Res<Args>,
    time: Res<Time>,
    streamer: Res<Streamer>,
    building: Query<(), With<crate::structures::Building>>,
    mut state: ResMut<Bench>,
    mut exit: MessageWriter<AppExit>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    if !args.opt("bench") {
        return;
    }
    let now = time.elapsed_secs();
    if !streamer.settled || !building.is_empty() {
        state.since = None;
        state.frames.clear();
        return;
    }
    let since = *state.since.get_or_insert(now);
    if now - since < BENCH_WARMUP {
        return;
    }
    if now - since < BENCH_WARMUP + BENCH_TIME {
        state.frames.push(time.delta_secs());
        return;
    }
    let n = state.frames.len().max(1) as f32;
    let average = state.frames.iter().sum::<f32>() / n;
    let worst = state.frames.iter().copied().fold(0.0, f32::max);
    info!("bench: {:.0} fps average ({:.2} ms), worst frame {:.2} ms", 1.0 / average, average * 1000.0, worst * 1000.0);
    // The costliest render passes, on the GPU and the CPU.
    let mut passes: Vec<(String, f64)> = diagnostics
        .iter()
        .filter(|d| d.path().as_str().starts_with("render/") && d.path().as_str().contains("elapsed"))
        .filter_map(|d| Some((d.path().as_str().to_string(), d.average()?)))
        .collect();
    passes.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (path, ms) in passes.iter().take(12) {
        info!("bench: {path} {ms:.2} ms");
    }
    exit.write(AppExit::Success);
}

/// The lab tour (`--opt lab --opt labshots`): every candidate from the same
/// three framings, saved to `screenshots/lab/<name>_<view>.png`.
#[derive(Resource, Default)]
struct Tour {
    /// (name, origin, radius, height) of each candidate, along the row.
    stops: Vec<(String, Vec3, f32, f32)>,
    step: usize,
    frames: u32,
    done_at: Option<u32>,
}

/// The views: name, compass angle (degrees), distance in radii and in
/// heights (the larger wins) plus metres, eye height in heights (plus
/// metres), and the height looked at, in heights. The close view stands a
/// short way off the candidate's edge, to judge detail.
const VIEWS: [(&str, f32, f32, f32, f32, f32, f32, f32); 4] = [
    ("ground", 30.0, 2.2, 0.6, 0.0, 0.0, 2.0, 0.45),
    ("three_quarter", 150.0, 3.0, 1.1, 0.0, 0.45, 10.0, 0.4),
    ("wide", 260.0, 4.5, 1.8, 0.0, 0.6, 20.0, 0.35),
    ("close", 150.0, 1.0, 0.0, 18.0, 0.15, 6.0, 0.2),
];
/// Frames to let the view settle (temporal effects, streaming) per shot.
const TOUR_SETTLE: u32 = 150;

#[allow(clippy::too_many_arguments)]
fn lab_tour(
    mut commands: Commands,
    args: Res<Args>,
    streamer: Res<Streamer>,
    building: Query<(), With<crate::structures::Building>>,
    items: Query<(&crate::structures::LabItem, &Transform, &crate::structures::Detail)>,
    mut tour: ResMut<Tour>,
    camera: Single<(&mut Transform, &mut crate::camera::FlyCam), Without<crate::structures::LabItem>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !args.opt("labshots") {
        return;
    }
    if tour.stops.is_empty() {
        // Start once everything is built.
        if !streamer.settled || !building.is_empty() || items.is_empty() {
            return;
        }
        // With --focus, only the candidates whose names contain it.
        let focus = args.focus.as_deref().unwrap_or("");
        let mut stops: Vec<_> = items
            .iter()
            .filter(|(item, _, _)| item.0.contains(focus))
            .map(|(item, t, d)| (item.0.clone(), t.translation, d.radius, d.height.max(1.0)))
            .collect();
        stops.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        info!("lab tour: {} candidates", stops.len());
        if stops.is_empty() {
            warn!("lab tour: no candidate matches --focus {focus}");
            exit.write(AppExit::Success);
            return;
        }
        tour.stops = stops;
    }
    if let Some(at) = tour.done_at {
        // Let the last screenshots reach the disk.
        tour.frames += 1;
        if tour.frames > at + 30 {
            exit.write(AppExit::Success);
        }
        return;
    }
    let (stop, view) = (tour.step / VIEWS.len(), tour.step % VIEWS.len());
    let (name, origin, radius, height) = tour.stops[stop].clone();
    let label = VIEWS[view].0;
    let (mut transform, mut fly) = camera.into_inner();
    if tour.frames == 0 {
        let (at, yaw, pitch) = view_of(origin, radius, height, view);
        fly.yaw = yaw;
        fly.pitch = pitch;
        transform.translation = at;
        transform.rotation = fly.rotation();
    }
    if !streamer.settled {
        return;
    }
    tour.frames += 1;
    if tour.frames < TOUR_SETTLE {
        return;
    }
    let _ = std::fs::create_dir_all("screenshots/lab");
    let path = format!("screenshots/lab/{name}_{label}.png");
    info!("lab tour: {path}");
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    tour.frames = 0;
    tour.step += 1;
    if tour.step >= tour.stops.len() * VIEWS.len() {
        tour.done_at = Some(0);
    }
}

/// The camera the tour uses for one view of a candidate: position, yaw and
/// pitch.
fn view_of(origin: Vec3, radius: f32, height: f32, view: usize) -> (Vec3, f32, f32) {
    let (_, angle, by_radius, by_height, metres, eye, eye_metres, look) = VIEWS[view];
    let distance = (radius * by_radius).max(height * by_height) + metres;
    let a = angle.to_radians();
    let at = origin + Vec3::new(a.cos(), 0.0, a.sin()) * distance + Vec3::Y * (height * eye + eye_metres);
    let to = origin + Vec3::Y * height * look - at;
    (at, (-to.x).atan2(-to.z), to.y.atan2(Vec2::new(to.x, to.z).length()))
}

/// `--focus name` in the lab: once that candidate is built, the camera
/// moves to the tour's three-quarter view of it (once).
fn lab_focus(
    args: Res<Args>,
    mut done: Local<bool>,
    items: Query<(&crate::structures::LabItem, &Transform, &crate::structures::Detail)>,
    camera: Single<(&mut Transform, &mut crate::camera::FlyCam), Without<crate::structures::LabItem>>,
) {
    let Some(focus) = &args.focus else { return };
    if *done || args.opt("labshots") {
        return;
    }
    let Some((_, t, d)) = items.iter().find(|(item, _, _)| &item.0 == focus) else { return };
    let (at, yaw, pitch) = view_of(t.translation, d.radius, d.height.max(1.0), 1);
    let (mut transform, mut fly) = camera.into_inner();
    fly.yaw = yaw;
    fly.pitch = pitch;
    transform.translation = at;
    transform.rotation = fly.rotation();
    *done = true;
    info!("lab: looking at {focus}");
}

fn manual_shot(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::F12) {
        let _ = std::fs::create_dir_all("screenshots");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(format!("screenshots/{stamp}.png")));
    }
}

fn auto_shot(
    mut commands: Commands,
    args: Res<Args>,
    streamer: Res<Streamer>,
    building: Query<(), With<crate::structures::Building>>,
    mut state: ResMut<AutoShot>,
    mut exit: MessageWriter<AppExit>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    let Some(path) = &args.shot else { return };
    if state.saved && args.opt("shotpair") && !state.second {
        state.after += 1;
        if state.after == 5 {
            let second = path.strip_suffix(".png").map_or(format!("{path}_b"), |p| format!("{p}_b.png"));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(second))
                .observe(|_: On<ScreenshotCaptured>, mut state: ResMut<AutoShot>| state.second = true);
        }
        return;
    }
    if state.saved {
        exit.write(AppExit::Success);
        return;
    }
    if state.requested {
        return;
    }
    // Once the terrain has settled and every structure has been built.
    if streamer.settled && building.is_empty() {
        state.settled_frames += 1;
    }
    // Give temporal effects (TAA, soft shadows, auto exposure) time to converge.
    if state.settled_frames >= 90 {
        state.requested = true;
        if let Some(fps) = diagnostics
            .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|d| d.smoothed())
        {
            info!("{fps:.0} fps when captured");
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()))
            .observe(|_: On<ScreenshotCaptured>, mut state: ResMut<AutoShot>| state.saved = true);
    }
}
