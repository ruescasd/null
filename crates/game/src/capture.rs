//! Screenshots: F12 at any time, or `--shot path.png` to render a single frame
//! once the terrain in view has loaded, save it and exit.

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};

use crate::{Args, terrain::Streamer};

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutoShot>().add_systems(Update, (manual_shot, auto_shot));
    }
}

#[derive(Resource, Default)]
struct AutoShot {
    settled_frames: u32,
    requested: bool,
    saved: bool,
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
