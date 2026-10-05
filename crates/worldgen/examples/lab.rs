//! Lab statistics: for each candidate in the data file's `lab` list, how
//! many pieces it is made of, how many of them are pipework, and how tall
//! it stands, and how many triangles its solids mesh to.
//!
//! `cargo run -p worldgen --release --example lab`

use worldgen::{lab, structure::Library};

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron");
    let library = Library::parse(&std::fs::read_to_string(path).expect("read structures.ron")).expect("parse");
    for (entry, (x, z)) in lab::layout(&library) {
        let parts = lab::build(&library, &entry, 250_000);
        let start = std::time::Instant::now();
        let triangles = worldgen::structure::mesh(&parts.solids).indices.len() / 3;
        let meshing = start.elapsed().as_millis();
        let far = worldgen::structure::mesh(&worldgen::structure::coarse(&parts.solids, 2.5)).indices.len() / 3;
        println!(
            "{:>28} at {:5.0},{:4.0}: {:6} pieces, {:4.0} m tall, {:8} triangles ({:8} far; box-only would be {:8}) meshed in {} ms",
            entry.label(),
            x,
            z,
            parts.solids.len() + parts.prisms.len(),
            lab::height(&parts.solids, &parts.prisms),
            triangles,
            far,
            parts.solids.len() * 12,
            meshing
        );
    }
}
