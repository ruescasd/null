//! Rounds, the combat lab's (`round KIND [N] [option ...]` at the console;
//! in the arena through its gates, elsewhere before you): a fight timed from
//! when it is asked for to when the last of it falls, or you do, and what it
//! took said at the end, on the screen, in the console and in the log: how
//! long, how much you were hurt, how many shots, how many of their shards
//! struck. While it goes on, the time, how many are left and the hurt, at
//! the screen's top left. A new round gives up the one going on; `reset`
//! clears every creature and gives you your health back (see `die`).

use bevy::prelude::*;

use super::{Feedback, Gun, SHARDS, Swarmer, bishop::Bishop, hunter::Hunter};
use crate::{
    chasm::arena::{LEAF, Leaf, SINK},
    player::Player,
};

/// The round going on, if any, and what was said of the last.
#[derive(Resource, Default)]
pub(super) struct Round {
    /// What it is (none: no round going on).
    what: Option<String>,
    began: f32,
    /// The gun's shots when it began; the shards that struck since; how
    /// much you were hurt, from your health last frame.
    shots: u32,
    hits: u32,
    hurt: f32,
    health: f32,
    /// Whether any of it has been alive yet (it comes a frame or two after
    /// it is asked for).
    come: bool,
    /// What was said of the last, shown till `until`.
    said: String,
    until: f32,
}

/// How long what a round took stays on the screen (s).
const SHOWN: f32 = 20.0;

/// The arena's gates (see `arena::Leaf`): whether they stand open, from
/// when. A round opens them, a breath after it begins (what comes through
/// it waits behind them); its end shuts them.
#[derive(Resource, Default)]
pub(super) struct Gates {
    open: bool,
    from: f32,
}

/// How long after a round begins its gates open (s); how fast they sink
/// and rise (m/s: 22 m in about 7 s).
const BREATH: f32 = 2.0;
const GATE_SPEED: f32 = 3.0;

/// The gates' leaves, sinking to stand open or rising to shut; not rising
/// while you stand where one would.
pub(super) fn gates(time: Res<Time>, gates: Res<Gates>, player: Single<&Transform, With<Player>>, mut leaves: Query<(&Leaf, &mut Transform), Without<Player>>) {
    if time.elapsed_secs() < gates.from {
        return;
    }
    for (leaf, mut transform) in &mut leaves {
        let target = if gates.open { leaf.shut.y - SINK } else { leaf.shut.y };
        if target > transform.translation.y {
            let local = transform.rotation.inverse() * (player.translation - leaf.shut);
            if local.x.abs() < LEAF.x * 0.5 + 0.6 && local.z.abs() < LEAF.z * 0.5 + 0.6 {
                continue;
            }
        }
        let y = transform.translation.y;
        transform.translation.y = y + (target - y).clamp(-GATE_SPEED * time.delta_secs(), GATE_SPEED * time.delta_secs());
    }
}

/// The readout at the screen's top left.
#[derive(Component)]
pub(super) struct Readout;

pub(super) fn setup(mut commands: Commands) {
    commands.spawn((
        Readout,
        Text::new(""),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(Color::srgb(0.85, 0.85, 0.85)),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), left: Val::Px(12.0), ..default() },
    ));
}

/// Keeps the round: before `die` (which ends it if you fall, clearing all,
/// and with `god` gives your health back before it is read), after `fire`
/// (whose shards that struck this frame `feedback` has not yet taken).
#[allow(clippy::too_many_arguments)]
pub(super) fn track(
    time: Res<Time>,
    gun: Res<Gun>,
    feedback: Res<Feedback>,
    mut spawns: ResMut<crate::console::Spawns>,
    mut round: ResMut<Round>,
    mut gates: ResMut<Gates>,
    mut console: ResMut<crate::console::Console>,
    player: Single<&Player>,
    hunters: Query<&Hunter>,
    swarm: Query<&Swarmer>,
    bishops: Query<&Bishop>,
    brutes: Query<&super::impaled::Impaled>,
    tumblers: Query<&super::tumbler::Tumbler>,
    mut readout: Single<&mut Text, With<Readout>>,
) {
    let now = time.elapsed_secs();
    if let Some(what) = spawns.round.take() {
        *round = Round { what: Some(what), began: now, shots: gun.shots, health: player.health, ..default() };
        *gates = Gates { open: true, from: now + BREATH };
    }
    let alive = hunters.iter().filter(|h| h.health > 0.0).count() + swarm.iter().count() + bishops.iter().filter(|b| b.health > 0.0).count() + brutes.iter().filter(|b| b.alive()).count() + tumblers.iter().filter(|t| t.alive()).count();
    let Some(what) = round.what.clone() else {
        readout.0 = if now < round.until { round.said.clone() } else { String::new() };
        return;
    };
    round.hits += feedback.hits;
    round.hurt += (round.health - player.health).max(0.0);
    round.health = player.health;
    round.come |= alive > 0;
    let took = now - round.began;
    let end = if player.health <= 0.0 {
        Some("you fell")
    } else if spawns.reset {
        Some("reset")
    } else if round.come && alive == 0 {
        Some("cleared")
    } else {
        None
    };
    let Some(how) = end else {
        readout.0 = format!("{what}: {took:.1} s, {alive} left, hurt {:.0}", round.hurt);
        return;
    };
    let fired = gun.shots - round.shots;
    let shards = fired * SHARDS as u32;
    let line = format!("{what}: {how} in {took:.1} s; hurt {:.0}; {fired} shots, {} of their {shards} shards struck ({:.0}%)", round.hurt, round.hits, 100.0 * round.hits as f32 / shards.max(1) as f32);
    info!("round: {line}");
    *gates = Gates { open: false, from: now };
    console.say(line.clone());
    readout.0 = line.clone();
    round.said = line;
    round.until = now + SHOWN;
    round.what = None;
}
