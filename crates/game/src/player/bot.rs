//! A scripted test pilot (`--opt bot`): drives the real movement code with
//! synthetic input and logs telemetry, so movement can be checked without
//! anyone at the keyboard. It stands, runs, strafe jumps at the optimal
//! angle, then fires the thrust beam at its feet, and exits.

use bevy::prelude::*;

use super::{AIR_ACCEL, MAX_SPEED, MoveInput, Player, STEP_DT};
use crate::{Args, camera::FlyCam, terrain::WorldGen};

#[derive(Default)]
pub struct BotState {
    started: Option<f32>,
    last_log: f32,
    /// Height of the ledge top the step test runs onto.
    step_target: Option<f32>,
}

pub fn drive(
    args: Res<Args>,
    time: Res<Time>,
    mut state: Local<BotState>,
    mut input: ResMut<MoveInput>,
    mut exit: MessageWriter<AppExit>,
    world: Res<WorldGen>,
    camera: Single<(&mut Transform, &mut FlyCam, &mut Player)>,
) {
    if !args.opt("bot") {
        return;
    }
    let (mut transform, mut fly, mut player) = camera.into_inner();
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
        t if t < 14.0 => "thrust down",
        t if t < 16.0 => "step up",
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
        "thrust down" => {
            fly.pitch = -1.5;
            input.fire = true;
        }
        "step up" => {
            if state.step_target.is_none() {
                // Find a ledge 0.2-0.55 m high with flat ground before it, put
                // the bot 3 m in front of it and run at it.
                let Some((start, yaw, top)) = find_ledge(&world, transform.translation) else {
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

    if now - state.last_log >= if phase == "run" { 0.25 } else { 0.5 } {
        state.last_log = now;
        let p = transform.translation;
        info!(
            "bot t={t:4.1} {phase:12} speed {speed:5.2} m/s ({:4.0} ups)  vy {:6.2}  y {:7.2}  grounded {}  energy {:3.0}  pos {:.0},{:.0}",
            speed / 0.032,
            player.velocity.y,
            p.y,
            player.grounded,
            player.energy,
            p.x,
            p.z,
        );
    }
}

/// A spot in front of a small ledge: (eye position, yaw facing it, ledge top).
fn find_ledge(world: &WorldGen, around: Vec3) -> Option<(Vec3, f32, f32)> {
    for ring in 1..30 {
        for k in 0..ring * 8 {
            let a = k as f32 / (ring * 8) as f32 * std::f32::consts::TAU;
            let p = Vec2::new(around.x, around.z) + Vec2::from_angle(a) * ring as f32 * 1.5;
            for d in 0..8 {
                let dir = Vec2::from_angle(d as f32 / 8.0 * std::f32::consts::TAU);
                let h = |q: Vec2| world.ground_height(q.x, q.y);
                let h0 = h(p);
                let rise = h(p + dir * 1.0) - h0;
                let runway = (0..=6).all(|i| (h(p - dir * (i as f32 * 0.5)) - h0).abs() < 0.01);
                let landing = (1..=4).all(|i| (h(p + dir * (1.0 + i as f32 * 0.5)) - h0 - rise).abs() < 0.01);
                if rise > 0.2 && rise < 0.55 && runway && landing {
                    let start = p - dir * 3.0;
                    // Forward is (-sin yaw, -cos yaw) in x, z.
                    let yaw = (-dir.x).atan2(-dir.y);
                    return Some((Vec3::new(start.x, h0 + super::EYE + 0.02, start.y), yaw, h0 + rise));
                }
            }
        }
    }
    None
}
