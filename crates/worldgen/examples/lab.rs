//! Lab statistics: for each candidate in the data file's `lab` list, how
//! many pieces it is made of, how many of them are pipework, and how tall
//! it stands.
//!
//! `cargo run -p worldgen --release --example lab`

use worldgen::{lab, structure::Library};

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron");
    let library = Library::parse(&std::fs::read_to_string(path).expect("read structures.ron")).expect("parse");
    for (entry, (x, z)) in lab::layout(&library) {
        let (solids, prisms) = lab::build(&library, &entry, 40_000);
        let bare = lab::build(&library, &lab::LabEntry { dress: None, ..entry.clone() }, 40_000);
        let pipes = solids.len().saturating_sub(bare.0.len());
        println!(
            "{:>28} at {:5.0},{:4.0}: {:6} pieces ({:5} pipework), {:4.0} m tall",
            entry.label(),
            x,
            z,
            solids.len() + prisms.len(),
            pipes,
            lab::height(&solids, &prisms)
        );
    }
}
