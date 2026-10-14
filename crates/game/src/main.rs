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
mod loading;
/// Every option name the game reads, gathered from its source by `build.rs`.
mod options {
    include!(concat!(env!("OUT_DIR"), "/options.rs"));
}
mod look;
mod player;
mod rig;
mod rope;
mod cloth;
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

    /// The names given (`--opt`, `--set`) that nothing reads, each with the
    /// nearest that is (see `options::KNOWN`).
    fn unknown(&self) -> Vec<(String, Option<&'static str>)> {
        let given = self.opts.iter().chain(self.sets.iter().map(|(k, _)| k));
        let mut out: Vec<(String, Option<&'static str>)> = given
            .filter(|name| !options::KNOWN.contains(&name.as_str()))
            .map(|name| {
                let near = options::KNOWN.iter().map(|k| (console::distance(k, name), *k)).filter(|(d, _)| *d <= 3).min().map(|(_, k)| k);
                (name.clone(), near)
            })
            .collect();
        out.dedup_by(|a, b| a.0 == b.0);
        out
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
                // `--set name=value`, or `--set name value`; a switch as at
                // the console, `--set god on` (the same as `--opt god`), or
                // off (the user: the command line's settings all one way).
                "--set" => {
                    let first = it.next().unwrap_or_default();
                    let (k, v) = match first.split_once('=') {
                        Some((k, v)) => (k.to_string(), v.to_string()),
                        None => (first.clone(), it.next().unwrap_or_default()),
                    };
                    match (v.as_str(), v.parse()) {
                        (_, _) if k.is_empty() => eprintln!("--set {first}: expected a name"),
                        ("on", _) => args.opts.push(k),
                        ("off", _) => args.opts.retain(|o| *o != k),
                        (_, Ok(v)) => args.sets.push((k, v)),
                        _ => eprintln!("--set {first}: expected name=value (or name value) with a number, or on or off"),
                    }
                }
                "--seed" => args.seed = it.next().and_then(|s| s.parse().ok()).unwrap_or(1),
                other => eprintln!("ignoring unknown argument {other}"),
            }
        }
        args
    }
}

/// When the process started (`--opt startlog`).
pub(crate) static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// `--opt startlog`: from the process's start, each half second
/// the frames, the worst, whether the ground has settled.
fn startlog(args: Res<Args>, time: Res<Time>, streamer: Res<terrain::Streamer>, mut state: Local<(f32, u32, f32, bool)>) {
    if !args.opt("startlog") {
        return;
    }
    let since = STARTED.get().unwrap().elapsed().as_secs_f32();
    let dt = time.delta_secs();
    if !state.3 {
        state.3 = true;
        state.0 = since;
        info!("startlog: first frame at {since:.2} s");
        return;
    }
    state.1 += 1;
    state.2 = state.2.max(dt);
    if since - state.0 >= 0.5 {
        info!("startlog: {since:5.2} s  {:4.0} fps  worst {:6.1} ms  settled {}", state.1 as f32 / (since - state.0), state.2 * 1000.0, streamer.settled);
        *state = (since, 0, 0.0, true);
    }
}

fn main() {
    STARTED.get_or_init(std::time::Instant::now);
    // `--check ...`: the chasm's checks over many seeds, nothing rendered.
    let argv: Vec<String> = std::env::args().collect();
    if argv.iter().any(|a| a == "--check") {
        check::run(&argv);
    }
    let args = Args::parse();
    // A name nothing reads is a mistake (the user: a mistyped `--set` did
    // nothing, silently): said so, with the nearest, and stopped.
    let unknown = args.unknown();
    if !unknown.is_empty() {
        for (name, near) in &unknown {
            eprintln!("error: no option is called {name}{}", near.map_or(String::new(), |n| format!(" (did you mean {n}?)")));
        }
        std::process::exit(2);
    }
    // Measurements and captures run flat out even without focus (Bevy
    // otherwise slows an unfocused window to 60 fps).
    let unattended = args.unattended();
    let mut app = App::new();

    // (And flat out while hidden, before the window is shown: see
    // `loading.rs`, which then slows it unfocused again.)
    app.insert_resource(bevy::winit::WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: bevy::winit::UpdateMode::Continuous,
    });
    app
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "terrain".into(),
                resolution: (1600, 900).into(),
                position: bevy::window::WindowPosition::Centered(bevy::window::MonitorSelection::Primary),
                present_mode: PresentMode::AutoNoVsync,
                // (Shown only once the view has settled (see `loading.rs`);
                // unattended, never, so whoever is at the desk can work on:
                // captures read what it renders, shown or not.)
                visible: false,
                focused: false,
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
            loading::LoadingPlugin,
        ))
        .add_systems(Update, startlog)
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
