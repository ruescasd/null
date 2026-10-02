//! Times column meshing: `cargo run -p worldgen --release --example bench`.
use std::time::Instant;
use worldgen::{World, WorldConfig, mesh_column};

fn main() {
    let t = Instant::now();
    let world = World::new(WorldConfig::default());
    println!("world init: {:?}, {} primitives", t.elapsed(), world.primitives().len());
    let (mut tris, mut verts) = (0, 0);
    for lod in 0..worldgen::LOD_LEVELS {
        let t = Instant::now();
        let n = 6;
        for cz in 0..n {
            for cx in 0..n {
                let m = mesh_column(&world, lod, cx * 3, cz * 5);
                tris += m.indices.len() / 3;
                verts += m.positions.len();
            }
        }
        let e = t.elapsed();
        println!("lod {lod}: {} columns in {:?} ({:?}/col), {} verts, {} tris", n * n, e, e / (n * n) as u32, verts, tris);
        (tris, verts) = (0, 0);
    }
}
