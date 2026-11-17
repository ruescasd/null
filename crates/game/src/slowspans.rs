//! Slow spans (`--features slowspans`, built apart, see Cargo.toml): every
//! span Bevy times (each system, each render step) measured as it runs, and
//! those longer than `SLOW_SPAN_MS` milliseconds (4 unless set; a frame's
//! containers, `SLOW_FRAME_MS`, 18) printed on
//! the spot, with the system's name and the time, to set beside the
//! `hitch:` lines. For hitches too rare for a whole trace (a Chrome trace
//! of a minute runs to tens of gigabytes): the user's, a 20-30 ms frame
//! every 20 s or so, nothing arriving in it (2026-11-17).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::log::{
    BoxedLayer,
    tracing_subscriber::{Layer, Registry, layer::Context, registry::LookupSpan},
};
use tracing::{
    Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id},
};

/// The span's own name (a system's is in its `name` field), and when it was
/// last entered.
struct Timing {
    name: String,
    entered: Option<Instant>,
}

struct Named(String);

impl Visit for Named {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" {
            self.0 = format!("{value:?}");
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0 = value.to_string();
        }
    }
}

struct SlowSpans {
    threshold: f64,
    /// For the frame's own containers (the update, a schedule, a sub app,
    /// presenting): only a slow frame's, `SLOW_FRAME_MS` (18).
    frame: f64,
}

/// The spans that hold a whole frame or a schedule of it.
const CONTAINERS: [&str; 10] = [
    "schedule",
    "sub app",
    "update",
    "main_render_schedule",
    "present_frames",
    "main app",
    "render thread",
    "bevy_app",
    "system: bevy_render::run_render_schedule",
    "system: bevy_render::renderer::render_system",
];

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for SlowSpans {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut named = Named(String::new());
        attrs.record(&mut named);
        let name = if named.0.is_empty() { attrs.metadata().name().to_string() } else { format!("{}: {}", attrs.metadata().name(), named.0) };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Timing { name, entered: None });
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(t) = span.extensions_mut().get_mut::<Timing>()
        {
            t.entered = Some(Instant::now());
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut ext = span.extensions_mut();
        let Some(t) = ext.get_mut::<Timing>() else { return };
        let Some(entered) = t.entered.take() else { return };
        let ms = entered.elapsed().as_secs_f64() * 1e3;
        let container = CONTAINERS.iter().any(|c| t.name == *c || t.name.starts_with(&format!("{c}: ")));
        if ms >= if container { self.frame } else { self.threshold } {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
            let (h, m, s) = ((now / 3600.0) as u64 % 24, (now / 60.0) as u64 % 60, now % 60.0);
            eprintln!("slowspan {h:02}:{m:02}:{s:06.3} {ms:6.1} ms  {}  (thread {:?})", t.name, std::thread::current().name().unwrap_or("?"));
        }
    }
}

/// The layer, for `LogPlugin::custom_layer`.
pub fn layer(_: &mut bevy::app::App) -> Option<BoxedLayer> {
    let env = |name: &str, default: f64| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
    Some(Box::new(SlowSpans { threshold: env("SLOW_SPAN_MS", 4.0), frame: env("SLOW_FRAME_MS", 18.0) }) as Box<dyn Layer<Registry> + Send + Sync>)
}
