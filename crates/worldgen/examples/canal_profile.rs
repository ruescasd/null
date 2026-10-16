//! A canal's floor along its length, against the ground 20 m beside it, and
//! how far its rim stands above that ground (an aqueduct) or below it (a
//! cutting).
//!
//! `cargo run -p worldgen --release --example canal_profile -- [canal] [rise m] [wave m]`
use worldgen::{canal::Rise, plates::PlateWorld};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let num = |k: usize, d: f64| args.get(k).and_then(|s| s.parse().ok()).unwrap_or(d);
    let mut world = PlateWorld::new(16384.0, 1);
    world.canals.rise = Rise { height: num(1, 0.0), wave: num(2, 1800.0) };
    let i = num(0, 0.0) as usize;
    let track = world.canals.track(i, 150.0, &|c| world.canal_floor(c));
    let across = world.canals.across(i);
    for (at, along, floor) in track {
        let beside = world.height_at((at.x + across.x * 20.0) as f32, (at.y + across.y * 20.0) as f32);
        println!("{along:6.0} m  at {:6.0},{:6.0}  floor {floor:7.1}  ground beside {beside:7.1}  rim minus ground {:6.1}", at.x, at.y, floor + 7.0 - beside as f64);
    }
}
