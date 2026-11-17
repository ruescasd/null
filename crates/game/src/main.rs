//! Prototype: run, strafe jump and thrust through a wrapping procedural world.

mod camera;
mod capture;
mod chasm;
mod check;
mod console;
mod combat;
mod ground;
mod far_shadow;
mod figure;
mod beams;
mod canal_light;
mod streetlights;
mod landmarks;
mod loading;
mod navmesh;
mod places;
mod show;
#[cfg(feature = "slowspans")]
mod slowspans;
/// Every option name the game reads, gathered from its source by `build.rs`.
mod options {
    include!(concat!(env!("OUT_DIR"), "/options.rs"));
}
mod look;
mod player;
mod ragdoll;
mod rig;
mod rope;
mod cloth;
mod fabric;
mod flight;
mod groundlab;
mod structures;
mod terrain;
mod traits;

use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, window::PresentMode};

/// Command-line options. All optional:
/// `--cam x,y,z,yaw,pitch` (y relative to the ground, angles in degrees)
/// `--time t` simulation time in seconds, which sets where the suns are
/// `--shot path.png` render one frame once terrain has loaded, save it and exit
/// `--seed n`
/// `--focus name` in the lab, start looking at that candidate
/// `--ground name` a patch of the ground lab's kind of ground (see `groundlab.rs`)
/// `--opt name` (repeatable) switch a rendering experiment on or off
/// `--set name=value` or `--set name value` (repeatable) override a tuning number
/// `--do command` (repeatable) a console command, run once the game is on
#[derive(Resource, Clone, Debug)]
pub struct Args {
    pub opts: Vec<String>,
    pub sets: Vec<(String, f32)>,
    pub cam: Option<[f32; 5]>,
    pub time: Option<f32>,
    pub shot: Option<String>,
    pub seed: u32,
    pub focus: Option<String>,
    pub ground: Option<String>,
    pub run: Vec<String>,
}

impl Args {
    pub fn opt(&self, name: &str) -> bool {
        self.opts.iter().any(|o| o == name)
    }

    /// A measurement or capture, running with nobody playing.
    pub fn unattended(&self) -> bool {
        self.opt("bench") || self.opt("flybench") || self.shot.is_some() || self.opt("bot") || self.opt("stairbot") || self.opt("pushbot") || self.opt("walkbot") || self.opt("wallbot") || self.opt("turnbot") || self.opt("labshots") || self.opt("roomshots") || self.opt("viewbench")
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
            Args { opts: Vec::new(), sets: Vec::new(), cam: None, time: None, shot: None, seed: 1, focus: None, ground: None, run: Vec::new() };
        let mut it = std::env::args().skip(1);
        let floats = |s: Option<String>| -> Vec<f32> {
            s.unwrap_or_default().split(',').filter_map(|v| v.trim().parse().ok()).collect()
        };
        let (mut tp, mut seed_given) = (None, false);
        while let Some(flag) = it.next() {
            match flag.as_str() {
                "--cam" => args.cam = floats(it.next()).try_into().ok(),
                // (A place kept by name, see `places.rs`: once all is
                // read, below.)
                "--tp" => tp = Some(it.next().unwrap_or_default()),
                "--time" => args.time = it.next().and_then(|s| s.parse().ok()),
                "--shot" => args.shot = it.next(),
                "--focus" => args.focus = it.next(),
                "--ground" => args.ground = it.next(),
                "--opt" => args.opts.extend(it.next()),
                "--do" => args.run.extend(it.next()),
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
                "--seed" => {
                    args.seed = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
                    seed_given = true;
                }
                other => eprintln!("ignoring unknown argument {other}"),
            }
        }
        // (`--tp NAME`: there, its world's flags brought along; one not
        // there, or a world the flags given say otherwise of, stops the
        // game, said why.)
        if let Some(name) = tp {
            match places::find(&name) {
                Ok(place) => {
                    place.world.bring(&mut args, seed_given);
                    args.cam = Some(place.at);
                    if args.opt("arena") && !args.opt("chasm") {
                        args.opts.push("chasm".into());
                    }
                    let here = places::World::of(&args);
                    if here != place.world {
                        eprintln!("--tp {name}: it is in the world of `{}`, these flags make `{}`", place.world.flags(), here.flags());
                        std::process::exit(2);
                    }
                }
                Err(e) => {
                    eprintln!("--tp {name}: {e}");
                    std::process::exit(2);
                }
            }
        }
        // (The arena is one of the chasm's built places: it brings it.)
        if args.opt("arena") && !args.opt("chasm") {
            args.opts.push("chasm".into());
        }
        args
    }
}

/// When the process started (`--opt startlog`).
pub(crate) static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// `--opt startlog`: from the process's start, each half second
/// the frames, the worst, whether the ground has settled.
fn startlog(args: Res<Args>, time: Res<Time<Real>>, streamer: Res<terrain::Streamer>, compiling: Res<loading::Compiling>, mut state: Local<(f32, u32, f32, bool)>) {
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
        info!("startlog: {since:5.2} s  {:4.0} fps  worst {:6.1} ms  settled {}  missing by level {:?}, {} being built, {} shaders compiling", state.1 as f32 / (since - state.0), state.2 * 1000.0, streamer.settled, streamer.missing, streamer.in_flight, compiling.0.load(std::sync::atomic::Ordering::Relaxed));
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
    rig::set_leap_gravity(args.num("leap_gravity", 60.0));
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
    // New meshes handed to the renderer at most this many MB a frame, see
    // `terrain::MeshBudget` (`--set upload_mb=N`, 0 for no limit).
    let upload = args.num("upload_mb", 4.0);
    app.insert_resource(terrain::MeshBudget { per_frame: if upload > 0.0 { (upload * 1048576.0) as isize } else { isize::MAX }, left: 0 });
    app
        // (The creatures with authored bodies, models and their clips, read
        // from `creatures/` as they are, not built in: see `clip.rs`.)
        .register_asset_source("creatures", bevy::asset::io::AssetSourceBuilder::platform_default(&combat::creatures_folder().to_string_lossy(), None))
        // (Half the cores for the background: the world's terrain and
        // structures are generated there. Bevy's default, a quarter and at
        // most four, left the ground streaming in for some 35 s at the
        // start, the loading screen giving up at 15. `--set workers` to
        // choose how many.)
        .add_plugins(DefaultPlugins.set(bevy::app::TaskPoolPlugin {
            task_pool_options: bevy::app::TaskPoolOptions {
                async_compute: bevy::app::TaskPoolThreadAssignmentPolicy {
                    min_threads: 1,
                    max_threads: args.num("workers", 64.0) as usize,
                    percent: 0.5,
                    on_thread_spawn: None,
                    on_thread_destroy: None,
                },
                ..default()
            },
        }).set(bevy::log::LogPlugin {
            // (`--features slowspans`: spans that ran long, see slowspans.rs.)
            #[cfg(feature = "slowspans")]
            custom_layer: slowspans::layer,
            ..default()
        }).set(WindowPlugin {
            primary_window: Some(Window {
                title: "terrain".into(),
                // (`--set width`, `height`: the benches at the user's
                // 1080p.)
                resolution: (args.num("width", 1600.0) as u32, args.num("height", 900.0) as u32).into(),
                position: bevy::window::WindowPosition::Centered(bevy::window::MonitorSelection::Primary),
                // (Uncapped; `--opt vsync`, frames to the display's refresh.
                // Uncapped, a stall outside the game, the driver's or the
                // system's, backs up into the frames' handover now and then,
                // a frame of 20-30 ms (the user, 2026-11-17: 9 in 3 min
                // uncapped, none in 2 synced), not felt; synced, a frame or
                // two more of input latency, which matters more (a pass of
                // its own later). The hitch log tells those apart, see
                // `hitches`.)
                present_mode: if args.opt("vsync") { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync },
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
        // (Vulkan, as wgpu picks first; `--opt dx12` for DirectX 12. Its
        // frames were faster (at the user's spot by day 9.6 ms to Vulkan's
        // 10.4, once the terrain's material stopped being rewritten every
        // frame), but its shader compiler, FXC, is slow: the window,
        // waiting for the shaders, showed at 11.8 s; Vulkan's soon. Its
        // other, DXC, compiles fast, but its shaders ran 15-30% slower.
        // Development first, the user. WGPU_BACKEND still chooses, if set.)
        .set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::WgpuSettings {
                backends: Some(bevy::render::settings::Backends::from_env().unwrap_or(if cfg!(windows) && args.opt("dx12") {
                    bevy::render::settings::Backends::DX12
                } else {
                    bevy::render::settings::Backends::all()
                })),
                ..default()
            }
            .into(),
            ..default()
        })
        // (Silent when nobody is playing, a bench or a bot or a capture, or
        // with `--opt mute`: the hunters' cries sounded through the house
        // while it was measured.)
        .set(bevy::audio::AudioPlugin { global_volume: bevy::audio::GlobalVolume::new(bevy::audio::Volume::Linear(if unattended || args.opt("mute") { 0.0 } else { 1.0 })), ..default() }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default());
    if !args.opt("gpubatch") {
        app.add_plugins(CpuBatching);
    }
    // (The render thread's own time, for the hitch log: see `RenderClock`.)
    let render_clock = RenderClock::default();
    app.insert_resource(render_clock.clone());
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        render.insert_resource(render_clock).init_resource::<RenderStart>().add_systems(
            bevy::render::Render,
            (
                render_clock_start.in_set(bevy::render::RenderSystems::ExtractCommands),
                render_clock_end.in_set(bevy::render::RenderSystems::PostCleanup),
            ),
        );
    }
    // (Timings of the render passes only to measure (`--opt diag`, and the
    // benches): they cost themselves, a millisecond, a query round each
    // pass.)
    if args.opt("diag") || args.opt("bench") || args.opt("viewbench") {
        app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
    }
    app.insert_resource(args)
        .add_plugins((
            (terrain::TerrainPlugin, far_shadow::FarShadowPlugin),
            camera::FlyCameraPlugin,
            look::LookPlugin,
            (beams::BeamsPlugin, canal_light::CanalLightPlugin, streetlights::StreetlightsPlugin, show::plugin),
            capture::CapturePlugin,
            combat::CombatPlugin,
            figure::FigurePlugin,
            landmarks::LandmarksPlugin,
            player::PlayerPlugin,
            structures::StructuresPlugin,
            chasm::ChasmPlugin,
            groundlab::GroundLabPlugin,
            console::ConsolePlugin,
            loading::LoadingPlugin,
            navmesh::NavmeshPlugin,
        ))
        .init_resource::<FrameClock>()
        .add_systems(First, clock_start)
        .add_systems(Last, clock_end)
        .add_systems(Update, (startlog, hitches))
        .run();
}

/// When this frame's main schedule began, and when the last one's ended:
/// a slow frame's time split into the main schedule's and the time outside
/// it (handing the frame to the render thread and waiting on it, the
/// window's events, the system not running the game). (The user's hitches,
/// 2026-11-17: no system and no schedule ran long in them.)
#[derive(Resource, Default)]
struct FrameClock {
    start: Option<std::time::Instant>,
    end: Option<std::time::Instant>,
    /// The last frame's main schedule, and the time from its end to this
    /// frame's start (ms).
    main: f32,
    outside: f32,
}

fn clock_start(mut clock: ResMut<FrameClock>) {
    let now = std::time::Instant::now();
    if let (Some(start), Some(end)) = (clock.start, clock.end) {
        clock.main = end.saturating_duration_since(start).as_secs_f32() * 1000.0;
        clock.outside = now.saturating_duration_since(end).as_secs_f32() * 1000.0;
    }
    clock.start = Some(now);
}

fn clock_end(mut clock: ResMut<FrameClock>) {
    clock.end = Some(std::time::Instant::now());
}

/// The longest the render thread's schedule has run since the hitch log
/// last looked (us), written from the render world: with the main
/// schedule's, whether a slow frame was the game's work or the time lost
/// outside it, handing frames over and presenting them (see `hitches`).
#[derive(Resource, Clone, Default)]
struct RenderClock(std::sync::Arc<std::sync::atomic::AtomicU32>);

/// When this frame's render schedule began (the render world's).
#[derive(Resource, Default)]
struct RenderStart(Option<std::time::Instant>);

fn render_clock_start(mut start: ResMut<RenderStart>) {
    start.0 = Some(std::time::Instant::now());
}

fn render_clock_end(start: Res<RenderStart>, clock: Res<RenderClock>) {
    if let Some(at) = start.0 {
        clock.0.fetch_max(at.elapsed().as_micros() as u32, std::sync::atomic::Ordering::Relaxed);
    }
}

/// A frame longer than `--set hitch=MS` (20 unless given) said in the
/// log, with how long and when (the user, 2026-11-15: frames at 3 fps,
/// keeping creatures round you; and the target is 100 fps and more, no
/// slop: two frames' worth is a stutter). The first `SETTLE` s of play,
/// terrain still coming in, told as one line, how many and the worst.
/// Each slow frame with what arrived in it (the user, 2026-11-16: hitches
/// flying, none walking; a straight flight's bench, with or without nodes
/// or creatures, did not make them): terrain columns, meshes, physics
/// bodies, creatures, node builds begun and ready. Always on: a few counts
/// a frame; a line only for a slow one.
#[allow(clippy::too_many_arguments)]
fn hitches(
    time: Res<Time<Real>>,
    clock: Res<FrameClock>,
    render_clock: Res<RenderClock>,
    playing: Res<loading::Playing>,
    args: Res<Args>,
    mut settle: Local<(Option<f32>, u32, f32, bool)>,
    columns: Query<(), Added<terrain::TerrainColumn>>,
    bodies: Query<(), Added<avian3d::prelude::RigidBody>>,
    creatures: Query<(), Added<navmesh::district::Blink>>,
    mut meshes: MessageReader<AssetEvent<Mesh>>,
    mut builds: Local<[u32; 2]>,
    camera: Option<Single<(&Transform, &camera::FlyCam)>>,
    world: Option<Res<terrain::WorldGen>>,
    lab: Option<Res<groundlab::LabGround>>,
) {
    let added = meshes.read().filter(|e| matches!(e, AssetEvent::Added { .. })).count();
    let counts = navmesh::district::BUILDS.each_ref().map(|b| b.load(std::sync::atomic::Ordering::Relaxed));
    let (started, ready) = (counts[0] - builds[0], counts[1] - builds[1]);
    *builds = counts;
    if !playing.0 {
        return;
    }
    let (dt, now) = (time.delta_secs(), time.elapsed_secs());
    let render = render_clock.0.swap(0, std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
    let begun = *settle.0.get_or_insert(now);
    let slow = dt * 1000.0 > args.num("hitch", 20.0);
    if now - begun < SETTLE {
        if slow {
            settle.1 += 1;
            settle.2 = settle.2.max(dt);
        }
        return;
    }
    if !settle.3 {
        settle.3 = true;
        info!("hitch: settling, the first {SETTLE:.0} s of play: {} slow frames, the worst {:.0} ms", settle.1, settle.2 * 1000.0);
    }
    if slow {
        // The game's own (its main or render schedule ran half the
        // threshold or more), or outside it, the frame's handover and its
        // presenting: told apart, the latter quietly (uncapped, they come
        // now and then from the driver or the system, not felt).
        // (Something arriving makes it ours too: its extraction to the
        // render world runs on the main thread between the schedules.)
        let arrived = !columns.is_empty() || added > 0 || !bodies.is_empty() || !creatures.is_empty() || started + ready > 0;
        let ours = arrived || clock.main.max(render) >= args.num("hitch", 20.0) * 0.5;
        let line = format!(
            "a frame of {:.0} ms at {now:.1} s (main schedule {:.1} ms, render {:.1} ms, outside them {:.1} ms); in it: {} terrain columns, {added} meshes, {} physics bodies, {} creatures, node builds {started} begun and {ready} ready; at {}",
            dt * 1000.0,
            clock.main,
            render,
            clock.outside,
            columns.iter().count(),
            bodies.iter().count(),
            creatures.iter().count(),
            // (Where, as `--cam` takes it: the spot flown to again on the
            // bench, `--opt flybench`.)
            camera.map_or("?".into(), |c| {
                let (t, fly) = *c;
                let p = t.translation;
                let above = p.y - places::ground(world.as_deref(), lab.as_deref(), args.opt("camfloor"), p.x, p.z);
                format!("--cam {:.0},{:.0},{:.0},{:.0},{:.0}", p.x, above, p.z, fly.yaw.to_degrees(), fly.pitch.to_degrees())
            })
        );
        if ours {
            warn!("hitch: {line}");
        } else {
            info!("hitch outside the game: {line}");
        }
    }
}

/// See `hitches` (s).
const SETTLE: f32 = 10.0;

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

