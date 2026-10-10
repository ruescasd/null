//! A scripted test pilot (`--opt bot`): drives the real movement code with
//! synthetic input and logs telemetry, so movement can be checked without
//! anyone at the keyboard. It stands, runs, strafe jumps at the optimal
//! angle, coasts, and exits. With `--opt walkbot` in the chasm, it walks
//! the whole way down instead, and logs wherever it cannot go on.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{AIR_ACCEL, MAX_SPEED, MoveInput, Player, STEP_DT};
use crate::{Args, camera::FlyCam, terrain::WorldGen};

/// What the bots found, line by line (logged too): for `--check`, which runs
/// them with nothing rendered and no log to read.
#[derive(Resource, Default)]
pub struct BotLog(pub Vec<String>);

/// A line from a bot: logged, and kept.
fn say(log: &mut Option<ResMut<BotLog>>, line: String) {
    info!("{line}");
    if let Some(log) = log {
        log.0.push(line);
    }
}

#[derive(Default)]
pub struct BotState {
    started: Option<f32>,
    last_log: f32,
    /// Height of the ledge top the step test runs onto.
    step_target: Option<f32>,
    /// Height of the ledge top the mantle test climbs onto.
    mantle_target: Option<f32>,
    saw_mantle: bool,
    canal_started: bool,
    canal_rim: f32,
    canal_peak: f32,
    canal_reported: bool,
    /// When the tether test started its drop, and from what height.
    tether_drop: Option<(f32, f32)>,
    /// The stairs test (`--opt stairbot`): where the flight starts, which
    /// way is up, the height of its top, and when the bot got there.
    stair: Option<(Vec3, Vec2, f32, f32)>,
    stair_mantles: u32,
    stair_was_mantling: bool,
    /// The walk down the chasm (`--opt walkbot`): the point it heads for,
    /// how near it has got and since when, what went wrong, when it set off.
    walk: usize,
    walk_best: f32,
    walk_since: f32,
    walk_problems: u32,
    walk_started: Option<f32>,
    /// The wall test (`--opt wallbot`): which, since when, and where the
    /// body was while pushing.
    wall: usize,
    wall_since: Option<f32>,
    wall_samples: Vec<Vec3>,
}

#[allow(clippy::too_many_arguments)]
pub fn drive(
    args: Res<Args>,
    time: Res<Time>,
    mut clock: ResMut<Time<Virtual>>,
    real: Res<Time<Real>>,
    way: Option<Res<crate::chasm::Way>>,
    walls: Option<Res<crate::chasm::WallTests>>,
    mut log: Option<ResMut<BotLog>>,
    mut state: Local<BotState>,
    mut input: ResMut<MoveInput>,
    mut exit: MessageWriter<AppExit>,
    world: Res<WorldGen>,
    query: SpatialQuery,
    camera: Single<(&mut Transform, &mut FlyCam, &mut Player)>,
) {
    let (mut transform, mut fly, mut player) = camera.into_inner();
    if args.opt("wallbot") {
        wall_test(&time, walls.as_deref(), &query, &mut log, &mut state, &mut input, &mut exit, &mut transform, &mut fly, &mut player);
        return;
    }
    if args.opt("walkbot") {
        // (With nothing rendered, time is stepped by hand: no speeding up.)
        let real_dt = if args.opt("headless") { 0.0 } else { real.delta_secs() };
        walk(&time, &mut clock, real_dt, way.as_deref(), &mut log, &mut state, &mut input, &mut exit, &mut transform, &mut fly, &mut player);
        return;
    }
    if args.opt("stairbot") {
        stairs(&time, &mut state, &mut input, &mut exit, &world, &mut transform, &mut fly, &mut player);
        return;
    }
    if !args.opt("bot") {
        return;
    }
    if !player.ready {
        return;
    }
    let now = time.elapsed_secs();
    let t = now - *state.started.get_or_insert(now);
    let speed = Vec2::new(player.velocity.x, player.velocity.z).length();

    let phase = match t {
        t if t < 1.0 => "stand",
        t if t < 3.0 => "run",
        t if t < 11.0 => "strafe jump",
        t if t < 14.0 => "coast",
        t if t < 16.5 => "tether down",
        t if t < 18.5 => "step up",
        t if t < 21.0 => "mantle",
        t if t < 25.0 => "canal",
        _ => {
            exit.write(AppExit::Success);
            return;
        }
    };
    *input = MoveInput::default();
    match phase {
        "run" => {
            // Half a second in each compass direction, to find open ground.
            input.wish = Vec2::Y;
            fly.yaw = ((t - 1.0) / 0.5).floor() * std::f32::consts::FRAC_PI_2;
        }
        "strafe jump" => {
            // Pure right-strafe, turning so the wish direction stays at the
            // angle to the velocity where air acceleration adds the most.
            input.wish = Vec2::X;
            input.jump = true;
            let vel = Vec2::new(player.velocity.x, player.velocity.z);
            if speed > 1.0 {
                let gain = AIR_ACCEL * MAX_SPEED * STEP_DT;
                let angle = ((MAX_SPEED - gain) / speed).clamp(-1.0, 1.0).acos();
                // Wish direction rotated `angle` from the velocity (turning
                // left); with a right-strafe, view yaw = wish yaw + 90 degrees.
                let vel_yaw = (-vel.x).atan2(-vel.y);
                fly.yaw = vel_yaw + angle + std::f32::consts::FRAC_PI_2;
            }
        }
        "mantle" => {
            if state.mantle_target.is_none() {
                // A ledge above plain jump height: needs a jump and a mantle.
                let Some((start, yaw, top)) = find_ledge(&world, transform.translation, 1.6, 2.3, 3.5)
                else {
                    info!("bot mantle: no suitable ledge nearby");
                    state.mantle_target = Some(f32::NAN);
                    return;
                };
                transform.translation = start;
                player.velocity = Vec3::ZERO;
                fly.yaw = yaw;
                fly.pitch = 0.0;
                state.mantle_target = Some(top);
                info!("bot mantle: ledge top at {top:.2}, {:.2} above the feet", top - (start.y - super::EYE));
            }
            input.wish = Vec2::Y;
            input.jump = true;
        }
        "canal" => {
            // Dropped onto the bottom of the nearest canal moving across it
            // (and a little along): a half-pipe should carry the bot up the
            // far wall and out above the lip, then back in. No input.
            if !state.canal_started {
                state.canal_started = true;
                let p = transform.translation;
                if let Some((center, dir, floor)) = world.nearest_canal(p.x, p.z) {
                    transform.translation = Vec3::new(center.x, floor + super::EYE + super::HALF_WIDTH, center.y);
                    let across = Vec3::new(-dir.y, 0.0, dir.x);
                    player.velocity = across * 22.0 + Vec3::new(dir.x, 0.0, dir.y) * 8.0;
                    fly.yaw = (-dir.x).atan2(-dir.y);
                    fly.pitch = 0.0;
                    state.canal_rim = floor + worldgen::canal::PIPE_RADIUS as f32;
                    state.canal_peak = f32::MIN;
                    info!("bot canal: dropped in at {:.0},{:.0}, rim at {:.1}", center.x, center.y, state.canal_rim);
                } else {
                    info!("bot canal: no canal");
                }
            }
            state.canal_peak = state.canal_peak.max(transform.translation.y - super::EYE);
            if t > 24.8 && !state.canal_reported {
                state.canal_reported = true;
                info!(
                    "bot canal: highest feet {:.2} m above the rim; {} the pipe now",
                    state.canal_peak - state.canal_rim,
                    if player.in_canal { "in" } else { "out of" },
                );
            }
        }
        "tether down" => {
            // From 40 m up and at rest, tether straight down.
            if state.tether_drop.is_none() {
                let p = transform.translation;
                let ground = world.ground_height(p.x, p.z);
                transform.translation.y = ground + 40.0 + super::EYE;
                player.velocity = Vec3::ZERO;
                state.tether_drop = Some((t, ground));
                info!("bot tether down: 40 m above ground at {ground:.2}");
            }
            fly.pitch = -1.55;
            input.tether = true;
        }
        "step up" => {
            if state.step_target.is_none() {
                // Find a ledge 0.2-0.55 m high with flat ground before it, put
                // the bot 3 m in front of it and run at it.
                let Some((start, yaw, top)) = find_ledge(&world, transform.translation, 0.2, 0.55, 3.0)
                else {
                    info!("bot step up: no suitable ledge nearby");
                    state.step_target = Some(f32::NAN);
                    return;
                };
                transform.translation = start;
                player.velocity = Vec3::ZERO;
                fly.yaw = yaw;
                fly.pitch = 0.0;
                state.step_target = Some(top);
                info!("bot step up: ledge top at {top:.2}, starting from eye height {:.2}", start.y);
            }
            input.wish = Vec2::Y;
        }
        _ => {}
    }

    if phase == "step up" && let Some(top) = state.step_target {
        let feet = transform.translation.y - super::EYE;
        if feet > top - 0.05 && player.grounded && !top.is_nan() {
            info!("bot step up: OK, stepped onto {top:.2} (feet at {feet:.2})");
            state.step_target = Some(f32::NAN);
        }
    }

    if phase == "mantle" && player.mantle.is_some() && !state.saw_mantle {
        state.saw_mantle = true;
        info!("bot mantle: climbing");
    }
    if phase == "mantle" && let Some(top) = state.mantle_target {
        // A plain jump from the start peaks below the top, so feet above it
        // mean the bot got up.
        let feet = transform.translation.y - super::EYE;
        if feet > top && !top.is_nan() {
            info!("bot mantle: OK, climbed onto {top:.2} (feet at {feet:.2})");
            state.mantle_target = Some(f32::NAN);
        }
    }

    if now - state.last_log >= if matches!(phase, "run" | "tether down" | "mantle" | "canal") { 0.25 } else { 0.5 } {
        state.last_log = now;
        let p = transform.translation;
        info!(
            "bot t={t:4.1} {phase:12} canal {}  speed {speed:5.2} m/s ({:4.0} ups)  vy {:6.2}  y {:7.2}  grounded {}  health {:3.0}  tether {}  pos {:.0},{:.0}",
            player.in_canal,
            speed / 0.032,
            player.velocity.y,
            p.y,
            player.grounded,
            player.health,
            match player.tether {
                super::Tether::Idle => "idle",
                super::Tether::Flying { .. } => "flying",
                super::Tether::Anchored { .. } => "anchored",
                super::Tether::Missed => "released",
            },
            p.x,
            p.z,
        );
    }
}

/// Walks up the biggest flight of stairs on a site near the spawn point and
/// logs every frame: a flight should be climbed by stepping, never by
/// mantling.
#[allow(clippy::too_many_arguments)]
fn stairs(
    time: &Time,
    state: &mut BotState,
    input: &mut MoveInput,
    exit: &mut MessageWriter<AppExit>,
    world: &WorldGen,
    transform: &mut Transform,
    fly: &mut FlyCam,
    player: &mut Player,
) {
    if !player.ready {
        return;
    }
    let now = time.elapsed_secs();
    if state.stair.is_none() {
        let plates = &world.0;
        let Ok(library) = crate::structures::load_library() else { return };
        let found = worldgen::sites::near(&library, plates, 1200.0, 900.0, 3000.0).into_iter().find_map(|site| {
            let built = worldgen::sites::build(&library, plates, &site, 1000);
            let flight = *built.flights.iter().max_by(|a, b| a.rise.total_cmp(&b.rise))?;
            Some((site.at, built.base, flight))
        });
        let Some(((x, z), base, flight)) = found else {
            info!("bot stairs: no stairs nearby");
            exit.write(AppExit::Success);
            return;
        };
        let foot = Vec3::new(x + flight.foot.x, base + flight.floor, z + flight.foot.y);
        info!("bot stairs: flight at {:.0},{:.0} rising {:.2} m from {:.2}", foot.x, foot.z, flight.rise, foot.y);
        state.stair = Some((foot, flight.up, foot.y + flight.rise, now));
    }
    let (foot, up, top, since) = state.stair.unwrap();
    let t = now - since;
    *input = MoveInput::default();
    fly.yaw = (-up.x).atan2(-up.y);
    fly.pitch = 0.0;
    // Stand at the foot while the terrain there loads.
    if t < 5.0 {
        transform.translation = foot + Vec3::Y * (super::EYE + 0.02);
        player.velocity = Vec3::ZERO;
        return;
    }
    if t < 8.0 {
        input.wish = Vec2::Y;
        let feet = transform.translation.y - super::EYE;
        let mantling = player.mantle.is_some();
        if mantling && !state.stair_was_mantling {
            state.stair_mantles += 1;
        }
        state.stair_was_mantling = mantling;
        let speed = Vec2::new(player.velocity.x, player.velocity.z).length();
        info!(
            "bot stairs t={:.3} feet {:+.2} (top {:+.2})  grounded {}  mantle {}  speed {:.1}  vy {:+.2}",
            t - 5.0,
            feet - foot.y,
            top - foot.y,
            player.grounded,
            mantling,
            speed,
            player.velocity.y
        );
        return;
    }
    let feet = transform.translation.y - super::EYE;
    info!(
        "bot stairs: {} (feet {:+.2}, top {:+.2}), {} mantles",
        if feet > top - 0.1 { "reached the top" } else { "did not reach the top" },
        feet - foot.y,
        top - foot.y,
        state.stair_mantles
    );
    exit.write(AppExit::Success);
}

/// The walk down the chasm: the real movement, steered from point to point
/// of the way (running, walking near a point), up to eight times as fast as
/// real time. Where it gets no nearer to the next point for 4 s, or falls well
/// below it, that is logged, and it is put on that point and goes on; at the
/// bottom, a count.
#[allow(clippy::too_many_arguments)]
fn walk(
    time: &Time,
    clock: &mut Time<Virtual>,
    real_dt: f32,
    way: Option<&crate::chasm::Way>,
    log: &mut Option<ResMut<BotLog>>,
    state: &mut BotState,
    input: &mut MoveInput,
    exit: &mut MessageWriter<AppExit>,
    transform: &mut Transform,
    fly: &mut FlyCam,
    player: &mut Player,
) {
    let Some(way) = way else {
        info!("walkbot: no way down (--opt chasm)");
        exit.write(AppExit::Success);
        return;
    };
    if !player.ready {
        return;
    }
    // (Up to eight times as fast, never more per frame than the movement
    // simulates, 0.1 s: or time would run on without it under load.)
    if real_dt > 0.0 {
        clock.set_relative_speed((0.095 / real_dt.max(1e-3)).min(8.0));
    }
    let now = time.elapsed_secs();
    let started = *state.walk_started.get_or_insert_with(|| {
        state.walk_best = f32::MAX;
        state.walk_since = now;
        now
    });
    *input = MoveInput::default();
    let Some((target, what)) = way.0.get(state.walk) else {
        say(log, format!("walkbot: at the bottom: {} points, {} problems, {:.0} s walking", way.0.len(), state.walk_problems, now - started));
        exit.write(AppExit::Success);
        return;
    };
    // (A way starting afresh, in the junction catalogue: there at once.)
    if what.starts_with("jump to") {
        transform.translation = *target + Vec3::Y * (super::EYE + 0.05);
        player.velocity = Vec3::ZERO;
        state.walk += 1;
        state.walk_best = f32::MAX;
        state.walk_since = now;
        return;
    }
    let p = transform.translation;
    let feet = p.y - super::EYE;
    let flat = Vec2::new(target.x - p.x, target.z - p.z);
    let d = flat.length();
    if d < 0.6 && (feet - target.y).abs() < 1.6 {
        state.walk += 1;
        state.walk_best = f32::MAX;
        state.walk_since = now;
        return;
    }
    if d < state.walk_best - 0.25 {
        state.walk_best = d;
        state.walk_since = now;
    }
    let below = state.walk.checked_sub(1).and_then(|i| way.0.get(i)).map_or(target.y, |q| q.0.y).min(target.y);
    let fell = feet < below - 4.0;
    if fell || now - state.walk_since > 4.0 {
        say(log, format!(
            "walkbot: {} before point {} ({what}, {:?}) at {:?}: {:.1} m from it, feet {:+.1} (moving {:?}, {}{})",
            if fell { "fell" } else { "stuck" },
            state.walk,
            target,
            p - Vec3::Y * super::EYE,
            d,
            feet - target.y,
            player.velocity,
            if player.grounded { "on the ground" } else { "in the air" },
            if player.mantle.is_some() { ", climbing" } else { "" }
        ));
        state.walk_problems += 1;
        transform.translation = *target + Vec3::Y * (super::EYE + 0.05);
        player.velocity = Vec3::ZERO;
        state.walk += 1;
        state.walk_best = f32::MAX;
        state.walk_since = now;
        return;
    }
    // Forward is (-sin yaw, -cos yaw) in x, z.
    fly.yaw = (-flat.x).atan2(-flat.y);
    fly.pitch = -0.3;
    input.wish = Vec2::Y;
    input.walk = d < 4.0;
}

/// The wall test: for each place, stand there a moment, then walk straight
/// into the wall for 3 s and log how far the body moves while pushing
/// against it, along, across and up: against a wall it should stand still.
#[allow(clippy::too_many_arguments)]
fn wall_test(
    time: &Time,
    walls: Option<&crate::chasm::WallTests>,
    query: &SpatialQuery,
    log: &mut Option<ResMut<BotLog>>,
    state: &mut BotState,
    input: &mut MoveInput,
    exit: &mut MessageWriter<AppExit>,
    transform: &mut Transform,
    fly: &mut FlyCam,
    player: &mut Player,
) {
    let Some(walls) = walls else {
        info!("wallbot: no walls (--opt chasm)");
        exit.write(AppExit::Success);
        return;
    };
    if !player.ready {
        return;
    }
    let now = time.elapsed_secs();
    *input = MoveInput::default();
    // (Each wall straight on, then at 10 and 35 degrees, the aim wobbling a
    // little as a hand's does.)
    const ANGLES: [f32; 3] = [0.0, 10.0, 35.0];
    let Some((what, at, square)) = walls.0.get(state.wall / ANGLES.len()) else {
        say(log, format!("wallbot: walls done: {} places, {} angles", walls.0.len(), ANGLES.len()));
        exit.write(AppExit::Success);
        return;
    };
    let angle = ANGLES[state.wall % ANGLES.len()];
    let what = format!("{what} at {angle:.0} degrees");
    let dir = &(Quat::from_rotation_y(angle.to_radians()) * *square);
    let since = *state.wall_since.get_or_insert_with(|| {
        // (Where nothing stands in the way: what stands out at a wall's foot
        // can reach further out than the place is set from the face; backed
        // off from it till clear.)
        let shell = Collider::cuboid(super::HALF_WIDTH * 2.0 - 0.04, super::HEIGHT - 0.04, super::HALF_WIDTH * 2.0 - 0.04);
        let lift = Vec3::Y * (super::HEIGHT * 0.5 + 0.05);
        let start = (0..40).map(|i| *at - *square * (i as f32 * 0.5)).find(|p| query.shape_intersections(&shell, *p + lift, Quat::IDENTITY, &default()).is_empty()).unwrap_or(*at);
        transform.translation = start + Vec3::Y * (super::EYE + 0.05);
        player.velocity = Vec3::ZERO;
        now
    });
    let t = now - since;
    fly.yaw = (-dir.x).atan2(-dir.z) + if angle > 0.0 { (t * 7.0).sin() * 0.6_f32.to_radians() } else { 0.0 };
    fly.pitch = 0.0;
    if t < 1.0 {
        return;
    }
    input.wish = Vec2::Y;
    if t > 2.0 {
        state.wall_samples.push(transform.translation);
    }
    if t > 5.0 {
        // (Sliding along the wall is as it should be; what counts is how
        // far from the wall the body stays, and how high: both should hold
        // still, not go back and forth.)
        let s = &state.wall_samples;
        let measure = |f: &dyn Fn(Vec3) -> f32| {
            let xs: Vec<f32> = s.iter().map(|p| f(*p)).collect();
            let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
            let steps: Vec<f32> = xs.windows(2).map(|p| p[1] - p[0]).filter(|d| d.abs() > 0.001).collect();
            let turns = steps.windows(2).filter(|p| p[0].signum() != p[1].signum()).count();
            ((hi - lo) * 100.0, turns)
        };
        let (off, off_turns) = measure(&|p| p.dot(*square));
        let tangent = Vec3::new(-square.z, 0.0, square.x);
        let slid = s.last().zip(s.first()).map_or(0.0, |(b, a)| (*b - *a).dot(tangent).abs());
        let (up, up_turns) = measure(&|p| p.y);
        // (Shaking: going back and forth off the wall, or up and down, by
        // more than a little, more than now and then, while getting nowhere.
        // Sliding on along the wall, the body follows what stands out of it,
        // as it should; at a steep slant it always slides, so only the
        // straight and slight slants are judged.)
        let shakes = angle < 30.0 && slid < 0.3 && ((off > 2.0 && off_turns >= 3) || (up > 2.0 && up_turns >= 3));
        // (Nor jumping: pressed against a wall, no step takes the body far;
        // judged where it shakes is, as at a steep slant it runs on along.)
        let pop = s.windows(2).map(|p| Vec2::new(p[1].x - p[0].x, p[1].z - p[0].z).length()).fold(0.0, f32::max);
        // (Unless it went off the edge: sliding along a tunnel's side, out
        // of its door and over the walkway's edge, it falls, as it should.)
        let fell = s.iter().any(|p| (p.y - s[0].y).abs() > 1.0);
        let pops = angle < 30.0 && !fell && pop > 0.2;
        say(log, format!(
            "wallbot: {}{what}: {:.2} m walked to the wall; slid {slid:.2} m along it; off the wall {off:.1} cm ({off_turns} turns), up and down {up:.1} cm ({up_turns} turns), {:.2} m at most in a step, while pushing ({} frames)",
            if shakes { "SHAKES: " } else if pops { "POPS: " } else { "" },
            (s.last().copied().unwrap_or(*at) - *at - Vec3::Y * super::EYE).dot(*dir),
            pop,
            s.len()
        ));
        state.wall += 1;
        state.wall_since = None;
        state.wall_samples.clear();
    }
}

/// A spot `run_up` metres in front of a ledge between `min` and `max` high,
/// with flat ground before and on top of it: (eye position, yaw facing it,
/// ledge top).
fn find_ledge(world: &WorldGen, around: Vec3, min: f32, max: f32, run_up: f32) -> Option<(Vec3, f32, f32)> {
    for ring in 1..30 {
        for k in 0..ring * 8 {
            let a = k as f32 / (ring * 8) as f32 * std::f32::consts::TAU;
            let p = Vec2::new(around.x, around.z) + Vec2::from_angle(a) * ring as f32 * 1.5;
            for d in 0..8 {
                let dir = Vec2::from_angle(d as f32 / 8.0 * std::f32::consts::TAU);
                let h = |q: Vec2| world.ground_height(q.x, q.y);
                let h0 = h(p);
                let rise = h(p + dir * 1.0) - h0;
                let steps = (run_up / 0.5).ceil() as i32 + 1;
                let runway = (0..=steps).all(|i| (h(p - dir * (i as f32 * 0.5)) - h0).abs() < 0.01);
                let landing = (1..=4).all(|i| (h(p + dir * (1.0 + i as f32 * 0.5)) - h0 - rise).abs() < 0.01);
                if rise > min && rise < max && runway && landing {
                    let start = p - dir * run_up;
                    // Forward is (-sin yaw, -cos yaw) in x, z.
                    let yaw = (-dir.x).atan2(-dir.y);
                    return Some((Vec3::new(start.x, h0 + super::EYE + 0.02, start.y), yaw, h0 + rise));
                }
            }
        }
    }
    None
}
