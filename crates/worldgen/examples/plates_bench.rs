//! Times plate meshing: `cargo run -p worldgen --release --example plates_bench`.
use std::time::Instant;
use worldgen::plates::PlateWorld;

fn main() {
    let world = PlateWorld::new(16384.0, 1);
    for lod in 0..worldgen::LOD_LEVELS {
        let t = Instant::now();
        let (mut tris, n) = (0, 8);
        for cz in 0..n {
            for cx in 0..n {
                tris += world.mesh_column(lod, cx * 3, cz * 5).indices.len() / 3;
            }
        }
        let e = t.elapsed();
        println!("lod {lod}: {} columns in {:?} ({:?}/col), {} tris", n * n, e, e / (n * n) as u32, tris);
    }
}
