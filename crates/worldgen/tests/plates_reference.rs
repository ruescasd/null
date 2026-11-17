//! The plates' pieces against a reference made before their outlines were
//! made a planar map (2026-11-16): the same pieces, of the same plates, the
//! same shapes (each piece's area and centre within a micrometre; corners
//! may be more, where a neighbour's lie on an edge). `PLATES_REFERENCE=write`
//! writes the reference (`tests/data/plates_reference.txt`).

use glam::DVec2;
use worldgen::plates::PlateWorld;

/// The squares: round points spread over the world, on two seeds.
fn squares() -> Vec<(u32, DVec2)> {
    let mut out = Vec::new();
    for seed in [1, 2] {
        for k in 0..10 {
            out.push((seed, DVec2::new((k * 1361 % 16) as f64 * 1024.0 + 300.0, (k * 7 % 16) as f64 * 1024.0 + 700.0)));
        }
    }
    out
}

fn lines() -> Vec<String> {
    let mut out = Vec::new();
    for (seed, c) in squares() {
        let world = PlateWorld::new(worldgen::WORLD_SIZE, seed);
        for (corners, plate, kept) in world.plate_pieces(c - 150.0, c + 150.0) {
            let poly = worldgen::planar::points(&corners);
            let n = poly.len() as f64;
            let centre = poly.iter().copied().sum::<DVec2>() / n;
            let twice: f64 = (0..poly.len()).map(|k| (poly[k] - centre).perp_dot(poly[(k + 1) % poly.len()] - centre)).sum();
            // (The centre of its area, not of its corners: corners added on
            // its edges move the latter.)
            let mut m = DVec2::ZERO;
            for k in 0..poly.len() {
                let (p, q) = (poly[k] - centre, poly[(k + 1) % poly.len()] - centre);
                m += (p + q) * p.perp_dot(q);
            }
            let middle = centre + m / (3.0 * twice);
            out.push(format!("{seed} {:?} {} {} {:.9} {:.9} {:.9}", plate.sites, plate.level, kept, twice.abs() / 2.0, middle.x, middle.y));
        }
    }
    out
}

#[test]
fn plates_as_before() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/plates_reference.txt");
    let now = lines();
    if std::env::var("PLATES_REFERENCE").is_ok_and(|v| v == "write") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, now.join("\n")).unwrap();
        return;
    }
    let before: Vec<String> = std::fs::read_to_string(&path).expect("the reference: PLATES_REFERENCE=write").lines().map(String::from).collect();
    assert_eq!(now.len(), before.len(), "as many pieces");
    let parse = |l: &str| {
        let f: Vec<&str> = l.rsplitn(4, ' ').collect();
        (f[3].to_string(), f[2].parse::<f64>().unwrap(), DVec2::new(f[1].parse().unwrap(), f[0].parse().unwrap()))
    };
    let mut worst = (0.0f64, 0.0f64);
    for (a, b) in now.iter().zip(&before) {
        let ((ka, area_a, ca), (kb, area_b, cb)) = (parse(a), parse(b));
        assert_eq!(ka, kb, "the same plate, in order");
        worst = (worst.0.max((area_a - area_b).abs()), worst.1.max(ca.distance(cb)));
        assert!((area_a - area_b).abs() < 1e-6 && ca.distance(cb) < 1e-6, "{a}\n{b}");
    }
    println!("{} pieces as before; areas within {:.1e} m², centres within {:.1e} m", now.len(), worst.0, worst.1);
}
