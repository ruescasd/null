//! Mega backdrops: a few colossal elements set behind a foreground, to try
//! what made the grand-stair wedges compelling behind the human-scale
//! fabric. Several of them, at different angles, far bigger than anything
//! in front. Kinds:
//! - stairs: colossal flights in giant steps, with buttresses along their
//!   sides and a landing at the top, ending in a sheer face;
//! - wedges: the same flights as plain sloped solids (how they first came
//!   out, "too polygonal");
//! - slabs: vast leaning plates, ribbed across;
//! - pylons: towers leaning off the vertical.
//!
//! They stand in a band right behind the origin (towards +z), spread across
//! its width, rising away from it: turned every which way (`varied`) or all
//! one way.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec2, Vec3};

use crate::noise::hash01;
use crate::structure::Solid;

const FOUNDATION: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Stairs,
    Wedges,
    /// Wedges in pairs crossing one another.
    Crossing,
    Slabs,
    Pylons,
    Mixed,
}

/// Finer grain on a wedge's slope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grain {
    None,
    /// Ribs across the slope.
    Ribs,
    /// Ridges running up it.
    Ridges,
    /// A human-scale flight up its middle.
    Flight,
}

impl Grain {
    pub fn from_name(name: &str) -> Grain {
        match name {
            "ribs" => Grain::Ribs,
            "ridges" => Grain::Ridges,
            "flight" => Grain::Flight,
            _ => Grain::None,
        }
    }
}

impl Kind {
    pub fn from_name(name: &str) -> Kind {
        match name {
            "wedges" => Kind::Wedges,
            "crossing" => Kind::Crossing,
            "slabs" => Kind::Slabs,
            "pylons" => Kind::Pylons,
            "mixed" => Kind::Mixed,
            _ => Kind::Stairs,
        }
    }
}

fn boxed(center: Vec3, half: Vec3, rotation: Quat, albedo: f32) -> Solid {
    Solid { detail: false, wedge: false, round: false, center, rotation, half, albedo }
}

/// A colossal flight in its own frame (rising along +x from x = 0), `steps`
/// giant steps up to `rise` over `run`, `width` wide; buttresses on both
/// sides, stepping with it, every few steps.
fn stair(out: &mut Vec<Solid>, run: f32, rise: f32, width: f32, steps: i32, tone: f32) {
    let (tread, riser) = (run / steps as f32, rise / steps as f32);
    for i in 0..steps {
        let top = riser * (i + 1) as f32;
        let x = tread * (i as f32 + 0.5);
        out.push(boxed(Vec3::new(x, (top - FOUNDATION) * 0.5, 0.0), Vec3::new(tread * 0.5 + 0.05, (top + FOUNDATION) * 0.5, width * 0.5), Quat::IDENTITY, tone));
    }
    // A landing at the top.
    let landing = tread * 3.0;
    out.push(boxed(Vec3::new(run + landing * 0.5, (rise - FOUNDATION) * 0.5, 0.0), Vec3::new(landing * 0.5, (rise + FOUNDATION) * 0.5, width * 0.5), Quat::IDENTITY, tone + 0.01));
    // Buttresses: fins proud of the sides, as tall as the flight beside
    // them plus a margin, every four steps.
    let fin = (width * 0.06).max(4.0);
    let mut i = 2;
    while i < steps {
        let x = tread * i as f32;
        let top = riser * i as f32 + riser * 1.5;
        for sgn in [-1.0, 1.0] {
            out.push(boxed(Vec3::new(x, (top - FOUNDATION) * 0.5, sgn * (width * 0.5 + fin * 0.5)), Vec3::new(fin * 0.6, (top + FOUNDATION) * 0.5, fin * 0.5), Quat::IDENTITY, tone + 0.03));
        }
        i += 4;
    }
}

/// A plain sloped solid in its own frame: its foot at x = 0, rising along +x
/// to `rise` at `run` (a sheer face there), `width` wide, from below the
/// ground; with `grain` on its slope (drawn at every distance: grain is
/// what reads from afar).
pub fn wedge(out: &mut Vec<Solid>, run: f32, rise: f32, width: f32, grain: Grain, tone: f32) {
    out.push(Solid { wedge: true, ..boxed(Vec3::new(run * 0.5, (rise - FOUNDATION) * 0.5, 0.0), Vec3::new(run * 0.5, (rise + FOUNDATION) * 0.5, width * 0.5), Quat::IDENTITY, tone) });
    let slope = rise.atan2(run);
    let length = (run * run + rise * rise).sqrt();
    let lie = Quat::from_rotation_z(slope);
    let normal = lie * Vec3::Y;
    let along = lie * Vec3::X;
    let mid = Vec3::new(run * 0.5, rise * 0.5, 0.0);
    match grain {
        Grain::None => {}
        Grain::Ribs => {
            let n = (length / 24.0) as i32;
            for i in 1..n {
                let c = along * (length * i as f32 / n as f32) + normal * 1.2;
                out.push(Solid { ..boxed(c, Vec3::new(1.0, 1.2, width * 0.5 + 0.8), lie, tone + 0.04) });
            }
        }
        Grain::Ridges => {
            for k in [-0.38f32, -0.13, 0.13, 0.38] {
                let c = mid + normal * 1.5 + Vec3::Z * (k * width);
                out.push(boxed(c, Vec3::new(length * 0.5, 1.5, 1.4), lie, tone + 0.04));
            }
        }
        Grain::Flight => {
            // Steps 0.45 m high up a strip in the middle, proud of the slope
            // by a step at most; low parapets either side.
            let steps = (rise / 0.45).round() as i32;
            let tread = run / steps as f32;
            let half = (width * 0.06).clamp(3.0, 8.0);
            for i in 0..steps {
                let (x0, x1) = (tread * i as f32, tread * (i + 1) as f32);
                let top = 0.45 * (i + 1) as f32;
                let low = rise * x0 / run - 1.0;
                out.push(Solid { ..boxed(Vec3::new((x0 + x1) * 0.5, (top + low) * 0.5, 0.0), Vec3::new((x1 - x0) * 0.5 + 0.01, (top - low) * 0.5, half), Quat::IDENTITY, tone + 0.05) });
            }
            for sgn in [-1.0f32, 1.0] {
                let c = mid + normal * 0.9 + Vec3::Z * (sgn * (half + 0.4));
                out.push(Solid { ..boxed(c, Vec3::new(length * 0.5, 0.9, 0.4), lie, tone + 0.06) });
            }
        }
    }
}

/// The backdrop's solids, spread behind `origin` (towards +z).
pub fn backdrop(origin: Vec3, kind: Kind, count: u32, varied: bool, grain: Grain, tone: f32, seed: u32) -> Vec<Solid> {
    let r = |a: u32, b: i32| hash01(a as i32, b, 0x3e6a, seed);
    let mut out = Vec::new();
    let n = count.max(1);
    for e in 0..n {
        // Where: in an arc behind, spread across it.
        let t = if n == 1 { 0.5 } else { e as f32 / (n - 1) as f32 };
        // Where: in a band right behind, spread across its width.
        let at = Vec2::new((t - 0.5) * 900.0 + (r(e, 1) - 0.5) * 120.0, 650.0 + r(e, 2) * 350.0);
        // Which way: rising away (their tall ends into the sky behind),
        // turned every which way, or all the same way.
        let mut heading = if varied { FRAC_PI_2 + (r(e, 3) - 0.5) * 1.8 } else { FRAC_PI_2 };
        let mut at = at;
        // Crossing: in pairs at one place, turned well apart.
        if kind == Kind::Crossing {
            let pair = e / 2;
            let t = if n <= 2 { 0.5 } else { pair as f32 / ((n - 1) / 2).max(1) as f32 };
            at = Vec2::new((t - 0.5) * 700.0, 700.0 + r(pair, 2) * 250.0);
            let spread = 0.55 + r(pair, 9) * 0.35;
            heading = FRAC_PI_2 + (r(pair, 3) - 0.5) * 0.6 + if e % 2 == 0 { spread } else { -spread };
        }
        // (The frames' +x is turned to `heading`, measured from +x towards +z.)
        let yaw = Quat::from_rotation_y(-heading);
        let kind = match kind {
            Kind::Mixed => [Kind::Stairs, Kind::Slabs, Kind::Pylons, Kind::Stairs][e as usize % 4],
            Kind::Crossing => Kind::Wedges,
            k => k,
        };
        let shade = tone + (r(e, 4) - 0.5) * 0.04;
        let mut local = Vec::new();
        match kind {
            Kind::Stairs | Kind::Wedges => {
                let run = 900.0 + r(e, 5) * 400.0;
                let rise = 650.0 + r(e, 6) * 300.0;
                let width = 120.0 + r(e, 7) * 80.0;
                if kind == Kind::Stairs {
                    stair(&mut local, run, rise, width, 26 + (r(e, 8) * 10.0) as i32, shade);
                } else {
                    wedge(&mut local, run, rise, width, grain, shade);
                }
                // The flight's foot at its place, rising away from it.
                for s in &mut local {
                    s.center -= Vec3::X * (run * 0.5);
                }
            }
            Kind::Slabs => {
                // A vast plate leaning up from the ground, ribbed across.
                let (len, wide, thick) = (800.0 + r(e, 5) * 400.0, 260.0 + r(e, 6) * 160.0, 36.0);
                let pitch = 0.3 + r(e, 7) * 0.35;
                let lean = Quat::from_rotation_z(pitch);
                let up = lean * Vec3::X;
                let center = up * (len * 0.5) + lean * Vec3::Y * (thick * 0.5) - Vec3::X * (len * 0.5 * pitch.cos());
                local.push(boxed(center, Vec3::new(len * 0.5, thick * 0.5, wide * 0.5), lean, shade));
                let ribs = 9;
                for i in 0..ribs {
                    let d = len * (i as f32 + 0.5) / ribs as f32 - len * 0.5;
                    let c = center + up * d + lean * Vec3::Y * (thick * 0.5 + 6.0);
                    local.push(boxed(c, Vec3::new(5.0, 6.0, wide * 0.5 + 4.0), lean, shade + 0.04));
                }
            }
            Kind::Pylons => {
                // A tower leaning off the vertical.
                let (h, w) = (700.0 + r(e, 5) * 400.0, 60.0 + r(e, 6) * 40.0);
                let lean = Quat::from_rotation_z(-(0.06 + r(e, 7) * 0.16));
                let center = lean * Vec3::Y * (h * 0.5 - FOUNDATION);
                local.push(boxed(center, Vec3::new(w * 0.5, h * 0.5, w * 0.5), lean, shade));
                // Bands up it.
                let mut y = 40.0;
                while y < h - 20.0 {
                    local.push(boxed(lean * Vec3::Y * (y - FOUNDATION), Vec3::new(w * 0.5 + 2.5, 2.5, w * 0.5 + 2.5), lean, shade + 0.04));
                    y += 60.0;
                }
            }
            Kind::Mixed | Kind::Crossing => unreachable!(),
        }
        let place = origin + Vec3::new(at.x, 0.0, at.y);
        out.extend(local.into_iter().map(|s| Solid { center: place + yaw * s.center, rotation: yaw * s.rotation, ..s }));
    }
    out
}
