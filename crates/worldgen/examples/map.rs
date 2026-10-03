//! A top-down map of the whole world, for judging layout at planet scale:
//! relief shading tinted by district, canals, landmarks and a grid.
//!
//! `cargo run -p worldgen --release --example map -- [out.png] [metres per pixel] [seed]`

use std::thread;

use glam::Vec3;
use image::{Rgb, RgbImage};
use worldgen::{district::District, landmarks, plates::PlateWorld};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.first().cloned().unwrap_or_else(|| "map.png".into());
    let scale: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(8.0);
    let seed: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

    let world = PlateWorld::new(16384.0, seed);
    let size = world.size();
    let px = (size / scale) as u32;

    // Heights and districts, in parallel bands of rows.
    let threads = thread::available_parallelism().map_or(8, |n| n.get());
    let rows_per = px.div_ceil(threads as u32);
    let mut heights = vec![0.0f32; (px * px) as usize];
    let mut kinds = vec![District::Floor; (px * px) as usize];
    thread::scope(|scope| {
        let world = &world;
        for (band, (h_rows, k_rows)) in heights
            .chunks_mut((rows_per * px) as usize)
            .zip(kinds.chunks_mut((rows_per * px) as usize))
            .enumerate()
        {
            scope.spawn(move || {
                for (i, (h, k)) in h_rows.iter_mut().zip(k_rows.iter_mut()).enumerate() {
                    let y = band as u32 * rows_per + i as u32 / px;
                    let x = i as u32 % px;
                    let (wx, wz) = ((x as f32 + 0.5) * scale, (y as f32 + 0.5) * scale);
                    *h = world.height_at(wx, wz);
                    *k = world.district(wx as f64, wz as f64);
                }
            });
        }
    });

    let at = |x: i64, y: i64| {
        let (x, y) = (x.rem_euclid(px as i64) as u32, y.rem_euclid(px as i64) as u32);
        heights[(y * px + x) as usize]
    };
    let mut img = RgbImage::new(px, px);
    for y in 0..px {
        for x in 0..px {
            let (xi, yi) = (x as i64, y as i64);
            // Light from the north-west; steps show as sharp edges.
            let slope = (at(xi - 1, yi) - at(xi + 1, yi)) + (at(xi, yi - 1) - at(xi, yi + 1));
            let shade = (0.62 + slope * 0.08 / scale).clamp(0.08, 1.0);
            let lift = (at(xi, yi) / 400.0).clamp(-0.15, 0.25);
            let tint = match kinds[(y * px + x) as usize] {
                District::Floor => [1.0, 0.92, 0.72],
                District::Tiers => [0.72, 0.95, 0.75],
                District::Stacks => [0.78, 0.7, 0.95],
                District::Broken => [0.75, 0.82, 0.9],
            };
            let c = |t: f32| (((shade + lift) * t).clamp(0.0, 1.0) * 255.0) as u8;
            let mut pixel = [c(tint[0]), c(tint[1]), c(tint[2])];
            let (wx, wz) = ((x as f32 + 0.5) * scale, (y as f32 + 0.5) * scale);
            if world.canal_at(wx, wz).is_some() {
                pixel = [40, 200, 230];
            }
            if (x as f32 * scale) % 1024.0 < scale || (y as f32 * scale) % 1024.0 < scale {
                pixel = pixel.map(|v| v / 2);
            }
            img.put_pixel(x, y, Rgb(pixel));
        }
    }

    let ground = |x: f32, z: f32| world.height_at(x, z);
    let district = |x: f32, z: f32| world.district(x as f64, z as f64);
    for landmark in landmarks::place(size, seed, ground, district) {
        let Vec3 { x, z, .. } = landmark.origin;
        let (cx, cy) = ((x / scale) as i64, (z / scale) as i64);
        for dy in -5..=5i64 {
            for dx in -5..=5i64 {
                if dx.abs().max(dy.abs()) >= 4 {
                    let (px_, py_) = ((cx + dx).rem_euclid(px as i64), (cy + dy).rem_euclid(px as i64));
                    img.put_pixel(px_ as u32, py_ as u32, Rgb([230, 30, 30]));
                }
            }
        }
        println!("{:>14} at {:6.0}, {:6.0} ({})", landmark.kind, x, z, district(x, z).name());
    }

    let mut counts = [0usize; 4];
    for k in &kinds {
        counts[District::ALL.iter().position(|d| d == k).unwrap()] += 1;
    }
    for (d, c) in District::ALL.iter().zip(counts) {
        println!("{:>8}: {:4.1}%", d.name(), c as f32 / kinds.len() as f32 * 100.0);
    }
    img.save(&out).expect("write map");
    println!("wrote {out} ({px}x{px}, {scale} m/px)");
}
