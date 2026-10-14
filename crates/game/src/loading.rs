//! The window shown only once the view has settled (the user's idea): the
//! world generated, the ground streamed in and the frames even. Before, the
//! window stood blank while the world was generated (blocking all else),
//! then ran unevenly for two or three seconds as shaders compiled and the
//! ground streamed. It renders hidden all the while, so all that is done
//! when it shows. (Unattended runs, captures, benches and bots, are never
//! shown.)

use bevy::prelude::*;

use crate::{Args, terrain::Streamer};

/// Shown once the frames have been even this long (s), none slower than
/// `EVEN` (s); after `AT_MOST` (s) whatever happens.
const EVEN_FOR: f32 = 0.5;
const EVEN: f32 = 1.0 / 40.0;
const AT_MOST: f32 = 15.0;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show);
    }
}

fn show(args: Res<Args>, mut winit: ResMut<bevy::winit::WinitSettings>, time: Res<Time<Real>>, streamer: Res<Streamer>, mut window: Single<&mut Window>, mut even: Local<f32>, mut shown: Local<bool>) {
    if *shown || args.unattended() {
        return;
    }
    let dt = time.delta_secs();
    *even = if streamer.settled && dt < EVEN { *even + dt } else { 0.0 };
    if *even >= EVEN_FOR || time.elapsed_secs() > AT_MOST {
        *shown = true;
        window.visible = true;
        window.focused = true;
        // (Maximised, the user; captures, never shown, keep their 1600 by
        // 900.)
        window.set_maximized(true);
        // (Flat out until now, hidden (see `main`); as games are, from
        // now: unfocused, slowed.)
        *winit = bevy::winit::WinitSettings::game();
        let since = crate::STARTED.get().map_or(0.0, |t| t.elapsed().as_secs_f32());
        info!("loading: shown {since:.1} s from the start{}", if *even >= EVEN_FOR { "" } else { " (never settled)" });
    }
}
