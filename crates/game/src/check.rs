//! `--check static|walk|wall|all --seeds 1-50,251-450 [--jobs n]`: the
//! chasm's checks over many seeds at once, with nothing rendered and no
//! window, as fast as the machine allows; a line per seed, a summary, and
//! the exit code (0 if all is well). `--check catalogue`: the static and
//! walk checks on the junction catalogue (`--opt junctions`).
//!
//! - static: the way reaches the bottom, and nothing runs into rock or into
//!   another route, crosses a route's way, or leaves a hole where a flight
//!   meets a walkway (`chasm::check`).
//! - walk: the walking bot goes the whole way down with the real movement,
//!   and gets stuck or falls nowhere.
//! - wall: walking into walls (straight on, at a slant), the body stands
//!   still against them (or slides along), and never jumps.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use crate::Args;
use crate::camera::FlyCam;
use crate::player::{self, BotLog, MoveInput};
use crate::terrain::{Streamer, WorldGen};

/// Runs the checks the command line asks for; never returns.
pub fn run(argv: &[String]) -> ! {
    let after = |flag: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).cloned();
    let catalogue = after("--check").as_deref() == Some("catalogue");
    let kinds: Vec<&str> = match after("--check").as_deref() {
        Some("all") => vec!["static", "walk", "wall"],
        Some("catalogue") => vec!["static", "walk"],
        Some("static") => vec!["static"],
        Some("walk") => vec!["walk"],
        Some("wall") => vec!["wall"],
        other => {
            eprintln!("--check {other:?}: expected static, walk, wall, all or catalogue");
            std::process::exit(2);
        }
    };
    let seeds = if catalogue { vec![0] } else { seeds(&after("--seeds").unwrap_or_else(|| "1-50".into())) };
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let jobs = after("--jobs").and_then(|j| j.parse().ok()).unwrap_or((cores * 2 / 3).max(1));
    let work: VecDeque<(&'static str, u32)> = kinds
        .iter()
        .flat_map(|&k| {
            let k: &'static str = match k {
                "static" => "static",
                "walk" => "walk",
                _ => "wall",
            };
            seeds.iter().map(move |&s| (k, s))
        })
        .collect();
    let total = work.len();
    eprintln!("checking {total} ({} on {} seeds), {jobs} at a time", kinds.join(", "), seeds.len());
    let work = Arc::new(Mutex::new(work));
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    for _ in 0..jobs {
        let (work, tx) = (work.clone(), tx.clone());
        std::thread::spawn(move || {
            loop {
                let Some((kind, seed)) = work.lock().unwrap().pop_front() else { break };
                let t = Instant::now();
                let found = std::panic::catch_unwind(|| match kind {
                    "static" => static_check(seed, catalogue),
                    kind => walk_check(kind, seed, catalogue),
                })
                .unwrap_or_else(|_| (false, "panicked".into(), Vec::new()));
                let _ = tx.send((kind, seed, found, t.elapsed()));
            }
        });
    }
    drop(tx);
    let mut failed = Vec::new();
    let mut done = 0;
    for (kind, seed, (ok, summary, problems), took) in rx {
        done += 1;
        println!("[{done}/{total}] {kind} seed {seed}: {} ({:.0} s) {summary}", if ok { "ok" } else { "FAIL" }, took.as_secs_f32());
        if !ok {
            for p in problems.iter().take(8) {
                println!("    {p}");
            }
            failed.push((kind, seed, problems.first().cloned().unwrap_or_default()));
        }
    }
    println!();
    for &kind in &kinds {
        let bad = failed.iter().filter(|f| f.0 == kind).count();
        println!("{kind}: {} of {} ok", seeds.len() - bad, seeds.len());
    }
    for (kind, seed, first) in &failed {
        println!("  FAIL {kind} seed {seed}: {first}");
    }
    println!("{} checks in {:.0} s", total, started.elapsed().as_secs_f32());
    std::process::exit(if failed.is_empty() { 0 } else { 1 });
}

/// Seeds from "1-50,251-450,7".
fn seeds(spec: &str) -> Vec<u32> {
    spec.split(',')
        .flat_map(|part| match part.split_once('-') {
            Some((a, b)) => {
                let (a, b): (u32, u32) = (a.trim().parse().unwrap_or(1), b.trim().parse().unwrap_or(0));
                (a..=b).collect::<Vec<_>>()
            }
            None => part.trim().parse().ok().into_iter().collect(),
        })
        .collect()
}

fn static_check(seed: u32, catalogue: bool) -> (bool, String, Vec<String>) {
    let found = crate::chasm::check(seed, catalogue);
    (found.ok, found.summary, found.problems)
}

/// Walks the chasm (or into its walls) with the real movement and the bot,
/// in an app with nothing but the physics, the movement and the bot, its
/// time stepped by hand 0.1 s at a time, as fast as it computes.
fn walk_check(kind: &str, seed: u32, catalogue: bool) -> (bool, String, Vec<String>) {
    let bot = if kind == "walk" { "walkbot" } else { "wallbot" };
    let args = Args {
        opts: vec!["chasm".into(), "peace".into(), bot.into(), "headless".into()],
        sets: Vec::new(),
        cam: None,
        time: None,
        shot: None,
        seed,
        focus: None,
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins((TransformPlugin, bevy::asset::AssetPlugin::default(), bevy::mesh::MeshPlugin, bevy::diagnostic::DiagnosticsPlugin))
        .add_plugins(PhysicsPlugins::default())
        // (The physics reports its timings into these; with the full game's
        // plugins they come with the rest.)
        .init_resource::<avian3d::collider_tree::ColliderTreeDiagnostics>()
        .init_resource::<avian3d::collision::CollisionDiagnostics>()
        .init_resource::<avian3d::dynamics::solver::SolverDiagnostics>()
        .init_resource::<avian3d::spatial_query::SpatialQueryDiagnostics>()
        // (Walking down, a tenth of a second at a time: coarser than the
        // game, so harsher, and fast. Into walls, at the game's own steps:
        // what shakes is what the player would see.)
        .insert_resource(TimeUpdateStrategy::ManualDuration(if kind == "walk" { Duration::from_millis(100) } else { Duration::from_secs_f64(1.0 / 60.0) }))
        .insert_resource(args)
        .init_resource::<MoveInput>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<BotLog>()
        .insert_resource(WorldGen(Arc::new(worldgen::plates::PlateWorld::lab(worldgen::WORLD_SIZE, seed))))
        .insert_resource({
            let mut streamer = Streamer::default();
            streamer.near = true;
            streamer
        })
        .add_systems(Update, (player::bot_drive, player::walk).chain());
    crate::chasm::headless(&mut app, seed, catalogue);
    // (The ground at the chasm's bottom: in the game, the terrain's.)
    app.world_mut().spawn((RigidBody::Static, Collider::cuboid(20_000.0, 1.0, 20_000.0), Transform::from_xyz(0.0, -0.5, 0.0)));
    let [x, y, z, yaw, pitch] = crate::chasm::start(seed, catalogue);
    let fly = FlyCam { yaw: yaw.to_radians(), pitch: pitch.to_radians(), speed: 25.0, noclip: false };
    let rotation = fly.rotation();
    app.world_mut().spawn((Transform::from_xyz(x, y, z).with_rotation(rotation), fly, player::Player::fresh()));
    // (At most an hour and a half of the game's time: a walk down takes ten
    // minutes or so.)
    for _ in 0..54_000 {
        app.update();
        if app.should_exit().is_some() {
            break;
        }
    }
    let lines = std::mem::take(&mut app.world_mut().resource_mut::<BotLog>().0);
    let finished = lines.iter().any(|l| l.contains("at the bottom") || l.contains("walls done"));
    let problems: Vec<String> = lines.iter().filter(|l| l.contains("stuck") || l.contains("fell") || l.contains("SHAKES") || l.contains("POPS")).cloned().collect();
    let summary = lines.iter().rev().find(|l| l.contains("at the bottom") || l.contains("walls done")).cloned().unwrap_or_else(|| "did not finish".into());
    (finished && problems.is_empty(), summary, problems)
}
