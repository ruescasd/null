//! The window shown only once the view has settled (the user's idea): the
//! world generated, the ground streamed in, the shaders for what is in view
//! compiled and the frames even. Before, the
//! window stood blank while the world was generated (blocking all else),
//! then ran unevenly for two or three seconds as shaders compiled and the
//! ground streamed. It renders hidden all the while, so all that is done
//! when it shows. (Unattended runs, captures, benches and bots, are never
//! shown.) (The shaders: what needs one not yet ready is not drawn; on
//! DirectX 12 (`--opt dx12`), whose compiler is slow, the window showed with
//! the main surfaces' and the post-processing's still compiling, and things
//! appeared late.)

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use bevy::{
    prelude::*,
    render::{Render, RenderApp, RenderSystems, render_resource::PipelineCache},
};

use crate::{Args, terrain::Streamer};

/// How many pipelines (shaders) are still compiling: counted in the render
/// world, read in the game's.
#[derive(Resource, Clone, Default)]
pub(crate) struct Compiling(pub(crate) Arc<AtomicUsize>);

/// Shown once the frames have been even this long (s), none slower than
/// `EVEN` (s); after `AT_MOST` (s) whatever happens.
const EVEN_FOR: f32 = 0.5;
const EVEN: f32 = 1.0 / 40.0;
const AT_MOST: f32 = 15.0;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        let compiling = Compiling::default();
        app.insert_resource(compiling.clone()).add_systems(Update, show);
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.insert_resource(compiling).add_systems(Render, count.in_set(RenderSystems::Cleanup));
        }
    }
}

fn count(cache: Res<PipelineCache>, compiling: Res<Compiling>) {
    compiling.0.store(cache.waiting_pipelines().count(), Ordering::Relaxed);
}

#[allow(clippy::too_many_arguments)]
fn show(
    args: Res<Args>,
    mut winit: ResMut<bevy::winit::WinitSettings>,
    time: Res<Time<Real>>,
    streamer: Res<Streamer>,
    compiling: Res<Compiling>,
    mut window: Single<&mut Window>,
    mut even: Local<f32>,
    mut shown: Local<bool>,
) {
    if *shown || args.unattended() {
        return;
    }
    let dt = time.delta_secs();
    let ready = streamer.settled && compiling.0.load(Ordering::Relaxed) == 0;
    *even = if ready && dt < EVEN { *even + dt } else { 0.0 };
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
