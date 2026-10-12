//! Prototype: run, strafe jump and thrust through a wrapping procedural world.

mod camera;
mod capture;
mod chasm;
mod check;
mod console;
mod combat;
mod ground;
mod figure;
mod landmarks;
mod look;
mod player;
mod rig;
mod rope;
mod structures;
mod terrain;

use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, window::PresentMode};

/// Command-line options. All optional:
/// `--cam x,y,z,yaw,pitch` (y relative to the ground, angles in degrees)
/// `--time t` simulation time in seconds, which sets where the suns are
/// `--shot path.png` render one frame once terrain has loaded, save it and exit
/// `--seed n`
/// `--focus name` in the lab, start looking at that candidate
/// `--opt name` (repeatable) switch a rendering experiment on or off
/// `--set name=value` or `--set name value` (repeatable) override a tuning number
#[derive(Resource, Clone, Debug)]
pub struct Args {
    pub opts: Vec<String>,
    pub sets: Vec<(String, f32)>,
    pub cam: Option<[f32; 5]>,
    pub time: Option<f32>,
    pub shot: Option<String>,
    pub seed: u32,
    pub focus: Option<String>,
}

impl Args {
    pub fn opt(&self, name: &str) -> bool {
        self.opts.iter().any(|o| o == name)
    }

    /// A measurement or capture, running with nobody playing.
    pub fn unattended(&self) -> bool {
        self.opt("bench") || self.shot.is_some() || self.opt("bot") || self.opt("stairbot") || self.opt("pushbot") || self.opt("walkbot") || self.opt("wallbot") || self.opt("labshots") || self.opt("roomshots") || self.opt("viewbench")
    }

    /// A tuning number, overridable with `--set name=value`.
    pub fn num(&self, name: &str, default: f32) -> f32 {
        self.sets.iter().rev().find(|(k, _)| k == name).map_or(default, |(_, v)| *v)
    }

    fn parse() -> Self {
        let mut args =
            Args { opts: Vec::new(), sets: Vec::new(), cam: None, time: None, shot: None, seed: 1, focus: None };
        let mut it = std::env::args().skip(1);
        let floats = |s: Option<String>| -> Vec<f32> {
            s.unwrap_or_default().split(',').filter_map(|v| v.trim().parse().ok()).collect()
        };
        while let Some(flag) = it.next() {
            match flag.as_str() {
                "--cam" => args.cam = floats(it.next()).try_into().ok(),
                "--time" => args.time = it.next().and_then(|s| s.parse().ok()),
                "--shot" => args.shot = it.next(),
                "--focus" => args.focus = it.next(),
                "--opt" => args.opts.extend(it.next()),
                // `--set name=value`, or `--set name value`.
                "--set" => {
                    let first = it.next().unwrap_or_default();
                    let (k, v) = match first.split_once('=') {
                        Some((k, v)) => (k.to_string(), v.to_string()),
                        None => (first.clone(), it.next().unwrap_or_default()),
                    };
                    match v.parse() {
                        Ok(v) if !k.is_empty() => args.sets.push((k, v)),
                        _ => eprintln!("--set {first}: expected name=value (or name value) with a number"),
                    }
                }
                "--seed" => args.seed = it.next().and_then(|s| s.parse().ok()).unwrap_or(1),
                other => eprintln!("ignoring unknown argument {other}"),
            }
        }
        args
    }
}

fn main() {
    // `--check ...`: the chasm's checks over many seeds, nothing rendered.
    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().any(|a| a == "--check") {
        check::run(&argv);
    }
    let args = Args::parse();
    // Measurements and captures run flat out even without focus (Bevy
    // otherwise slows an unfocused window to 60 fps).
    let unattended = args.unattended();
    let mut app = App::new();

    if unattended {
        app.insert_resource(bevy::winit::WinitSettings {
            focused_mode: bevy::winit::UpdateMode::Continuous,
            unfocused_mode: bevy::winit::UpdateMode::Continuous,
        });
    }
    app
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "terrain".into(),
                resolution: (1600, 900).into(),
                position: bevy::window::WindowPosition::Centered(bevy::window::MonitorSelection::Primary),
                present_mode: PresentMode::AutoNoVsync,
                // (Unattended, it is never shown, so whoever is at the desk
                // can work on: captures read what it renders, shown or not.)
                visible: !unattended,
                focused: !unattended,
                ..default()
            }),
            ..default()
        // (Draws prepared on the CPU, not by Bevy's compute passes for
        // every view (`--opt gpubatch`, Bevy's way, for scenes of very many
        // meshes): with ours, each shadow view's passes were a fixed cost,
        // and with eight lamps casting shadows the region ran at 57-74
        // frames a second, 98-136 so. See `CpuBatching`.)
        }).set(bevy::pbr::PbrPlugin { use_gpu_instance_buffer_builder: args.opt("gpubatch"), ..default() })
        // (Silent when nobody is playing, a bench or a bot or a capture, or
        // with `--opt mute`: the hunters' cries sounded through the house
        // while it was measured.)
        .set(bevy::audio::AudioPlugin { global_volume: bevy::audio::GlobalVolume::new(bevy::audio::Volume::Linear(if unattended || args.opt("mute") { 0.0 } else { 1.0 })), ..default() }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default());
    if !args.opt("gpubatch") {
        app.add_plugins(CpuBatching);
    }
    // (Timings of the render passes only to measure (`--opt diag`, and the
    // benches): they cost themselves, a millisecond, a query round each
    // pass.)
    if args.opt("diag") || args.opt("bench") || args.opt("viewbench") {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    app.insert_resource(args)
        .add_plugins((
            terrain::TerrainPlugin,
            camera::FlyCameraPlugin,
            look::LookPlugin,
            capture::CapturePlugin,
            combat::CombatPlugin,
            figure::FigurePlugin,
            landmarks::LandmarksPlugin,
            player::PlayerPlugin,
            structures::StructuresPlugin,
            chasm::ChasmPlugin,
            console::ConsolePlugin,
        ))
        .run();
}

/// Draws prepared on the CPU in every view: with Bevy's GPU instance
/// building off (see `PbrPlugin`), the views must be told too, or each
/// still expects what the GPU builds, and nothing is drawn. Told as each
/// frame is extracted (what the GPU supports is found once the device is
/// ready, after the plugins are set up, and would overwrite it).
struct CpuBatching;

impl Plugin for CpuBatching {
    fn build(&self, app: &mut App) {
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.add_systems(bevy::render::ExtractSchedule, cpu_batching);
        }
    }
}

fn cpu_batching(support: Option<ResMut<bevy::render::batching::gpu_preprocessing::GpuPreprocessingSupport>>) {
    use bevy::render::batching::gpu_preprocessing::GpuPreprocessingMode;
    if let Some(mut support) = support
        && support.max_supported_mode != GpuPreprocessingMode::None
    {
        support.max_supported_mode = GpuPreprocessingMode::None;
    }
}
