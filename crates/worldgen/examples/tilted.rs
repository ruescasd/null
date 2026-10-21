//! Tilted-native ground, step 1 (see `tilted.rs`): its grain from above. A
//! patch shaded by its sheets' slopes (lit from the north-west, so dip
//! slopes and escarpments show), each sheet outlined faintly, the faults
//! between blocks in red.
//!
//! `cargo run -p worldgen --release --example tilted -- [out.png] [metres per pixel] [span m] [centre x] [centre z] [seed]`

use glam::DVec2;
use image::{Rgb, RgbImage};
use worldgen::tilted::Tilted;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let num = |i: usize, d: f64| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let out = a.first().cloned().unwrap_or_else(|| "tilted.png".into());
    let (scale, span) = (num(1, 2.0), num(2, 3000.0));
    let centre = DVec2::new(num(3, 8192.0), num(4, 8192.0));
    let seed = num(5, 1.0) as u32;
    let t = Tilted::new(worldgen::WORLD_SIZE, seed);
    let px = (span / scale) as u32;
    let at = |x: u32, z: u32| centre + DVec2::new(x as f64 - px as f64 / 2.0, z as f64 - px as f64 / 2.0) * scale;
    // Heights and sheets, in parallel rows.
    let rows: Vec<Vec<(f64, (i32, i32), (i32, i32))>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..px)
            .collect::<Vec<_>>()
            .chunks(px.div_ceil(16) as usize)
            .map(|zs| {
                let zs = zs.to_vec();
                let (t, at) = (&t, &at);
                scope.spawn(move || {
                    zs.iter()
                        .map(|&z| {
                            (0..px)
                                .map(|x| {
                                    let p = at(x, z);
                                    let s = t.sheet_at(p);
                                    (t.height(p), s.block, s.id)
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let light = glam::DVec3::new(-1.0, 1.4, -1.0).normalize();
    let mut img = RgbImage::new(px, px);
    for z in 0..px {
        for x in 0..px {
            let (h, block, id) = rows[z as usize][x as usize];
            let hx = rows[z as usize][(x + 1).min(px - 1) as usize].0 - h;
            let hz = rows[(z + 1).min(px - 1) as usize][x as usize].0 - h;
            // (Escarpments, a step between pixels, darkest facing away.)
            let n = glam::DVec3::new(-hx / scale, 1.0, -hz / scale).normalize();
            let shade = n.dot(light).clamp(0.0, 1.0);
            let tone = 0.15 + 0.75 * shade + (h / 400.0).clamp(-0.1, 0.1);
            let mut c = [tone, tone, tone];
            let edge = |dx: i32, dz: i32| {
                let (xx, zz) = ((x as i32 + dx).clamp(0, px as i32 - 1) as usize, (z as i32 + dz).clamp(0, px as i32 - 1) as usize);
                (rows[zz][xx].1, rows[zz][xx].2)
            };
            if edge(1, 0).0 != block || edge(0, 1).0 != block {
                c = [0.85, 0.15, 0.1];
            } else if edge(1, 0).1 != id || edge(0, 1).1 != id {
                c = [c[0] * 0.75, c[1] * 0.75, c[2] * 0.75];
            }
            img.put_pixel(x, z, Rgb(c.map(|v: f64| (v.clamp(0.0, 1.0) * 255.0) as u8)));
        }
    }
    img.save(&out).expect("writing the map");
    println!("{out}: {px} px, {scale} m a pixel, {span} m across, centred on ({:.0}, {:.0})", centre.x, centre.y);
}
