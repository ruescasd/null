//! Prototype: run, strafe jump and thrust through a wrapping procedural world.

mod camera;
mod capture;
mod landmarks;
mod look;
mod player;
mod terrain;

use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, window::PresentMode};

/// Command-line options. All optional:
/// `--cam x,y,z,yaw,pitch` (y relative to the ground, angles in degrees)
/// `--time t` simulation time in seconds, which sets where the suns are
/// `--shot path.png` render one frame once terrain has loaded, save it and exit
/// `--seed n`
/// `--opt name` (repeatable) switch a rendering experiment on or off
/// `--set name=value` (repeatable) override a tuning number
#[derive(Resource, Clone, Debug)]
pub struct Args {
    pub opts: Vec<String>,
    pub sets: Vec<(String, f32)>,
    pub cam: Option<[f32; 5]>,
    pub time: Option<f32>,
    pub shot: Option<String>,
    pub seed: u32,
}

impl Args {
    pub fn opt(&self, name: &str) -> bool {
        self.opts.iter().any(|o| o == name)
    }

    /// A tuning number, overridable with `--set name=value`.
    pub fn num(&self, name: &str, default: f32) -> f32 {
        self.sets.iter().rev().find(|(k, _)| k == name).map_or(default, |(_, v)| *v)
    }

    fn parse() -> Self {
        let mut args = Args { opts: Vec::new(), sets: Vec::new(), cam: None, time: None, shot: None, seed: 1 };
        let mut it = std::env::args().skip(1);
        let floats = |s: Option<String>| -> Vec<f32> {
            s.unwrap_or_default().split(',').filter_map(|v| v.trim().parse().ok()).collect()
        };
        while let Some(flag) = it.next() {
            match flag.as_str() {
                "--cam" => args.cam = floats(it.next()).try_into().ok(),
                "--time" => args.time = it.next().and_then(|s| s.parse().ok()),
                "--shot" => args.shot = it.next(),
                "--opt" => args.opts.extend(it.next()),
                "--set" => {
                    if let Some((k, v)) = it.next().as_deref().and_then(|s| s.split_once('='))
                        && let Ok(v) = v.parse()
                    {
                        args.sets.push((k.to_string(), v));
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
    let args = Args::parse();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "terrain".into(),
                resolution: (1600, 900).into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .insert_resource(args)
        .add_plugins((
            terrain::TerrainPlugin,
            camera::FlyCameraPlugin,
            look::LookPlugin,
            capture::CapturePlugin,
            landmarks::LandmarksPlugin,
            player::PlayerPlugin,
        ))
        .run();
}
