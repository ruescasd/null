//! The world's structures are built the same every time (the user,
//! 2026-11-08: places saved on a hanging city's deck found no deck there
//! again). Nothing may hang on a hash map's order, which differs from map to
//! map and from run to run: the sites at the user's places in `places.txt`
//! on hanging cities are each built twice and compared, solid for solid.

use worldgen::plates::PlateWorld;
use worldgen::sites::{self, Layer};
use worldgen::structure::Library;

#[test]
fn hanging_cities_built_the_same_every_time() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron")).expect("data/structures.ron reads");
    let library = Library::parse(&text).expect("data/structures.ron parses");
    let world = PlateWorld::new(worldgen::WORLD_SIZE, 1).with_sites(&library);
    let all = sites::all_in(Layer::Colossi, &library, &world);
    // (city_one, xxx, city, zzz in places.txt.)
    for (px, pz) in [(2723.0f32, 2965.0f32), (1422.0, 2795.0), (1309.0, 3550.0), (5285.0, 15205.0)] {
        let site = all
            .iter()
            .min_by(|a, b| {
                let d = |s: &&sites::Site| (s.at.0 - px).powi(2) + (s.at.1 - pz).powi(2);
                d(a).total_cmp(&d(b))
            })
            .expect("there are colossi");
        let built = || {
            let b = sites::build(&library, &world, site, 500_000);
            let mut out: Vec<String> = b.solids.iter().map(|s| format!("{s:?}")).collect();
            out.extend(b.prisms.iter().map(|p| format!("{p:?}")));
            (b.base, out)
        };
        let (a, b) = (built(), built());
        assert_eq!(a.0, b.0, "{:?}: its base differs", site.cell);
        assert!(a.1 == b.1, "{:?} near ({px}, {pz}): {} and {} pieces, built differently", site.cell, a.1.len(), b.1.len());
        eprintln!("{:?} near ({px}, {pz}): {} pieces, the same twice", site.cell, a.1.len());
    }
}

/// The hand-placed structures (the spire) too.
#[test]
fn placed_structures_built_the_same_every_time() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/structures.ron")).expect("data/structures.ron reads");
    let library = Library::parse(&text).expect("data/structures.ron parses");
    assert!(!library.structures.is_empty());
    for placement in &library.structures {
        let built = || worldgen::structure::build(&library, placement, 40_000).iter().map(|s| format!("{s:?}")).collect::<Vec<_>>();
        let (a, b) = (built(), built());
        assert!(a == b, "{} at {:?}: {} and {} pieces, built differently", placement.style, placement.at, a.len(), b.len());
        eprintln!("{} at {:?}: {} pieces, the same twice", placement.style, placement.at, a.len());
    }
}
