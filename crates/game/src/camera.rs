//! Mouse look, the free-flying (noclip) camera, and wrapping the camera's
//! position around the torus world. Walking is in `player.rs`.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::terrain::{StreamAnchor, StreamSet, WorldGen};

pub struct FlyCameraPlugin;

impl Plugin for FlyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                grab_cursor,
                fly,
                crate::player::gather_input,
                crate::player::bot_drive,
                crate::player::walk,
                wrap_position,
            )
                .chain()
                .before(StreamSet),
        );
    }
}

#[derive(Component)]
#[require(StreamAnchor)]
pub struct FlyCam {
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    /// Fly freely through everything instead of walking.
    pub noclip: bool,
}

impl FlyCam {
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }
}

fn grab_cursor(
    mut cursor: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
}

fn fly(
    time: Res<Time>,
    args: Res<crate::Args>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    cursor: Single<&CursorOptions>,
    mut cam: Single<(&mut Transform, &mut FlyCam)>,
) {
    let (transform, fly) = &mut *cam;
    // `--opt spin` turns the camera steadily, to check how things look in motion.
    if args.opt("spin") {
        fly.yaw += 0.8 * time.delta_secs();
    }
    if cursor.grab_mode != CursorGrabMode::None {
        let sensitivity = 0.0022;
        fly.yaw -= motion.delta.x * sensitivity;
        fly.pitch = (fly.pitch - motion.delta.y * sensitivity).clamp(-1.54, 1.54);
    }
    transform.rotation = fly.rotation();
    if !fly.noclip {
        return;
    }
    if scroll.delta.y != 0.0 {
        fly.speed = (fly.speed * 1.15f32.powf(scroll.delta.y)).clamp(1.0, 2000.0);
    }

    let mut dir = Vec3::ZERO;
    let forward = *transform.forward();
    let right = *transform.right();
    if keys.pressed(KeyCode::KeyW) { dir += forward; }
    if keys.pressed(KeyCode::KeyS) { dir -= forward; }
    if keys.pressed(KeyCode::KeyD) { dir += right; }
    if keys.pressed(KeyCode::KeyA) { dir -= right; }
    if keys.pressed(KeyCode::Space) || keys.pressed(KeyCode::KeyE) { dir += Vec3::Y; }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::KeyQ) { dir -= Vec3::Y; }
    let boost = if keys.pressed(KeyCode::ShiftLeft) { 5.0 } else { 1.0 };
    transform.translation += dir.normalize_or_zero() * fly.speed * boost * time.delta_secs();
}

/// Keep the camera inside [0, size) on x and z. Terrain instances are placed
/// relative to the camera, so the jump is invisible.
fn wrap_position(
    world: Res<WorldGen>,
    cam: Single<(&mut Transform, Option<&mut crate::player::Player>), With<FlyCam>>,
) {
    let (mut transform, player) = cam.into_inner();
    let size = world.size();
    let before = transform.translation;
    let t = &mut transform.translation;
    t.x = t.x.rem_euclid(size);
    t.z = t.z.rem_euclid(size);
    // Anything held in world space moves with the camera's jump.
    let shift = *t - before;
    if shift != Vec3::ZERO
        && let Some(mut player) = player
    {
        player.tether.shift(shift);
    }
}
