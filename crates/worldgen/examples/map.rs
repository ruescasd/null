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

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron");
    let library = Library::parse(&std::fs::read_to_string(path).expect("read structures.ron")).expect("parse");
    let world = PlateWorld::new(16384.0, seed).with_sites(&library);
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
    // What grows on a site: its centrepiece's style and its plates' form.
    let kind = |site: &sites::Site| {
        let names: Vec<&str> =
            site.centrepiece.iter().map(|p| p.style.as_str()).chain(site.plates.as_deref()).collect();
        names.join("+")
    };
    let all = sites::all(&library, &world);
    let mut cores = Vec::new();
    let mut styles = std::collections::BTreeMap::<String, usize>::new();
    for site in &all {
        if let Some(p) = &site.centrepiece {
            outline(p.at, p.size, p.yaw, [230, 30, 30]);
        }
        // The core, as a circle.
        if let Some(ground) = &site.ground {
            let steps = (ground.core * 8.0 / scale as f64) as i32;
            for i in 0..steps {
                let a = i as f64 / steps as f64 * std::f64::consts::TAU;
                let (x, y) = (ground.center.x + a.cos() * ground.core, ground.center.y + a.sin() * ground.core);
                cores.push(((x / scale as f64) as i64, (y / scale as f64) as i64));
            }
        }
        *styles.entry(kind(site)).or_default() += 1;
    }
    for p in &library.structures {
        outline(p.at, p.size, p.yaw, [240, 220, 40]);
    }
    for (x, y) in cores {
        img.put_pixel(x.rem_euclid(px as i64) as u32, y.rem_euclid(px as i64) as u32, Rgb([230, 30, 30]));
    }
    println!("{} sites: {styles:?}", all.len());
    // The nearest few to the default spawn point, for pointing the camera.
    let mut near = sites::near(&library, &world, 1200.0, 900.0, 4000.0);
    near.sort_by(|a, b| {
        let d = |s: &sites::Site| (s.at.0 - 1200.0).hypot(s.at.1 - 900.0);
        d(a).total_cmp(&d(b))
    });
    // ...the nearest few, and the nearest in each district.
    let mut shown: Vec<&sites::Site> = near.iter().take(6).collect();
    for district in District::ALL {
        let at = |s: &&sites::Site| world.district(s.at.0 as f64, s.at.1 as f64);
        if let Some(site) = near.iter().find(|s| at(s) == district) {
            if !shown.iter().any(|s| s.cell == site.cell) {
                shown.push(site);
            }
        }
    }
    for site in shown {
        let (x, z) = site.at;
        println!(
            "{:>22} at {:6.0}, {:6.0}  ({}), core {:+.1} m from the landform, radius {:.0} m",
            kind(site),
            x,
            z,
            world.district(x as f64, z as f64).name(),
            site.ground.map_or(0.0, |g| g.top - world.shaped(x as f64, z as f64)),
            site.ground.map_or(0.0, |g| g.radius),
        );
    }

    // The nearest of each kind (for photographing every kind).
    println!("nearest of each kind:");
    let mut seen: Vec<String> = Vec::new();
    for site in &near {
        let k = kind(site);
        if seen.contains(&k) {
            continue;
        }
        let (x, z) = site.at;
        println!("  kind {k} {x:.0} {z:.0} {:.0}", site.ground.map_or(0.0, |g| g.radius));
        seen.push(k);
    }

    // The colossi: how many, and the nearest few with their height.
    let colossi = sites::all_in(sites::Layer::Colossi, &library, &world);
    println!("{} colossi", colossi.len());
    let mut near: Vec<&sites::Site> = colossi.iter().collect();
    near.sort_by(|a, b| {
        let d = |s: &sites::Site| (s.at.0 - 1200.0).hypot(s.at.1 - 900.0);
        d(a).total_cmp(&d(b))
    });
    let mut seen: Vec<String> = Vec::new();
    for site in &near {
        let f = site.form.as_ref().map_or(String::new(), |f| f.0.clone());
        if seen.contains(&f) {
            continue;
        }
        seen.push(f.clone());
        println!("  colossus {f} {:.0} {:.0} {:.0}", site.at.0, site.at.1, site.form.as_ref().map_or(0.0, |f| f.1));
    }
    for site in near.iter().take(6) {
        let built = sites::build(&library, &world, site, 500_000);
        let top = built.prisms.iter().map(|p| p.y1).fold(0.0, f32::max);
        let (x, z) = site.at;
        println!(
            "{:>22} at {:6.0}, {:6.0}  ({}), {} pieces + {} blocks, {:.0} m tall, footprint {:.0} m",
            site.form.as_ref().map_or("", |f| f.0.as_str()),
            x,
            z,
            world.district(x as f64, z as f64).name(),
            built.prisms.len(),
            built.solids.len(),
            top.max(built.solids.iter().map(|s| s.center.y + s.half.y).fold(0.0, f32::max)),
            site.form.as_ref().map_or(0.0, |f| f.1),
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
