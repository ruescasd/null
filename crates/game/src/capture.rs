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
            .add_systems(Update, (manual_shot, auto_shot, bench));
    }
}

#[derive(Resource, Default)]
struct AutoShot {
    settled_frames: u32,
    requested: bool,
    saved: bool,
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
    mut state: ResMut<AutoShot>,
    mut exit: MessageWriter<AppExit>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    let Some(path) = &args.shot else { return };
    if state.saved {
        exit.write(AppExit::Success);
        return;
    }
    if state.requested {
        return;
    }
    if streamer.settled {
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
