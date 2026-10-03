//! A top-down map of the whole world, for judging layout at planet scale:
//! relief shading tinted by district, canals, the footprints of the sites
//! and test structures in `data/structures.ron`, and a 1 km grid.
//!
//! `cargo run -p worldgen --release --example map -- [out.png] [metres per pixel] [seed]`

use std::thread;

use glam::Vec3;
use image::{Rgb, RgbImage};
use worldgen::{district::District, plates::PlateWorld, sites, structure::Library};

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

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron");
    let library = Library::parse(&std::fs::read_to_string(path).expect("read structures.ron")).expect("parse");
    // Footprint outlines: sites in red, test structures in yellow.
    let mut outline = |at: (f32, f32), size: (f32, f32, f32), yaw: f32, color: [u8; 3]| {
        let rotation = glam::Quat::from_rotation_y(yaw.to_radians());
        let (hx, hz) = (size.0 * 0.5, size.2 * 0.5);
        let steps = ((hx.max(hz) * 4.0 / scale) as i32).max(8);
        for side in 0..4 {
            for i in 0..=steps {
                let t = i as f32 / steps as f32 * 2.0 - 1.0;
                let (lx, lz) = match side {
                    0 => (t * hx, -hz),
                    1 => (t * hx, hz),
                    2 => (-hx, t * hz),
                    _ => (hx, t * hz),
                };
                let w = rotation * Vec3::new(lx, 0.0, lz);
                let (x, y) = (((at.0 + w.x) / scale) as i64, ((at.1 + w.z) / scale) as i64);
                img.put_pixel(x.rem_euclid(px as i64) as u32, y.rem_euclid(px as i64) as u32, Rgb(color));
            }
        }
    };
    let all = sites::all(&library, &world);
    let mut styles = std::collections::BTreeMap::<&str, usize>::new();
    for site in &all {
        let p = &site.placement;
        outline(p.at, p.size, p.yaw, [230, 30, 30]);
        *styles.entry(&p.style).or_default() += 1;
    }
    for p in &library.structures {
        outline(p.at, p.size, p.yaw, [240, 220, 40]);
    }
    println!("{} sites: {styles:?}", all.len());
    // The nearest few to the default spawn point, for pointing the camera.
    let mut near = sites::near(&library, &world, 1200.0, 900.0, 2500.0);
    near.sort_by(|a, b| {
        let d = |s: &sites::Site| (s.placement.at.0 - 1200.0).hypot(s.placement.at.1 - 900.0);
        d(a).total_cmp(&d(b))
    });
    for site in near.iter().take(8) {
        let p = &site.placement;
        println!(
            "{:>10} at {:6.0}, {:6.0}  size {:.0} x {:.0} x {:.0}  ({}), podium {:.1} m above the ground at its centre",
            p.style, p.at.0, p.at.1, p.size.0, p.size.1, p.size.2,
            world.district(p.at.0 as f64, p.at.1 as f64).name(),
            sites::build(&library, &world, site, 100).base - world.height_at(p.at.0, p.at.1)
        );
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
