//! Holes in the ground onto the void: meshes the ground round a spot as the
//! game does (level 0, across the world's wrap), and casts rays from it in
//! every direction downwards; those that sink 5 m under all of it without
//! striking it, still over it, fell through it.
//!
//! `cargo run -p worldgen --release --example holes -- X Y Z [SEED]`
//! (`where` at the game's console gives X Y Z; Y `auto`, standing on the
//! ground there). And to see why: `--trace YAW PITCH` follows one ray,
//! `--line X` sets the height function against the mesh across the
//! world's z edge.
use glam::Vec3;
use worldgen::{WORLD_SIZE, column_size, plates::PlateWorld, structure::Library};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |i: usize, d: f32| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let seed = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1u32);
    let text = std::fs::read_to_string("data/structures.ron").expect("run from the repository's root: data/structures.ron");
    let library = Library::parse(&text).expect("data/structures.ron parses");
    let world = PlateWorld::new(WORLD_SIZE, seed).with_sites(&library);
    // (Y `auto`: standing on the ground there.)
    let (x, z) = (num(1, 0.0), num(3, 0.0));
    let y = if a.get(2).is_some_and(|s| s == "auto") { world.height_at(x, z) + 1.7 } else { num(2, 1.7) };
    let eye = Vec3::new(x, y, z);

    // The columns round the spot (a ring of them), each from its wrapped
    // key, set where the unwrapped one stands.
    let size = column_size(0);
    let n = (WORLD_SIZE / size) as i32;
    let (cx, cz) = ((eye.x / size).floor() as i32, (eye.z / size).floor() as i32);
    let reach = 2;
    let mut tris: Vec<[Vec3; 3]> = Vec::new();
    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let (kx, kz) = (cx + dx, cz + dz);
            let mesh = world.mesh_column(0, kx.rem_euclid(n), kz.rem_euclid(n));
            let origin = Vec3::new(kx as f32 * size, 0.0, kz as f32 * size);
            for t in mesh.indices.chunks(3) {
                let p = |i: u32| origin + Vec3::from(mesh.positions[i as usize]);
                tris.push([p(t[0]), p(t[1]), p(t[2])]);
            }
        }
    }
    let lo = Vec3::new((cx - reach) as f32 * size, 0.0, (cz - reach) as f32 * size);
    let span = (2 * reach + 1) as f32 * size;
    let lowest = tris.iter().flatten().map(|p| p.y).fold(f32::MAX, f32::min);
    println!("district {:?}; canal {}", world.district(eye.x as f64, eye.z as f64), world.canal_at(eye.x, eye.z).is_some());
    println!("{} triangles in {}x{} columns of {size} m round ({:.0}, {:.0}); lowest ground {lowest:.1}", tris.len(), 2 * reach + 1, 2 * reach + 1, eye.x, eye.z);

    // Triangles binned by where they lie across the ground.
    let cell = 2.0;
    let cells = (span / cell).ceil() as usize;
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); cells * cells];
    let bin = |x: f32, z: f32| -> Option<usize> {
        let (i, j) = (((x - lo.x) / cell).floor(), ((z - lo.z) / cell).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < cells && (j as usize) < cells).then(|| j as usize * cells + i as usize)
    };
    for (k, t) in tris.iter().enumerate() {
        let (x0, x1) = (t.iter().map(|p| p.x).fold(f32::MAX, f32::min), t.iter().map(|p| p.x).fold(f32::MIN, f32::max));
        let (z0, z1) = (t.iter().map(|p| p.z).fold(f32::MAX, f32::min), t.iter().map(|p| p.z).fold(f32::MIN, f32::max));
        let mut z = (z0 / cell).floor() * cell;
        while z <= z1 {
            let mut x = (x0 / cell).floor() * cell;
            while x <= x1 {
                if let Some(b) = bin(x + 0.01, z + 0.01) {
                    bins[b].push(k);
                }
                x += cell;
            }
            z += cell;
        }
    }
    let hit = |o: Vec3, d: Vec3, t: &[Vec3; 3]| -> Option<f32> {
        let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-9 {
            return None;
        }
        let s = o - t[0];
        let u = s.dot(p) / det;
        let q = s.cross(e1);
        let v = d.dot(q) / det;
        let t = e2.dot(q) / det;
        (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 1e-4).then_some(t)
    };
    // Down from the eye at every heading and slant: marched in half-metre
    // steps through the bins it crosses.
    let mut holes = Vec::new();
    let mut cast = 0;
    for yaw in (0..360).step_by(2) {
        for pitch in (1..=80).step_by(1) {
            let (y, p) = ((yaw as f32).to_radians(), -(pitch as f32).to_radians());
            let d = Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
            cast += 1;
            let mut best = f32::MAX;
            let mut s = 0.0;
            let mut out = None;
            loop {
                let at = eye + d * s;
                let Some(b) = bin(at.x, at.z) else {
                    out = Some(at);
                    break;
                };
                for &k in &bins[b] {
                    if let Some(t) = hit(eye, d, &tris[k]) {
                        best = best.min(t);
                    }
                }
                if best < s + 1.0 || at.y < lowest - 5.0 {
                    break;
                }
                s += 0.5;
            }
            // (A ray leaving the meshed ground, shallow, is no proof: the
            // ground beyond may be lower. Only one sunk 5 m under all of it,
            // still over it, fell through.)
            if best == f32::MAX && out.is_none() {
                let below = eye + d * s;
                holes.push((yaw, pitch, below));
            }
        }
    }
    println!("{cast} rays, {} through the ground", holes.len());
    // (`--trace YAW PITCH`: one ray, step by step: its height, and the
    // ground's top under it (the highest top under that point), where it
    // passes under the ground.)
    if let Some(i) = a.iter().position(|s| s == "--trace") {
        let (y, p) = (num(i + 1, 0.0).to_radians(), -num(i + 2, 30.0).to_radians());
        let d = Vec3::new(y.sin() * p.cos(), p.sin(), y.cos() * p.cos());
        let mut was_above = true;
        for k in 0..400 {
            let at = eye + d * (k as f32 * 0.25);
            let o = Vec3::new(at.x, 400.0, at.z);
            let Some(b) = bin(o.x, o.z) else { break };
            let tops: Vec<f32> = bins[b].iter().filter_map(|&t| hit(o, Vec3::NEG_Y, &tris[t]).map(|h| 400.0 - h)).collect();
            let under = tops.iter().copied().filter(|&h| h <= at.y).fold(f32::MIN, f32::max);
            let over = tops.iter().copied().filter(|&h| h > at.y).fold(f32::MAX, f32::min);
            let inside = tops.is_empty() || under == f32::MIN;
            if inside && was_above {
                println!("  {:.2} m along, at ({:.2}, {:.2}, {:.2}): nothing below it any more; tops above it from {over:.1}", k as f32 * 0.25, at.x, at.y, at.z);
                println!("    the step before: tops {:?}", {
                    let pa = eye + d * ((k as f32 - 1.0) * 0.25);
                    let ob = Vec3::new(pa.x, 400.0, pa.z);
                    bin(ob.x, ob.z).map(|b| bins[b].iter().filter_map(|&t| hit(ob, Vec3::NEG_Y, &tris[t]).map(|h| ((400.0 - h) * 10.0).round() / 10.0)).collect::<Vec<f32>>())
                });
            }
            was_above = !inside;
        }
    }
    // (`--line X`: along z across the world's edge at x = X: the height
    // function, unwrapped and wrapped, against the mesh's highest top.)
    if let Some(i) = a.iter().position(|s| s == "--line") {
        let x = num(i + 1, eye.x);
        for k in 0..16 {
            let z = WORLD_SIZE - 2.0 + k as f32 * 0.25;
            let o = Vec3::new(x, 400.0, z);
            let tops: Vec<f32> = bin(o.x, o.z).map(|b| bins[b].iter().filter_map(|&t| hit(o, Vec3::NEG_Y, &tris[t]).map(|h| ((400.0 - h) * 10.0).round() / 10.0)).collect()).unwrap_or_default();
            println!("  z {z:9.2}: height {:6.2}, wrapped {:6.2}; mesh tops {tops:?}", world.height_at(x, z), world.height_at(x, z - WORLD_SIZE));
        }
    }
    for (yaw, pitch, at) in holes.iter().take(40) {
        println!("  heading {yaw} deg, {pitch} deg down: through at about ({:.1}, {:.1}, {:.1})", at.x, at.y, at.z);
    }
}
