//! Paintings (the user's: on a prominent wall, a framed panel with a
//! fractal on it, lit; not an intrusion, artistic contrast). The frame is
//! the room's own stone, laid with it (see `B::painting`); here, the
//! pictures: each a fractal by its seed, in greys (the grading takes the
//! colour out anyway), as an etching might be: a Julia set, Newton's basins
//! for a polynomial's roots, or the attractor of a few contracting maps
//! drawn by the chaos game.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use worldgen::noise::hash01;

/// Pixels on the picture's longer side.
const LONG: u32 = 512;

/// A painting on a wall, in metres from the room's corner: the middle of
/// its picture (just off the panel), which way the picture faces, its
/// size, and its seed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Painting {
    pub at: Vec3,
    pub facing: Vec3,
    pub size: Vec2,
    pub seed: u32,
    /// Its picture on a dark ground (turned over if mostly light).
    pub dark: bool,
}

/// The picture for a painting `size` across and high, by its seed.
pub(crate) fn picture(seed: u32, size: Vec2, dark: bool) -> Image {
    let (w, h) = if size.x >= size.y { (LONG, ((LONG as f32 * size.y / size.x) as u32).max(64)) } else { (((LONG as f32 * size.x / size.y) as u32).max(64), LONG) };
    let r = |k: i32| hash01(seed as i32, k, 0, 0x7a1);
    let values: Vec<f32> = match seed % 3 {
        0 => julia(w, h, &r),
        1 => newton(w, h, &r),
        _ => ifs(w, h, &r),
    };
    // (A dark ground: mostly light, it read as a lit screen, a television,
    // the user; turned over, an etching.)
    let mean = values.iter().sum::<f32>() / values.len().max(1) as f32;
    let values: Vec<f32> = if dark && mean > 0.45 { values.into_iter().map(|v| 1.0 - v).collect() } else { values };
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for v in values {
        let g = (v.clamp(0.0, 1.0) * 255.0) as u8;
        data.extend_from_slice(&[g, g, g, 255]);
    }
    Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)
}

/// Where a pixel is on the complex plane: the picture's middle at
/// `centre`, `span` across its shorter side.
fn plane(x: u32, y: u32, w: u32, h: u32, centre: (f32, f32), span: f32) -> (f32, f32) {
    let s = span / w.min(h) as f32;
    (centre.0 + (x as f32 - w as f32 * 0.5) * s, centre.1 + (y as f32 - h as f32 * 0.5) * s)
}

/// A Julia set: c on the edge of the Mandelbrot set's main body (where the
/// sets are richest), banded by how fast each point escapes; the set
/// itself dark.
fn julia(w: u32, h: u32, r: &dyn Fn(i32) -> f32) -> Vec<f32> {
    let a = r(1) * std::f32::consts::TAU;
    // (The main cardioid's edge, a little in or out.)
    let edge = 0.5 * (a.cos() - 0.5 * (2.0 * a).cos());
    let (cr, ci) = (edge * (1.0 + 0.04 * (r(2) - 0.5)), 0.5 * (a.sin() - 0.5 * (2.0 * a).sin()) * (1.0 + 0.04 * (r(3) - 0.5)));
    let bands = 0.12 + 0.12 * r(4);
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let (mut zr, mut zi) = plane(x, y, w, h, (0.0, 0.0), 3.2);
            let mut k = 0;
            while k < 256 && zr * zr + zi * zi < 64.0 {
                (zr, zi) = (zr * zr - zi * zi + cr, 2.0 * zr * zi + ci);
                k += 1;
            }
            out.push(if k == 256 {
                0.06
            } else {
                // (Smooth escape count, banded.)
                let smooth = k as f32 + 1.0 - ((zr * zr + zi * zi).sqrt().ln().ln() / std::f32::consts::LN_2);
                0.25 + 0.6 * (0.5 + 0.5 * (smooth * bands).cos())
            });
        }
    }
    out
}

/// Newton's method on z^n - 1 (n 3 to 6), relaxed (each step scaled by a
/// complex factor near 1), framed somewhere on the basins' borders at a
/// zoom of its own: each point shaded by the root it runs to, darker the
/// longer it takes. (Unrelaxed and whole, every one was the same cross of
/// butterflies.)
fn newton(w: u32, h: u32, r: &dyn Fn(i32) -> f32) -> Vec<f32> {
    let n = 3 + (r(1) * 4.0) as i32;
    let turn = r(2) * std::f32::consts::TAU;
    let (ar, ai) = (0.75 + 0.6 * r(5), (r(6) - 0.5) * 0.6);
    let span = 0.6 + 2.0 * r(7);
    let centre = ((r(8) - 0.5) * (2.6 - span), (r(9) - 0.5) * (2.6 - span));
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = plane(x, y, w, h, centre, span);
            let (mut zr, mut zi) = (px * turn.cos() - py * turn.sin(), px * turn.sin() + py * turn.cos());
            let mut k = 0;
            while k < 60 {
                // z - (z^n - 1) / (n z^(n-1))
                let (mut pr, mut pi) = (1.0f32, 0.0f32);
                for _ in 0..n - 1 {
                    (pr, pi) = (pr * zr - pi * zi, pr * zi + pi * zr);
                }
                let (fr, fi) = (pr * zr - pi * zi - 1.0, pr * zi + pi * zr);
                let (dr, di) = (pr * n as f32, pi * n as f32);
                let d = dr * dr + di * di;
                if d < 1e-12 {
                    break;
                }
                let (qr, qi) = ((fr * dr + fi * di) / d, (fi * dr - fr * di) / d);
                let (qr, qi) = (qr * ar - qi * ai, qr * ai + qi * ar);
                zr -= qr;
                zi -= qi;
                k += 1;
                if qr * qr + qi * qi < 1e-8 {
                    break;
                }
            }
            let root = ((zi.atan2(zr) / std::f32::consts::TAU * n as f32).round() as i32).rem_euclid(n);
            let tone = 0.3 + 0.55 * root as f32 / (n - 1) as f32;
            out.push(tone * (1.0 - k as f32 / 40.0).clamp(0.15, 1.0));
        }
    }
    out
}

/// The attractor of 2-4 contracting maps (each a turn, a shrink, a shift),
/// drawn by the chaos game, its density in log steps: ferns, spirals,
/// branching dusts. Maps whose attractor covers too little of the picture
/// (a thin dust, near black) tried again, up to eight times.
fn ifs(w: u32, h: u32, r: &dyn Fn(i32) -> f32) -> Vec<f32> {
    let mut last = Vec::new();
    for attempt in 0..8 {
        let out = attractor(w, h, &|k: i32| r(k + attempt * 1000));
        let covered = out.iter().filter(|v| **v > 0.2).count() as f32 / out.len() as f32;
        if covered > 0.06 {
            return out;
        }
        last = out;
    }
    last
}

fn attractor(w: u32, h: u32, r: &dyn Fn(i32) -> f32) -> Vec<f32> {
    let n = 2 + (r(1) * 3.0) as i32;
    let maps: Vec<[f32; 6]> = (0..n)
        .map(|m| {
            let k = 10 + m * 10;
            let (a, s) = ((r(k) - 0.5) * std::f32::consts::TAU, 0.35 + 0.5 * r(k + 1));
            let skew = 0.7 + 0.6 * r(k + 2);
            [s * a.cos(), -s * a.sin() * skew, s * a.sin(), s * a.cos() * skew, (r(k + 3) - 0.5) * 1.2, (r(k + 4) - 0.5) * 1.2]
        })
        .collect();
    // (Where it lies first, to frame it.)
    let mut p = (0.0f32, 0.0f32);
    let mut pts = Vec::with_capacity(400_000);
    let mut seed = 1u32;
    for i in 0..400_000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let m = &maps[(seed >> 16) as usize % maps.len()];
        p = (m[0] * p.0 + m[1] * p.1 + m[4], m[2] * p.0 + m[3] * p.1 + m[5]);
        if i > 20 {
            pts.push(p);
        }
    }
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for &(x, y) in &pts {
        (lo, hi) = ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y)));
    }
    let scale = ((w as f32 * 0.85) / (hi.0 - lo.0).max(1e-3)).min((h as f32 * 0.85) / (hi.1 - lo.1).max(1e-3));
    let (ox, oy) = (w as f32 * 0.5 - (lo.0 + hi.0) * 0.5 * scale, h as f32 * 0.5 - (lo.1 + hi.1) * 0.5 * scale);
    let mut density = vec![0u32; (w * h) as usize];
    for &(x, y) in &pts {
        let (px, py) = ((x * scale + ox) as i32, (y * scale + oy) as i32);
        if (0..w as i32).contains(&px) && (0..h as i32).contains(&py) {
            density[(px as u32 + py as u32 * w) as usize] += 1;
        }
    }
    let most = density.iter().copied().max().unwrap_or(1).max(1) as f32;
    density.into_iter().map(|d| 0.1 + 0.8 * ((1.0 + d as f32).ln() / (1.0 + most).ln())).collect()
}

/// The paintings put in the world: each its picture on a quad facing out
/// of its wall, lit by the room's light and glowing a little of its own.
pub(crate) fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>, paintings: &[Painting]) {
    for p in paintings {
        let image = images.add(picture(p.seed, p.size, p.dark));
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image.clone()),
            emissive_texture: Some(image),
            emissive: LinearRgba::gray(0.6),
            perceptual_roughness: 0.85,
            ..default()
        });
        let quad = meshes.add(Rectangle::new(p.size.x, p.size.y));
        commands.spawn((Mesh3d(quad), MeshMaterial3d(material), Transform::from_translation(p.at).with_rotation(Quat::from_rotation_y(p.facing.x.atan2(p.facing.z))), bevy::light::NotShadowCaster));
    }
}
