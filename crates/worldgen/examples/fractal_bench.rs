//! Times building the prototype fractal structures at full size:
//! `cargo run -p worldgen --release --example fractal_bench`.
use std::time::Instant;

use glam::Mat3;
use glam::Vec3;
use worldgen::fractal::{Kifs, MandelboxBlock, Shape, build};

fn main() {
    let shapes: Vec<(&str, Box<dyn Shape>, f32)> = vec![
        ("mandelbox block", Box::new(MandelboxBlock { size: 220.0, iterations: 4, bailout: 6.0 }), 1.25),
        (
            "kifs spire",
            Box::new(Kifs {
                size: Vec3::new(80.0, 380.0, 80.0),
                taper: 0.55,
                iterations: 3,
                scale: 3.0,
                offset: Vec3::ONE,
                twist: Mat3::from_euler(glam::EulerRot::YXZ, 0.25, 0.08, 0.04),
            }),
            1.0,
        ),
    ];
    for (name, shape, voxel) in &shapes {
        let t = Instant::now();
        let mesh = build(shape.as_ref(), *voxel);
        println!("{name}: {} vertices, {} triangles in {:?}", mesh.positions.len(), mesh.indices.len() / 3, t.elapsed());
    }
}
